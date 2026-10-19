//! A faithful, edit-preserving XML tree for package-preserving `.docx` editing.
//!
//! The `.docx` reader projects `document.xml` into a *lossy* [`crate::DocModel`];
//! that view is perfect for reading/rendering but cannot be edited-and-saved
//! without dropping everything it doesn't model (fields, content controls,
//! `mc:AlternateContent` shapes, tracked changes, bookmarks…). This module is the
//! opposite: a generic element tree that keeps **every** element, attribute (in
//! order), namespace declaration, comment, and processing instruction. Editing
//! mutates nodes in place; serialization re-emits the same tree, so unmodeled
//! siblings ride along untouched.
//!
//! Fidelity is *structural*, not byte-exact: element/attribute identity and order,
//! namespace declarations, and `Raw` markup (declaration/comment/PI/CDATA) are
//! preserved, but attribute values and text are stored unescaped and re-emitted with
//! **canonical** escaping (`&amp; &lt; &gt; &quot;`), so entity spelling and quote
//! style are normalized on any part that is re-serialized — the same trade-off
//! `lxml`/python-docx make. Crucially, only an *edited* part is ever re-serialized;
//! untouched parts stay `Raw` bytes in the package and round-trip byte-for-byte.
//!
//! Design (Rust-idiomatic, no `Rc<RefCell>`): an **arena** of nodes addressed by
//! `Copy` [`NodeId`] handles. Wrappers and edits go through `&mut XmlTree`, so there
//! is no aliasing/borrow tangle. Parsing is depth-bounded and panic-free on hostile
//! input, matching the rest of the crate.

use std::collections::{HashMap, HashSet};

use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{Error, Result};

/// Maximum element nesting accepted while parsing (stack-overflow / zip-bomb guard;
/// mirrors the reader's depth cap). Deeper input is rejected, never crashes.
const MAX_DEPTH: usize = 256;
/// Maximum node count — a fast-fail ceiling that rejects absurd inputs early and keeps
/// the `NodeId` `u32` index safe. It is the [`crate::opc`] per-part ceiling expressed in
/// nodes: a part is capped at 64 MiB and dense OOXML runs ~8 bytes/node, so a *legitimate*
/// `document.xml` at the size limit can genuinely need on the order of this many nodes —
/// setting it much lower would reject valid large documents. The actual out-of-memory
/// guard is **fallible allocation** ([`Vec::try_reserve`] in [`XmlTree::push`]): a hostile
/// part that would exhaust memory returns an [`Error`] rather than aborting the process,
/// so the cap need not (and cannot, without rejecting valid input) be the memory bound.
/// Enforced at parse *and* on fragment-insert edits; `set_element_text` reuses a text slot
/// rather than growing the arena, so repeated text edits stay within budget too.
const MAX_NODES: usize = 8_000_000;

// A lowerable copy of the node budget for tests, so the over-budget edit path can be
// exercised without building an 8-million-node fixture. Production always uses MAX_NODES.
#[cfg(test)]
thread_local! {
    static TEST_NODE_BUDGET: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_NODES) };
}

/// Set the per-tree node budget for the current test thread.
#[cfg(test)]
pub(crate) fn set_test_node_budget(n: usize) {
    TEST_NODE_BUDGET.with(|c| c.set(n));
}

/// Restore the node budget to the production value (test threads are reused, so a test
/// that lowered it must reset to `MAX_NODES`, not `usize::MAX`).
#[cfg(test)]
pub(crate) fn reset_test_node_budget() {
    TEST_NODE_BUDGET.with(|c| c.set(MAX_NODES));
}

/// The effective node budget — `MAX_NODES` in production, the test override under `cfg(test)`.
/// A tree may hold at most this many nodes (parse and edits agree on the boundary).
pub(crate) fn node_budget() -> usize {
    #[cfg(test)]
    {
        TEST_NODE_BUDGET.with(|c| c.get())
    }
    #[cfg(not(test))]
    {
        MAX_NODES
    }
}

// A test-only seam that forces the Nth *commit-time* tree edit
// (`set_element_text` / fragment insertion) to fail, simulating the
// genuine-but-untriggerable `try_reserve` out-of-memory those paths now guard against.
// It lets transactional clone-and-swap edit paths be tested: the edit must leave the
// document completely unchanged when a commit step fails.
// `set_test_fail_commit_after(k)` lets the first `k` commit edits succeed, then the next
// one fails (one-shot, self-disarming). Production never compiles this.
#[cfg(test)]
thread_local! {
    static FAIL_COMMIT_AFTER: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Arm the commit-failure seam: succeed `n` more commit edits, then fail the next one.
#[cfg(test)]
pub(crate) fn set_test_fail_commit_after(n: usize) {
    FAIL_COMMIT_AFTER.with(|c| c.set(Some(n)));
}

/// Disarm the commit-failure seam (test threads are reused).
#[cfg(test)]
pub(crate) fn reset_test_fail_commit() {
    FAIL_COMMIT_AFTER.with(|c| c.set(None));
}

/// Whether the current commit edit should fail (decrements the countdown; fires once at 0).
#[cfg(test)]
fn commit_should_fail() -> bool {
    FAIL_COMMIT_AFTER.with(|c| match c.get() {
        None => false,
        Some(0) => {
            c.set(None);
            true
        }
        Some(n) => {
            c.set(Some(n - 1));
            false
        }
    })
}

/// The WordprocessingML namespace URI — used to resolve which `t` elements are real
/// body text runs vs DrawingML text.
pub(crate) const WML_NS: &[u8] = b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const MC_NS: &[u8] = b"http://schemas.openxmlformats.org/markup-compatibility/2006";
const OOXML_REL_NS: &[u8] = b"http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn wml_ns_str() -> &'static str {
    std::str::from_utf8(WML_NS).expect("WML namespace is valid UTF-8")
}

/// A `Copy` handle into an [`XmlTree`]'s arena.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub(crate) struct NodeId(u32);

#[derive(Clone, Copy, PartialEq, Eq)]
enum WmlVMerge {
    None,
    Restart,
    Continue,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WmlRevisionEditPolicy {
    Accept,
    Reject,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WmlRevisionEditAction {
    Keep,
    Remove,
    Unwrap,
    RenameDeletedText,
}

#[derive(Clone, Copy)]
enum AlternateContentBranch {
    NotAlternateContent,
    Selected(NodeId),
    Missing,
}

#[derive(Clone, Copy)]
enum AlternateContentTraversal {
    Selected,
    All,
}

#[derive(Clone, Copy)]
struct WmlActiveVMerge {
    col: usize,
    span: usize,
    cell: NodeId,
}

/// A single XML node. Element attribute values and text are stored **unescaped**
/// and canonically re-escaped on serialization; `Raw` markup (declaration, comment,
/// PI, CDATA, doctype) is kept verbatim.
#[derive(Debug, Clone)]
pub(crate) enum Node {
    /// An element: raw qualified name (e.g. `b"w:p"`), attributes in source order
    /// (raw key, unescaped value — `xmlns:*` declarations included), and whether it
    /// was written self-closing (`<x/>`).
    Element {
        name: Vec<u8>,
        attrs: Vec<(Vec<u8>, Vec<u8>)>,
        self_closing: bool,
    },
    /// Character data, stored unescaped.
    Text(Vec<u8>),
    /// Verbatim markup that is re-emitted exactly: `<?xml …?>`, `<!-- … -->`,
    /// `<?pi …?>`, `<![CDATA[ … ]]>`, `<!DOCTYPE …>`.
    Raw(Vec<u8>),
}

#[derive(Debug, Clone)]
struct NodeData {
    node: Node,
    children: Vec<NodeId>,
}

/// A parsed XML document as an arena tree. Top-level nodes (declaration, root
/// element, trailing comments…) are held in `roots` in order.
#[derive(Debug, Clone)]
pub(crate) struct XmlTree {
    nodes: Vec<NodeData>,
    roots: Vec<NodeId>,
}

impl XmlTree {
    /// Parse XML bytes into a faithful tree. Depth-bounded and panic-free: malformed
    /// or hostile input yields an [`Error`], never a crash.
    pub(crate) fn parse(xml: &[u8]) -> Result<XmlTree> {
        let mut reader = Reader::from_reader(xml);
        let cfg = reader.config_mut();
        cfg.expand_empty_elements = false; // keep `<x/>` distinct from `<x></x>`
        cfg.check_end_names = true; // reject mismatched end tags rather than silently
                                    // "repairing" malformed input into a different tree

        let mut tree = XmlTree {
            nodes: Vec::new(),
            roots: Vec::new(),
        };
        // Stack of open element ids (the current insertion path).
        let mut stack: Vec<NodeId> = Vec::new();
        let mut buf = Vec::new();

        loop {
            // Bound total node count (a size-capped part can still hold millions of
            // tiny elements); keeps arena memory finite and the `NodeId` u32 safe. The
            // boundary is `> budget` (not `>=`) so a tree with exactly `node_budget()`
            // nodes — the most an edit preflight will allow — also re-parses cleanly.
            if tree.nodes.len() > node_budget() {
                return Err(Error::Docx("xml has too many nodes".into()));
            }
            let ev = reader
                .read_event_into(&mut buf)
                .map_err(|e| Error::Docx(format!("xml parse: {e}")))?;
            match ev {
                Event::Start(e) => {
                    if stack.len() >= MAX_DEPTH {
                        return Err(Error::Docx("xml nesting too deep".into()));
                    }
                    let node = element_node(&e, false)?;
                    let id = tree.push(node, stack.last().copied())?;
                    stack.push(id);
                }
                Event::Empty(e) => {
                    // A self-closing element occupies a level too: enforce the same depth
                    // cap as `Start` so an empty element can't sit one level past MAX_DEPTH.
                    if stack.len() >= MAX_DEPTH {
                        return Err(Error::Docx("xml nesting too deep".into()));
                    }
                    let node = element_node(&e, true)?;
                    tree.push(node, stack.last().copied())?;
                }
                Event::End(_) => {
                    stack.pop();
                }
                Event::Text(t) => {
                    let raw = t.into_inner();
                    if raw.is_empty() {
                        continue;
                    }
                    match stack.last().copied() {
                        // Element content: store unescaped (re-escaped on write).
                        Some(parent) => {
                            tree.push_text(unescape_bytes(&raw)?, parent)?;
                        }
                        // Prolog/epilog text (e.g. the `\r\n` between the XML
                        // declaration and the root element) is whitespace where
                        // character references are NOT allowed — keep it verbatim.
                        None => {
                            tree.push(Node::Raw(raw.into_owned()), None)?;
                        }
                    }
                }
                Event::GeneralRef(reference) => {
                    let Some(parent) = stack.last().copied() else {
                        return Err(Error::Docx(
                            "xml character reference is not allowed outside the root element"
                                .into(),
                        ));
                    };
                    tree.push_text(resolve_reference(&reference)?, parent)?;
                }
                Event::CData(c) => {
                    let mut raw = b"<![CDATA[".to_vec();
                    raw.extend_from_slice(&c.into_inner());
                    raw.extend_from_slice(b"]]>");
                    tree.push(Node::Raw(raw), stack.last().copied())?;
                }
                Event::Comment(c) => {
                    let mut raw = b"<!--".to_vec();
                    raw.extend_from_slice(c.as_ref());
                    raw.extend_from_slice(b"-->");
                    tree.push(Node::Raw(raw), stack.last().copied())?;
                }
                Event::PI(p) => {
                    let mut raw = b"<?".to_vec();
                    raw.extend_from_slice(p.as_ref());
                    raw.extend_from_slice(b"?>");
                    tree.push(Node::Raw(raw), stack.last().copied())?;
                }
                Event::Decl(d) => {
                    if !stack.is_empty() || !tree.roots.is_empty() {
                        return Err(Error::Docx(
                            "xml declaration is only allowed at the start of the document".into(),
                        ));
                    }
                    let mut raw = b"<?".to_vec();
                    raw.extend_from_slice(d.as_ref());
                    raw.extend_from_slice(b"?>");
                    tree.push(Node::Raw(raw), stack.last().copied())?;
                }
                Event::DocType(d) => {
                    let mut raw = b"<!DOCTYPE ".to_vec();
                    raw.extend_from_slice(d.as_ref());
                    raw.extend_from_slice(b">");
                    tree.push(Node::Raw(raw), stack.last().copied())?;
                }
                Event::Eof => {
                    // Truncated input: `quick_xml` returns Eof even with elements still
                    // open (e.g. `<a><b>`). Reject it rather than inventing close tags —
                    // an edit must never silently rewrite a damaged part into new content.
                    if !stack.is_empty() {
                        return Err(Error::Docx("xml ended with unclosed elements".into()));
                    }
                    break;
                }
            }
            buf.clear();
        }
        Ok(tree)
    }

    /// Serialize the tree back to XML bytes. Untouched nodes re-emit with their
    /// original structure (attribute order/names/values, namespace decls, comments,
    /// PIs preserved); attribute values and text are canonically escaped.
    pub(crate) fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for &root in &self.roots {
            self.write_node(root, &mut out);
        }
        out
    }

    /// Serialize one element subtree without promoting or mutating the source
    /// package part. Streaming semantic readers use this to inventory one
    /// accepted-current story root at a time.
    pub(crate) fn serialize_subtree(&self, root: NodeId) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_node(root, &mut out);
        out
    }

    fn write_node(&self, id: NodeId, out: &mut Vec<u8>) {
        let data = &self.nodes[id.0 as usize];
        match &data.node {
            Node::Raw(bytes) => out.extend_from_slice(bytes),
            Node::Text(bytes) => esc_text_into(bytes, out),
            Node::Element {
                name,
                attrs,
                self_closing,
            } => {
                out.push(b'<');
                out.extend_from_slice(name);
                for (k, v) in attrs {
                    out.push(b' ');
                    out.extend_from_slice(k);
                    out.extend_from_slice(b"=\"");
                    esc_attr_into(v, out);
                    out.push(b'"');
                }
                if *self_closing && data.children.is_empty() {
                    out.extend_from_slice(b"/>");
                } else {
                    out.push(b'>');
                    for &c in &data.children {
                        self.write_node(c, out);
                    }
                    out.extend_from_slice(b"</");
                    out.extend_from_slice(name);
                    out.push(b'>');
                }
            }
        }
    }

    // --- Navigation accessors (reserved for the richer NodeId-handle editing the
    // TRD describes; currently exercised by the round-trip tests). ---

    /// Top-level nodes in order (declaration, root element, …).
    #[cfg(test)]
    pub(crate) fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// The node payload behind a handle.
    #[cfg(test)]
    pub(crate) fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize].node
    }

    /// Children of a node, in order.
    #[cfg(test)]
    pub(crate) fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id.0 as usize].children
    }

    /// Element local name (after any `prefix:`), or `None` for non-elements.
    pub(crate) fn local_name(&self, id: NodeId) -> Option<&[u8]> {
        match &self.nodes[id.0 as usize].node {
            Node::Element { name, .. } => Some(match name.iter().position(|&b| b == b':') {
                Some(i) => &name[i + 1..],
                None => name,
            }),
            _ => None,
        }
    }

    fn push(&mut self, node: Node, parent: Option<NodeId>) -> Result<NodeId> {
        // Fallible allocation is the real OOM guard (see `MAX_NODES`): reserve every slot
        // we are about to fill *before* mutating, so a hostile part that would exhaust
        // memory returns an `Error` here instead of aborting the process — and the arena
        // stays consistent on failure (nothing is pushed if either reservation fails).
        self.nodes
            .try_reserve(1)
            .map_err(|_| Error::Docx("xml: out of memory growing node arena".into()))?;
        match parent {
            Some(p) => self.nodes[p.0 as usize]
                .children
                .try_reserve(1)
                .map_err(|_| Error::Docx("xml: out of memory growing child list".into()))?,
            None => self
                .roots
                .try_reserve(1)
                .map_err(|_| Error::Docx("xml: out of memory growing root list".into()))?,
        }
        let id = NodeId(self.nodes.len() as u32);
        self.nodes.push(NodeData {
            node,
            children: Vec::new(),
        });
        match parent {
            Some(p) => self.nodes[p.0 as usize].children.push(id),
            None => self.roots.push(id),
        }
        Ok(id)
    }

    /// Append character data to the previous text node when quick-xml split it at a
    /// `GeneralRef`. This preserves the pre-0.41 tree shape while retaining fallible
    /// allocation for hostile inputs.
    fn push_text(&mut self, text: Vec<u8>, parent: NodeId) -> Result<()> {
        let previous = self.nodes[parent.0 as usize].children.last().copied();
        if let Some(previous) = previous {
            if let Node::Text(existing) = &mut self.nodes[previous.0 as usize].node {
                existing
                    .try_reserve(text.len())
                    .map_err(|_| Error::Docx("xml: out of memory growing text node".into()))?;
                existing.extend_from_slice(&text);
                return Ok(());
            }
        }
        self.push(Node::Text(text), Some(parent))?;
        Ok(())
    }

    fn parent_child_index(&self, target: NodeId) -> Option<(NodeId, usize)> {
        fn rec(t: &XmlTree, parent: NodeId, target: NodeId) -> Option<(NodeId, usize)> {
            for (index, &child) in t.nodes[parent.0 as usize].children.iter().enumerate() {
                if child == target {
                    return Some((parent, index));
                }
                if let Some(found) = rec(t, child, target) {
                    return Some(found);
                }
            }
            None
        }

        for &root in &self.roots {
            if let Some(found) = rec(self, root, target) {
                return Some(found);
            }
        }
        None
    }
}

/// Maximum attributes accepted on a single element — a size-capped part could otherwise
/// pack one element with millions of attributes, amplifying into large heap use.
const MAX_ATTRS_PER_ELEMENT: usize = 65_536;

// Test-lowerable copy of the attribute cap, so the over-cap path can be exercised on a
// tiny fixture instead of a 65k-attribute one. Production always uses the const.
#[cfg(test)]
thread_local! {
    static TEST_MAX_ATTRS: std::cell::Cell<usize> =
        const { std::cell::Cell::new(MAX_ATTRS_PER_ELEMENT) };
}

/// Set the per-element attribute cap for the current test thread.
#[cfg(test)]
pub(crate) fn set_test_max_attrs(n: usize) {
    TEST_MAX_ATTRS.with(|c| c.set(n));
}

fn max_attrs() -> usize {
    #[cfg(test)]
    {
        TEST_MAX_ATTRS.with(|c| c.get())
    }
    #[cfg(not(test))]
    {
        MAX_ATTRS_PER_ELEMENT
    }
}

/// Build an [`Node::Element`] from a quick-xml start/empty tag, capturing the raw
/// qualified name and attributes (unescaped values) in source order.
fn element_node(e: &quick_xml::events::BytesStart<'_>, self_closing: bool) -> Result<Node> {
    let name = e.name().as_ref().to_vec();
    let mut attrs = Vec::new();
    let cap = max_attrs();
    for a in e.attributes() {
        if attrs.len() >= cap {
            return Err(Error::Docx("element has too many attributes".into()));
        }
        let a = a.map_err(|err| Error::Docx(format!("xml attr: {err}")))?;
        let key = a.key.as_ref().to_vec();
        // Propagate (not swallow) a malformed entity reference: otherwise the raw `&`
        // survives, then re-serialization canonicalizes it to `&amp;` — silently
        // rewriting malformed XML the edit never targeted.
        let val = a
            .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, e.decoder())
            .map_err(|err| Error::Docx(format!("xml attr value: {err}")))?
            .into_owned()
            .into_bytes();
        attrs.push((key, val));
    }
    Ok(Node::Element {
        name,
        attrs,
        self_closing,
    })
}

/// Unescape XML entities in text bytes (UTF-8). **Errors** on non-UTF-8 or a malformed
/// entity reference rather than falling back to the raw bytes — a raw `&` would otherwise
/// be canonicalized to `&amp;` on re-serialization, silently rewriting malformed XML.
fn unescape_bytes(raw: &[u8]) -> Result<Vec<u8>> {
    let s = std::str::from_utf8(raw).map_err(|e| Error::Docx(format!("xml text utf-8: {e}")))?;
    let c =
        quick_xml::escape::unescape(s).map_err(|e| Error::Docx(format!("xml text entity: {e}")))?;
    Ok(c.into_owned().into_bytes())
}

/// Resolve a quick-xml 0.41 `GeneralRef` event with the same predefined-only
/// policy used by the old text unescape API. External/custom entities stay errors.
fn resolve_reference(reference: &quick_xml::events::BytesRef<'_>) -> Result<Vec<u8>> {
    if let Some(character) = reference
        .resolve_char_ref()
        .map_err(|e| Error::Docx(format!("xml text entity: {e}")))?
    {
        return Ok(character.to_string().into_bytes());
    }
    let name = reference
        .decode()
        .map_err(|e| Error::Docx(format!("xml text entity: {e}")))?;
    quick_xml::escape::resolve_predefined_entity(&name)
        .map(|value| value.as_bytes().to_vec())
        .ok_or_else(|| Error::Docx(format!("xml text entity: unknown entity '&{name};'")))
}

/// XML 1.0 character validity. Edited strings can contain Unicode scalar values
/// (`U+FFFE`/`U+FFFF`) that are valid Rust but forbidden in XML, so filter them
/// alongside illegal C0 controls before serializing.
fn is_xml_legal_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
        || matches!(
            c as u32,
            0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF
        )
}

fn push_char_utf8(c: char, out: &mut Vec<u8>) {
    let mut buf = [0u8; 4];
    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
}

fn esc_text_into(s: &[u8], out: &mut Vec<u8>) {
    for c in String::from_utf8_lossy(s).chars() {
        match c {
            '&' => out.extend_from_slice(b"&amp;"),
            '<' => out.extend_from_slice(b"&lt;"),
            '>' => out.extend_from_slice(b"&gt;"),
            // A literal CR would be folded to LF by XML end-of-line normalization on
            // the next read; emit it as a character reference so the byte survives.
            '\r' => out.extend_from_slice(b"&#13;"),
            _ if !is_xml_legal_char(c) => {}
            _ => push_char_utf8(c, out),
        }
    }
}

fn esc_attr_into(s: &[u8], out: &mut Vec<u8>) {
    for c in String::from_utf8_lossy(s).chars() {
        match c {
            '&' => out.extend_from_slice(b"&amp;"),
            '<' => out.extend_from_slice(b"&lt;"),
            '>' => out.extend_from_slice(b"&gt;"),
            '"' => out.extend_from_slice(b"&quot;"),
            // Attribute-value normalization collapses literal tab/newline/CR to a
            // space; emit them as character references so they round-trip intact.
            '\t' => out.extend_from_slice(b"&#9;"),
            '\n' => out.extend_from_slice(b"&#10;"),
            '\r' => out.extend_from_slice(b"&#13;"),
            _ if !is_xml_legal_char(c) => {}
            _ => push_char_utf8(c, out),
        }
    }
}

fn escaped_text(s: &str) -> String {
    let mut out = Vec::new();
    esc_text_into(s.as_bytes(), &mut out);
    String::from_utf8(out).expect("XML text escaping preserves UTF-8")
}

fn escaped_attr(s: &str) -> String {
    let mut out = Vec::new();
    esc_attr_into(s.as_bytes(), &mut out);
    String::from_utf8(out).expect("XML attribute escaping preserves UTF-8")
}

fn wml_text_run_content_xml_with_prefix(
    text: &str,
    prefix: &str,
    declare_namespace: bool,
    keep_text_anchor: bool,
) -> String {
    fn qualified_name(prefix: &str, local: &str) -> String {
        if prefix.is_empty() {
            local.to_string()
        } else {
            format!("{prefix}:{local}")
        }
    }

    fn namespace_attr(prefix: &str, declare_namespace: bool) -> String {
        if !declare_namespace {
            String::new()
        } else if prefix.is_empty() {
            format!(r#" xmlns="{}""#, wml_ns_str())
        } else {
            format!(r#" xmlns:{prefix}="{}""#, wml_ns_str())
        }
    }

    fn flush(out: &mut String, buf: &mut String, text_name: &str, namespace: &str) -> bool {
        if buf.is_empty() {
            return false;
        }
        let preserve = buf.as_str() != buf.trim_matches([' ', '\t', '\n', '\r']);
        let text = escaped_text(buf);
        if preserve {
            out.push_str(&format!(
                r#"<{text_name}{namespace} xml:space="preserve">{text}</{text_name}>"#
            ));
        } else {
            out.push_str(&format!(r#"<{text_name}{namespace}>{text}</{text_name}>"#));
        }
        buf.clear();
        true
    }

    let text_name = qualified_name(prefix, "t");
    let tab_name = qualified_name(prefix, "tab");
    let break_name = qualified_name(prefix, "br");
    let namespace = namespace_attr(prefix, declare_namespace);
    let mut out = String::new();
    let mut buf = String::new();
    let mut has_text = false;
    for ch in text.chars() {
        match ch {
            '\t' => {
                has_text |= flush(&mut out, &mut buf, &text_name, &namespace);
                out.push_str(&format!("<{tab_name}{namespace}/>"));
            }
            '\n' => {
                has_text |= flush(&mut out, &mut buf, &text_name, &namespace);
                out.push_str(&format!("<{break_name}{namespace}/>"));
            }
            '\r' => {}
            c if (c as u32) < 0x20 => {}
            c => buf.push(c),
        }
    }
    has_text |= flush(&mut out, &mut buf, &text_name, &namespace);
    if keep_text_anchor && !out.is_empty() && !has_text {
        out.push_str(&format!("<{text_name}{namespace}></{text_name}>"));
    }
    out
}

fn wml_text_run_content_xml(text: &str) -> String {
    wml_text_run_content_xml_with_prefix(text, "w", false, false)
}

pub(crate) fn wml_text_run_content_node_count(text: &str) -> Result<usize> {
    let xml = wml_text_run_content_xml(text);
    if xml.is_empty() {
        return Ok(0);
    }
    XmlTree::parse(xml.as_bytes()).map(|tree| tree.node_count())
}

pub(crate) fn wml_anchored_text_run_content_node_count(text: &str) -> Result<usize> {
    let xml = wml_text_run_content_xml_with_prefix(text, "w", false, true);
    if xml.is_empty() {
        return Ok(0);
    }
    XmlTree::parse(xml.as_bytes()).map(|tree| tree.node_count())
}

#[derive(Clone, Debug, Default)]
pub(crate) struct WmlTextRunGroup {
    text_runs: Vec<NodeId>,
    text_run_parents: Vec<Option<NodeId>>,
    marker_nodes: Vec<NodeId>,
}

impl WmlTextRunGroup {
    fn push_text_run(&mut self, id: NodeId, parent: Option<NodeId>) {
        self.text_runs.push(id);
        self.text_run_parents.push(parent);
    }

    pub(crate) fn text_runs(&self) -> &[NodeId] {
        &self.text_runs
    }

    pub(crate) fn replacement_text_runs(&self) -> impl Iterator<Item = NodeId> + '_ {
        let first_parent = self.text_run_parents.first().copied().flatten();
        self.text_runs
            .iter()
            .copied()
            .zip(self.text_run_parents.iter().copied())
            .enumerate()
            .filter_map(move |(index, (id, parent))| {
                (index == 0 || first_parent.is_none() || parent != first_parent).then_some(id)
            })
    }

    fn removable_text_runs(&self) -> impl Iterator<Item = NodeId> + '_ {
        let first_parent = self.text_run_parents.first().copied().flatten();
        self.text_runs
            .iter()
            .copied()
            .zip(self.text_run_parents.iter().copied())
            .enumerate()
            .filter_map(move |(index, (id, parent))| {
                (index > 0 && first_parent.is_some() && parent == first_parent).then_some(id)
            })
    }

    pub(crate) fn marker_nodes(&self) -> &[NodeId] {
        &self.marker_nodes
    }

    pub(crate) fn into_text_runs(self) -> Vec<NodeId> {
        self.text_runs
    }

    pub(crate) fn into_replacement_text_runs(self) -> Vec<NodeId> {
        let Self {
            mut text_runs,
            text_run_parents,
            ..
        } = self;
        let first_parent = text_run_parents.first().copied().flatten();
        let mut index = 0usize;
        text_runs.retain(|_| {
            let parent = text_run_parents.get(index).copied().flatten();
            let keep = index == 0 || first_parent.is_none() || parent != first_parent;
            index += 1;
            keep
        });
        text_runs
    }
}

#[derive(Debug)]
struct FieldScan {
    wanted: usize,
    seen: usize,
    target: Option<WmlTextRunGroup>,
    complex: Vec<ComplexFieldScan>,
}

#[derive(Debug)]
struct ComplexFieldScan {
    result_phase: bool,
    result: WmlTextRunGroup,
}

#[derive(Debug, Default)]
struct FieldInstructionScan {
    fields: Vec<String>,
    complex: Vec<ComplexFieldInstructionScan>,
}

#[derive(Debug)]
struct ComplexFieldInstructionScan {
    instruction: String,
    result_phase: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WmlRunRange {
    first_run: NodeId,
    last_run: NodeId,
}

#[derive(Debug)]
struct WmlCommentTextEditTarget {
    text_runs: Vec<NodeId>,
}

#[derive(Clone, Copy)]
struct WmlAtomicBodyBlock {
    child_index: usize,
    node: NodeId,
    kind: WmlAtomicBodyBlockKind,
    section_boundary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WmlAtomicBodyBlockKind {
    Paragraph,
    Table,
    ContentControl,
}

#[derive(Default)]
struct WmlBodyRangeScan {
    paired: HashMap<Vec<u8>, usize>,
    comment_ranges: HashMap<Vec<u8>, usize>,
    comment_references: HashMap<Vec<u8>, usize>,
    complex_fields: Vec<(usize, bool)>,
    proofing: HashMap<Vec<u8>, usize>,
}

fn paired_wml_range_marker(local: &[u8]) -> Option<(&[u8], bool)> {
    match local {
        b"bookmarkStart" => Some((b"bookmark", true)),
        b"bookmarkEnd" => Some((b"bookmark", false)),
        b"permStart" => Some((b"perm", true)),
        b"permEnd" => Some((b"perm", false)),
        _ => local
            .strip_suffix(b"RangeStart")
            .map(|family| (family, true))
            .or_else(|| {
                local
                    .strip_suffix(b"RangeEnd")
                    .map(|family| (family, false))
            }),
    }
}

fn body_marker_key(family: &[u8], id: &[u8]) -> Result<Vec<u8>> {
    let capacity = family
        .len()
        .checked_add(id.len())
        .and_then(|len| len.checked_add(1))
        .ok_or_else(|| Error::Docx("body range identity is too large".into()))?;
    let mut key = Vec::new();
    key.try_reserve(capacity)
        .map_err(|_| Error::Docx("body range identity: out of memory".into()))?;
    key.extend_from_slice(family);
    key.push(0);
    key.extend_from_slice(id);
    Ok(key)
}

fn body_marker_id(id: &[u8]) -> Result<Vec<u8>> {
    let mut owned = Vec::new();
    owned
        .try_reserve(id.len())
        .map_err(|_| Error::Docx("body marker identity: out of memory".into()))?;
    owned.extend_from_slice(id);
    Ok(owned)
}

fn attr_value_local<'a>(attrs: &'a [(Vec<u8>, Vec<u8>)], local: &[u8]) -> Option<&'a [u8]> {
    attrs.iter().find_map(|(k, v)| {
        let lname = k
            .iter()
            .position(|&b| b == b':')
            .map_or(k.as_slice(), |i| &k[i + 1..]);
        (lname == local).then_some(v.as_slice())
    })
}

fn trim_ascii_whitespace(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn wml_break_is_text_wrapping(attrs: &[(Vec<u8>, Vec<u8>)], scope: &[(Vec<u8>, Vec<u8>)]) -> bool {
    attr_value_ns_local(attrs, scope, WML_NS, b"type")
        .or_else(|| attr_value_ns_local(attrs, scope, b"", b"type"))
        .map(trim_ascii_whitespace)
        .is_none_or(|value| value == b"textWrapping")
}

fn attr_value_ns_local<'a>(
    attrs: &'a [(Vec<u8>, Vec<u8>)],
    scope: &[(Vec<u8>, Vec<u8>)],
    ns: &[u8],
    local: &[u8],
) -> Option<&'a [u8]> {
    attrs.iter().find_map(|(k, v)| {
        let (prefix, lname): (&[u8], &[u8]) = match k.iter().position(|&b| b == b':') {
            Some(i) => (&k[..i], &k[i + 1..]),
            None => (b"", k),
        };
        if lname != local {
            return None;
        }
        let matches_ns = if prefix.is_empty() {
            ns.is_empty()
        } else {
            scope
                .iter()
                .rev()
                .find(|(p, _)| p.as_slice() == prefix)
                .map(|(_, u)| u.as_slice())
                == Some(ns)
        };
        matches_ns.then_some(v.as_slice())
    })
}

#[cfg(test)]
impl Node {
    /// Convenience for tests: the unescaped text of a [`Node::Text`].
    fn clone_text(&self) -> Option<Vec<u8>> {
        match self {
            Node::Text(t) => Some(t.clone()),
            _ => None,
        }
    }
}

/// The edit surface (milestone B): query and mutate the live tree in place. Every
/// untouched node is preserved; only what these methods change is rewritten.
impl XmlTree {
    /// Concatenated text of every `Text` descendant of `id` (lossy UTF-8).
    pub(crate) fn text_of(&self, id: NodeId) -> String {
        let mut out = String::new();
        self.collect_text(id, &mut out);
        out
    }

    fn collect_text(&self, id: NodeId, out: &mut String) {
        match &self.nodes[id.0 as usize].node {
            Node::Text(t) => out.push_str(&String::from_utf8_lossy(t)),
            // A `Raw` `<![CDATA[…]]>` is character data too — decode its payload so
            // `<w:t><![CDATA[OLD]]></w:t>` reads (and so `replace_body_text` matches)
            // as "OLD". Other `Raw` markup (comments/PIs) carries no character data.
            Node::Raw(r) => {
                if let Some(inner) = r
                    .strip_prefix(b"<![CDATA[".as_slice())
                    .and_then(|s| s.strip_suffix(b"]]>".as_slice()))
                {
                    out.push_str(&String::from_utf8_lossy(inner));
                }
            }
            Node::Element { .. } => {
                for i in 0..self.nodes[id.0 as usize].children.len() {
                    let c = self.nodes[id.0 as usize].children[i];
                    self.collect_text(c, out);
                }
            }
        }
    }

    /// Set an element's text to a single text node. **Reuses** an existing `Text` child
    /// slot in place when present (so repeated edits — e.g. rewriting every `w:t` — do
    /// not grow the arena), otherwise pushes one new text node; any other children are
    /// detached. If the new text has significant leading/trailing whitespace,
    /// `xml:space="preserve"` is set so Word/consumers don't collapse it.
    ///
    /// **Fallible** only when adding `xml:space` would push the element past the attribute
    /// cap (so a re-parse couldn't accept the result), or when the no-carrier fallback
    /// allocation fails. Callers that may target a capped element (e.g.
    /// [`crate::Document::replace_body_text`]) preflight with [`Self::can_set_attr`], so on
    /// a checked edit this never fails — the signature just keeps parse/edit budget
    /// symmetry honest instead of silently producing an un-re-parseable element.
    pub(crate) fn set_element_text(&mut self, id: NodeId, text: &str) -> Result<()> {
        // Test seam: simulate a commit-time allocation failure (see `commit_should_fail`).
        #[cfg(test)]
        if commit_should_fail() {
            return Err(Error::Docx(
                "simulated commit-time allocation failure (test seam)".into(),
            ));
        }
        // Add `xml:space="preserve"` when the new text has significant edge
        // whitespace; remove a now-unneeded one otherwise (don't leave it stale).
        if text != text.trim_matches([' ', '\t', '\n', '\r']) {
            self.set_attr(id, b"xml:space", b"preserve")?;
        } else {
            self.remove_attr(id, b"xml:space");
        }
        // Reuse the first existing text-carrying child slot — a `Text` node OR a CDATA
        // `Raw` node (the carrier when the run's text came in as `<![CDATA[..]]>`) — by
        // overwriting it in place, so repeated edits (rewriting every `w:t`, including
        // CDATA ones) never grow the arena. Fall back to one allocation only if there is
        // no carrier at all.
        let reuse = self.nodes[id.0 as usize]
            .children
            .iter()
            .copied()
            .find(|&c| match &self.nodes[c.0 as usize].node {
                Node::Text(_) => true,
                Node::Raw(r) => r.starts_with(b"<![CDATA["),
                Node::Element { .. } => false,
            });
        match reuse {
            Some(tid) => {
                self.nodes[tid.0 as usize].node = Node::Text(text.as_bytes().to_vec());
                self.nodes[id.0 as usize].children = vec![tid];
            }
            None => {
                self.nodes[id.0 as usize].children.clear();
                self.push(Node::Text(text.as_bytes().to_vec()), Some(id))?;
            }
        }
        Ok(())
    }

    /// Replace one existing WML text element with WML run content (`w:t`,
    /// `w:tab`, `w:br`) at the same child position inside its parent run.
    pub(crate) fn replace_wml_text_element_with_run_content(
        &mut self,
        id: NodeId,
        text: &str,
    ) -> Result<()> {
        self.replace_wml_text_element_with_run_content_impl(id, text, false)
    }

    /// Marker-aware replacement that retains an empty `w:t` carrier when the
    /// replacement consists only of tabs and breaks, allowing later refills.
    pub(crate) fn replace_wml_text_element_with_anchored_run_content(
        &mut self,
        id: NodeId,
        text: &str,
    ) -> Result<()> {
        self.replace_wml_text_element_with_run_content_impl(id, text, true)
    }

    fn replace_wml_text_element_with_run_content_impl(
        &mut self,
        id: NodeId,
        text: &str,
        keep_text_anchor: bool,
    ) -> Result<()> {
        let (parent, index) = self
            .parent_child_index(id)
            .ok_or_else(|| Error::Docx("wml text node has no parent".into()))?;
        let raw_prefix = match &self.nodes[id.0 as usize].node {
            Node::Element { name, .. } => name
                .iter()
                .position(|&byte| byte == b':')
                .map_or(&b""[..], |colon| &name[..colon]),
            _ => return Err(Error::Docx("wml text node is not an element".into())),
        };
        let parsed_prefix = std::str::from_utf8(raw_prefix).ok();
        let prefix = parsed_prefix.unwrap_or("w");
        let parent_scope = self.ns_scope_at(parent);
        let parent_binds_prefix = parsed_prefix.is_some()
            && parent_scope
                .iter()
                .rev()
                .find(|(candidate, _)| candidate.as_slice() == raw_prefix)
                .is_some_and(|(_, uri)| uri.as_slice() == WML_NS);
        let xml = wml_text_run_content_xml_with_prefix(
            text,
            prefix,
            !parent_binds_prefix,
            keep_text_anchor,
        );
        self.nodes[parent.0 as usize].children.remove(index);
        if !xml.is_empty() {
            self.insert_fragment_at(parent, index, xml.as_bytes())?;
        }
        Ok(())
    }

    /// Detach replaceable tab/break nodes and extra split text fragments from
    /// each selected group's anchor run. Later run wrappers and their first text
    /// carriers remain available for formatting-preserving cleanup.
    pub(crate) fn prepare_wml_text_groups_for_replacement<'a>(
        &mut self,
        groups: impl IntoIterator<Item = &'a WmlTextRunGroup>,
    ) -> Result<()> {
        let mut removed = HashSet::new();
        for group in groups {
            removed
                .try_reserve(
                    group
                        .marker_nodes()
                        .len()
                        .saturating_add(group.text_runs().len()),
                )
                .map_err(|_| Error::Docx("xml: out of memory collecting run content".into()))?;
            removed.extend(group.marker_nodes().iter().copied());
            removed.extend(group.removable_text_runs());
        }
        if removed.is_empty() {
            return Ok(());
        }

        #[cfg(test)]
        if commit_should_fail() {
            return Err(Error::Docx(
                "simulated commit-time allocation failure (test seam)".into(),
            ));
        }

        for node in &mut self.nodes {
            node.children.retain(|child| !removed.contains(child));
        }
        self.roots.retain(|root| !removed.contains(root));
        Ok(())
    }

    /// Set (or add) an attribute on an element, preserving the order of existing
    /// attributes. No-op on non-elements. **Errors** when *adding* a new attribute would
    /// exceed [`max_attrs`] (replacing an existing one always succeeds — it doesn't grow
    /// the list), so an edit can never build an element the parser would later reject for
    /// having too many attributes (parse/edit budget symmetry).
    pub(crate) fn set_attr(&mut self, id: NodeId, key: &[u8], val: &[u8]) -> Result<()> {
        if let Node::Element { attrs, .. } = &mut self.nodes[id.0 as usize].node {
            match attrs.iter_mut().find(|(k, _)| k.as_slice() == key) {
                Some((_, v)) => *v = val.to_vec(),
                None => {
                    if attrs.len() >= max_attrs() {
                        return Err(Error::Docx(
                            "element has too many attributes to add another".into(),
                        ));
                    }
                    attrs.push((key.to_vec(), val.to_vec()));
                }
            }
        }
        Ok(())
    }

    /// Whether [`Self::set_attr`]`(id, key, _)` would succeed: `true` if `id` already has
    /// `key` (replace, no growth) or has room under the attribute cap. Lets an edit
    /// preflight reject — before any mutation — a run that would overflow on a new attr.
    pub(crate) fn can_set_attr(&self, id: NodeId, key: &[u8]) -> bool {
        match &self.nodes[id.0 as usize].node {
            Node::Element { attrs, .. } => {
                attrs.iter().any(|(k, _)| k.as_slice() == key) || attrs.len() < max_attrs()
            }
            _ => true,
        }
    }

    /// Remove an attribute if present (no-op otherwise / on non-elements).
    fn remove_attr(&mut self, id: NodeId, key: &[u8]) {
        if let Node::Element { attrs, .. } = &mut self.nodes[id.0 as usize].node {
            attrs.retain(|(k, _)| k.as_slice() != key);
        }
    }

    /// Number of nodes currently in the arena.
    pub(crate) fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Whether element `id` already has a reusable text carrier — a `Text` child or a
    /// CDATA `Raw` child. If not, [`Self::set_element_text`] must allocate a new node, so
    /// callers preflight the node budget by counting carrier-less matches.
    pub(crate) fn has_text_carrier(&self, id: NodeId) -> bool {
        self.nodes[id.0 as usize]
            .children
            .iter()
            .any(|&c| match &self.nodes[c.0 as usize].node {
                Node::Text(_) => true,
                Node::Raw(r) => r.starts_with(b"<![CDATA["),
                Node::Element { .. } => false,
            })
    }

    /// The document's main body: the `w:body` that is a **direct child** of the sole root
    /// `w:document` (both namespace-resolved), after validating that the part is a single
    /// **well-formed document** — exactly one top-level element, no non-whitespace character
    /// data outside it, and that sole root resolving to `w:document`. Edits anchor on this:
    /// a `document.xml` with several top-level elements (e.g. `<w:document/>…<w:document/>`)
    /// or stray top-level text is malformed and must stay passthrough-only, never promoted
    /// and rewritten into a still-malformed multi-root part. (`XmlTree::parse` itself stays
    /// fragment-tolerant for `insert_fragment_before_ns_local`, whose input legitimately has
    /// multiple top-level nodes.) Schema-aware: a nested or foreign `<…:body>` elsewhere is
    /// never mistaken for the real body.
    pub(crate) fn wml_body_strict(&self) -> Result<NodeId> {
        // Exactly one top-level *element*.
        let mut elements = self
            .roots
            .iter()
            .copied()
            .filter(|&r| matches!(self.nodes[r.0 as usize].node, Node::Element { .. }));
        let root = elements
            .next()
            .ok_or_else(|| Error::Docx("document.xml has no root element".into()))?;
        if elements.next().is_some() {
            return Err(Error::Docx(
                "document.xml has more than one top-level element".into(),
            ));
        }
        // No non-whitespace character data outside the root element. Top-level declaration,
        // comments, and PIs are kept as raw markup and are legal; CDATA is character data,
        // and a doctype is not part of a WordprocessingML document part's body surface.
        for &r in &self.roots {
            if let Node::Raw(bytes) = &self.nodes[r.0 as usize].node {
                let is_ws = bytes
                    .iter()
                    .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'));
                let allowed_markup = bytes.starts_with(b"<!--") || bytes.starts_with(b"<?");
                if !is_ws && !allowed_markup {
                    return Err(Error::Docx(
                        "document.xml has invalid content outside the root element".into(),
                    ));
                }
            }
        }
        // The sole root must be `w:document` and carry a direct `w:body` child.
        let mut scope: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        self.push_xmlns(root, &mut scope);
        if !self.resolves_to(root, WML_NS, b"document", &scope) {
            return Err(Error::Docx(
                "document.xml root is not a WordprocessingML w:document".into(),
            ));
        }
        let mut body = None;
        for &c in &self.nodes[root.0 as usize].children {
            let cb = scope.len();
            self.push_xmlns(c, &mut scope);
            let is_body = self.resolves_to(c, WML_NS, b"body", &scope);
            scope.truncate(cb);
            if is_body && body.replace(c).is_some() {
                return Err(Error::Docx(
                    "document.xml has more than one direct w:body".into(),
                ));
            }
        }
        body.ok_or_else(|| Error::Docx("document.xml has no w:body".into()))
    }

    /// Insert one plain paragraph before an atomic direct body block, or before
    /// final section properties when `block_index` equals the block count.
    pub(crate) fn insert_wml_plain_body_paragraph_under(
        &mut self,
        body: NodeId,
        block_index: usize,
        text: &str,
    ) -> Result<()> {
        let blocks = self.wml_atomic_body_blocks_under(body)?;
        if block_index > blocks.len() {
            return Err(Error::Docx(format!(
                "body block insertion index {block_index} is out of range for {} atomic block positions",
                blocks.len() + 1
            )));
        }

        let content = wml_text_run_content_xml(text);
        let paragraph = if content.is_empty() {
            format!(r#"<w:p xmlns:w="{}"/>"#, wml_ns_str())
        } else {
            format!(
                r#"<w:p xmlns:w="{}"><w:r>{content}</w:r></w:p>"#,
                wml_ns_str()
            )
        };
        if block_index == blocks.len() {
            self.insert_fragment_before_ns_local(body, paragraph.as_bytes(), WML_NS, b"sectPr")
        } else {
            self.insert_fragment_at(body, blocks[block_index].child_index, paragraph.as_bytes())
        }
    }

    /// Remove one conservative atomic direct body block (`w:p`, `w:tbl`, or
    /// `w:sdt`) after validating that body-level structure and range markers do
    /// not make subtree removal ambiguous.
    pub(crate) fn remove_wml_atomic_body_block_under(
        &mut self,
        body: NodeId,
        block_index: usize,
    ) -> Result<()> {
        let blocks = self.wml_atomic_body_blocks_under(body)?;
        let block = blocks.get(block_index).copied().ok_or_else(|| {
            Error::Docx(format!(
                "body block index {block_index} is out of range for {} atomic blocks",
                blocks.len()
            ))
        })?;
        if block.section_boundary {
            return Err(Error::Docx(
                "cannot remove a body block that carries section properties".into(),
            ));
        }
        self.nodes[body.0 as usize]
            .children
            .remove(block.child_index);
        Ok(())
    }

    /// Enumerate the same conservative atomic direct body blocks addressed by
    /// the structural edit methods.
    pub(crate) fn wml_atomic_body_block_kinds_under(
        &self,
        body: NodeId,
    ) -> Result<Vec<WmlAtomicBodyBlockKind>> {
        let blocks = self.wml_atomic_body_blocks_under(body)?;
        let mut kinds = Vec::new();
        kinds
            .try_reserve(blocks.len())
            .map_err(|_| Error::Docx("body block kind inventory: out of memory".into()))?;
        kinds.extend(blocks.into_iter().map(|block| block.kind));
        Ok(kinds)
    }

    /// Reorder one conservative atomic direct body block. `to_index` is the
    /// block's final zero-based position among the same direct block set.
    pub(crate) fn move_wml_atomic_body_block_under(
        &mut self,
        body: NodeId,
        from_index: usize,
        to_index: usize,
    ) -> Result<()> {
        let blocks = self.wml_atomic_body_blocks_under(body)?;
        let count = blocks.len();
        if from_index >= count {
            return Err(Error::Docx(format!(
                "body block source index {from_index} is out of range for {count} atomic blocks"
            )));
        }
        if to_index >= count {
            return Err(Error::Docx(format!(
                "body block destination index {to_index} is out of range for {count} atomic blocks"
            )));
        }
        if from_index == to_index {
            return Ok(());
        }
        let (first, last) = if from_index <= to_index {
            (from_index, to_index)
        } else {
            (to_index, from_index)
        };
        if blocks[first..=last]
            .iter()
            .any(|block| block.section_boundary)
        {
            return Err(Error::Docx(
                "cannot move a body block across a block carrying section properties".into(),
            ));
        }
        let mut order = Vec::new();
        order
            .try_reserve(blocks.len())
            .map_err(|_| Error::Docx("body block move: out of memory".into()))?;
        order.extend(blocks.iter().map(|block| block.node));
        let moved = order.remove(from_index);
        order.insert(to_index, moved);
        for (block, node) in blocks.iter().zip(order) {
            self.nodes[body.0 as usize].children[block.child_index] = node;
        }
        Ok(())
    }

    fn wml_atomic_body_blocks_under(&self, body: NodeId) -> Result<Vec<WmlAtomicBodyBlock>> {
        let children = &self.nodes[body.0 as usize].children;
        let mut blocks = Vec::new();
        blocks
            .try_reserve(children.len())
            .map_err(|_| Error::Docx("body block inventory: out of memory".into()))?;
        let mut scan = WmlBodyRangeScan::default();
        let mut scope = self.ns_scope_at(body);
        let mut seen_direct_sect_pr = false;

        for (child_index, &child) in children.iter().enumerate() {
            match &self.nodes[child.0 as usize].node {
                Node::Text(text) => {
                    if !text.iter().all(u8::is_ascii_whitespace) {
                        return Err(Error::Docx(
                            "document body has non-whitespace text outside a block".into(),
                        ));
                    }
                    continue;
                }
                Node::Raw(raw) => {
                    let is_whitespace = raw.iter().all(u8::is_ascii_whitespace);
                    let is_markup = raw.starts_with(b"<!--") || raw.starts_with(b"<?");
                    if !is_whitespace && !is_markup {
                        return Err(Error::Docx(
                            "document body has unsupported raw content between blocks".into(),
                        ));
                    }
                    continue;
                }
                Node::Element { .. } => {}
            }

            let base = scope.len();
            self.push_xmlns(child, &mut scope);
            if self.resolves_to(child, WML_NS, b"sectPr", &scope) {
                if seen_direct_sect_pr {
                    return Err(Error::Docx(
                        "document body has more than one direct w:sectPr".into(),
                    ));
                }
                seen_direct_sect_pr = true;
                scope.truncate(base);
                continue;
            }

            let kind = if self.resolves_to(child, WML_NS, b"p", &scope) {
                Some(WmlAtomicBodyBlockKind::Paragraph)
            } else if self.resolves_to(child, WML_NS, b"tbl", &scope) {
                Some(WmlAtomicBodyBlockKind::Table)
            } else if self.resolves_to(child, WML_NS, b"sdt", &scope) {
                Some(WmlAtomicBodyBlockKind::ContentControl)
            } else {
                None
            };
            let Some(kind) = kind else {
                let name = self
                    .local_name(child)
                    .map(String::from_utf8_lossy)
                    .unwrap_or_default();
                return Err(Error::Docx(format!(
                    "document body contains unsupported direct element {name:?}"
                )));
            };
            if seen_direct_sect_pr {
                return Err(Error::Docx(
                    "document body has a block after its direct w:sectPr".into(),
                ));
            }

            let owner = blocks.len();
            self.scan_wml_body_ranges(child, owner, &mut scope, &mut scan)?;
            let section_boundary = self.wml_block_has_sect_pr(child, &mut scope);
            blocks.push(WmlAtomicBodyBlock {
                child_index,
                node: child,
                kind,
                section_boundary,
            });
            scope.truncate(base);
        }

        if !scan.paired.is_empty() || !scan.complex_fields.is_empty() || !scan.proofing.is_empty() {
            return Err(Error::Docx(
                "document body has an unclosed range or complex field across body blocks".into(),
            ));
        }
        Ok(blocks)
    }

    fn wml_block_has_sect_pr(&self, block: NodeId, scope: &mut Vec<(Vec<u8>, Vec<u8>)>) -> bool {
        if self.resolves_to(block, WML_NS, b"sectPr", scope) {
            return true;
        }
        for &child in &self.nodes[block.0 as usize].children {
            let base = scope.len();
            self.push_xmlns(child, scope);
            if self.wml_block_has_sect_pr(child, scope) {
                scope.truncate(base);
                return true;
            }
            scope.truncate(base);
        }
        false
    }

    fn scan_wml_body_ranges(
        &self,
        id: NodeId,
        owner: usize,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        scan: &mut WmlBodyRangeScan,
    ) -> Result<()> {
        if self.resolves_to_ns(id, WML_NS, scope) {
            let local = self.local_name(id).unwrap_or_default();
            let attrs = match &self.nodes[id.0 as usize].node {
                Node::Element { attrs, .. } => attrs.as_slice(),
                _ => &[],
            };
            if let Some((family, is_start)) = paired_wml_range_marker(local) {
                let range_id =
                    attr_value_ns_local(attrs, scope, WML_NS, b"id").ok_or_else(|| {
                        Error::Docx(format!(
                            "body range marker {} is missing its id",
                            String::from_utf8_lossy(local)
                        ))
                    })?;
                let key = body_marker_key(family, range_id)?;
                if is_start {
                    if scan.paired.contains_key(key.as_slice()) {
                        return Err(Error::Docx(
                            "document body has a duplicate open range".into(),
                        ));
                    }
                    scan.paired
                        .try_reserve(1)
                        .map_err(|_| Error::Docx("body range inventory: out of memory".into()))?;
                    scan.paired.insert(key, owner);
                } else {
                    let Some(start_owner) = scan.paired.remove(key.as_slice()) else {
                        return Err(Error::Docx(
                            "document body has a closing range without a matching start".into(),
                        ));
                    };
                    if start_owner != owner {
                        return Err(Error::Docx(
                            "document body range crosses atomic body blocks".into(),
                        ));
                    }
                    if family == b"comment" {
                        let id = body_marker_id(range_id)?;
                        if scan.comment_ranges.contains_key(id.as_slice()) {
                            return Err(Error::Docx(
                                "document body has a duplicate comment range".into(),
                            ));
                        }
                        if scan
                            .comment_references
                            .get(id.as_slice())
                            .is_some_and(|reference_owner| *reference_owner != owner)
                        {
                            return Err(Error::Docx(
                                "document body comment range and reference cross atomic body blocks"
                                    .into(),
                            ));
                        }
                        scan.comment_ranges.try_reserve(1).map_err(|_| {
                            Error::Docx("comment range inventory: out of memory".into())
                        })?;
                        scan.comment_ranges.insert(id, owner);
                    }
                }
            } else if local == b"commentReference" {
                let comment_id =
                    attr_value_ns_local(attrs, scope, WML_NS, b"id").ok_or_else(|| {
                        Error::Docx("body comment reference is missing its id".into())
                    })?;
                if scan.comment_references.contains_key(comment_id) {
                    return Err(Error::Docx(
                        "document body has a duplicate comment reference".into(),
                    ));
                }
                if scan
                    .comment_ranges
                    .get(comment_id)
                    .is_some_and(|range_owner| *range_owner != owner)
                {
                    return Err(Error::Docx(
                        "document body comment range and reference cross atomic body blocks".into(),
                    ));
                }
                let id = body_marker_id(comment_id)?;
                scan.comment_references.try_reserve(1).map_err(|_| {
                    Error::Docx("comment reference inventory: out of memory".into())
                })?;
                scan.comment_references.insert(id, owner);
            } else if local == b"fldChar" {
                match attr_value_ns_local(attrs, scope, WML_NS, b"fldCharType") {
                    Some(b"begin") => {
                        scan.complex_fields.try_reserve(1).map_err(|_| {
                            Error::Docx("complex field inventory: out of memory".into())
                        })?;
                        scan.complex_fields.push((owner, false));
                    }
                    Some(b"separate") => {
                        let Some((start_owner, separated)) = scan.complex_fields.last_mut() else {
                            return Err(Error::Docx(
                                "document body has a field separator without a begin marker".into(),
                            ));
                        };
                        if *start_owner != owner {
                            return Err(Error::Docx(
                                "document body complex field crosses atomic body blocks".into(),
                            ));
                        }
                        if *separated {
                            return Err(Error::Docx(
                                "document body complex field has more than one separator".into(),
                            ));
                        }
                        *separated = true;
                    }
                    Some(b"end") => {
                        let Some((start_owner, _)) = scan.complex_fields.pop() else {
                            return Err(Error::Docx(
                                "document body has a field end without a begin marker".into(),
                            ));
                        };
                        if start_owner != owner {
                            return Err(Error::Docx(
                                "document body complex field crosses atomic body blocks".into(),
                            ));
                        }
                    }
                    _ => {
                        return Err(Error::Docx(
                            "document body has a malformed complex field marker".into(),
                        ));
                    }
                }
            } else if local == b"proofErr" {
                let proof_type =
                    attr_value_ns_local(attrs, scope, WML_NS, b"type").ok_or_else(|| {
                        Error::Docx("document body proofing range is missing its type".into())
                    })?;
                if let Some(family) = proof_type.strip_suffix(b"Start") {
                    if scan.proofing.contains_key(family) {
                        return Err(Error::Docx(
                            "document body has a duplicate open proofing range".into(),
                        ));
                    }
                    let key = body_marker_id(family)?;
                    scan.proofing.try_reserve(1).map_err(|_| {
                        Error::Docx("proofing range inventory: out of memory".into())
                    })?;
                    scan.proofing.insert(key, owner);
                } else if let Some(family) = proof_type.strip_suffix(b"End") {
                    let Some(start_owner) = scan.proofing.remove(family) else {
                        return Err(Error::Docx(
                            "document body has a proofing range end without a start".into(),
                        ));
                    };
                    if start_owner != owner {
                        return Err(Error::Docx(
                            "document body proofing range crosses atomic body blocks".into(),
                        ));
                    }
                } else {
                    return Err(Error::Docx(
                        "document body has a malformed proofing range type".into(),
                    ));
                }
            }
        }

        for &child in &self.nodes[id.0 as usize].children {
            let base = scope.len();
            self.push_xmlns(child, scope);
            self.scan_wml_body_ranges(child, owner, scope, scan)?;
            scope.truncate(base);
        }
        Ok(())
    }

    /// The sole top-level WML root of a non-body WordprocessingML part, such as
    /// `w:hdr` or `w:ftr`. This applies the same strict outside-root checks as
    /// [`Self::wml_body_strict`] before returning the root element itself.
    pub(crate) fn wml_part_root_strict(
        &self,
        part_name: &str,
        root_local: &[u8],
    ) -> Result<NodeId> {
        self.part_root_strict_ns(part_name, WML_NS, root_local, "WordprocessingML w")
    }

    /// The sole top-level WordprocessingML root of an XML part, accepting any
    /// local root name. Use this for explicit part-scoped edits where the caller
    /// already named the part and the edit surface is limited to descendant `w:t`.
    pub(crate) fn wml_any_part_root_strict(&self, part_name: &str) -> Result<NodeId> {
        self.part_root_strict_ns_any_local(part_name, WML_NS, "WordprocessingML w")
    }

    /// The sole top-level root of an XML part, namespace-resolved and with the same
    /// outside-root safety checks used by WordprocessingML edit surfaces.
    pub(crate) fn part_root_strict_ns(
        &self,
        part_name: &str,
        root_ns: &[u8],
        root_local: &[u8],
        expected_prefix: &str,
    ) -> Result<NodeId> {
        let root = self.part_root_strict_ns_any_local(part_name, root_ns, expected_prefix)?;
        let mut scope: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        self.push_xmlns(root, &mut scope);
        if self.resolves_to(root, root_ns, root_local, &scope) {
            Ok(root)
        } else {
            let expected = String::from_utf8_lossy(root_local);
            Err(Error::Docx(format!(
                "{part_name} root is not a {expected_prefix}:{expected}"
            )))
        }
    }

    fn part_root_strict_ns_any_local(
        &self,
        part_name: &str,
        root_ns: &[u8],
        expected_prefix: &str,
    ) -> Result<NodeId> {
        let mut elements = self
            .roots
            .iter()
            .copied()
            .filter(|&r| matches!(self.nodes[r.0 as usize].node, Node::Element { .. }));
        let root = elements
            .next()
            .ok_or_else(|| Error::Docx(format!("{part_name} has no root element")))?;
        if elements.next().is_some() {
            return Err(Error::Docx(format!(
                "{part_name} has more than one top-level element"
            )));
        }
        for &r in &self.roots {
            if let Node::Raw(bytes) = &self.nodes[r.0 as usize].node {
                let is_ws = bytes
                    .iter()
                    .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'));
                let allowed_markup = bytes.starts_with(b"<!--") || bytes.starts_with(b"<?");
                if !is_ws && !allowed_markup {
                    return Err(Error::Docx(format!(
                        "{part_name} has invalid content outside the root element"
                    )));
                }
            }
        }
        let mut scope: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        self.push_xmlns(root, &mut scope);
        if self.resolves_to_ns(root, root_ns, &scope) {
            Ok(root)
        } else {
            Err(Error::Docx(format!(
                "{part_name} root is not in the {expected_prefix} namespace"
            )))
        }
    }

    /// Append `id`'s own `xmlns`/`xmlns:*` declarations onto `scope`.
    fn push_xmlns(&self, id: NodeId, scope: &mut Vec<(Vec<u8>, Vec<u8>)>) {
        if let Node::Element { attrs, .. } = &self.nodes[id.0 as usize].node {
            for (k, v) in attrs {
                if k.as_slice() == b"xmlns" {
                    scope.push((Vec::new(), v.clone()));
                } else if let Some(p) = k.strip_prefix(b"xmlns:".as_slice()) {
                    scope.push((p.to_vec(), v.clone()));
                }
            }
        }
    }

    /// Whether element `id`'s qualified name resolves to (`ns`, `local`) under `scope`
    /// (which must already include `id`'s own xmlns declarations).
    fn resolves_to(
        &self,
        id: NodeId,
        ns: &[u8],
        local: &[u8],
        scope: &[(Vec<u8>, Vec<u8>)],
    ) -> bool {
        let Node::Element { name, .. } = &self.nodes[id.0 as usize].node else {
            return false;
        };
        let (prefix, lname): (&[u8], &[u8]) = match name.iter().position(|&b| b == b':') {
            Some(i) => (&name[..i], &name[i + 1..]),
            None => (b"", name),
        };
        lname == local
            && scope
                .iter()
                .rev()
                .find(|(p, _)| p.as_slice() == prefix)
                .map(|(_, u)| u.as_slice())
                == Some(ns)
    }

    fn resolves_to_ns(&self, id: NodeId, ns: &[u8], scope: &[(Vec<u8>, Vec<u8>)]) -> bool {
        let Node::Element { name, .. } = &self.nodes[id.0 as usize].node else {
            return false;
        };
        let prefix: &[u8] = match name.iter().position(|&b| b == b':') {
            Some(i) => &name[..i],
            None => b"",
        };
        scope
            .iter()
            .rev()
            .find(|(p, _)| p.as_slice() == prefix)
            .map(|(_, u)| u.as_slice())
            == Some(ns)
    }

    fn selected_alternate_content_branch(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
    ) -> AlternateContentBranch {
        if !self.resolves_to(id, MC_NS, b"AlternateContent", scope) {
            return AlternateContentBranch::NotAlternateContent;
        }
        for i in 0..self.nodes[id.0 as usize].children.len() {
            let child = self.nodes[id.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(child, scope);
            let selected = self.resolves_to(child, MC_NS, b"Choice", scope)
                || self.resolves_to(child, MC_NS, b"Fallback", scope);
            scope.truncate(base);
            if selected {
                return AlternateContentBranch::Selected(child);
            }
        }
        AlternateContentBranch::Missing
    }

    fn alternate_content_matches_local_reader_policy(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
    ) -> bool {
        let base = scope.len();
        self.push_xmlns(id, scope);
        if self.wml_revision_edit_action(id, scope, WmlRevisionEditPolicy::Accept)
            == WmlRevisionEditAction::Remove
        {
            scope.truncate(base);
            return true;
        }
        if self.local_name(id) == Some(b"AlternateContent") {
            if !self.resolves_to(id, MC_NS, b"AlternateContent", scope) {
                scope.truncate(base);
                return false;
            }
            let mut first_local_branch = None;
            let mut first_mc_branch = None;
            for i in 0..self.nodes[id.0 as usize].children.len() {
                let child = self.nodes[id.0 as usize].children[i];
                let child_base = scope.len();
                self.push_xmlns(child, scope);
                if first_local_branch.is_none()
                    && matches!(self.local_name(child), Some(b"Choice") | Some(b"Fallback"))
                {
                    first_local_branch = Some(child);
                }
                if first_mc_branch.is_none()
                    && (self.resolves_to(child, MC_NS, b"Choice", scope)
                        || self.resolves_to(child, MC_NS, b"Fallback", scope))
                {
                    first_mc_branch = Some(child);
                }
                scope.truncate(child_base);
            }
            let compatible = match (first_local_branch, first_mc_branch) {
                (Some(local_branch), Some(mc_branch)) if local_branch == mc_branch => {
                    self.alternate_content_matches_local_reader_policy(local_branch, scope)
                }
                _ => false,
            };
            scope.truncate(base);
            return compatible;
        }
        for i in 0..self.nodes[id.0 as usize].children.len() {
            let child = self.nodes[id.0 as usize].children[i];
            if !self.alternate_content_matches_local_reader_policy(child, scope) {
                scope.truncate(base);
                return false;
            }
        }
        scope.truncate(base);
        true
    }

    fn scope_binds_prefix(scope: &[(Vec<u8>, Vec<u8>)], prefix: &[u8], ns: &[u8]) -> bool {
        scope
            .iter()
            .rev()
            .find(|(p, _)| p.as_slice() == prefix)
            .map(|(_, u)| u.as_slice())
            == Some(ns)
    }

    fn fallback_attr_needed(scope: &[(Vec<u8>, Vec<u8>)], name: &str, value: &str) -> bool {
        if name == "xmlns" {
            !Self::scope_binds_prefix(scope, b"", value.as_bytes())
        } else if let Some(prefix) = name.strip_prefix("xmlns:") {
            !Self::scope_binds_prefix(scope, prefix.as_bytes(), value.as_bytes())
        } else {
            true
        }
    }

    /// The namespace bindings (`prefix → uri`, `""` = default) in effect for `target`'s
    /// children — every `xmlns`/`xmlns:*` from the document root down to and including
    /// `target` itself. Used to resolve the namespace of `target`'s direct children.
    fn ns_scope_at(&self, target: NodeId) -> Vec<(Vec<u8>, Vec<u8>)> {
        fn rec(
            t: &XmlTree,
            id: NodeId,
            target: NodeId,
            scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        ) -> bool {
            let base = scope.len();
            if let Node::Element { attrs, .. } = &t.nodes[id.0 as usize].node {
                for (k, v) in attrs {
                    if k.as_slice() == b"xmlns" {
                        scope.push((Vec::new(), v.clone()));
                    } else if let Some(p) = k.strip_prefix(b"xmlns:".as_slice()) {
                        scope.push((p.to_vec(), v.clone()));
                    }
                }
            }
            if id == target {
                return true; // keep scope (including target's own declarations)
            }
            for i in 0..t.nodes[id.0 as usize].children.len() {
                let c = t.nodes[id.0 as usize].children[i];
                if rec(t, c, target, scope) {
                    return true;
                }
            }
            scope.truncate(base);
            false
        }
        let mut scope = Vec::new();
        for r in 0..self.roots.len() {
            if rec(self, self.roots[r], target, &mut scope) {
                break;
            }
        }
        scope
    }

    /// Every `t` element **under `body`** that resolves to the **WordprocessingML**
    /// namespace — i.e. genuine `w:t` body text, *including* text-box runs in the
    /// selected branch of nested `w:drawing`/`mc:AlternateContent`, while correctly
    /// excluding untaken branches and DrawingML text (`a:t`, or a bare `<t>` under a
    /// subtree that binds DrawingML as the default namespace). The walk is anchored
    /// to `body` (typically [`Self::wml_body_strict`]), so a stray `w:t` *sibling* of
    /// the body — in malformed or extension-heavy input — is not touched. The
    /// namespace scope is seeded from `body`'s ancestors + own declarations so
    /// prefixes still resolve.
    pub(crate) fn wml_text_runs_under(&self, body: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut scope = self.ns_scope_at(body); // ancestors + body's own xmlns
        for i in 0..self.nodes[body.0 as usize].children.len() {
            let c = self.nodes[body.0 as usize].children[i];
            self.collect_wml_t(c, &mut scope, &mut out);
        }
        out
    }

    /// Every descendant WordprocessingML `w:t` under `root`, retaining text from
    /// every `mc:AlternateContent` branch. This is reserved for the explicit
    /// part-level text escape hatch; semantic edits use [`Self::wml_text_runs_under`].
    pub(crate) fn wml_all_branch_text_runs_under(&self, root: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut scope = self.ns_scope_at(root);
        for i in 0..self.nodes[root.0 as usize].children.len() {
            let child = self.nodes[root.0 as usize].children[i];
            self.collect_wml_t_with_alternate_content(
                child,
                &mut scope,
                &mut out,
                AlternateContentTraversal::All,
            );
        }
        out
    }

    /// Relationship ids from body-scoped `w:hyperlink r:id="..."` elements in XML order.
    pub(crate) fn wml_hyperlink_rids_under(&self, body: NodeId) -> Vec<String> {
        let mut out = Vec::new();
        let mut scope = self.ns_scope_at(body);
        for i in 0..self.nodes[body.0 as usize].children.len() {
            let c = self.nodes[body.0 as usize].children[i];
            self.collect_wml_hyperlink_rids(c, &mut scope, &mut out);
        }
        out
    }

    /// OPC relationship ids referenced by namespace-qualified XML attributes.
    /// Scanning the parsed tree avoids substring matches in text, comments, or
    /// unrelated attributes and lets callers check many ids in one traversal.
    pub(crate) fn relationship_references(&self) -> HashSet<Vec<u8>> {
        fn walk(
            tree: &XmlTree,
            id: NodeId,
            scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
            out: &mut HashSet<Vec<u8>>,
        ) {
            let base = scope.len();
            if let Node::Element { attrs, .. } = &tree.nodes[id.0 as usize].node {
                for (key, value) in attrs {
                    if key == b"xmlns" {
                        scope.push((Vec::new(), value.clone()));
                    } else if let Some(prefix) = key.strip_prefix(b"xmlns:".as_slice()) {
                        scope.push((prefix.to_vec(), value.clone()));
                    }
                }
                for (key, value) in attrs {
                    let Some(separator) = key.iter().position(|&byte| byte == b':') else {
                        continue;
                    };
                    let prefix = &key[..separator];
                    let namespace = scope
                        .iter()
                        .rev()
                        .find(|(bound_prefix, _)| bound_prefix.as_slice() == prefix)
                        .map(|(_, uri)| uri.as_slice());
                    if namespace == Some(OOXML_REL_NS) {
                        out.insert(value.clone());
                    }
                }
            }

            for &child in &tree.nodes[id.0 as usize].children {
                walk(tree, child, scope, out);
            }
            scope.truncate(base);
        }

        let mut references = HashSet::new();
        let mut scope = Vec::new();
        for &root in &self.roots {
            walk(self, root, &mut scope, &mut references);
        }
        references
    }

    /// Cached result text and tab/break marker nodes for the zero-based field in
    /// body order. The order matches the public `Document::fields()` extraction:
    /// simple fields are counted at `w:fldSimple`, while complex fields are counted
    /// when their `w:fldChar w:fldCharType="end"` marker closes the cached result.
    pub(crate) fn wml_field_result_text_group_under(
        &self,
        body: NodeId,
        field_index: usize,
    ) -> Option<WmlTextRunGroup> {
        let mut scan = FieldScan {
            wanted: field_index,
            seen: 0,
            target: None,
            complex: Vec::new(),
        };
        let mut scope = self.ns_scope_at(body);
        for i in 0..self.nodes[body.0 as usize].children.len() {
            if scan.target.is_some() {
                break;
            }
            let c = self.nodes[body.0 as usize].children[i];
            self.collect_wml_field_results(c, Some(body), &mut scope, false, &mut scan);
        }
        scan.target
    }

    /// Raw instructions for fields in the exact namespace-aware order used by
    /// [`Self::wml_field_result_text_group_under`].
    pub(crate) fn wml_field_instructions_under(&self, body: NodeId) -> Vec<String> {
        let mut scan = FieldInstructionScan::default();
        let mut scope = self.ns_scope_at(body);
        for i in 0..self.nodes[body.0 as usize].children.len() {
            let child = self.nodes[body.0 as usize].children[i];
            self.collect_wml_field_instructions(child, &mut scope, &mut scan);
        }
        scan.fields
    }

    /// Whether the namespace-aware editor and the current local-name streaming
    /// field reader choose the same compatibility branch everywhere under `body`.
    pub(crate) fn wml_field_alternate_content_policy_matches_reader(&self, body: NodeId) -> bool {
        let mut scope = self.ns_scope_at(body);
        for i in 0..self.nodes[body.0 as usize].children.len() {
            let child = self.nodes[body.0 as usize].children[i];
            if !self.alternate_content_matches_local_reader_policy(child, &mut scope) {
                return false;
            }
        }
        true
    }

    /// Visible text and tab/break marker nodes for body content controls whose
    /// `w:sdtPr/w:tag/@w:val` exactly matches `tag`, in body order.
    pub(crate) fn wml_content_control_text_groups_by_tag_under(
        &self,
        body: NodeId,
        tag: &str,
    ) -> Vec<WmlTextRunGroup> {
        let mut out = Vec::new();
        let mut scope = self.ns_scope_at(body);
        for i in 0..self.nodes[body.0 as usize].children.len() {
            let c = self.nodes[body.0 as usize].children[i];
            self.collect_wml_content_control_text_by_tag(c, &mut scope, tag.as_bytes(), &mut out);
        }
        out
    }

    /// Real note entry roots directly under a footnotes/endnotes root or its
    /// selected compatibility branches, excluding Word's separator boilerplate.
    pub(crate) fn wml_real_note_entries_under(
        &self,
        root: NodeId,
        note_local: &[u8],
    ) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut scope = self.ns_scope_at(root);
        for i in 0..self.nodes[root.0 as usize].children.len() {
            let child = self.nodes[root.0 as usize].children[i];
            self.collect_wml_real_note_entry(child, &mut scope, note_local, &mut out);
        }
        out
    }

    /// Accept tracked revision markup under a WML body subtree.
    ///
    /// Insertions and move destinations are unwrapped so their current content
    /// remains. Deletions, move sources, and tracked property-change history are
    /// removed. The edit is structural only and does not allocate new nodes.
    pub(crate) fn accept_wml_revisions_under(&mut self, body: NodeId) -> usize {
        let mut scope = self.ns_scope_at(body);
        self.edit_wml_revisions_descendants(body, &mut scope, WmlRevisionEditPolicy::Accept)
    }

    /// Reject tracked revision markup under a WML body subtree.
    ///
    /// Deletions and move sources are unwrapped so the original content remains,
    /// with `w:delText` normalized back to `w:t`. Insertions, move destinations,
    /// and tracked property-change history are removed.
    pub(crate) fn reject_wml_revisions_under(&mut self, body: NodeId) -> usize {
        let mut scope = self.ns_scope_at(body);
        self.edit_wml_revisions_descendants(body, &mut scope, WmlRevisionEditPolicy::Reject)
    }

    /// Replace the visible text for one `w:comment`, using WML run markers for tabs
    /// and breaks while clearing any later existing text runs.
    pub(crate) fn set_wml_comment_text_under(
        &mut self,
        root: NodeId,
        comment_id: &str,
        text: &str,
    ) -> Result<()> {
        let target = self
            .wml_comment_text_edit_target_under(root, comment_id)
            .ok_or_else(|| Error::Docx(format!("comment id {comment_id:?} not found")))?;
        let mut text_runs = target.text_runs.into_iter();
        let first_text = text_runs
            .next()
            .ok_or_else(|| Error::Docx(format!("comment id {comment_id:?} has no visible text")))?;

        self.replace_wml_text_element_with_run_content(first_text, text)?;
        for id in text_runs {
            self.set_element_text(id, "")?;
        }
        Ok(())
    }

    /// Cached visible direct-content `w:t` nodes for a physical cell in a top-level
    /// body table. Descendant nested tables are excluded so a parent-cell edit
    /// cannot rewrite a nested table's text.
    pub(crate) fn wml_table_cell_text_runs_under(
        &self,
        body: NodeId,
        table_index: usize,
        row_index: usize,
        cell_index: usize,
    ) -> Option<Vec<NodeId>> {
        let table = self.wml_body_table_at(body, table_index)?;
        let mut scope = self.ns_scope_at(table);
        self.wml_table_cell_text_runs(table, &mut scope, row_index, cell_index)
    }

    fn wml_body_table_at(&self, body: NodeId, table_index: usize) -> Option<NodeId> {
        let mut seen = 0usize;
        let mut scope = self.ns_scope_at(body);
        for i in 0..self.nodes[body.0 as usize].children.len() {
            let c = self.nodes[body.0 as usize].children[i];
            if let Some(table) = self.find_wml_body_table(c, &mut scope, table_index, &mut seen) {
                return Some(table);
            }
        }
        None
    }

    fn find_wml_body_table(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        table_index: usize,
        seen: &mut usize,
    ) -> Option<NodeId> {
        let base = scope.len();
        self.push_xmlns(id, scope);
        if self.wml_revision_edit_action(id, scope, WmlRevisionEditPolicy::Accept)
            == WmlRevisionEditAction::Remove
        {
            scope.truncate(base);
            return None;
        }
        if self.resolves_to(id, WML_NS, b"tbl", scope) {
            let found = (*seen == table_index).then_some(id);
            *seen += 1;
            scope.truncate(base);
            return found;
        }
        if self.is_wml_body_table_wrapper(id, scope) {
            for i in 0..self.nodes[id.0 as usize].children.len() {
                let c = self.nodes[id.0 as usize].children[i];
                if let Some(table) = self.find_wml_body_table(c, scope, table_index, seen) {
                    scope.truncate(base);
                    return Some(table);
                }
            }
        }
        scope.truncate(base);
        None
    }

    fn is_wml_body_table_wrapper(&self, id: NodeId, scope: &[(Vec<u8>, Vec<u8>)]) -> bool {
        [
            b"sdt".as_slice(),
            b"sdtContent".as_slice(),
            b"customXml".as_slice(),
            b"smartTag".as_slice(),
            b"ins".as_slice(),
            b"moveTo".as_slice(),
        ]
        .into_iter()
        .any(|local| self.resolves_to(id, WML_NS, local, scope))
    }

    /// Cached visible `w:t` nodes for real footnote/endnote entries under a notes root,
    /// skipping separator/continuation boilerplate.
    pub(crate) fn wml_note_text_runs_under(&self, root: NodeId, note_local: &[u8]) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut scope = self.ns_scope_at(root);
        for i in 0..self.nodes[root.0 as usize].children.len() {
            let c = self.nodes[root.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(c, &mut scope);
            if self.resolves_to(c, WML_NS, note_local, &scope) {
                let skip = match &self.nodes[c.0 as usize].node {
                    Node::Element { attrs, .. } => {
                        attr_value_local(attrs, b"type").is_some_and(|value| {
                            matches!(
                                trim_ascii_whitespace(value),
                                b"separator" | b"continuationSeparator" | b"continuationNotice"
                            )
                        })
                    }
                    _ => false,
                };
                if !skip {
                    self.collect_wml_t(c, &mut scope, &mut out);
                }
            }
            scope.truncate(base);
        }
        out
    }

    /// Insert Word comment range markers around the first adjacent `w:r` sequence under
    /// `body` whose visible `w:t` text exactly equals `anchor_text`.
    pub(crate) fn add_wml_comment_anchor_on_text(
        &mut self,
        body: NodeId,
        anchor_text: &str,
        comment_id: &str,
    ) -> Result<()> {
        let mut scope = self.ns_scope_at(body);
        let mut target = None;
        for i in 0..self.nodes[body.0 as usize].children.len() {
            let c = self.nodes[body.0 as usize].children[i];
            if self.find_wml_run_range_with_text(c, &mut scope, anchor_text, &mut target) {
                break;
            }
        }
        let Some(range) = target else {
            return Err(Error::Docx(format!(
                "comment anchor text {anchor_text:?} not found"
            )));
        };

        let id = escaped_attr(comment_id);
        let start = format!(
            r#"<w:commentRangeStart xmlns:w="{}" w:id="{id}"/>"#,
            wml_ns_str()
        );
        let end = format!(
            r#"<w:commentRangeEnd xmlns:w="{}" w:id="{id}"/><w:r xmlns:w="{}"><w:commentReference w:id="{id}"/></w:r>"#,
            wml_ns_str(),
            wml_ns_str()
        );
        let (first_parent, first_index) =
            self.parent_child_index(range.first_run).ok_or_else(|| {
                Error::Docx("comment anchor start run is detached from the XML tree".into())
            })?;
        self.insert_fragment_at(first_parent, first_index, start.as_bytes())?;
        let (last_parent, last_index) =
            self.parent_child_index(range.last_run).ok_or_else(|| {
                Error::Docx("comment anchor end run is detached from the XML tree".into())
            })?;
        self.insert_fragment_at(last_parent, last_index + 1, end.as_bytes())?;
        Ok(())
    }

    /// Insert a Word footnote reference run after the first adjacent `w:r` sequence
    /// under `body` whose visible `w:t` text exactly equals `anchor_text`.
    pub(crate) fn add_wml_footnote_reference_on_text(
        &mut self,
        body: NodeId,
        anchor_text: &str,
        footnote_id: &str,
    ) -> Result<()> {
        let mut scope = self.ns_scope_at(body);
        let mut target = None;
        for i in 0..self.nodes[body.0 as usize].children.len() {
            let c = self.nodes[body.0 as usize].children[i];
            if self.find_wml_run_range_with_text(c, &mut scope, anchor_text, &mut target) {
                break;
            }
        }
        let Some(range) = target else {
            return Err(Error::Docx(format!(
                "footnote anchor text {anchor_text:?} not found"
            )));
        };

        let id = escaped_attr(footnote_id);
        let reference = format!(
            r#"<w:r xmlns:w="{}"><w:footnoteReference w:id="{id}"/></w:r>"#,
            wml_ns_str()
        );
        let (parent, index) = self.parent_child_index(range.last_run).ok_or_else(|| {
            Error::Docx("footnote anchor run is detached from the XML tree".into())
        })?;
        self.insert_fragment_at(parent, index + 1, reference.as_bytes())
    }

    /// Insert a Word endnote reference run after the first adjacent `w:r` sequence under
    /// `body` whose visible `w:t` text exactly equals `anchor_text`.
    pub(crate) fn add_wml_endnote_reference_on_text(
        &mut self,
        body: NodeId,
        anchor_text: &str,
        endnote_id: &str,
    ) -> Result<()> {
        let mut scope = self.ns_scope_at(body);
        let mut target = None;
        for i in 0..self.nodes[body.0 as usize].children.len() {
            let c = self.nodes[body.0 as usize].children[i];
            if self.find_wml_run_range_with_text(c, &mut scope, anchor_text, &mut target) {
                break;
            }
        }
        let Some(range) = target else {
            return Err(Error::Docx(format!(
                "endnote anchor text {anchor_text:?} not found"
            )));
        };

        let id = escaped_attr(endnote_id);
        let reference = format!(
            r#"<w:r xmlns:w="{}"><w:endnoteReference w:id="{id}"/></w:r>"#,
            wml_ns_str()
        );
        let (parent, index) = self.parent_child_index(range.last_run).ok_or_else(|| {
            Error::Docx("endnote anchor run is detached from the XML tree".into())
        })?;
        self.insert_fragment_at(parent, index + 1, reference.as_bytes())
    }

    /// Append one `w:comment` record to a `w:comments` root.
    pub(crate) fn append_wml_comment(
        &mut self,
        root: NodeId,
        comment_id: &str,
        text: &str,
        author: &str,
    ) -> Result<()> {
        let id = escaped_attr(comment_id);
        let author = escaped_attr(author);
        let text = wml_text_run_content_xml(text);
        let xml = format!(
            r#"<w:comment xmlns:w="{}" w:id="{id}" w:author="{author}"><w:p><w:r>{text}</w:r></w:p></w:comment>"#,
            wml_ns_str()
        );
        let pos = self.nodes[root.0 as usize].children.len();
        self.insert_fragment_at(root, pos, xml.as_bytes())
    }

    /// Append one real `w:footnote` record to a `w:footnotes` root.
    pub(crate) fn append_wml_footnote(
        &mut self,
        root: NodeId,
        footnote_id: &str,
        text: &str,
    ) -> Result<()> {
        let id = escaped_attr(footnote_id);
        let text = wml_text_run_content_xml(text);
        let xml = format!(
            r#"<w:footnote xmlns:w="{}" w:id="{id}"><w:p><w:r>{text}</w:r></w:p></w:footnote>"#,
            wml_ns_str()
        );
        let pos = self.nodes[root.0 as usize].children.len();
        self.insert_fragment_at(root, pos, xml.as_bytes())
    }

    /// Append one real `w:endnote` record to a `w:endnotes` root.
    pub(crate) fn append_wml_endnote(
        &mut self,
        root: NodeId,
        endnote_id: &str,
        text: &str,
    ) -> Result<()> {
        let id = escaped_attr(endnote_id);
        let text = wml_text_run_content_xml(text);
        let xml = format!(
            r#"<w:endnote xmlns:w="{}" w:id="{id}"><w:p><w:r>{text}</w:r></w:p></w:endnote>"#,
            wml_ns_str()
        );
        let pos = self.nodes[root.0 as usize].children.len();
        self.insert_fragment_at(root, pos, xml.as_bytes())
    }

    /// Set a direct child element's text by namespace/local-name, appending a new
    /// namespaced element fragment with extra fallback attributes when absent.
    pub(crate) fn set_child_text_ns_local_with_attrs(
        &mut self,
        root: NodeId,
        ns: &[u8],
        local: &[u8],
        fallback_qname: &str,
        fallback_attrs: &[(&str, &str)],
        text: &str,
    ) -> Result<()> {
        let mut scope = self.ns_scope_at(root);
        for i in 0..self.nodes[root.0 as usize].children.len() {
            let c = self.nodes[root.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(c, &mut scope);
            if self.resolves_to(c, ns, local, &scope) {
                scope.truncate(base);
                for (name, value) in fallback_attrs {
                    if Self::fallback_attr_needed(&scope, name, value) {
                        self.set_attr(c, name.as_bytes(), value.as_bytes())?;
                    }
                }
                return self.set_element_text(c, text);
            }
            scope.truncate(base);
        }

        let prefix = fallback_qname
            .split_once(':')
            .map(|(prefix, _)| prefix)
            .unwrap_or("");
        let prefix_bytes = prefix.as_bytes();
        let xmlns = if Self::scope_binds_prefix(&scope, prefix_bytes, ns) {
            None
        } else if prefix.is_empty() {
            Some(format!(
                r#"xmlns="{}""#,
                escaped_attr(&String::from_utf8_lossy(ns))
            ))
        } else {
            Some(format!(
                r#"xmlns:{prefix}="{}""#,
                escaped_attr(&String::from_utf8_lossy(ns))
            ))
        };
        let mut attrs = String::new();
        if let Some(xmlns) = xmlns {
            attrs.push_str(&xmlns);
        }
        for (name, value) in fallback_attrs {
            if !Self::fallback_attr_needed(&scope, name, value) {
                continue;
            }
            if !attrs.is_empty() {
                attrs.push(' ');
            }
            attrs.push_str(name);
            attrs.push_str("=\"");
            attrs.push_str(&escaped_attr(value));
            attrs.push('"');
        }
        let text = escaped_text(text);
        let xml = if attrs.is_empty() {
            format!("<{fallback_qname}>{text}</{fallback_qname}>")
        } else {
            format!("<{fallback_qname} {attrs}>{text}</{fallback_qname}>")
        };
        let pos = self.nodes[root.0 as usize].children.len();
        self.insert_fragment_at(root, pos, xml.as_bytes())
    }

    /// Whole-document variant (test-only): collect WML `t` from every root, threading
    /// bindings top-down. Production edits use the body-anchored [`Self::wml_text_runs_under`].
    #[cfg(test)]
    pub(crate) fn wml_text_runs(&self) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut scope: Vec<(Vec<u8>, Vec<u8>)> = Vec::new(); // (prefix, uri); "" = default
        for r in 0..self.roots.len() {
            self.collect_wml_t(self.roots[r], &mut scope, &mut out);
        }
        out
    }

    fn wml_table_cell_text_runs(
        &self,
        table: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        row_index: usize,
        cell_index: usize,
    ) -> Option<Vec<NodeId>> {
        let cell = self.wml_table_cell_at(table, scope, row_index, cell_index)?;
        let mut runs = Vec::new();
        let mut cell_scope = self.ns_scope_at(cell);
        for i in 0..self.nodes[cell.0 as usize].children.len() {
            let c = self.nodes[cell.0 as usize].children[i];
            self.collect_wml_t_excluding_tables(c, &mut cell_scope, &mut runs);
        }
        Some(runs)
    }

    fn wml_table_cell_at(
        &self,
        table: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        row_index: usize,
        cell_index: usize,
    ) -> Option<NodeId> {
        let mut seen = 0usize;
        let mut active_vmerges = Vec::new();
        for i in 0..self.nodes[table.0 as usize].children.len() {
            let c = self.nodes[table.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(c, scope);
            if self.resolves_to(c, WML_NS, b"tr", scope) {
                let result = self.wml_row_cell_at(c, scope, cell_index, &mut active_vmerges);
                if seen == row_index {
                    scope.truncate(base);
                    return result;
                }
                seen += 1;
            }
            scope.truncate(base);
        }
        None
    }

    fn wml_row_cell_at(
        &self,
        row: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        cell_index: usize,
        active_vmerges: &mut Vec<WmlActiveVMerge>,
    ) -> Option<NodeId> {
        let previous_vmerges = active_vmerges.clone();
        let mut next_vmerges = Vec::new();
        let mut found = None;
        let mut logical_col = 0usize;
        for i in 0..self.nodes[row.0 as usize].children.len() {
            let c = self.nodes[row.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(c, scope);
            if self.resolves_to(c, WML_NS, b"tc", scope) {
                let vmerge = self.wml_table_cell_vmerge(c, scope);
                let active = previous_vmerges.iter().find(|m| m.col == logical_col);
                let span = if vmerge == WmlVMerge::Continue {
                    active.map_or_else(
                        || self.wml_table_cell_grid_span(c, scope),
                        |m| m.span.max(1),
                    )
                } else {
                    self.wml_table_cell_grid_span(c, scope)
                };
                let next_col = logical_col.saturating_add(span);
                if (logical_col..next_col).contains(&cell_index) {
                    found = Some(match vmerge {
                        WmlVMerge::Continue => active.map_or(c, |m| m.cell),
                        WmlVMerge::None | WmlVMerge::Restart => c,
                    });
                }
                match vmerge {
                    WmlVMerge::Restart => next_vmerges.push(WmlActiveVMerge {
                        col: logical_col,
                        span,
                        cell: c,
                    }),
                    WmlVMerge::Continue => {
                        if let Some(active) = active {
                            next_vmerges.push(WmlActiveVMerge {
                                col: logical_col,
                                span,
                                cell: active.cell,
                            });
                        }
                    }
                    WmlVMerge::None => {}
                }
                logical_col = next_col;
            }
            scope.truncate(base);
        }
        *active_vmerges = next_vmerges;
        found
    }

    fn wml_table_cell_grid_span(&self, cell: NodeId, scope: &mut Vec<(Vec<u8>, Vec<u8>)>) -> usize {
        for i in 0..self.nodes[cell.0 as usize].children.len() {
            let c = self.nodes[cell.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(c, scope);
            if self.resolves_to(c, WML_NS, b"tcPr", scope) {
                let span = self.wml_tcpr_grid_span(c, scope);
                scope.truncate(base);
                return span;
            }
            scope.truncate(base);
        }
        1
    }

    fn wml_tcpr_grid_span(&self, tcpr: NodeId, scope: &mut Vec<(Vec<u8>, Vec<u8>)>) -> usize {
        let mut span = 1usize;
        for i in 0..self.nodes[tcpr.0 as usize].children.len() {
            let c = self.nodes[tcpr.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(c, scope);
            if self.resolves_to(c, WML_NS, b"gridSpan", scope) {
                if let Node::Element { attrs, .. } = &self.nodes[c.0 as usize].node {
                    span = attr_value_local(attrs, b"val")
                        .map(trim_ascii_whitespace)
                        .and_then(|v| std::str::from_utf8(v).ok())
                        .and_then(|v| v.parse::<usize>().ok())
                        .filter(|&v| v > 0)
                        .unwrap_or(1);
                }
            }
            scope.truncate(base);
        }
        span
    }

    fn wml_table_cell_vmerge(
        &self,
        cell: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
    ) -> WmlVMerge {
        for i in 0..self.nodes[cell.0 as usize].children.len() {
            let c = self.nodes[cell.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(c, scope);
            if self.resolves_to(c, WML_NS, b"tcPr", scope) {
                let vmerge = self.wml_tcpr_vmerge(c, scope);
                scope.truncate(base);
                return vmerge;
            }
            scope.truncate(base);
        }
        WmlVMerge::None
    }

    fn wml_tcpr_vmerge(&self, tcpr: NodeId, scope: &mut Vec<(Vec<u8>, Vec<u8>)>) -> WmlVMerge {
        let mut vmerge = WmlVMerge::None;
        for i in 0..self.nodes[tcpr.0 as usize].children.len() {
            let c = self.nodes[tcpr.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(c, scope);
            if self.resolves_to(c, WML_NS, b"vMerge", scope) {
                vmerge = if let Node::Element { attrs, .. } = &self.nodes[c.0 as usize].node {
                    match attr_value_local(attrs, b"val").map(trim_ascii_whitespace) {
                        Some(b"restart") => WmlVMerge::Restart,
                        _ => WmlVMerge::Continue,
                    }
                } else {
                    WmlVMerge::Continue
                };
            }
            scope.truncate(base);
        }
        vmerge
    }

    fn edit_wml_revisions_descendants(
        &mut self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        policy: WmlRevisionEditPolicy,
    ) -> usize {
        let mut changed = 0usize;
        let mut i = 0usize;
        while i < self.nodes[id.0 as usize].children.len() {
            let child = self.nodes[id.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(child, scope);
            match self.wml_revision_edit_action(child, scope, policy) {
                WmlRevisionEditAction::Remove => {
                    self.nodes[id.0 as usize].children.remove(i);
                    changed += 1;
                }
                WmlRevisionEditAction::Unwrap => {
                    changed += self.edit_wml_revisions_descendants(child, scope, policy);
                    let replacement = self.nodes[child.0 as usize].children.clone();
                    let replacement_len = replacement.len();
                    self.nodes[id.0 as usize]
                        .children
                        .splice(i..=i, replacement);
                    changed += 1;
                    i += replacement_len;
                }
                WmlRevisionEditAction::RenameDeletedText => {
                    if self.rename_wml_del_text_to_text(child) {
                        changed += 1;
                    }
                    changed += self.edit_wml_revisions_descendants(child, scope, policy);
                    i += 1;
                }
                WmlRevisionEditAction::Keep => {
                    changed += self.edit_wml_revisions_descendants(child, scope, policy);
                    i += 1;
                }
            }
            scope.truncate(base);
        }
        changed
    }

    fn wml_revision_edit_action(
        &self,
        id: NodeId,
        scope: &[(Vec<u8>, Vec<u8>)],
        policy: WmlRevisionEditPolicy,
    ) -> WmlRevisionEditAction {
        let is_inserted = self.resolves_to(id, WML_NS, b"ins", scope)
            || self.resolves_to(id, WML_NS, b"moveTo", scope);
        let is_deleted = self.resolves_to(id, WML_NS, b"del", scope)
            || self.resolves_to(id, WML_NS, b"moveFrom", scope);
        let is_property_change = self.resolves_to(id, WML_NS, b"pPrChange", scope)
            || self.resolves_to(id, WML_NS, b"rPrChange", scope)
            || self.resolves_to(id, WML_NS, b"tblPrChange", scope)
            || self.resolves_to(id, WML_NS, b"trPrChange", scope)
            || self.resolves_to(id, WML_NS, b"tcPrChange", scope)
            || self.resolves_to(id, WML_NS, b"sectPrChange", scope);
        if is_property_change {
            return WmlRevisionEditAction::Remove;
        }
        match policy {
            WmlRevisionEditPolicy::Accept => {
                if is_inserted {
                    WmlRevisionEditAction::Unwrap
                } else if is_deleted {
                    WmlRevisionEditAction::Remove
                } else {
                    WmlRevisionEditAction::Keep
                }
            }
            WmlRevisionEditPolicy::Reject => {
                if is_inserted {
                    WmlRevisionEditAction::Remove
                } else if is_deleted {
                    WmlRevisionEditAction::Unwrap
                } else if self.resolves_to(id, WML_NS, b"delText", scope) {
                    WmlRevisionEditAction::RenameDeletedText
                } else {
                    WmlRevisionEditAction::Keep
                }
            }
        }
    }

    fn rename_wml_del_text_to_text(&mut self, id: NodeId) -> bool {
        let Node::Element { name, .. } = &mut self.nodes[id.0 as usize].node else {
            return false;
        };
        let local_start = name.iter().position(|&b| b == b':').map_or(0, |i| i + 1);
        if &name[local_start..] != b"delText" {
            return false;
        }
        name.truncate(local_start);
        name.extend_from_slice(b"t");
        true
    }

    fn collect_wml_t(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        out: &mut Vec<NodeId>,
    ) {
        self.collect_wml_t_with_alternate_content_policy(
            id,
            scope,
            out,
            AlternateContentTraversal::Selected,
            false,
        );
    }

    fn collect_wml_t_excluding_tables(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        out: &mut Vec<NodeId>,
    ) {
        self.collect_wml_t_with_alternate_content_policy(
            id,
            scope,
            out,
            AlternateContentTraversal::Selected,
            true,
        );
    }

    fn collect_wml_t_with_alternate_content(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        out: &mut Vec<NodeId>,
        alternate_content: AlternateContentTraversal,
    ) {
        self.collect_wml_t_with_alternate_content_policy(id, scope, out, alternate_content, false);
    }

    fn collect_wml_t_with_alternate_content_policy(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        out: &mut Vec<NodeId>,
        alternate_content: AlternateContentTraversal,
        exclude_tables: bool,
    ) {
        let base = scope.len();
        self.push_xmlns(id, scope);
        if self.wml_revision_edit_action(id, scope, WmlRevisionEditPolicy::Accept)
            == WmlRevisionEditAction::Remove
        {
            scope.truncate(base);
            return;
        }
        if exclude_tables && self.resolves_to(id, WML_NS, b"tbl", scope) {
            scope.truncate(base);
            return;
        }
        if matches!(alternate_content, AlternateContentTraversal::Selected) {
            match self.selected_alternate_content_branch(id, scope) {
                AlternateContentBranch::Selected(branch) => {
                    self.collect_wml_t_with_alternate_content_policy(
                        branch,
                        scope,
                        out,
                        alternate_content,
                        exclude_tables,
                    );
                    scope.truncate(base);
                    return;
                }
                AlternateContentBranch::Missing => {
                    scope.truncate(base);
                    return;
                }
                AlternateContentBranch::NotAlternateContent => {}
            }
        }
        if let Node::Element { name, .. } = &self.nodes[id.0 as usize].node {
            let (prefix, lname): (&[u8], &[u8]) = match name.iter().position(|&b| b == b':') {
                Some(i) => (&name[..i], &name[i + 1..]),
                None => (b"", name),
            };
            if lname == b"t" {
                let uri = scope.iter().rev().find(|(p, _)| p.as_slice() == prefix);
                if uri.map(|(_, u)| u.as_slice()) == Some(WML_NS) {
                    out.push(id);
                }
            }
        }
        for i in 0..self.nodes[id.0 as usize].children.len() {
            let c = self.nodes[id.0 as usize].children[i];
            self.collect_wml_t_with_alternate_content_policy(
                c,
                scope,
                out,
                alternate_content,
                exclude_tables,
            );
        }
        scope.truncate(base);
    }

    fn collect_wml_text_run_group(
        &self,
        id: NodeId,
        parent: Option<NodeId>,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        in_run_content: bool,
        out: &mut WmlTextRunGroup,
    ) {
        let base = scope.len();
        self.push_xmlns(id, scope);
        if self.wml_revision_edit_action(id, scope, WmlRevisionEditPolicy::Accept)
            == WmlRevisionEditAction::Remove
        {
            scope.truncate(base);
            return;
        }
        match self.selected_alternate_content_branch(id, scope) {
            AlternateContentBranch::Selected(branch) => {
                self.collect_wml_text_run_group(branch, Some(id), scope, in_run_content, out);
                scope.truncate(base);
                return;
            }
            AlternateContentBranch::Missing => {
                scope.truncate(base);
                return;
            }
            AlternateContentBranch::NotAlternateContent => {}
        }
        let child_in_run_content = self.resolves_to(id, WML_NS, b"r", scope)
            || (in_run_content
                && (self.resolves_to(id, MC_NS, b"Choice", scope)
                    || self.resolves_to(id, MC_NS, b"Fallback", scope)));
        if in_run_content && self.resolves_to(id, WML_NS, b"t", scope) {
            out.push_text_run(id, parent);
        } else if in_run_content && self.resolves_to(id, WML_NS, b"tab", scope) {
            out.marker_nodes.push(id);
        } else if in_run_content && self.resolves_to(id, WML_NS, b"br", scope) {
            let text_wrapping = match &self.nodes[id.0 as usize].node {
                Node::Element { attrs, .. } => wml_break_is_text_wrapping(attrs, scope),
                _ => false,
            };
            if text_wrapping {
                out.marker_nodes.push(id);
            }
        }
        for i in 0..self.nodes[id.0 as usize].children.len() {
            let child = self.nodes[id.0 as usize].children[i];
            self.collect_wml_text_run_group(child, Some(id), scope, child_in_run_content, out);
        }
        scope.truncate(base);
    }

    fn collect_wml_content_control_text_by_tag(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        tag: &[u8],
        out: &mut Vec<WmlTextRunGroup>,
    ) {
        let base = scope.len();
        self.push_xmlns(id, scope);
        if self.wml_revision_edit_action(id, scope, WmlRevisionEditPolicy::Accept)
            == WmlRevisionEditAction::Remove
        {
            scope.truncate(base);
            return;
        }
        match self.selected_alternate_content_branch(id, scope) {
            AlternateContentBranch::Selected(branch) => {
                self.collect_wml_content_control_text_by_tag(branch, scope, tag, out);
                scope.truncate(base);
                return;
            }
            AlternateContentBranch::Missing => {
                scope.truncate(base);
                return;
            }
            AlternateContentBranch::NotAlternateContent => {}
        }
        if self.resolves_to(id, WML_NS, b"sdt", scope) && self.wml_sdt_tag_matches(id, scope, tag) {
            let mut group = WmlTextRunGroup::default();
            self.collect_wml_text_run_group(id, None, scope, false, &mut group);
            out.push(group);
            scope.truncate(base);
            return;
        }
        for i in 0..self.nodes[id.0 as usize].children.len() {
            let c = self.nodes[id.0 as usize].children[i];
            self.collect_wml_content_control_text_by_tag(c, scope, tag, out);
        }
        scope.truncate(base);
    }

    fn collect_wml_real_note_entry(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        note_local: &[u8],
        out: &mut Vec<NodeId>,
    ) {
        let base = scope.len();
        self.push_xmlns(id, scope);
        match self.selected_alternate_content_branch(id, scope) {
            AlternateContentBranch::Selected(branch) => {
                self.collect_wml_real_note_entries_in_container(branch, scope, note_local, out);
            }
            AlternateContentBranch::Missing => {}
            AlternateContentBranch::NotAlternateContent => {
                if self.resolves_to(id, WML_NS, note_local, scope)
                    && self.is_real_wml_note_entry(id)
                {
                    out.push(id);
                }
            }
        }
        scope.truncate(base);
    }

    fn collect_wml_real_note_entries_in_container(
        &self,
        container: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        note_local: &[u8],
        out: &mut Vec<NodeId>,
    ) {
        let base = scope.len();
        self.push_xmlns(container, scope);
        for i in 0..self.nodes[container.0 as usize].children.len() {
            let child = self.nodes[container.0 as usize].children[i];
            self.collect_wml_real_note_entry(child, scope, note_local, out);
        }
        scope.truncate(base);
    }

    fn is_real_wml_note_entry(&self, id: NodeId) -> bool {
        let Node::Element { attrs, .. } = &self.nodes[id.0 as usize].node else {
            return false;
        };
        let boilerplate = matches!(
            attr_value_local(attrs, b"type").map(trim_ascii_whitespace),
            Some(b"separator") | Some(b"continuationSeparator") | Some(b"continuationNotice")
        );
        !boilerplate
            && attr_value_local(attrs, b"id")
                .map(trim_ascii_whitespace)
                .is_some_and(|id| !id.is_empty())
    }

    fn wml_sdt_tag_matches(
        &self,
        sdt: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        tag: &[u8],
    ) -> bool {
        for i in 0..self.nodes[sdt.0 as usize].children.len() {
            let c = self.nodes[sdt.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(c, scope);
            if self.resolves_to(c, WML_NS, b"sdtPr", scope) {
                let found = self.wml_sdtpr_tag_matches(c, scope, tag);
                scope.truncate(base);
                return found;
            }
            scope.truncate(base);
        }
        false
    }

    fn wml_sdtpr_tag_matches(
        &self,
        sdt_pr: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        tag: &[u8],
    ) -> bool {
        for i in 0..self.nodes[sdt_pr.0 as usize].children.len() {
            let c = self.nodes[sdt_pr.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(c, scope);
            let matches = self.resolves_to(c, WML_NS, b"tag", scope)
                && match &self.nodes[c.0 as usize].node {
                    Node::Element { attrs, .. } => attr_value_local(attrs, b"val")
                        .is_some_and(|value| trim_ascii_whitespace(value) == tag),
                    _ => false,
                };
            scope.truncate(base);
            if matches {
                return true;
            }
        }
        false
    }

    fn collect_wml_hyperlink_rids(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        out: &mut Vec<String>,
    ) {
        let base = scope.len();
        self.push_xmlns(id, scope);
        if self.wml_revision_edit_action(id, scope, WmlRevisionEditPolicy::Accept)
            == WmlRevisionEditAction::Remove
        {
            scope.truncate(base);
            return;
        }
        if self.resolves_to(id, WML_NS, b"hyperlink", scope) {
            if let Node::Element { attrs, .. } = &self.nodes[id.0 as usize].node {
                if let Some(rid) = attr_value_ns_local(attrs, scope, OOXML_REL_NS, b"id")
                    .map(trim_ascii_whitespace)
                    .and_then(|v| std::str::from_utf8(v).ok())
                {
                    out.push(rid.to_string());
                }
            }
        }
        for i in 0..self.nodes[id.0 as usize].children.len() {
            let c = self.nodes[id.0 as usize].children[i];
            self.collect_wml_hyperlink_rids(c, scope, out);
        }
        scope.truncate(base);
    }

    fn find_wml_run_range_with_text(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        anchor_text: &str,
        out: &mut Option<WmlRunRange>,
    ) -> bool {
        let base = scope.len();
        self.push_xmlns(id, scope);
        if self.wml_revision_edit_action(id, scope, WmlRevisionEditPolicy::Accept)
            == WmlRevisionEditAction::Remove
        {
            scope.truncate(base);
            return false;
        }
        if self.find_wml_child_run_range(id, scope, anchor_text, out) {
            scope.truncate(base);
            return true;
        }
        for i in 0..self.nodes[id.0 as usize].children.len() {
            let c = self.nodes[id.0 as usize].children[i];
            if self.find_wml_run_range_with_text(c, scope, anchor_text, out) {
                scope.truncate(base);
                return true;
            }
        }
        scope.truncate(base);
        false
    }

    fn find_wml_child_run_range(
        &self,
        parent: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        anchor_text: &str,
        out: &mut Option<WmlRunRange>,
    ) -> bool {
        for runs in self.wml_child_run_groups(parent, scope) {
            for start in 0..runs.len() {
                let mut candidate = runs[start].1.clone();
                if candidate == anchor_text {
                    *out = Some(WmlRunRange {
                        first_run: runs[start].0,
                        last_run: runs[start].0,
                    });
                    return true;
                }
                if !anchor_text.starts_with(&candidate) {
                    continue;
                }
                for end in (start + 1)..runs.len() {
                    candidate.push_str(&runs[end].1);
                    if candidate == anchor_text {
                        *out = Some(WmlRunRange {
                            first_run: runs[start].0,
                            last_run: runs[end].0,
                        });
                        return true;
                    }
                    if !anchor_text.starts_with(&candidate) {
                        break;
                    }
                }
            }
        }
        false
    }

    fn wml_child_run_groups(
        &self,
        parent: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Vec<Vec<(NodeId, String)>> {
        let mut groups = Vec::new();
        let mut current = Vec::new();
        self.collect_wml_transparent_child_runs(parent, scope, &mut current, &mut groups);
        Self::finish_wml_run_group(&mut current, &mut groups);
        groups
    }

    fn collect_wml_transparent_child_runs(
        &self,
        parent: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        current: &mut Vec<(NodeId, String)>,
        groups: &mut Vec<Vec<(NodeId, String)>>,
    ) {
        for i in 0..self.nodes[parent.0 as usize].children.len() {
            let child = self.nodes[parent.0 as usize].children[i];
            let base = scope.len();
            self.push_xmlns(child, scope);
            match self.wml_revision_edit_action(child, scope, WmlRevisionEditPolicy::Accept) {
                WmlRevisionEditAction::Unwrap => {
                    self.collect_wml_transparent_child_runs(child, scope, current, groups);
                }
                WmlRevisionEditAction::Keep if self.resolves_to(child, WML_NS, b"r", scope) => {
                    if let Some(text) = self.wml_run_text(child, scope) {
                        current.push((child, text));
                    } else {
                        Self::finish_wml_run_group(current, groups);
                    }
                }
                WmlRevisionEditAction::Remove
                | WmlRevisionEditAction::Keep
                | WmlRevisionEditAction::RenameDeletedText => {
                    Self::finish_wml_run_group(current, groups);
                }
            }
            scope.truncate(base);
        }
    }

    fn finish_wml_run_group(
        current: &mut Vec<(NodeId, String)>,
        groups: &mut Vec<Vec<(NodeId, String)>>,
    ) {
        if !current.is_empty() {
            groups.push(std::mem::take(current));
        }
    }

    fn wml_run_text(&self, id: NodeId, scope: &mut Vec<(Vec<u8>, Vec<u8>)>) -> Option<String> {
        let base = scope.len();
        self.push_xmlns(id, scope);
        let text = if self.resolves_to(id, WML_NS, b"r", scope) {
            let mut runs = Vec::new();
            self.collect_wml_t(id, scope, &mut runs);
            Some(runs.into_iter().map(|run| self.text_of(run)).collect())
        } else {
            None
        };
        scope.truncate(base);
        text
    }

    fn wml_comment_text_edit_target_under(
        &self,
        root: NodeId,
        comment_id: &str,
    ) -> Option<WmlCommentTextEditTarget> {
        let mut out = None;
        let mut scope = self.ns_scope_at(root);
        for i in 0..self.nodes[root.0 as usize].children.len() {
            if out.is_some() {
                break;
            }
            let c = self.nodes[root.0 as usize].children[i];
            self.collect_wml_comment_text_edit_target(
                c,
                &mut scope,
                comment_id.as_bytes(),
                &mut out,
            );
        }
        out
    }

    fn collect_wml_comment_text_edit_target(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        comment_id: &[u8],
        out: &mut Option<WmlCommentTextEditTarget>,
    ) {
        if out.is_some() {
            return;
        }
        let base = scope.len();
        if let Node::Element { attrs, .. } = &self.nodes[id.0 as usize].node {
            for (k, v) in attrs {
                if k.as_slice() == b"xmlns" {
                    scope.push((Vec::new(), v.clone()));
                } else if let Some(p) = k.strip_prefix(b"xmlns:".as_slice()) {
                    scope.push((p.to_vec(), v.clone()));
                }
            }
            if self.resolves_to(id, WML_NS, b"comment", scope)
                && attr_value_local(attrs, b"id")
                    .is_some_and(|value| trim_ascii_whitespace(value) == comment_id)
            {
                let mut target = WmlCommentTextEditTarget {
                    text_runs: Vec::new(),
                };
                self.collect_wml_comment_text_edit_descendants(id, scope, &mut target);
                *out = Some(target);
                scope.truncate(base);
                return;
            }
        }

        for i in 0..self.nodes[id.0 as usize].children.len() {
            let c = self.nodes[id.0 as usize].children[i];
            self.collect_wml_comment_text_edit_target(c, scope, comment_id, out);
            if out.is_some() {
                break;
            }
        }
        scope.truncate(base);
    }

    fn collect_wml_comment_text_edit_descendants(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        target: &mut WmlCommentTextEditTarget,
    ) {
        let base = scope.len();
        self.push_xmlns(id, scope);
        if self.wml_revision_edit_action(id, scope, WmlRevisionEditPolicy::Accept)
            == WmlRevisionEditAction::Remove
        {
            scope.truncate(base);
            return;
        }
        if self.resolves_to(id, WML_NS, b"t", scope) {
            target.text_runs.push(id);
        }
        for i in 0..self.nodes[id.0 as usize].children.len() {
            let c = self.nodes[id.0 as usize].children[i];
            self.collect_wml_comment_text_edit_descendants(c, scope, target);
        }
        scope.truncate(base);
    }

    fn collect_wml_field_instructions(
        &self,
        id: NodeId,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        scan: &mut FieldInstructionScan,
    ) {
        let base = scope.len();
        let mut fld_char_type: Option<Vec<u8>> = None;
        self.push_xmlns(id, scope);
        if self.wml_revision_edit_action(id, scope, WmlRevisionEditPolicy::Accept)
            == WmlRevisionEditAction::Remove
        {
            scope.truncate(base);
            return;
        }
        match self.selected_alternate_content_branch(id, scope) {
            AlternateContentBranch::Selected(branch) => {
                self.collect_wml_field_instructions(branch, scope, scan);
                scope.truncate(base);
                return;
            }
            AlternateContentBranch::Missing => {
                scope.truncate(base);
                return;
            }
            AlternateContentBranch::NotAlternateContent => {}
        }
        if let Node::Element { attrs, .. } = &self.nodes[id.0 as usize].node {
            if self.resolves_to(id, WML_NS, b"fldSimple", scope) {
                let instruction = attr_value_local(attrs, b"instr")
                    .map(String::from_utf8_lossy)
                    .map(std::borrow::Cow::into_owned)
                    .unwrap_or_default();
                scan.fields.push(instruction);
                scope.truncate(base);
                return;
            }
            if self.resolves_to(id, WML_NS, b"fldChar", scope) {
                fld_char_type = attr_value_local(attrs, b"fldCharType")
                    .map(trim_ascii_whitespace)
                    .map(Vec::from);
            } else if self.resolves_to(id, WML_NS, b"instrText", scope) {
                if let Some(complex) = scan.complex.last_mut() {
                    if !complex.result_phase {
                        complex.instruction.push_str(&self.text_of(id));
                    }
                }
            }
        }

        match fld_char_type.as_deref() {
            Some(b"begin") => {
                scan.complex.push(ComplexFieldInstructionScan {
                    instruction: String::new(),
                    result_phase: false,
                });
            }
            Some(b"separate") => {
                if let Some(complex) = scan.complex.last_mut() {
                    complex.result_phase = true;
                }
            }
            Some(b"end") => {
                if let Some(complex) = scan.complex.pop() {
                    scan.fields.push(complex.instruction);
                }
                scope.truncate(base);
                return;
            }
            _ => {}
        }

        for i in 0..self.nodes[id.0 as usize].children.len() {
            let child = self.nodes[id.0 as usize].children[i];
            self.collect_wml_field_instructions(child, scope, scan);
        }
        scope.truncate(base);
    }

    fn collect_wml_field_results(
        &self,
        id: NodeId,
        parent: Option<NodeId>,
        scope: &mut Vec<(Vec<u8>, Vec<u8>)>,
        in_run_content: bool,
        scan: &mut FieldScan,
    ) {
        if scan.target.is_some() {
            return;
        }
        let base = scope.len();
        let mut fld_char_type: Option<Vec<u8>> = None;
        self.push_xmlns(id, scope);
        if self.wml_revision_edit_action(id, scope, WmlRevisionEditPolicy::Accept)
            == WmlRevisionEditAction::Remove
        {
            scope.truncate(base);
            return;
        }
        match self.selected_alternate_content_branch(id, scope) {
            AlternateContentBranch::Selected(branch) => {
                self.collect_wml_field_results(branch, Some(id), scope, in_run_content, scan);
                scope.truncate(base);
                return;
            }
            AlternateContentBranch::Missing => {
                scope.truncate(base);
                return;
            }
            AlternateContentBranch::NotAlternateContent => {}
        }
        let child_in_run_content = self.resolves_to(id, WML_NS, b"r", scope)
            || (in_run_content
                && (self.resolves_to(id, MC_NS, b"Choice", scope)
                    || self.resolves_to(id, MC_NS, b"Fallback", scope)));
        if let Node::Element { attrs, .. } = &self.nodes[id.0 as usize].node {
            if self.resolves_to(id, WML_NS, b"fldSimple", scope) {
                if scan.seen == scan.wanted {
                    let mut result = WmlTextRunGroup::default();
                    for i in 0..self.nodes[id.0 as usize].children.len() {
                        let child = self.nodes[id.0 as usize].children[i];
                        self.collect_wml_text_run_group(child, Some(id), scope, false, &mut result);
                    }
                    scan.target = Some(result);
                }
                scan.seen += 1;
                scope.truncate(base);
                return;
            }
            if self.resolves_to(id, WML_NS, b"fldChar", scope) {
                fld_char_type = attr_value_local(attrs, b"fldCharType")
                    .map(trim_ascii_whitespace)
                    .map(Vec::from);
            } else if in_run_content && self.resolves_to(id, WML_NS, b"t", scope) {
                if let Some(complex) = scan.complex.last_mut() {
                    if complex.result_phase {
                        complex.result.push_text_run(id, parent);
                    }
                }
            } else if in_run_content
                && (self.resolves_to(id, WML_NS, b"tab", scope)
                    || (self.resolves_to(id, WML_NS, b"br", scope)
                        && wml_break_is_text_wrapping(attrs, scope)))
            {
                if let Some(complex) = scan.complex.last_mut() {
                    if complex.result_phase {
                        complex.result.marker_nodes.push(id);
                    }
                }
            }
        }

        match fld_char_type.as_deref() {
            Some(b"begin") => {
                scan.complex.push(ComplexFieldScan {
                    result_phase: false,
                    result: WmlTextRunGroup::default(),
                });
            }
            Some(b"separate") => {
                if let Some(complex) = scan.complex.last_mut() {
                    complex.result_phase = true;
                }
            }
            Some(b"end") => {
                if let Some(complex) = scan.complex.pop() {
                    if let Some(parent) = scan.complex.last_mut() {
                        if parent.result_phase {
                            parent
                                .result
                                .text_runs
                                .extend(complex.result.text_runs.iter().copied());
                            parent
                                .result
                                .text_run_parents
                                .extend(complex.result.text_run_parents.iter().copied());
                            parent
                                .result
                                .marker_nodes
                                .extend(complex.result.marker_nodes.iter().copied());
                        }
                    }
                    if scan.seen == scan.wanted {
                        scan.target = Some(complex.result);
                    }
                    scan.seen += 1;
                }
                scope.truncate(base);
                return;
            }
            _ => {}
        }

        for i in 0..self.nodes[id.0 as usize].children.len() {
            if scan.target.is_some() {
                break;
            }
            let c = self.nodes[id.0 as usize].children[i];
            self.collect_wml_field_results(c, Some(id), scope, child_in_run_content, scan);
        }
        scope.truncate(base);
    }

    /// Smallest unused positive value for a drawing id, scanning every `docPr` /
    /// `cNvPr` `id` attribute. Guaranteed collision-free (pigeonhole over the used
    /// set), so it never reuses an existing id even at the `u32` ceiling.
    pub(crate) fn fresh_drawing_id(&self) -> u32 {
        let mut used = std::collections::HashSet::new();
        for r in 0..self.roots.len() {
            self.collect_drawing_ids(self.roots[r], &mut used);
        }
        (1..=used.len() as u32 + 1)
            .find(|i| !used.contains(i))
            .unwrap_or(1)
    }

    fn collect_drawing_ids(&self, id: NodeId, used: &mut std::collections::HashSet<u32>) {
        if matches!(self.local_name(id), Some(b"docPr") | Some(b"cNvPr")) {
            if let Node::Element { attrs, .. } = &self.nodes[id.0 as usize].node {
                if let Some((_, v)) = attrs.iter().find(|(k, _)| k.as_slice() == b"id") {
                    if let Some(n) = std::str::from_utf8(v)
                        .ok()
                        .and_then(|s| s.parse::<u32>().ok())
                    {
                        used.insert(n);
                    }
                }
            }
        }
        for i in 0..self.nodes[id.0 as usize].children.len() {
            let c = self.nodes[id.0 as usize].children[i];
            self.collect_drawing_ids(c, used);
        }
    }

    /// Parse an XML fragment and insert its node(s) as children of `parent`, **before**
    /// the last direct child that resolves to namespace `before_ns` with local name
    /// `before_local` (e.g. `w:sectPr`, which OOXML requires to remain the final body
    /// child); appends at the end if no such child exists. **Namespace-aware**: the
    /// child's namespace is resolved against the bindings in scope at `parent`, so a
    /// foreign `<x:sectPr>` does not displace the real one. The fragment's own namespace
    /// declarations ride along, so it need not assume the host's.
    pub(crate) fn insert_fragment_before_ns_local(
        &mut self,
        parent: NodeId,
        xml: &[u8],
        before_ns: &[u8],
        before_local: &[u8],
    ) -> Result<()> {
        // Test seam: simulate a commit-time allocation failure (see `commit_should_fail`).
        #[cfg(test)]
        if commit_should_fail() {
            return Err(Error::Docx(
                "simulated commit-time allocation failure (test seam)".into(),
            ));
        }
        // Bindings in scope for `parent`'s children (computed before any mutation).
        let scope = self.ns_scope_at(parent);
        // Position among the ORIGINAL direct children (indices stay valid: we graft
        // the new nodes onto the end afterwards, then move them before `pos`).
        let children = self.nodes[parent.0 as usize].children.clone();
        let pos = (0..children.len()).rev().find(|&i| {
            let cid = children[i];
            let Node::Element { name, attrs, .. } = &self.nodes[cid.0 as usize].node else {
                return false;
            };
            let (prefix, lname): (&[u8], &[u8]) = match name.iter().position(|&b| b == b':') {
                Some(j) => (&name[..j], &name[j + 1..]),
                None => (b"", name),
            };
            if lname != before_local {
                return false;
            }
            // Resolve the child's prefix: its own `xmlns` decls first, then `parent`'s.
            let own = attrs.iter().rev().find_map(|(k, v)| {
                let binds = (k.as_slice() == b"xmlns" && prefix.is_empty())
                    || k.strip_prefix(b"xmlns:".as_slice()) == Some(prefix);
                binds.then_some(v.as_slice())
            });
            let uri = own.or_else(|| {
                scope
                    .iter()
                    .rev()
                    .find(|(p, _)| p.as_slice() == prefix)
                    .map(|(_, u)| u.as_slice())
            });
            uri == Some(before_ns)
        });
        let frag = XmlTree::parse(xml)?;
        // Keep the arena bounded across edits, not just at the initial parse.
        if self.nodes.len().saturating_add(frag.nodes.len()) > node_budget() {
            return Err(Error::Docx("edit would exceed the node budget".into()));
        }
        let added: Vec<NodeId> = frag
            .roots
            .iter()
            .map(|&r| self.graft(&frag, r, parent))
            .collect::<Result<_>>()?;
        let n = added.len();
        let pi = parent.0 as usize;
        let head_len = self.nodes[pi].children.len() - n;
        if let Some(p) = pos {
            let ch = &mut self.nodes[pi].children;
            let tail: Vec<NodeId> = ch.split_off(head_len);
            for (k, id) in tail.into_iter().enumerate() {
                ch.insert(p + k, id);
            }
        }
        Ok(())
    }

    fn insert_fragment_at(&mut self, parent: NodeId, index: usize, xml: &[u8]) -> Result<()> {
        // Test seam: simulate a commit-time allocation failure (see `commit_should_fail`).
        #[cfg(test)]
        if commit_should_fail() {
            return Err(Error::Docx(
                "simulated commit-time allocation failure (test seam)".into(),
            ));
        }
        let pos = index.min(self.nodes[parent.0 as usize].children.len());
        let frag = XmlTree::parse(xml)?;
        if self.nodes.len().saturating_add(frag.nodes.len()) > node_budget() {
            return Err(Error::Docx("edit would exceed the node budget".into()));
        }
        let added: Vec<NodeId> = frag
            .roots
            .iter()
            .map(|&r| self.graft(&frag, r, parent))
            .collect::<Result<_>>()?;
        let n = added.len();
        let pi = parent.0 as usize;
        let head_len = self.nodes[pi].children.len() - n;
        let ch = &mut self.nodes[pi].children;
        let tail: Vec<NodeId> = ch.split_off(head_len);
        for (k, id) in tail.into_iter().enumerate() {
            ch.insert(pos + k, id);
        }
        Ok(())
    }

    fn graft(&mut self, src: &XmlTree, src_id: NodeId, parent: NodeId) -> Result<NodeId> {
        let node = src.nodes[src_id.0 as usize].node.clone();
        let new_id = self.push(node, Some(parent))?;
        // Recursion is bounded: `src` was parsed under `MAX_DEPTH`, so the call depth
        // here cannot exceed it (no stack-overflow path on grafted fragments).
        for &c in &src.nodes[src_id.0 as usize].children {
            self.graft(src, c, new_id)?;
        }
        Ok(new_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(tree: &XmlTree) -> String {
        String::from_utf8(tree.serialize()).unwrap()
    }

    #[test]
    fn round_trips_unknown_elements_and_structure() {
        // A body with the things the lossy model drops: a field, a content control,
        // and an mc:AlternateContent shape — all must survive.
        let xml = br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="urn:w" xmlns:mc="urn:mc"><w:body><w:sdt><w:sdtContent><w:p><w:r><w:t>keep me</w:t></w:r></w:p></w:sdtContent></w:sdt><w:fldSimple w:instr=" PAGE "><w:r><w:t>1</w:t></w:r></w:fldSimple><mc:AlternateContent><mc:Choice><w:drawing/></mc:Choice></mc:AlternateContent></w:body></w:document>"#;
        let tree = XmlTree::parse(xml).unwrap();
        let out = s(&tree);
        for needle in [
            "w:sdt",
            "w:sdtContent",
            "w:fldSimple",
            "w:instr=\" PAGE \"",
            "mc:AlternateContent",
            "mc:Choice",
            "w:drawing/",
            "keep me",
            "xmlns:mc=\"urn:mc\"",
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>",
        ] {
            assert!(out.contains(needle), "lost {needle:?} in:\n{out}");
        }
    }

    #[test]
    fn preserves_attribute_order() {
        let xml = br#"<w:p w:a="1" w:c="3" w:b="2"/>"#;
        let out = s(&XmlTree::parse(xml).unwrap());
        assert_eq!(out, r#"<w:p w:a="1" w:c="3" w:b="2"/>"#);
    }

    #[test]
    fn attribute_whitespace_normalization_matches_xml_1_0() {
        // Literal XML attribute whitespace normalizes to spaces, while whitespace
        // introduced through character references remains the referenced character.
        // The serializer re-escapes the latter so another parse has identical meaning.
        let xml = b"<w:p literal=\"a\tb\r\nc\nd\" refs=\"a&#9;b&#10;c&#13;d\"/>";
        let out = s(&XmlTree::parse(xml).unwrap());
        assert_eq!(out, r#"<w:p literal="a b c d" refs="a&#9;b&#10;c&#13;d"/>"#);
        assert_eq!(s(&XmlTree::parse(out.as_bytes()).unwrap()), out);
    }

    #[test]
    fn serialize_is_idempotent() {
        let xml = br#"<a><b x="1"><c/>txt</b><!-- note --><d>x &amp; y</d></a>"#;
        let once = XmlTree::parse(xml).unwrap().serialize();
        let twice = XmlTree::parse(&once).unwrap().serialize();
        assert_eq!(once, twice, "second round-trip changed bytes");
    }

    #[test]
    fn text_entities_round_trip() {
        let xml = br#"<t>a &amp; b &lt;c&gt; "q"</t>"#;
        let tree = XmlTree::parse(xml).unwrap();
        // The text node holds the unescaped value…
        let root = tree.roots()[0];
        let child = tree.children(root)[0];
        assert_eq!(
            tree.node(child).clone_text(),
            Some(br#"a & b <c> "q""#.to_vec())
        );
        // …and re-escapes canonically on the way out.
        assert_eq!(s(&tree), r#"<t>a &amp; b &lt;c&gt; "q"</t>"#);
    }

    #[test]
    fn empty_vs_explicit_close_preserved() {
        let xml = br#"<a><b/><c></c></a>"#;
        assert_eq!(s(&XmlTree::parse(xml).unwrap()), r#"<a><b/><c></c></a>"#);
    }

    #[test]
    fn carriage_return_escaped_to_survive_normalization() {
        // Edited text with a CR must serialize as `&#13;`; a literal CR would be
        // folded to LF by XML end-of-line normalization on the next read.
        let mut t = XmlTree::parse(b"<w:t>x</w:t>").unwrap();
        let id = t.roots()[0];
        t.set_element_text(id, "a\rb").unwrap();
        assert_eq!(s(&t), "<w:t>a&#13;b</w:t>");
        // And it round-trips back to the same byte on re-parse.
        let re = XmlTree::parse(s(&t).as_bytes()).unwrap();
        let child = re.children(re.roots()[0])[0];
        assert_eq!(re.node(child).clone_text(), Some(b"a\rb".to_vec()));
    }

    #[test]
    fn prolog_crlf_preserved_verbatim() {
        // The `\r\n` between the XML declaration and the root element is prolog
        // whitespace where character references are illegal — it must round-trip
        // verbatim, NOT become `&#13;`.
        let xml = b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n<a><b>x</b></a>";
        assert_eq!(XmlTree::parse(xml).unwrap().serialize(), xml);
    }

    #[test]
    fn edited_values_drop_xml_forbidden_scalars() {
        let mut t = XmlTree::parse(b"<w:t>x</w:t>").unwrap();
        let id = t.roots()[0];
        t.set_element_text(id, "a\u{FFFE}b\u{FFFF}c").unwrap();
        assert_eq!(s(&t), "<w:t>abc</w:t>");

        let mut attr = XmlTree::parse(b"<w:t/>").unwrap();
        let id = attr.roots()[0];
        attr.set_attr(id, b"data-x", "a\u{FFFF}b".as_bytes())
            .unwrap();
        assert_eq!(s(&attr), r#"<w:t data-x="ab"/>"#);
    }

    #[test]
    fn wml_text_runs_resolves_namespaces() {
        // w:t (wml) is collected; a:t and a default-ns <t> under DrawingML are not.
        let xml = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><w:body><w:t>A</w:t><w:drawing><a:t>B</a:t><a:x xmlns="http://schemas.openxmlformats.org/drawingml/2006/main"><t>C</t></a:x></w:drawing></w:body></w:document>"#;
        let tree = XmlTree::parse(xml).unwrap();
        let runs = tree.wml_text_runs();
        let texts: Vec<String> = runs.iter().map(|&id| tree.text_of(id)).collect();
        assert_eq!(
            texts,
            vec!["A".to_string()],
            "only the WML w:t should match"
        );
    }

    #[test]
    fn wml_text_runs_selects_only_markup_compatibility_alternatives() {
        let xml = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:alias="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:foreign="urn:foreign"><w:body><alias:AlternateContent><alias:Fallback><w:r><w:t>Selected fallback</w:t></w:r></alias:Fallback><alias:Choice><w:r><w:t>Untaken choice</w:t></w:r></alias:Choice></alias:AlternateContent><foreign:AlternateContent><foreign:Choice><w:r><w:t>Foreign choice</w:t></w:r></foreign:Choice><foreign:Fallback><w:r><w:t>Foreign fallback</w:t></w:r></foreign:Fallback></foreign:AlternateContent><alias:AlternateContent><w:r><w:t>Malformed direct content</w:t></w:r></alias:AlternateContent></w:body></w:document>"#;
        let tree = XmlTree::parse(xml).unwrap();
        let body = tree.wml_body_strict().unwrap();
        let texts: Vec<String> = tree
            .wml_text_runs_under(body)
            .into_iter()
            .map(|id| tree.text_of(id))
            .collect();

        assert_eq!(
            texts,
            ["Selected fallback", "Foreign choice", "Foreign fallback"]
        );
    }

    #[test]
    fn wml_body_strict_rejects_top_level_cdata_and_doctype() {
        let doc = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body/></w:document>"#;
        let mut cdata = b"<![CDATA[junk]]>".to_vec();
        cdata.extend_from_slice(doc);
        assert!(
            XmlTree::parse(&cdata).unwrap().wml_body_strict().is_err(),
            "top-level CDATA is character data outside the root"
        );

        let mut doctype = b"<!DOCTYPE w:document>".to_vec();
        doctype.extend_from_slice(doc);
        assert!(
            XmlTree::parse(&doctype).unwrap().wml_body_strict().is_err(),
            "document.xml must not accept a top-level DOCTYPE"
        );

        let mut comment = b"<!--ok--><?ok?>".to_vec();
        comment.extend_from_slice(doc);
        assert!(
            XmlTree::parse(&comment).unwrap().wml_body_strict().is_ok(),
            "comments and processing instructions stay allowed outside the root"
        );
    }

    #[test]
    fn local_name_strips_prefix() {
        let tree = XmlTree::parse(br#"<w:p/>"#).unwrap();
        assert_eq!(tree.local_name(tree.roots()[0]), Some(&b"p"[..]));
    }

    #[test]
    fn garbage_and_deep_nesting_never_panic() {
        let _ = XmlTree::parse(&[0xff, 0xfe, 0x00, 0x3c]);
        let _ = XmlTree::parse(b"<a><b><c");
        let _ = XmlTree::parse(b"plain text no tags");
        // Pathological nesting is rejected cleanly, not a stack overflow.
        let deep = "<a>".repeat(5000);
        assert!(XmlTree::parse(deep.as_bytes()).is_err());
    }

    #[test]
    fn truncated_xml_with_open_elements_is_rejected() {
        // EOF while elements are still open must error, not silently close them.
        assert!(XmlTree::parse(b"<a><b>").is_err());
        assert!(XmlTree::parse(b"<w:p><w:r><w:t>OLD").is_err());
        assert!(XmlTree::parse(b"<a></a>").is_ok()); // the well-formed control
    }

    #[test]
    fn too_many_attributes_is_rejected() {
        // An element with more attributes than the cap is rejected, not amplified into
        // the heap. (Cap lowered for the test; production uses MAX_ATTRS_PER_ELEMENT.)
        set_test_max_attrs(4);
        let r = XmlTree::parse(br#"<w:p a0="" a1="" a2="" a3="" a4=""/>"#);
        set_test_max_attrs(MAX_ATTRS_PER_ELEMENT);
        assert!(r.is_err());
        // Under the cap parses fine.
        assert!(XmlTree::parse(br#"<w:p a0="" a1=""/>"#).is_ok());
    }

    #[test]
    fn large_unique_attribute_flood_parses_within_budget() {
        use std::fmt::Write as _;

        // RUSTSEC-2026-0194's pathological shape: many unique attributes on one
        // element. Keep the advisory-sized input as a functional regression; the
        // patched dependency itself is enforced by Cargo.lock and `cargo audit`.
        const ATTRIBUTE_COUNT: usize = 80_000;
        let mut xml = String::with_capacity(ATTRIBUTE_COUNT * 12);
        xml.push_str("<w:p");
        for index in 0..ATTRIBUTE_COUNT {
            write!(xml, " a{index}=\"\"").unwrap();
        }
        xml.push_str("/>");

        set_test_max_attrs(ATTRIBUTE_COUNT);
        let result = XmlTree::parse(xml.as_bytes());
        set_test_max_attrs(MAX_ATTRS_PER_ELEMENT);

        assert!(result.is_ok(), "in-budget attribute flood was rejected");
    }

    #[test]
    fn malformed_entity_refs_are_rejected() {
        // A bad entity reference in text OR an attribute must error, not survive raw and
        // get canonicalized (to `&amp;…`) on a later re-serialization.
        assert!(XmlTree::parse(b"<w:t>x&bogus;y</w:t>").is_err());
        assert!(XmlTree::parse(br#"<w:p w:x="a&bogus;b"/>"#).is_err());
        // The well-formed controls (proper entities) still parse.
        assert!(XmlTree::parse(b"<w:t>x&amp;y</w:t>").is_ok());
        assert!(XmlTree::parse(br#"<w:p w:x="a&amp;b"/>"#).is_ok());
    }

    #[test]
    fn node_budget_boundary_matches_edits() {
        // A tree with exactly `node_budget()` nodes parses (the edit preflights allow up
        // to exactly the budget, so the parser must accept what an edit can produce).
        set_test_node_budget(3);
        assert!(XmlTree::parse(b"<a><b/><c/></a>").is_ok()); // 3 nodes == budget
        assert!(XmlTree::parse(b"<a><b/><c/><d/></a>").is_err()); // 4 > budget
        reset_test_node_budget();
    }

    #[test]
    fn set_element_text_reuses_cdata_slot_without_growth() {
        // Rewriting a `w:t` whose text is held as CDATA must not grow the arena (the
        // CDATA Raw slot is converted to Text in place).
        let mut t = XmlTree::parse(b"<w:t><![CDATA[OLD]]></w:t>").unwrap();
        let before = t.node_count();
        let id = t.roots()[0];
        t.set_element_text(id, "NEW").unwrap();
        assert_eq!(t.node_count(), before, "CDATA edit grew the arena");
        assert_eq!(t.text_of(id), "NEW");
        // And it serializes as a normal text node now.
        assert_eq!(String::from_utf8(t.serialize()).unwrap(), "<w:t>NEW</w:t>");
    }

    #[test]
    fn empty_element_respects_depth_cap() {
        // The depth cap applies to self-closing elements too: MAX_DEPTH open elements plus
        // a `<b/>` one level deeper must be rejected (not accepted one past the advertised
        // maximum). One level shallower still parses — the boundary matches `Start`.
        let nest = |opens: usize| {
            let mut s = String::new();
            for _ in 0..opens {
                s.push_str("<a>");
            }
            s.push_str("<b/>");
            for _ in 0..opens {
                s.push_str("</a>");
            }
            s
        };
        assert!(
            XmlTree::parse(nest(MAX_DEPTH).as_bytes()).is_err(),
            "an empty element past MAX_DEPTH must be rejected"
        );
        assert!(
            XmlTree::parse(nest(MAX_DEPTH - 1).as_bytes()).is_ok(),
            "an empty element at the cap should still parse"
        );
    }

    #[test]
    fn atomic_body_blocks_move_and_remove_exact_subtrees() {
        let xml = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p data-id="A"><w:r><w:t>A</w:t></w:r><w:unknown keep="1"/></w:p>
            <w:tbl data-id="B"><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl>
            <w:sdt data-id="C"><w:sdtPr/><w:sdtContent><w:p><w:r><w:t>C</w:t></w:r></w:p></w:sdtContent></w:sdt>
            <w:sectPr/>
        </w:body></w:document>"#;
        let mut tree = XmlTree::parse(xml).unwrap();
        let body = tree.wml_body_strict().unwrap();

        tree.move_wml_atomic_body_block_under(body, 0, 2).unwrap();
        let moved = s(&tree);
        let b = moved.find("data-id=\"B\"").unwrap();
        let c = moved.find("data-id=\"C\"").unwrap();
        let a = moved.find("data-id=\"A\"").unwrap();
        let sect = moved.find("<w:sectPr/>").unwrap();
        assert!(b < c && c < a && a < sect, "unexpected order: {moved}");
        assert!(moved.contains("<w:unknown keep=\"1\"/>"));

        tree.remove_wml_atomic_body_block_under(body, 1).unwrap();
        let removed = s(&tree);
        assert!(!removed.contains("data-id=\"C\""));
        assert!(removed.contains("data-id=\"A\"") && removed.contains("data-id=\"B\""));
    }

    #[test]
    fn atomic_body_blocks_insert_plain_paragraph_at_positions() {
        let xml = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p data-id="A"><w:r><w:t>A</w:t></w:r></w:p>
            <w:tbl data-id="B"><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl>
            <w:sectPr/>
        </w:body></w:document>"#;
        for (position, expected) in [
            (0, ["Inserted", "data-id=\"A\"", "data-id=\"B\""]),
            (1, ["data-id=\"A\"", "Inserted", "data-id=\"B\""]),
            (2, ["data-id=\"A\"", "data-id=\"B\"", "Inserted"]),
        ] {
            let mut tree = XmlTree::parse(xml).unwrap();
            let body = tree.wml_body_strict().unwrap();
            tree.insert_wml_plain_body_paragraph_under(body, position, "Inserted")
                .unwrap();
            let serialized = s(&tree);
            let first = serialized.find(expected[0]).unwrap();
            let second = serialized.find(expected[1]).unwrap();
            let third = serialized.find(expected[2]).unwrap();
            let section = serialized.find("<w:sectPr/>").unwrap();
            assert!(
                first < second && second < third && third < section,
                "unexpected position {position}: {serialized}"
            );
        }
    }

    #[test]
    fn atomic_body_block_insertion_encodes_text_and_default_namespace() {
        let xml = br#"<document xmlns="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><body><p><r><t>A</t></r></p><sectPr/></body></document>"#;
        let mut tree = XmlTree::parse(xml).unwrap();
        let body = tree.wml_body_strict().unwrap();

        tree.insert_wml_plain_body_paragraph_under(
            body,
            0,
            " <&> 한글\tline\nnext\u{1}\u{ffff} tail ",
        )
        .unwrap();

        let serialized = s(&tree);
        assert!(
            serialized.contains(
                r#"<w:t xml:space="preserve"> &lt;&amp;&gt; 한글</w:t><w:tab/><w:t>line</w:t><w:br/><w:t xml:space="preserve">next tail </w:t>"#
            ),
            "{serialized}"
        );
        assert!(serialized.find("<w:p xmlns:w=").unwrap() < serialized.find("<p>").unwrap());
    }

    #[test]
    fn atomic_body_block_insertion_rejects_positions_and_hazards_without_mutation() {
        for (body_xml, position) in [
            (r#"<w:p/><w:sectPr/>"#, 2),
            (r#"<w:p/><w:altChunk/><w:sectPr/>"#, 0),
            (
                r#"<w:p><w:bookmarkStart w:id="7"/></w:p><w:p><w:bookmarkEnd w:id="7"/></w:p><w:sectPr/>"#,
                0,
            ),
        ] {
            let xml = format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body_xml}</w:body></w:document>"#
            );
            let mut tree = XmlTree::parse(xml.as_bytes()).unwrap();
            let body = tree.wml_body_strict().unwrap();
            let before = s(&tree);

            assert!(tree
                .insert_wml_plain_body_paragraph_under(body, position, "X")
                .is_err());
            assert_eq!(s(&tree), before);
        }
    }

    #[test]
    fn atomic_body_blocks_reject_cross_block_and_opaque_hazards() {
        let wrap = |body: &str| {
            format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{body}<w:sectPr/></w:body></w:document>"#
            )
        };
        for body_xml in [
            r#"<w:p/><w:altChunk w:id="x"/>"#,
            r#"<w:p><w:bookmarkStart w:id="7"/></w:p><w:p><w:bookmarkEnd w:id="7"/></w:p>"#,
            r#"<w:p><w:r><w:fldChar w:fldCharType="begin"/></w:r></w:p><w:p><w:r><w:fldChar w:fldCharType="end"/></w:r></w:p>"#,
        ] {
            let mut tree = XmlTree::parse(wrap(body_xml).as_bytes()).unwrap();
            let body = tree.wml_body_strict().unwrap();
            assert!(tree.remove_wml_atomic_body_block_under(body, 0).is_err());
        }
    }

    #[test]
    fn atomic_body_blocks_use_wml_attributes_and_track_comment_references() {
        let wrap = |body: &str| {
            format!(
                r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:x="urn:foreign"><w:body>{body}<w:sectPr/></w:body></w:document>"#
            )
        };
        for body_xml in [
            concat!(
                r#"<w:p><w:bookmarkStart x:id="fake-a" w:id="7"/>"#,
                r#"<w:bookmarkEnd x:id="fake-a" w:id="8"/></w:p>"#,
                r#"<w:p><w:bookmarkStart x:id="fake-b" w:id="8"/>"#,
                r#"<w:bookmarkEnd x:id="fake-b" w:id="7"/></w:p>"#,
            ),
            concat!(
                r#"<w:p><w:commentRangeStart w:id="4"/><w:r><w:t>A</w:t></w:r>"#,
                r#"<w:commentRangeEnd w:id="4"/></w:p>"#,
                r#"<w:p><w:r><w:commentReference w:id="4"/></w:r></w:p>"#,
            ),
            concat!(
                r#"<w:p><w:r><w:fldChar x:fldCharType="begin" w:fldCharType="end"/>"#,
                r#"<w:fldChar x:fldCharType="end" w:fldCharType="begin"/></w:r></w:p>"#,
            ),
        ] {
            let tree = XmlTree::parse(wrap(body_xml).as_bytes()).unwrap();
            let body = tree.wml_body_strict().unwrap();
            assert!(tree.wml_atomic_body_block_kinds_under(body).is_err());
        }
    }

    #[test]
    fn atomic_body_blocks_reject_duplicate_bodies_and_repeated_field_separator() {
        let duplicate = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p/></w:body><w:body><w:p/></w:body></w:document>"#;
        assert!(XmlTree::parse(duplicate)
            .unwrap()
            .wml_body_strict()
            .is_err());

        let repeated_separator = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:fldChar w:fldCharType="begin"/><w:fldChar w:fldCharType="separate"/><w:fldChar w:fldCharType="separate"/><w:fldChar w:fldCharType="end"/></w:r></w:p><w:sectPr/></w:body></w:document>"#;
        let tree = XmlTree::parse(repeated_separator).unwrap();
        let body = tree.wml_body_strict().unwrap();
        assert!(tree.wml_atomic_body_block_kinds_under(body).is_err());
    }

    #[test]
    fn atomic_body_blocks_allow_internal_ranges_but_protect_sections() {
        let xml = br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
            <w:p data-id="A"><w:bookmarkStart w:id="7"/><w:r><w:t>A</w:t></w:r><w:bookmarkEnd w:id="7"/></w:p>
            <w:p data-id="S"><w:pPr><w:sectPr/></w:pPr><w:r><w:t>section</w:t></w:r></w:p>
            <w:p data-id="B"><w:r><w:t>B</w:t></w:r></w:p>
            <w:sectPr/>
        </w:body></w:document>"#;
        let mut tree = XmlTree::parse(xml).unwrap();
        let body = tree.wml_body_strict().unwrap();

        assert!(tree.remove_wml_atomic_body_block_under(body, 1).is_err());
        assert!(tree.move_wml_atomic_body_block_under(body, 0, 2).is_err());
        tree.move_wml_atomic_body_block_under(body, 0, 0).unwrap();
        tree.move_wml_atomic_body_block_under(body, 1, 1).unwrap();
        tree.remove_wml_atomic_body_block_under(body, 0).unwrap();
        assert!(!s(&tree).contains("data-id=\"A\""));
    }

    #[test]
    fn set_attr_enforces_attribute_cap_symmetric_with_parse() {
        // Parse a `w:t` already at the (lowered) attribute cap, then try to give it edge
        // whitespace — which needs a *new* `xml:space` attr. That must be refused, because
        // the resulting element would have cap+1 attributes and `XmlTree::parse` would
        // reject it (parse/edit budget symmetry); `can_set_attr` reports it up front.
        set_test_max_attrs(2);
        let mut t = XmlTree::parse(br#"<w:t a="1" b="2">x</w:t>"#).unwrap();
        let id = t.roots()[0];
        assert!(!t.can_set_attr(id, b"xml:space"), "should report no room");
        assert!(
            t.set_element_text(id, " x ").is_err(),
            "adding xml:space past the cap must error, not build an un-re-parseable element"
        );
        // Replacing an *existing* attribute never grows the list, so it still works…
        assert!(t.can_set_attr(id, b"a"));
        t.set_attr(id, b"a", b"9").unwrap();
        // …and an edit that needs no new attribute (no edge whitespace) is fine too.
        t.set_element_text(id, "y").unwrap();
        assert_eq!(t.text_of(id), "y");
        set_test_max_attrs(MAX_ATTRS_PER_ELEMENT);
    }
}
