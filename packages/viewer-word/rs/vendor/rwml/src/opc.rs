//! Open Packaging Conventions (OPC) **round-trip** layer — the foundation for
//! package-preserving `.docx` editing.
//!
//! Unlike the from-scratch package builder in [`crate::write`], this retains the
//! *whole* package on open: every ZIP entry's payload is kept as raw bytes in its
//! original order, so a no-op `from_zip → to_zip` re-emits every part's **payload**
//! byte-for-byte and nothing the editor doesn't understand is ever dropped (themes,
//! settings, fonts, comments, custom XML, charts, embeddings, future parts).
//! `[Content_Types].xml` and the `_rels/*.rels` graph are *also* kept verbatim, with
//! parsed views layered on top for querying and (when a part is added/replaced)
//! regeneration.
//!
//! **Scope of the round-trip guarantee:** part *payloads* are byte-stable, not the
//! ZIP container. `to_zip` writes through a fresh writer with default options, so
//! container metadata (compression method, timestamps, extra fields, file comments,
//! external attributes, directory entries) is normalized. Duplicate part names —
//! invalid OPC, which a few corpus files contain — are collapsed to the **single entry
//! the ZIP reader exposes** for that name (the `zip` crate keeps the last value),
//! deterministically, and re-emitted once. The output opens identically in any OPC
//! consumer; it is not a bit-exact copy of the input archive.
//!
//! This is the OPC half; the body producers (regenerate `document.xml`, or edit a
//! preserved element tree) sit above it. Reused conceptually by a future `.xlsx`
//! editor — the packaging rules live here, format-agnostic.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};

use quick_xml::events::Event;
use quick_xml::name::{Namespace, ResolveResult};
use quick_xml::NsReader;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::error::{Error, Result};
use crate::xmltree::XmlTree;

type CtRecordIdentity = (String, String);

const CT_NS: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;
const CONTENT_TYPES: &str = "[Content_Types].xml";

/// Largest accepted decompressed size for a single part (zip-bomb guard).
const MAX_PART: u64 = 64 << 20;
/// Namespace bindings accepted on one OPC metadata element. This mirrors quick-xml's
/// conservative default explicitly so the untrusted-input memory bound cannot drift.
const MAX_NAMESPACE_DECLARATIONS_PER_ELEMENT: usize = 256;
/// Maximum number of simultaneously open elements in an OPC metadata XML part.
/// Keep this explicit while using quick-xml 0.41, whose reader does not enforce a
/// nesting limit, so hostile metadata cannot grow its namespace stack without bound.
const MAX_OPC_XML_DEPTH: usize = 128;

// Test-lowerable copy of the per-part budget for the WRITE-side checks (to_zip /
// add_image_png), so oversize handling is testable without a 64 MiB fixture. `from_zip`
// always uses the const, so opening a normal doc is never affected by the seam.
#[cfg(test)]
thread_local! {
    static TEST_MAX_PART: std::cell::Cell<u64> = const { std::cell::Cell::new(MAX_PART) };
}
/// Lower the write-side per-part budget for the current test thread.
#[cfg(test)]
pub(crate) fn set_test_max_part(n: u64) {
    TEST_MAX_PART.with(|c| c.set(n));
}
/// Restore the write-side per-part budget to the production value.
#[cfg(test)]
pub(crate) fn reset_test_max_part() {
    TEST_MAX_PART.with(|c| c.set(MAX_PART));
}
/// The effective write-side per-part budget (`MAX_PART`, or the test override).
pub(crate) fn max_part() -> u64 {
    #[cfg(test)]
    {
        TEST_MAX_PART.with(|c| c.get())
    }
    #[cfg(not(test))]
    {
        MAX_PART
    }
}
/// Whole-archive decompressed budget across all parts.
const MAX_TOTAL: u64 = 512 << 20;
/// Max ZIP entries (files + directories) — bounds memory/CPU on a many-tiny-entry
/// archive that stays under the byte budgets.
const MAX_ENTRIES: usize = 1 << 16;

// Test-lowerable copy of the entry cap, so the open/save entry-count boundary is testable
// without a 65k-entry fixture. Production always uses the const.
#[cfg(test)]
thread_local! {
    static TEST_MAX_ENTRIES: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_ENTRIES) };
}
/// Set the entry cap for the current test thread.
#[cfg(test)]
pub(crate) fn set_test_max_entries(n: usize) {
    TEST_MAX_ENTRIES.with(|c| c.set(n));
}
/// Restore the entry cap to the production value.
#[cfg(test)]
pub(crate) fn reset_test_max_entries() {
    TEST_MAX_ENTRIES.with(|c| c.set(MAX_ENTRIES));
}
fn max_entries() -> usize {
    #[cfg(test)]
    {
        TEST_MAX_ENTRIES.with(|c| c.get())
    }
    #[cfg(not(test))]
    {
        MAX_ENTRIES
    }
}
/// Max part-name length.
pub(crate) const MAX_NAME_LEN: usize = 4096;
/// Max parsed records from one OPC metadata part — a single size-capped
/// `[Content_Types].xml` / `.rels` could otherwise pack millions of tiny
/// `Default`/`Override`/`Relationship` elements, amplifying into large heap use.
const MAX_META_RECORDS: usize = 1 << 16;

// Test-lowerable copy of the metadata-record cap, so the over-cap path is exercised on
// a tiny fixture rather than a 65k-element one. Production always uses the const.
#[cfg(test)]
thread_local! {
    static TEST_MAX_META: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_META_RECORDS) };
}

/// Set the metadata-record cap for the current test thread.
#[cfg(test)]
pub(crate) fn set_test_max_meta(n: usize) {
    TEST_MAX_META.with(|c| c.set(n));
}

fn max_meta_records() -> usize {
    #[cfg(test)]
    {
        TEST_MAX_META.with(|c| c.get())
    }
    #[cfg(not(test))]
    {
        MAX_META_RECORDS
    }
}

/// One relationship entry of a `_rels/*.rels` part.
#[derive(Debug, Clone)]
pub(crate) struct Rel {
    /// Relationship id, referenced from a part body (`r:id`, `r:embed`).
    pub id: String,
    /// Relationship type URI.
    pub rel_type: String,
    /// Target part path (relative to the source part) or external URL.
    pub target: String,
    /// `true` ⇒ `TargetMode="External"` (hyperlinks); `false` ⇒ internal part.
    pub external: bool,
}

#[derive(Debug, Clone)]
struct CtDefault {
    extension: String,
    content_type: String,
}

impl CtDefault {
    fn new(extension: impl Into<String>, content_type: impl Into<String>) -> Self {
        Self {
            extension: extension.into(),
            content_type: content_type.into(),
        }
    }
}

#[derive(Debug, Clone)]
struct CtOverride {
    part_name: String,
    content_type: String,
}

impl CtOverride {
    fn new(part_name: impl Into<String>, content_type: impl Into<String>) -> Self {
        Self {
            part_name: part_name.into(),
            content_type: content_type.into(),
        }
    }
}

/// Parsed `[Content_Types].xml`: extension defaults + per-part overrides.
#[derive(Debug, Default, Clone)]
struct ContentTypes {
    defaults: Vec<CtDefault>,
    overrides: Vec<CtOverride>,
    /// `true` when the mandatory `rels` Default was absent from the serialized
    /// `[Content_Types].xml` and injected into this view. Such a package must regenerate
    /// `[Content_Types].xml` whenever an edit (re)writes a `.rels` part, so the injected
    /// default reaches disk and the relationship part is typed (see [`Package::regen_rels`]).
    rels_default_injected: bool,
}

impl ContentTypes {
    /// Whether a part (path without leading `/`) has a resolvable content type — a
    /// per-part `<Override>` or an extension `<Default>`.
    fn resolves(&self, part: &str) -> bool {
        let pn = format!("/{part}");
        if self
            .overrides
            .iter()
            .any(|o| o.part_name.eq_ignore_ascii_case(&pn))
        {
            return true;
        }
        match part.rsplit_once('.') {
            Some((_, ext)) => self
                .defaults
                .iter()
                .any(|d| d.extension.eq_ignore_ascii_case(ext)),
            None => false,
        }
    }

    fn resolves_as(&self, part: &str, content_type: &str) -> bool {
        let pn = format!("/{part}");
        if let Some(o) = self
            .overrides
            .iter()
            .find(|o| o.part_name.eq_ignore_ascii_case(&pn))
        {
            return o.content_type == content_type;
        }
        match part.rsplit_once('.') {
            Some((_, ext)) => self
                .defaults
                .iter()
                .any(|d| d.extension.eq_ignore_ascii_case(ext) && d.content_type == content_type),
            None => false,
        }
    }
}

/// A part's content: raw bytes by default, or a parsed [`XmlTree`] once it has been
/// promoted for editing. **Lazy promotion** is the byte-stability win — only a part
/// actually edited is parsed and re-serialized; every untouched part stays `Raw` and
/// round-trips byte-for-byte (unlike python-docx/POI, which re-serialize parsed
/// parts even on a no-op).
#[derive(Clone)]
enum Part {
    Raw(Vec<u8>),
    Xml(XmlTree),
}

impl Part {
    /// The part's serialized bytes — borrowed for `Raw`, re-serialized for `Xml`.
    fn bytes(&self) -> Cow<'_, [u8]> {
        match self {
            Part::Raw(b) => Cow::Borrowed(b),
            Part::Xml(t) => Cow::Owned(t.serialize()),
        }
    }
}

/// A retained OPC package: every part as raw bytes (source of truth) plus parsed
/// content-type and relationship views.
#[derive(Clone)]
pub(crate) struct Package {
    /// Part names in original ZIP order, for deterministic re-emit. Includes
    /// `[Content_Types].xml` and `_rels/*.rels`.
    order: Vec<String>,
    /// part name (no leading `/`) → content (raw bytes, or a parsed tree once
    /// promoted for editing). The authoritative store.
    parts: HashMap<String, Part>,
    /// Parsed view of `[Content_Types].xml` (regenerated into `parts` when edited).
    ctypes: ContentTypes,
    /// Parsed view of every `_rels/*.rels` part, keyed by the rels-part path.
    rels: HashMap<String, Vec<Rel>>,
    /// Next relationship-id ordinal, seeded above all existing `rId`s. `u64` so that
    /// seeding past a hostile `rId4294967295` still yields a fresh, non-colliding id.
    rid_next: u64,
    /// Content parts we added/replaced this session. Only these are content-type
    /// validated on `to_zip` — an original (passthrough) part keeps its own typing,
    /// even if the source package is itself non-conformant (e.g. an extensionless
    /// extra entry), so editing never *rejects* a file it could otherwise preserve.
    touched: HashSet<String>,
    /// `false` if `from_zip` skipped any unreadable/unopenable central-directory entry,
    /// i.e. not every advertised part was retained. The package then can't honor the
    /// preservation guarantee, so `Document::save` refuses it (read still works).
    complete: bool,
    /// `true` if a metadata part (`[Content_Types].xml` / a `.rels`) failed to parse
    /// cleanly. Read + no-op save still work (raw bytes preserved), but the editor refuses
    /// to *regenerate* metadata from the resulting partial view — so an edit can never
    /// silently rewrite malformed metadata lossily.
    meta_lossy: bool,
    /// `true` when the serialized `[Content_Types].xml` lacked the mandatory `rels` Default
    /// and it was injected into [`Package::ctypes`]. An edit that (re)writes a `.rels` part
    /// then forces `[Content_Types].xml` regeneration so the injected default is serialized
    /// (otherwise the new `.rels` would be untyped). No effect on read or no-op save.
    ct_rels_injected: bool,
}

impl Package {
    /// Whether every advertised part was retained on open (see [`Package::complete`]).
    pub(crate) fn is_complete(&self) -> bool {
        self.complete
    }

    /// Whether any OPC metadata part parsed lossily (see [`Package::meta_lossy`]).
    pub(crate) fn is_meta_lossy(&self) -> bool {
        self.meta_lossy
    }
}

impl Package {
    /// Parse a `.docx`/OPC ZIP into a retained package. Every entry is kept as raw
    /// bytes; `[Content_Types].xml` and `*.rels` are additionally parsed into views.
    pub(crate) fn from_zip(bytes: &[u8]) -> Result<Package> {
        check_zip_entry_budget(bytes)?;
        let mut zip = ZipArchive::new(std::io::Cursor::new(bytes))
            .map_err(|e| Error::Docx(format!("not a valid OPC (zip) package: {e}")))?;
        let entry_cap = max_entries();
        if zip.len() > entry_cap {
            return Err(Error::Docx(format!(
                "package has too many entries ({} > {entry_cap})",
                zip.len()
            )));
        }
        let mut order = Vec::new();
        let mut seen: HashSet<String> = HashSet::new(); // O(1) dedup (vs order.contains)
        let mut seen_ci: HashSet<String> = HashSet::new();
        let mut parts: HashMap<String, Part> = HashMap::new();
        let mut total: u64 = 0;
        let mut meta_lossy = false;
        // Cleared if any central-directory entry can't be opened/read. The OPEN path
        // stays lenient (`Document::open` is also the reader and must still read a
        // recoverable-but-imperfect file), but a package missing a part it advertised is
        // NOT fully retained — `Document::save` refuses such a package so it can never
        // silently drop the part. (Intentional normalizations below — directory dedup,
        // duplicate names, non-file entries — are not data loss and keep `complete`.)
        let mut complete = true;
        for i in 0..zip.len() {
            let mut f = match zip.by_index(i) {
                Ok(f) => f,
                Err(_) => {
                    complete = false;
                    continue;
                }
            };
            let name = f.name().to_string();
            if name.len() > MAX_NAME_LEN {
                return Err(Error::Docx("part name too long".into()));
            }
            // Preserve directory entries (names only; they carry no content) so the
            // round-trip keeps them. Deduped via the seen-set, not an O(n^2) scan.
            if name.ends_with('/') {
                if seen.insert(name.clone()) {
                    order.push(name);
                }
                continue;
            }
            if !f.is_file() {
                continue;
            }
            // Fast reject on the ZIP-declared size, then enforce the real budget on
            // the ACTUAL decompressed length (the declared size can lie).
            if f.size() > MAX_PART {
                return Err(Error::Docx(format!("part {name} exceeds the size budget")));
            }
            let mut buf = Vec::new();
            if (&mut f).take(MAX_PART + 1).read_to_end(&mut buf).is_err() {
                complete = false; // unreadable entry: skip (lenient read; save() refuses)
                continue;
            }
            total = total.saturating_add(buf.len() as u64);
            if buf.len() as u64 > MAX_PART || total > MAX_TOTAL {
                return Err(Error::Docx(format!("part {name} exceeds the size budget")));
            }
            // Duplicate part names are invalid OPC. The ZIP reader already collapses them
            // to one entry per name; this `seen` guard keeps our `order`/`parts` in lock
            // step (one slot per name) so re-emit is deterministic and single.
            if !seen.insert(name.clone()) {
                continue;
            }
            if !seen_ci.insert(name.to_ascii_lowercase()) {
                meta_lossy = true;
            }
            order.push(name.clone());
            parts.insert(name, Part::Raw(buf));
        }

        // Parse the metadata VIEWS leniently: `Document::open` is also the reader, so a
        // malformed `[Content_Types].xml` / unrelated `.rels` must NOT fail open — the raw
        // bytes are still retained and a no-op save preserves them. Instead flag the
        // package `meta_lossy`; the editor refuses to *regenerate* metadata from a
        // possibly-incomplete view (so an edit never silently rewrites malformed metadata).
        let ctypes = match parts.get(CONTENT_TYPES) {
            Some(p) => match parse_content_types(p.bytes().as_ref()) {
                Ok(c) => c,
                Err(_) => {
                    meta_lossy = true;
                    ContentTypes {
                        defaults: vec![CtDefault::new("rels", CT_RELS)],
                        overrides: Vec::new(),
                        rels_default_injected: false,
                    }
                }
            },
            // No `[Content_Types].xml` at all — the package is non-conformant. Read +
            // no-op save still work, but mark it `meta_lossy` so the editor refuses: a
            // regenerated `[Content_Types].xml` built from this empty view would type only
            // the parts the edit touched and leave referenced parts (styles, numbering, …)
            // untyped, producing a file Word rejects. Seed the `rels` Default regardless.
            None => {
                meta_lossy = true;
                ContentTypes {
                    defaults: vec![CtDefault::new("rels", CT_RELS)],
                    overrides: Vec::new(),
                    rels_default_injected: false,
                }
            }
        };
        let ct_rels_injected = ctypes.rels_default_injected;
        let mut rels: HashMap<String, Vec<Rel>> = HashMap::new();
        for name in &order {
            if is_rels(name) {
                if let Some(p) = parts.get(name) {
                    match parse_rels(p.bytes().as_ref()) {
                        Ok(r) => {
                            rels.insert(name.clone(), r);
                        }
                        Err(_) => meta_lossy = true, // raw bytes still preserved
                    }
                }
            }
        }
        let rid_next = rels
            .values()
            .flat_map(|v| v.iter())
            .filter_map(|r| r.id.strip_prefix("rId").and_then(|n| n.parse::<u64>().ok()))
            .max()
            .map(|m| m.saturating_add(1)) // alloc_rid scans for a free id, so a
            .unwrap_or(1); // saturated seed never yields a duplicate.

        Ok(Package {
            order,
            parts,
            ctypes,
            rels,
            rid_next,
            touched: HashSet::new(),
            complete,
            meta_lossy,
            ct_rels_injected,
        })
    }

    /// Re-emit the package to ZIP bytes, parts in original order. Untouched parts'
    /// **payloads** are written byte-for-byte; only parts replaced via
    /// [`Package::set_part`] (or regenerated `[Content_Types]`/`*.rels`) differ. The
    /// ZIP *container* metadata is normalized (fresh writer, default options), so the
    /// result is part-payload-stable, not a bit-exact copy of the input archive.
    pub(crate) fn to_zip(&self) -> Result<Vec<u8>> {
        let in_order = self.validate_write_metadata()?;
        let mut zw = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opt = SimpleFileOptions::default();
        let mut total = 0u64;
        self.visit_write_entries(&in_order, |name, bytes| match bytes {
            Some(bytes) => {
                validate_write_payload(name, bytes, &mut total)?;
                zw.start_file(name, opt)
                    .map_err(|e| Error::Docx(format!("zip start {name}: {e}")))?;
                zw.write_all(bytes)
                    .map_err(|e| Error::Docx(format!("zip write {name}: {e}")))
            }
            None => zw
                .add_directory(name, opt)
                .map_err(|e| Error::Docx(format!("zip dir {name}: {e}"))),
        })?;
        let cur = zw
            .finish()
            .map_err(|e| Error::Docx(format!("zip finish: {e}")))?;
        Ok(cur.into_inner())
    }

    /// Validate every deterministic condition required by [`Self::to_zip`] without
    /// allocating a ZIP archive. Preservation edits call this on their staged clone
    /// before making it authoritative.
    pub(crate) fn validate_for_save(&self) -> Result<()> {
        let in_order = self.validate_write_metadata()?;
        let mut total = 0u64;
        self.visit_write_entries(&in_order, |name, bytes| {
            if let Some(bytes) = bytes {
                validate_write_payload(name, bytes, &mut total)?;
            }
            Ok(())
        })
    }

    fn validate_write_metadata(&self) -> Result<HashSet<&String>> {
        // Every part WE added/replaced must be typed (an `<Override>` or matching
        // `<Default>`), or Word rejects the file — this catches an internal/caller
        // mistake before writing a broken package. Original passthrough parts are
        // trusted as-is, so editing never *rejects* a (possibly non-conformant) file
        // it could otherwise preserve byte-for-byte.
        // `[Content_Types].xml` types itself implicitly (OPC doesn't list it); directory
        // entries carry no content. Everything else WE touched — including regenerated
        // `.rels` parts (which must resolve via the `rels` Default) — must be typed.
        for name in &self.touched {
            if name.ends_with('/') || name == CONTENT_TYPES {
                continue;
            }
            if !self.has_part(name) {
                continue;
            }
            if !self.ctypes.resolves(name) {
                return Err(Error::Docx(format!(
                    "part {name} has no resolvable content type"
                )));
            }
        }
        let meta_cap = max_meta_records();
        if self.touched.contains(CONTENT_TYPES)
            && self
                .ctypes
                .defaults
                .len()
                .saturating_add(self.ctypes.overrides.len())
                > meta_cap
        {
            return Err(Error::Docx(
                "[Content_Types].xml has too many entries on save".into(),
            ));
        }
        for (rels_path, entries) in &self.rels {
            if self.touched.contains(rels_path) && entries.len() > meta_cap {
                return Err(Error::Docx(format!(
                    "{rels_path} has too many relationships on save"
                )));
            }
        }
        for (rels_path, entries) in &self.rels {
            if !self.touched.contains(rels_path) {
                continue;
            }
            let source_part = source_part_of_rels_path(rels_path).ok_or_else(|| {
                Error::Docx(format!(
                    "{rels_path} is not a valid relationships part path"
                ))
            })?;
            for rel in entries {
                if rel.external {
                    continue;
                }
                let target = resolve_rel_target(&source_part, rel_target_part_path(&rel.target));
                if !self.has_part(&target) {
                    return Err(Error::Docx(format!(
                        "{rels_path} relationship {} targets missing part {target}",
                        rel.id
                    )));
                }
            }
        }
        // Re-apply the same entry-count budget `from_zip` enforces, so an edit can't
        // produce a package with more entries than the crate will reopen. Count the parts
        // ACTUALLY emitted: every `order` entry (dirs + parts) plus only the parts added
        // after open (not already in `order`) — `order` and `parts` overlap, so a naive
        // `order.len() + parts.len()` would double-count existing parts.
        let in_order: HashSet<&String> = self.order.iter().collect();
        let added_after_open = self.parts.keys().filter(|k| !in_order.contains(k)).count();
        if self.order.len().saturating_add(added_after_open) > max_entries() {
            return Err(Error::Docx("package has too many entries on save".into()));
        }
        Ok(in_order)
    }

    fn visit_write_entries(
        &self,
        in_order: &HashSet<&String>,
        mut visit: impl FnMut(&str, Option<&[u8]>) -> Result<()>,
    ) -> Result<()> {
        for name in &self.order {
            if name.ends_with('/') {
                visit(name, None)?;
            } else if let Some(p) = self.parts.get(name) {
                let bytes = p.bytes();
                visit(name, Some(bytes.as_ref()))?;
            }
        }
        // Any part added after open (not in `order`) is appended deterministically.
        let mut extra: Vec<&String> = self
            .parts
            .keys()
            .filter(|k| !in_order.contains(k))
            .collect();
        extra.sort();
        for name in extra {
            if let Some(p) = self.parts.get(name) {
                let bytes = p.bytes();
                visit(name, Some(bytes.as_ref()))?;
            }
        }
        Ok(())
    }

    /// Serialized bytes of a part, if present (re-serializing a promoted tree).
    pub(crate) fn part(&self, name: &str) -> Option<Vec<u8>> {
        self.parts.get(name).map(|p| p.bytes().into_owned())
    }

    /// Whether a part exists (no allocation, unlike [`Package::part`]).
    pub(crate) fn has_part(&self, name: &str) -> bool {
        self.parts.keys().any(|p| p.eq_ignore_ascii_case(name))
    }

    /// Remove one retained part and any matching content-type override.
    /// The part name remains in `touched` so edit diagnostics include a removed
    /// part, while write validation skips it because it is no longer emitted.
    fn remove_part(&mut self, name: &str) -> bool {
        let Some(actual) = self
            .parts
            .keys()
            .find(|part| part.eq_ignore_ascii_case(name))
            .cloned()
        else {
            return false;
        };
        self.parts.remove(&actual);
        self.order
            .retain(|part| !part.eq_ignore_ascii_case(&actual));
        self.touched.insert(actual.clone());

        let part_name = format!("/{actual}");
        let before = self.ctypes.overrides.len();
        self.ctypes
            .overrides
            .retain(|override_part| !override_part.part_name.eq_ignore_ascii_case(&part_name));
        if self.ctypes.overrides.len() != before {
            self.regen_content_types();
        }
        true
    }

    /// Whether the package declares exactly `content_type` for `part`, through
    /// either a matching Override or the part extension's Default.
    pub(crate) fn part_resolves_as(&self, part: &str, content_type: &str) -> bool {
        self.ctypes.resolves_as(part, content_type)
    }

    /// All retained part names, without a leading slash.
    pub(crate) fn part_names(&self) -> impl Iterator<Item = &str> {
        self.parts.keys().map(String::as_str)
    }

    /// Package parts that were added, replaced, promoted for XML editing, or
    /// regenerated during preservation edits in this session.
    pub(crate) fn touched_parts(&self) -> Vec<String> {
        let mut parts: Vec<String> = self.touched.iter().cloned().collect();
        parts.sort();
        parts
    }

    /// A read-only handle to a part's tree **iff it is already promoted** (`Part::Xml`).
    /// Returns `None` for a still-`Raw` (unedited) part — the caller can parse a throwaway
    /// copy instead. Unlike [`Package::part_tree_mut`] this never promotes or marks the
    /// part touched, so a preflight that reads the live tree can't dirty an unedited part.
    pub(crate) fn part_tree_ref(&self, name: &str) -> Option<&XmlTree> {
        match self.parts.get(name) {
            Some(Part::Xml(t)) => Some(t),
            _ => None,
        }
    }

    /// Promote a part to an editable [`XmlTree`] (lazy: parsed on first call,
    /// cached), returning a mutable handle. Subsequent [`Package::to_zip`] /
    /// [`Package::part`] re-serialize the edited tree; every other part stays raw.
    pub(crate) fn part_tree_mut(&mut self, name: &str) -> Result<&mut XmlTree> {
        let entry = self
            .parts
            .get_mut(name)
            .ok_or_else(|| Error::Docx(format!("no part {name}")))?;
        if let Part::Raw(bytes) = entry {
            // Promote first; only mark the part touched once promotion has succeeded, so
            // a failed lookup/parse leaves the package's validation state unchanged.
            *entry = Part::Xml(XmlTree::parse(bytes)?);
        }
        self.touched.insert(name.to_string());
        match entry {
            Part::Xml(t) => Ok(t),
            Part::Raw(_) => unreachable!("just promoted to Xml"),
        }
    }

    /// Add or replace a part's bytes. `content_type` `Some` ⇒ ensure an `<Override>`
    /// declares exactly that type for the part (adding it, or correcting a stale/
    /// mismatched one), regenerating `[Content_Types].xml` only when it changes.
    pub(crate) fn set_part(&mut self, name: &str, bytes: Vec<u8>, content_type: Option<&str>) {
        let store_name = match self
            .parts
            .keys()
            .find(|p| p.eq_ignore_ascii_case(name))
            .cloned()
        {
            Some(existing) => existing,
            None => {
                self.order.push(name.to_string());
                name.to_string()
            }
        };
        self.touched.insert(store_name.clone());
        self.parts.insert(store_name.clone(), Part::Raw(bytes));
        if let Some(ct) = content_type {
            let pn = format!("/{store_name}");
            let mut matched = false;
            let mut changed = false;
            for o in self
                .ctypes
                .overrides
                .iter_mut()
                .filter(|o| o.part_name.eq_ignore_ascii_case(&pn))
            {
                matched = true;
                if o.content_type != ct {
                    o.content_type = ct.to_string(); // correct a mismatched override
                    changed = true;
                }
            }
            if matched {
                if changed {
                    self.regen_content_types();
                }
            } else if !self.ctypes.resolves_as(&store_name, ct) {
                self.ctypes.overrides.push(CtOverride::new(pn, ct));
                self.regen_content_types();
            }
        }
    }

    /// Ensure an existing part has **exactly** the given content type via an `<Override>`
    /// (adding or correcting it; regenerating `[Content_Types].xml` only when it changes),
    /// **without** touching the part's bytes. Used so an element-tree edit of a core part
    /// (`word/document.xml`) guarantees the saved package is correctly typed even if the
    /// source omitted/mistyped the override — a generic `Default` would resolve but leave
    /// the part mistyped for Word. No-op (byte-stable) when the override is already correct.
    pub(crate) fn ensure_content_type(&mut self, name: &str, content_type: &str) {
        let pn = format!("/{name}");
        let mut matched = false;
        let mut changed = false;
        for o in self
            .ctypes
            .overrides
            .iter_mut()
            .filter(|o| o.part_name.eq_ignore_ascii_case(&pn))
        {
            matched = true;
            if o.content_type != content_type {
                o.content_type = content_type.to_string();
                changed = true;
            }
        }
        if matched {
            if changed {
                self.regen_content_types();
            }
        } else {
            self.ctypes
                .overrides
                .push(CtOverride::new(pn, content_type));
            self.regen_content_types();
        }
        self.touched.insert(name.to_string());
    }

    /// Relationships declared for `content_part` (resolves its sibling `_rels`).
    pub(crate) fn rels_for(&self, content_part: &str) -> &[Rel] {
        self.rels
            .get(&rels_path_of(content_part))
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Remove image relationships from an edited XML part only when their ids no
    /// longer occur in that part, and remove the target media part only when no
    /// other retained relationship points at it. This is deliberately narrower
    /// than general OPC garbage collection: unknown relationships and unrelated
    /// media remain preserved.
    pub(crate) fn prune_unreferenced_media_relationships(
        &mut self,
        content_part: &str,
    ) -> Result<()> {
        let rels_path = rels_path_of(content_part);
        let (retained, removed_targets) = {
            let Some(tree) = self.part_tree_ref(content_part) else {
                return Err(Error::Docx(format!("no XML part {content_part}")));
            };
            let references = tree.relationship_references();
            let Some(entries) = self.rels.get(&rels_path) else {
                return Ok(());
            };
            let mut retained = Vec::with_capacity(entries.len());
            let mut removed_targets = Vec::new();
            for rel in entries {
                let target = resolve_rel_target(content_part, rel_target_part_path(&rel.target));
                let removable = !rel.external
                    && rel.rel_type == REL_IMAGE
                    && is_word_media_part(&target)
                    && !references.contains(rel.id.as_bytes());
                if removable {
                    removed_targets.push(target);
                } else {
                    retained.push(rel.clone());
                }
            }
            (retained, removed_targets)
        };

        if removed_targets.is_empty() {
            return Ok(());
        }
        self.rels.insert(rels_path.clone(), retained);
        self.regen_rels(&rels_path);

        for target in removed_targets {
            let still_referenced = self.rels.iter().any(|(source_rels, entries)| {
                let Some(source_part) = source_part_of_rels_path(source_rels) else {
                    return false;
                };
                entries.iter().any(|rel| {
                    !rel.external
                        && resolve_rel_target(&source_part, rel_target_part_path(&rel.target))
                            .eq_ignore_ascii_case(&target)
                })
            });
            if !still_referenced {
                self.remove_part(&target);
            }
        }
        Ok(())
    }

    /// Whether `part` has a resolvable content type (Override or matching Default).
    /// Test-only, for asserting an added part is correctly typed.
    #[cfg(test)]
    pub(crate) fn part_has_content_type(&self, part: &str) -> bool {
        self.ctypes.resolves(part)
    }

    /// Allocate a fresh, collision-free relationship id. Starts above all existing
    /// ids but verifies against the live rels graph and `wrapping_add`s, so it is
    /// panic-free and never repeats even for a hostile `rId18446744073709551615`
    /// (which would saturate a naive counter into duplicates).
    pub(crate) fn alloc_rid(&mut self) -> String {
        loop {
            let cand = self.rid_next;
            self.rid_next = self.rid_next.wrapping_add(1);
            if !self.rid_in_use(cand) {
                return format!("rId{cand}");
            }
        }
    }

    /// Whether `rId{n}` is already declared in any `_rels` part.
    fn rid_in_use(&self, n: u64) -> bool {
        self.rels
            .values()
            .flatten()
            .any(|r| r.id.strip_prefix("rId").and_then(|s| s.parse::<u64>().ok()) == Some(n))
    }

    /// Add a part and a relationship to it from `src_part`, allocating the `rId`,
    /// content-type override, and rels entry together. Returns the new `rId`.
    pub(crate) fn add_related_part(
        &mut self,
        src_part: &str,
        rel_type: &str,
        new_part: &str,
        content_type: Option<&str>,
        bytes: Vec<u8>,
    ) -> String {
        self.set_part(new_part, bytes, content_type);
        let rid = self.alloc_rid();
        let target = rel_target(src_part, new_part);
        let rels_path = rels_path_of(src_part);
        self.rels.entry(rels_path.clone()).or_default().push(Rel {
            id: rid.clone(),
            rel_type: rel_type.to_string(),
            target,
            external: false,
        });
        self.regen_rels(&rels_path);
        rid
    }

    /// Ensure `src_part` has an internal relationship of `rel_type` to `target_part`,
    /// returning the existing or newly allocated relationship id.
    pub(crate) fn ensure_relationship(
        &mut self,
        src_part: &str,
        rel_type: &str,
        target_part: &str,
    ) -> String {
        let rels_path = rels_path_of(src_part);
        if let Some(existing) = self.rels.get(&rels_path).and_then(|entries| {
            entries.iter().find(|rel| {
                !rel.external
                    && rel.rel_type == rel_type
                    && resolve_rel_target(src_part, &rel.target).eq_ignore_ascii_case(target_part)
            })
        }) {
            return existing.id.clone();
        }

        let rid = self.alloc_rid();
        let target = rel_target(src_part, target_part);
        self.rels.entry(rels_path.clone()).or_default().push(Rel {
            id: rid.clone(),
            rel_type: rel_type.to_string(),
            target,
            external: false,
        });
        self.regen_rels(&rels_path);
        rid
    }

    /// Update the target of an existing external relationship and regenerate only
    /// that relationship part. The relationship must match the expected type so a
    /// caller cannot accidentally retarget images, headers, or internal package parts.
    pub(crate) fn set_external_relationship_target(
        &mut self,
        src_part: &str,
        rel_type: &str,
        rel_id: &str,
        target: &str,
    ) -> Result<()> {
        if target.is_empty() {
            return Err(Error::Docx("relationship target must not be empty".into()));
        }
        if target.chars().any(|c| !is_xml_legal_char(c)) {
            return Err(Error::Docx(
                "relationship target contains an XML-illegal character".into(),
            ));
        }

        let rels_path = rels_path_of(src_part);
        let changed = {
            let entries = self
                .rels
                .get_mut(&rels_path)
                .ok_or_else(|| Error::Docx(format!("missing relationships for {src_part}")))?;
            let rel = entries
                .iter_mut()
                .find(|rel| rel.id == rel_id && rel.rel_type == rel_type && rel.external)
                .ok_or_else(|| {
                    Error::Docx(format!(
                        "relationship {rel_id:?} is not an external relationship of the expected type"
                    ))
                })?;
            if rel.target == target {
                false
            } else {
                rel.target = target.to_string();
                true
            }
        };
        if changed {
            self.regen_rels(&rels_path);
        }
        Ok(())
    }

    /// Regenerate the `[Content_Types].xml` part bytes from the parsed view.
    fn regen_content_types(&mut self) {
        let mut s = String::new();
        s.push_str(XML_DECL);
        s.push_str(&format!(r#"<Types xmlns="{CT_NS}">"#));
        for d in &self.ctypes.defaults {
            s.push_str(&format!(
                r#"<Default Extension="{}" ContentType="{}"/>"#,
                esc(&d.extension),
                esc(&d.content_type)
            ));
        }
        for o in &self.ctypes.overrides {
            s.push_str(&format!(
                r#"<Override PartName="{}" ContentType="{}"/>"#,
                esc(&o.part_name),
                esc(&o.content_type)
            ));
        }
        s.push_str("</Types>");
        if !self.parts.contains_key(CONTENT_TYPES) {
            self.order.insert(0, CONTENT_TYPES.to_string());
        }
        self.parts
            .insert(CONTENT_TYPES.to_string(), Part::Raw(s.into_bytes()));
        self.touched.insert(CONTENT_TYPES.to_string());
    }

    /// Regenerate a `_rels/*.rels` part's bytes from the parsed view.
    fn regen_rels(&mut self, rels_path: &str) {
        let Some(entries) = self.rels.get(rels_path) else {
            return;
        };
        let mut s = String::new();
        s.push_str(XML_DECL);
        s.push_str(&format!(r#"<Relationships xmlns="{REL_NS}">"#));
        for r in entries {
            s.push_str(&format!(
                r#"<Relationship Id="{}" Type="{}" Target="{}""#,
                esc(&r.id),
                esc(&r.rel_type),
                esc(&r.target)
            ));
            if r.external {
                s.push_str(r#" TargetMode="External""#);
            }
            s.push_str("/>");
        }
        s.push_str("</Relationships>");
        if !self.parts.contains_key(rels_path) {
            self.order.push(rels_path.to_string());
        }
        self.parts
            .insert(rels_path.to_string(), Part::Raw(s.into_bytes()));
        self.touched.insert(rels_path.to_string());
        // If the source `[Content_Types].xml` lacked the mandatory `rels` Default (only
        // injected into the in-memory view), the `.rels` we just wrote would be untyped on
        // save. Regenerate `[Content_Types].xml` now so the injected default is serialized.
        // Only fires on an actual edit that writes relationships — no-op saves never reach
        // here, so the byte-stability of an unedited package is unaffected.
        if self.ct_rels_injected {
            self.regen_content_types();
        }
    }
}

fn validate_write_payload(name: &str, bytes: &[u8], total: &mut u64) -> Result<()> {
    if name.len() > MAX_NAME_LEN {
        return Err(Error::Docx(format!("part name too long on save: {name}")));
    }
    if bytes.len() as u64 > max_part() {
        return Err(Error::Docx(format!(
            "part {name} exceeds the per-part size budget on save"
        )));
    }
    *total = total.saturating_add(bytes.len() as u64);
    if *total > MAX_TOTAL {
        return Err(Error::Docx(
            "package exceeds the total size budget on save".into(),
        ));
    }
    Ok(())
}

/// `word/document.xml` → `word/_rels/document.xml.rels`; `` (root) handled too.
fn rels_path_of(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

/// Inverse of [`rels_path_of`]: `_rels/.rels` → ``,
/// `word/_rels/document.xml.rels` → `word/document.xml`.
fn source_part_of_rels_path(rels_path: &str) -> Option<String> {
    if rels_path == "_rels/.rels" {
        return Some(String::new());
    }
    let rels_source = rels_path.strip_suffix(".rels")?;
    if let Some(file) = rels_source.strip_prefix("_rels/") {
        return (!file.is_empty()).then(|| file.to_string());
    }
    let (dir, file) = rels_source.rsplit_once("/_rels/")?;
    (!dir.is_empty() && !file.is_empty()).then(|| format!("{dir}/{file}"))
}

/// Is this a relationships part — `.rels` under a `_rels/` directory? (A content
/// part merely *named* `foo.rels` outside `_rels/` is not a relationships part.)
fn is_rels(name: &str) -> bool {
    name.ends_with(".rels") && (name.starts_with("_rels/") || name.contains("/_rels/"))
}

fn is_word_media_part(name: &str) -> bool {
    name.strip_prefix("word/media/")
        .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
}

/// Relationship target for `new_part` relative to `src_part`'s directory.
/// (Common case: both under `word/` ⇒ a path relative to `word/`.)
fn rel_target(src_part: &str, new_part: &str) -> String {
    let src_dir = src_part.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    if src_dir.is_empty() {
        return new_part.to_string();
    }
    if !src_dir.is_empty() {
        if let Some(rest) = new_part.strip_prefix(&format!("{src_dir}/")) {
            return rest.to_string();
        }
    }
    // Fall back to an absolute package path.
    format!("/{new_part}")
}

fn rel_target_part_path(target: &str) -> &str {
    let before_fragment = target.split_once('#').map_or(target, |(path, _)| path);
    before_fragment
        .split_once('?')
        .map_or(before_fragment, |(path, _)| path)
}

/// Resolve an internal relationship target against `src_part`'s directory and
/// normalize dot segments to an OPC package part name without a leading slash.
pub(crate) fn resolve_rel_target(src_part: &str, target: &str) -> String {
    let base: Vec<&str> = if target.starts_with('/') {
        Vec::new()
    } else {
        src_part
            .rsplit_once('/')
            .map(|(dir, _)| dir.split('/').filter(|s| !s.is_empty()).collect())
            .unwrap_or_default()
    };
    let mut segs = base;
    for seg in target.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            s => segs.push(s),
        }
    }
    segs.join("/")
}

/// Minimal XML attribute-value escape.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ if is_xml_legal_char(c) => o.push(c),
            _ => {}
        }
    }
    o
}

fn is_xml_legal_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r')
        || matches!(
            c as u32,
            0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF
        )
}

fn is_xml_whitespace(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|b| matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
}

/// Local name of a possibly-prefixed qualified name (`ct:Default` or `Default`). OPC
/// vocabularies may be namespace-prefixed, so element matching must be local-name
/// based (the OPC attributes themselves are always unprefixed / in no namespace).
fn local(name: &[u8]) -> &[u8] {
    match name.iter().position(|&b| b == b':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

/// All of an element's attributes as `(key, unescaped value)`, **propagating** any
/// attribute-iterator error (malformed syntax, duplicate names). Used by the OPC
/// metadata parsers so a malformed `[Content_Types].xml` / `.rels` fails cleanly rather
/// than being silently accepted as a partial graph that a later edit regenerates lossily.
fn attrs_of(e: &quick_xml::events::BytesStart<'_>) -> Result<Vec<(Vec<u8>, String)>> {
    let mut out = Vec::new();
    for a in e.attributes() {
        if out.len() >= max_meta_records() {
            return Err(Error::Docx("opc element has too many attributes".into()));
        }
        let a = a.map_err(|err| Error::Docx(format!("opc attr: {err}")))?;
        // Propagate (not swallow) a bad entity reference / non-UTF-8 value: malformed
        // metadata must fail cleanly, never parse to a lossy partial graph.
        let v = a
            .decoded_and_normalized_value(quick_xml::XmlVersion::Implicit1_0, e.decoder())
            .map_err(|err| Error::Docx(format!("opc attr value: {err}")))?
            .into_owned();
        if v.chars().any(|c| !is_xml_legal_char(c)) {
            return Err(Error::Docx(
                "opc attr value contains an XML-illegal character".into(),
            ));
        }
        out.push((a.key.as_ref().to_vec(), v));
    }
    Ok(out)
}

fn validate_modeled_attrs(
    attrs: &[(Vec<u8>, String)],
    modeled: &[&[u8]],
    where_: &str,
) -> Result<()> {
    for (k, _) in attrs {
        if !modeled.contains(&k.as_slice()) {
            return Err(Error::Docx(format!("{where_}: unmodeled attribute")));
        }
    }
    Ok(())
}

fn validate_opc_root_attrs(attrs: &[(Vec<u8>, String)], want_ns: &str, part: &str) -> Result<()> {
    match attrs {
        [(k, v)] if k.as_slice() == b"xmlns" && v == want_ns => Ok(()),
        _ => Err(Error::Docx(format!(
            "{part}: root must carry only the default OPC namespace declaration"
        ))),
    }
}

/// First attribute value for `key` among already-extracted `(key, value)` pairs.
fn find_attr<'a>(attrs: &'a [(Vec<u8>, String)], key: &[u8]) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(k, _)| k.as_slice() == key)
        .map(|(_, v)| v.as_str())
}

/// Reject a ZIP whose End-Of-Central-Directory declares more than `MAX_ENTRIES` entries,
/// **before** any `ZipArchive::new` parses (and allocates) the central directory. Shared
/// by the package layer ([`Package::from_zip`]) and the `.docx` reader so both the
/// read-model and preservation paths are guarded by one authoritative limit. Best-effort
/// (a non-locatable EOCD passes here; the post-construction `zip.len()` check backstops it).
pub(crate) fn check_zip_entry_budget(bytes: &[u8]) -> Result<()> {
    if let Some(n) = eocd_entry_count(bytes) {
        if n > max_entries() as u64 {
            return Err(Error::Docx(format!(
                "package declares too many entries ({n} > {})",
                max_entries()
            )));
        }
    }
    Ok(())
}

/// Best-effort total-entry count from a ZIP's End-Of-Central-Directory record (handling
/// ZIP64), read straight from the bytes without parsing the central directory — so a
/// hostile archive can be rejected before the `zip` crate allocates per-entry state.
/// Returns `None` if the EOCD can't be located/parsed (caller falls back to the
/// post-construction count). Bounded: the backward scan covers only the EOCD + max
/// comment, and all indexing is checked.
fn eocd_entry_count(bytes: &[u8]) -> Option<u64> {
    const EOCD_SIG: [u8; 4] = [0x50, 0x4b, 0x05, 0x06];
    const Z64_LOC_SIG: [u8; 4] = [0x50, 0x4b, 0x06, 0x07];
    const Z64_EOCD_SIG: [u8; 4] = [0x50, 0x4b, 0x06, 0x06];
    const EOCD_MIN: usize = 22; // fixed EOCD size (no comment)
    if bytes.len() < EOCD_MIN {
        return None;
    }
    // Scan backward for an EOCD signature within the last EOCD_MIN + 65535 bytes, and
    // accept only a candidate whose declared comment length actually reaches EOF — a ZIP
    // comment can embed fake EOCD-looking bytes, so the signature alone isn't enough.
    let scan_start = bytes.len().saturating_sub(EOCD_MIN + 0xFFFF);
    let mut e = None;
    let mut i = bytes.len() - EOCD_MIN;
    loop {
        if bytes[i..i + 4] == EOCD_SIG {
            let comment_len = u16::from_le_bytes([bytes[i + 20], bytes[i + 21]]) as usize;
            if i + EOCD_MIN + comment_len == bytes.len() {
                e = Some(i);
                break;
            }
        }
        if i == scan_start {
            break;
        }
        i -= 1;
    }
    let e = e?;
    let total16 = u16::from_le_bytes([bytes[e + 10], bytes[e + 11]]);
    if total16 != 0xFFFF {
        return Some(u64::from(total16));
    }
    // ZIP64: the locator sits 20 bytes before the EOCD and points at the ZIP64 EOCD,
    // whose total-entries field (u64) is at offset +32.
    let loc = e.checked_sub(20)?;
    if bytes[loc..loc + 4] != Z64_LOC_SIG {
        return Some(u64::from(total16));
    }
    let z64_off = u64::from_le_bytes(bytes.get(loc + 8..loc + 16)?.try_into().ok()?) as usize;
    let z64 = bytes.get(z64_off..z64_off.checked_add(40)?)?;
    if z64[0..4] != Z64_EOCD_SIG {
        return None;
    }
    Some(u64::from_le_bytes(z64[32..40].try_into().ok()?))
}

/// Whether `name`'s local part is a `[Content_Types]` record element (`Default`/`Override`).
fn is_ct_record_name(name: &[u8]) -> bool {
    matches!(local(name), b"Default" | b"Override")
}

/// Dispatch a non-root `[Content_Types].xml` child. A correct-namespace record is folded in;
/// an element whose *local* name is a record (`Default`/`Override`) but whose namespace is
/// NOT the content-types namespace (e.g. an `xmlns=""`-stripped or foreign-namespaced record)
/// is **rejected** — it must not be silently dropped, because a later edit would regenerate
/// `[Content_Types].xml` from the resulting partial view and lose that part's type. A
/// genuinely foreign element (a different local name) is ignored (forward-compatible).
fn ct_child(
    ns: &ResolveResult<'_>,
    e: &quick_xml::events::BytesStart<'_>,
    ct: &mut ContentTypes,
) -> Result<()> {
    if ns_is(ns, CT_NS) {
        ct_record(e, ct)
    } else if is_ct_record_name(e.name().as_ref()) {
        Err(Error::Docx(
            "[Content_Types].xml: a Default/Override is outside the content-types namespace".into(),
        ))
    } else {
        Ok(())
    }
}

/// Dispatch a non-root `.rels` child — the relationships analogue of [`ct_child`]: a
/// `Relationship` outside `REL_NS` is rejected (not silently dropped); other foreign
/// elements are ignored.
fn rel_child(
    ns: &ResolveResult<'_>,
    e: &quick_xml::events::BytesStart<'_>,
    out: &mut Vec<Rel>,
) -> Result<()> {
    if ns_is(ns, REL_NS) {
        rel_record(e, out)
    } else if local(e.name().as_ref()) == b"Relationship" {
        Err(Error::Docx(
            ".rels: a Relationship is outside the relationships namespace".into(),
        ))
    } else {
        Ok(())
    }
}

fn is_mime_token(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| {
            b.is_ascii()
                && !b.is_ascii_control()
                && !b.is_ascii_whitespace()
                && !matches!(
                    b,
                    b'(' | b')'
                        | b'<'
                        | b'>'
                        | b'@'
                        | b','
                        | b';'
                        | b':'
                        | b'\\'
                        | b'"'
                        | b'/'
                        | b'['
                        | b']'
                        | b'?'
                        | b'='
                )
        })
}

fn valid_media_type(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    let core = value.split(';').next().unwrap_or("").trim_end();
    let Some((ty, subtype)) = core.split_once('/') else {
        return false;
    };
    is_mime_token(ty) && is_mime_token(subtype) && !subtype.contains('/')
}

fn has_valid_percent_escapes(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            if i + 2 >= b.len() || !b[i + 1].is_ascii_hexdigit() || !b[i + 2].is_ascii_hexdigit() {
                return false;
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    true
}

fn valid_ct_extension(ext: &str) -> bool {
    !ext.is_empty()
        && ext.trim() == ext
        && !ext
            .chars()
            .any(|c| !is_xml_legal_char(c) || c.is_ascii_whitespace())
        && !ext
            .as_bytes()
            .iter()
            .any(|&b| matches!(b, b'/' | b'\\' | b'.'))
        && has_valid_percent_escapes(ext)
}

/// Fold one `[Content_Types]` element (`Default`/`Override`) into `ct`. A correct-namespace
/// `Default`/`Override` missing its required attributes is an **error** (not silently
/// skipped): regenerating `[Content_Types].xml` from a view that dropped a malformed record
/// would lose the part it was meant to type — so such a package becomes `meta_lossy`
/// (read-only) instead. Unknown CT-namespace elements are ignored (forward-compatible).
fn ct_record(e: &quick_xml::events::BytesStart<'_>, ct: &mut ContentTypes) -> Result<()> {
    match local(e.name().as_ref()) {
        b"Default" => {
            let a = attrs_of(e)?;
            validate_modeled_attrs(
                &a,
                &[&b"Extension"[..], &b"ContentType"[..]],
                "[Content_Types].xml: <Default>",
            )?;
            match (find_attr(&a, b"Extension"), find_attr(&a, b"ContentType")) {
                (Some(x), Some(c)) => {
                    if !valid_ct_extension(x) {
                        return Err(Error::Docx(format!(
                            "[Content_Types].xml: <Default> has an invalid Extension {x:?}"
                        )));
                    }
                    if !valid_media_type(c) {
                        return Err(Error::Docx(format!(
                            "[Content_Types].xml: <Default> has an invalid ContentType {c:?}"
                        )));
                    }
                    ct.defaults.push(CtDefault::new(x, c));
                }
                _ => {
                    return Err(Error::Docx(
                        "[Content_Types].xml: <Default> missing Extension/ContentType".into(),
                    ))
                }
            }
        }
        b"Override" => {
            let a = attrs_of(e)?;
            validate_modeled_attrs(
                &a,
                &[&b"PartName"[..], &b"ContentType"[..]],
                "[Content_Types].xml: <Override>",
            )?;
            match (find_attr(&a, b"PartName"), find_attr(&a, b"ContentType")) {
                (Some(p), Some(c)) => {
                    if !valid_override_part_name(p) {
                        return Err(Error::Docx(format!(
                            "[Content_Types].xml: <Override> has an invalid PartName {p:?}"
                        )));
                    }
                    if !valid_media_type(c) {
                        return Err(Error::Docx(format!(
                            "[Content_Types].xml: <Override> has an invalid ContentType {c:?}"
                        )));
                    }
                    ct.overrides.push(CtOverride::new(p, c));
                }
                _ => {
                    return Err(Error::Docx(
                        "[Content_Types].xml: <Override> missing PartName/ContentType".into(),
                    ))
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// OPC Override `PartName` values are absolute pack part names (`/word/document.xml`).
/// Reject malformed values instead of parsing a graph that a later edit would regenerate
/// from lossy or ambiguous metadata.
fn valid_override_part_name(name: &str) -> bool {
    if name.eq_ignore_ascii_case("/[Content_Types].xml") {
        return false;
    }
    let Some(rest) = name.strip_prefix('/') else {
        return false;
    };
    if rest.is_empty() || rest.len() > MAX_NAME_LEN {
        return false;
    }
    rest.split('/').all(|seg| {
        !seg.is_empty()
            && seg != "."
            && seg != ".."
            && !seg.as_bytes().contains(&b'\\')
            && has_valid_percent_escapes(seg)
    })
}

/// Whether a resolved element namespace equals `want` (an OPC vocabulary URI). OPC
/// metadata records are only valid in their own namespace, so a foreign-namespace
/// `<x:Override>`/`<x:Relationship>` must NOT be treated as real metadata.
fn ns_is(ns: &ResolveResult<'_>, want: &str) -> bool {
    matches!(ns, ResolveResult::Bound(Namespace(uri)) if *uri == want.as_bytes())
}

/// Validate the single top-level element of an OPC metadata part: it must resolve to the
/// expected (`want_ns`, `want_local`) and there must be only one such root. A wrong root
/// (foreign/absent namespace, wrong name) or a second root makes the part malformed, so the
/// caller treats it as `meta_lossy` rather than regenerating from a misread (empty) graph.
fn validate_opc_root(
    ns: &ResolveResult<'_>,
    e: &quick_xml::events::BytesStart<'_>,
    want_ns: &str,
    want_local: &[u8],
    part: &str,
    saw_root: bool,
) -> Result<()> {
    if saw_root {
        return Err(Error::Docx(format!("{part}: multiple root elements")));
    }
    if !ns_is(ns, want_ns) || local(e.name().as_ref()) != want_local {
        return Err(Error::Docx(format!(
            "{part}: root is not <{}> in the expected namespace",
            String::from_utf8_lossy(want_local)
        )));
    }
    Ok(())
}

fn parse_content_types(xml: &[u8]) -> Result<ContentTypes> {
    let mut r = NsReader::from_reader(xml);
    r.config_mut().check_end_names = true; // reject mismatched end tags
    r.resolver_mut()
        .set_max_declarations_per_element(MAX_NAMESPACE_DECLARATIONS_PER_ELEMENT);
    let mut ct = ContentTypes::default();
    let mut buf = Vec::new();
    let mut depth: i32 = 0;
    let mut open_record_depth: Option<i32> = None;
    // The document SHAPE must be validated, not just individual namespaced children: a
    // wrong-namespace (e.g. unnamespaced) `[Content_Types].xml` would otherwise parse to a
    // clean *empty* graph, and a later edit would regenerate it — dropping the real records
    // for styles/numbering/media/etc. Require exactly one `<Types>` root in `CT_NS`.
    let mut saw_root = false;
    let mut saw_decl = false;
    loop {
        // `>` (not `>=`) so a file with exactly the cap reaches EOF; only cap+1 is rejected.
        if ct.defaults.len() + ct.overrides.len() > max_meta_records() {
            return Err(Error::Docx(
                "[Content_Types].xml has too many entries".into(),
            ));
        }
        match r.read_resolved_event_into(&mut buf) {
            Ok((ns, Event::Start(e))) => {
                if depth >= MAX_OPC_XML_DEPTH as i32 {
                    return Err(Error::Docx(format!(
                        "[Content_Types].xml: XML nesting exceeds {MAX_OPC_XML_DEPTH} elements"
                    )));
                }
                if open_record_depth.is_some_and(|d| depth >= d) {
                    return Err(Error::Docx(
                        "[Content_Types].xml: Default/Override must be empty".into(),
                    ));
                }
                if depth == 0 {
                    validate_opc_root(&ns, &e, CT_NS, b"Types", "[Content_Types].xml", saw_root)?;
                    let attrs = attrs_of(&e)?;
                    validate_opc_root_attrs(&attrs, CT_NS, "[Content_Types].xml")?;
                    saw_root = true;
                } else if depth == 1 {
                    ct_child(&ns, &e, &mut ct)?;
                    if ns_is(&ns, CT_NS) && is_ct_record_name(e.name().as_ref()) {
                        open_record_depth = Some(depth + 1);
                    }
                } else if is_ct_record_name(e.name().as_ref()) {
                    return Err(Error::Docx(
                        "[Content_Types].xml: Default/Override is not a direct child of Types"
                            .into(),
                    ));
                }
                depth += 1;
            }
            Ok((ns, Event::Empty(e))) => {
                if open_record_depth.is_some_and(|d| depth >= d) {
                    return Err(Error::Docx(
                        "[Content_Types].xml: Default/Override must be empty".into(),
                    ));
                }
                if depth == 0 {
                    // A self-closing root: `<Types …/>` (valid empty content types).
                    validate_opc_root(&ns, &e, CT_NS, b"Types", "[Content_Types].xml", saw_root)?;
                    let attrs = attrs_of(&e)?;
                    validate_opc_root_attrs(&attrs, CT_NS, "[Content_Types].xml")?;
                    saw_root = true;
                } else if depth == 1 {
                    ct_child(&ns, &e, &mut ct)?;
                } else if is_ct_record_name(e.name().as_ref()) {
                    return Err(Error::Docx(
                        "[Content_Types].xml: Default/Override is not a direct child of Types"
                            .into(),
                    ));
                }
            }
            Ok((_, Event::End(_))) => {
                depth -= 1;
                if open_record_depth.is_some_and(|d| depth < d) {
                    open_record_depth = None;
                }
            }
            Ok((_, Event::Text(t))) => {
                if open_record_depth.is_some_and(|d| depth >= d) && !t.as_ref().is_empty() {
                    return Err(Error::Docx(
                        "[Content_Types].xml: Default/Override must be empty".into(),
                    ));
                }
                if !is_xml_whitespace(t.as_ref()) {
                    return Err(Error::Docx(
                        "[Content_Types].xml: non-whitespace text outside metadata records".into(),
                    ));
                }
            }
            Ok((_, Event::Comment(_))) | Ok((_, Event::PI(_)))
                if open_record_depth.is_some_and(|d| depth >= d) =>
            {
                return Err(Error::Docx(
                    "[Content_Types].xml: Default/Override must be empty".into(),
                ));
            }
            Ok((_, Event::Decl(_))) => {
                if depth != 0 || saw_root || saw_decl {
                    return Err(Error::Docx(
                        "[Content_Types].xml: XML declaration is only allowed before the root"
                            .into(),
                    ));
                }
                saw_decl = true;
            }
            Ok((_, Event::CData(_))) => {
                return Err(Error::Docx(
                    "[Content_Types].xml: character data outside metadata records".into(),
                ));
            }
            Ok((_, Event::GeneralRef(_))) => {
                return Err(Error::Docx(
                    "[Content_Types].xml: character data outside metadata records".into(),
                ));
            }
            Ok((_, Event::DocType(_))) => {
                return Err(Error::Docx(
                    "[Content_Types].xml: doctype is not allowed".into(),
                ));
            }
            // A malformed `[Content_Types].xml` must NOT be accepted as a partial graph:
            // a later edit would regenerate it from that lossy view and silently drop
            // records. Reject EOF with elements still open (unclosed root), and propagate
            // parse errors. (A no-op save still preserves the raw part — never re-parsed.)
            Ok((_, Event::Eof)) => {
                if depth != 0 {
                    return Err(Error::Docx("[Content_Types].xml: unclosed element".into()));
                }
                break;
            }
            Err(e) => return Err(Error::Docx(format!("[Content_Types].xml parse: {e}"))),
            _ => {}
        }
        buf.clear();
    }
    if !saw_root {
        return Err(Error::Docx(
            "[Content_Types].xml: no <Types> root element".into(),
        ));
    }
    // Collapse exact-duplicate records (same key AND value). They are harmless in the source,
    // but if left in place an edit that repairs a part's type via `set_part`/`ensure_content_type`
    // would rewrite only the FIRST matching `<Override>` and strand the rest as now-conflicting
    // duplicates — producing a `[Content_Types].xml` this very parser rejects on reopen.
    dedup_identical_defaults(&mut ct.defaults);
    dedup_identical_overrides(&mut ct.overrides);
    // Conflicting duplicate records (same key, different content type) are ambiguous —
    // regenerating from such a view could pick the wrong type. Reject (→ `meta_lossy`,
    // read-only) rather than guess. Extensions and part names are matched
    // case-insensitively.
    if has_default_conflict(&ct.defaults) || has_override_conflict(&ct.overrides) {
        return Err(Error::Docx(
            "[Content_Types].xml has conflicting duplicate records".into(),
        ));
    }
    // The mandatory `rels` Default must resolve `.rels` parts to the relationships type.
    // A *wrong-typed* one is malformed and ambiguous → reject (→ `meta_lossy`, read-only).
    // A *missing* one is injected into this view AND flagged: real-world `.docx` files
    // (including some in the wild) omit it, so refusing them would needlessly block edits;
    // instead `Package::regen_rels` regenerates `[Content_Types].xml` on any edit that
    // writes a `.rels` part, so the injected default reaches disk and the part is typed
    // (a no-op save still preserves the original raw bytes verbatim — never re-serialized).
    match ct
        .defaults
        .iter()
        .find(|d| d.extension.eq_ignore_ascii_case("rels"))
    {
        Some(d) if d.content_type == CT_RELS => {}
        Some(_) => {
            return Err(Error::Docx(
                "[Content_Types].xml: `rels` Default has the wrong content type".into(),
            ));
        }
        None => {
            ct.defaults.push(CtDefault::new("rels", CT_RELS));
            ct.rels_default_injected = true;
        }
    }
    Ok(ct)
}

/// Drop exact-duplicate content-type records, keeping the first.
fn dedup_identical_defaults(defaults: &mut Vec<CtDefault>) {
    let mut seen: HashSet<CtRecordIdentity> = HashSet::new();
    defaults.retain(|d| seen.insert((d.extension.to_ascii_lowercase(), d.content_type.clone())));
}

fn dedup_identical_overrides(overrides: &mut Vec<CtOverride>) {
    let mut seen: HashSet<CtRecordIdentity> = HashSet::new();
    overrides.retain(|o| seen.insert((o.part_name.to_ascii_lowercase(), o.content_type.clone())));
}

/// Whether the same content-type identity key has two different content types.
fn has_content_type_conflict<I>(items: I) -> bool
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut seen: HashMap<String, String> = HashMap::new();
    for (key, value) in items {
        if let Some(prev) = seen.insert(key, value.clone()) {
            if prev != value {
                return true;
            }
        }
    }
    false
}

fn has_default_conflict(defaults: &[CtDefault]) -> bool {
    has_content_type_conflict(
        defaults
            .iter()
            .map(|d| (d.extension.to_ascii_lowercase(), d.content_type.clone())),
    )
}

fn has_override_conflict(overrides: &[CtOverride]) -> bool {
    has_content_type_conflict(
        overrides
            .iter()
            .map(|o| (o.part_name.to_ascii_lowercase(), o.content_type.clone())),
    )
}

fn is_xml_ncname_start(c: char) -> bool {
    let u = c as u32;
    c == '_'
        || matches!(
            u,
            0x41..=0x5A
                | 0x61..=0x7A
                | 0xC0..=0xD6
                | 0xD8..=0xF6
                | 0xF8..=0x2FF
                | 0x370..=0x37D
                | 0x37F..=0x1FFF
                | 0x200C..=0x200D
                | 0x2070..=0x218F
                | 0x2C00..=0x2FEF
                | 0x3001..=0xD7FF
                | 0xF900..=0xFDCF
                | 0xFDF0..=0xFFFD
                | 0x10000..=0xEFFFF
        )
}

fn is_xml_ncname_char(c: char) -> bool {
    let u = c as u32;
    is_xml_ncname_start(c)
        || c == '-'
        || c == '.'
        || matches!(u, 0x30..=0x39 | 0xB7 | 0x0300..=0x036F | 0x203F..=0x2040)
}

fn valid_rel_id(id: &str) -> bool {
    let mut chars = id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_xml_ncname_start(first) && chars.all(is_xml_ncname_char)
}

/// Fold one `Relationship` element into `out`. A correct-namespace `Relationship` missing
/// its required attributes is an **error** (not silently skipped) — see [`ct_record`] for
/// why dropping a malformed record before a potential metadata regeneration is unsafe.
fn rel_record(e: &quick_xml::events::BytesStart<'_>, out: &mut Vec<Rel>) -> Result<()> {
    if local(e.name().as_ref()) == b"Relationship" {
        let a = attrs_of(e)?;
        validate_modeled_attrs(
            &a,
            &[&b"Id"[..], &b"Type"[..], &b"Target"[..], &b"TargetMode"[..]],
            ".rels: <Relationship>",
        )?;
        match (
            find_attr(&a, b"Id"),
            find_attr(&a, b"Type"),
            find_attr(&a, b"Target"),
        ) {
            (Some(id), Some(rel_type), Some(target)) => {
                if !valid_rel_id(id) {
                    return Err(Error::Docx(format!(
                        ".rels: <Relationship> has an invalid Id {id:?}"
                    )));
                }
                if rel_type.trim().is_empty() {
                    return Err(Error::Docx(
                        ".rels: <Relationship> has an empty Type".into(),
                    ));
                }
                if target.is_empty() {
                    return Err(Error::Docx(
                        ".rels: <Relationship> has an empty Target".into(),
                    ));
                }
                // `TargetMode` is a closed OPC enum: absent (≡ Internal), `Internal`, or
                // `External`. Any other value (e.g. lowercase `external`) is malformed — do
                // NOT coerce it to internal, because `regen_rels` would then drop `TargetMode`
                // and silently turn an external target into a package-internal one. Reject it
                // → `meta_lossy` so the edit surface refuses to regenerate the part lossily.
                let external = match find_attr(&a, b"TargetMode") {
                    None | Some("Internal") => false,
                    Some("External") => true,
                    Some(other) => {
                        return Err(Error::Docx(format!(
                            ".rels: <Relationship> has an invalid TargetMode {other:?}"
                        )))
                    }
                };
                out.push(Rel {
                    id: id.to_string(),
                    rel_type: rel_type.to_string(),
                    target: target.to_string(),
                    external,
                });
            }
            _ => {
                return Err(Error::Docx(
                    ".rels: <Relationship> missing Id/Type/Target".into(),
                ))
            }
        }
    }
    Ok(())
}

fn parse_rels(xml: &[u8]) -> Result<Vec<Rel>> {
    let mut r = NsReader::from_reader(xml);
    r.config_mut().check_end_names = true;
    r.resolver_mut()
        .set_max_declarations_per_element(MAX_NAMESPACE_DECLARATIONS_PER_ELEMENT);
    let mut out = Vec::new();
    let mut buf = Vec::new();
    let mut depth: i32 = 0;
    let mut open_record_depth: Option<i32> = None;
    // Validate the document shape (see `parse_content_types`): a wrong-namespace `.rels`
    // would otherwise parse to an empty graph, and an edit regenerating it would drop the
    // original relationships. Require exactly one `<Relationships>` root in `REL_NS`.
    let mut saw_root = false;
    let mut saw_decl = false;
    loop {
        // `>` (not `>=`) so a file with exactly the cap reaches EOF; only cap+1 is rejected.
        if out.len() > max_meta_records() {
            return Err(Error::Docx(
                "a .rels part has too many relationships".into(),
            ));
        }
        match r.read_resolved_event_into(&mut buf) {
            Ok((ns, Event::Start(e))) => {
                if depth >= MAX_OPC_XML_DEPTH as i32 {
                    return Err(Error::Docx(format!(
                        ".rels: XML nesting exceeds {MAX_OPC_XML_DEPTH} elements"
                    )));
                }
                if open_record_depth.is_some_and(|d| depth >= d) {
                    return Err(Error::Docx(".rels: Relationship must be empty".into()));
                }
                if depth == 0 {
                    validate_opc_root(&ns, &e, REL_NS, b"Relationships", ".rels", saw_root)?;
                    let attrs = attrs_of(&e)?;
                    validate_opc_root_attrs(&attrs, REL_NS, ".rels")?;
                    saw_root = true;
                } else if depth == 1 {
                    rel_child(&ns, &e, &mut out)?;
                    if ns_is(&ns, REL_NS) && local(e.name().as_ref()) == b"Relationship" {
                        open_record_depth = Some(depth + 1);
                    }
                } else if local(e.name().as_ref()) == b"Relationship" {
                    return Err(Error::Docx(
                        ".rels: Relationship is not a direct child of Relationships".into(),
                    ));
                }
                depth += 1;
            }
            Ok((ns, Event::Empty(e))) => {
                if open_record_depth.is_some_and(|d| depth >= d) {
                    return Err(Error::Docx(".rels: Relationship must be empty".into()));
                }
                if depth == 0 {
                    // A self-closing root: `<Relationships …/>` (valid empty rels part).
                    validate_opc_root(&ns, &e, REL_NS, b"Relationships", ".rels", saw_root)?;
                    let attrs = attrs_of(&e)?;
                    validate_opc_root_attrs(&attrs, REL_NS, ".rels")?;
                    saw_root = true;
                } else if depth == 1 {
                    rel_child(&ns, &e, &mut out)?;
                } else if local(e.name().as_ref()) == b"Relationship" {
                    return Err(Error::Docx(
                        ".rels: Relationship is not a direct child of Relationships".into(),
                    ));
                }
            }
            Ok((_, Event::End(_))) => {
                depth -= 1;
                if open_record_depth.is_some_and(|d| depth < d) {
                    open_record_depth = None;
                }
            }
            Ok((_, Event::Text(t))) => {
                if open_record_depth.is_some_and(|d| depth >= d) && !t.as_ref().is_empty() {
                    return Err(Error::Docx(".rels: Relationship must be empty".into()));
                }
                if !is_xml_whitespace(t.as_ref()) {
                    return Err(Error::Docx(
                        ".rels: non-whitespace text outside relationship records".into(),
                    ));
                }
            }
            Ok((_, Event::Comment(_))) | Ok((_, Event::PI(_)))
                if open_record_depth.is_some_and(|d| depth >= d) =>
            {
                return Err(Error::Docx(".rels: Relationship must be empty".into()));
            }
            Ok((_, Event::Decl(_))) => {
                if depth != 0 || saw_root || saw_decl {
                    return Err(Error::Docx(
                        ".rels: XML declaration is only allowed before the root".into(),
                    ));
                }
                saw_decl = true;
            }
            Ok((_, Event::CData(_))) => {
                return Err(Error::Docx(
                    ".rels: character data outside relationship records".into(),
                ));
            }
            Ok((_, Event::GeneralRef(_))) => {
                return Err(Error::Docx(
                    ".rels: character data outside relationship records".into(),
                ));
            }
            Ok((_, Event::DocType(_))) => {
                return Err(Error::Docx(".rels: doctype is not allowed".into()));
            }
            // Malformed `.rels` must fail cleanly, not parse to a partial graph that a
            // later edit would regenerate lossily (see `parse_content_types`).
            Ok((_, Event::Eof)) => {
                if depth != 0 {
                    return Err(Error::Docx(".rels: unclosed element".into()));
                }
                break;
            }
            Err(e) => return Err(Error::Docx(format!(".rels parse: {e}"))),
            _ => {}
        }
        buf.clear();
    }
    if !saw_root {
        return Err(Error::Docx(".rels: no <Relationships> root element".into()));
    }
    // Duplicate relationship Ids are invalid OPC and make `rId` references ambiguous —
    // reject (→ `meta_lossy`, read-only) rather than regenerate from an ambiguous graph.
    let mut ids: HashSet<&str> = HashSet::with_capacity(out.len());
    for r in &out {
        if !ids.insert(r.id.as_str()) {
            return Err(Error::Docx(".rels has duplicate relationship Id".into()));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested_metadata(root: &str, namespace: &str, depth: usize) -> String {
        assert!(depth > 0);
        let mut xml = format!(r#"<{root} xmlns="{namespace}">"#);
        for level in 1..depth {
            if level == 1 {
                xml.push_str(r#"<x:n xmlns:x="urn:rwml:test">"#);
            } else {
                xml.push_str("<x:n>");
            }
        }
        for _ in 1..depth {
            xml.push_str("</x:n>");
        }
        xml.push_str(&format!("</{root}>"));
        xml
    }

    #[test]
    fn namespace_declaration_flood_is_bounded() {
        fn flooded_root(name: &str, namespace: &str) -> String {
            let mut xml = format!(r#"<{name} xmlns="{namespace}""#);
            for index in 0..MAX_NAMESPACE_DECLARATIONS_PER_ELEMENT {
                xml.push_str(&format!(r#" xmlns:n{index}="urn:n{index}""#));
            }
            xml.push_str("/>");
            xml
        }

        let content_types = flooded_root("Types", CT_NS);
        let content_types_error = parse_content_types(content_types.as_bytes())
            .expect_err("namespace declaration flood must be rejected")
            .to_string();
        assert!(
            content_types_error.contains("namespace bindings"),
            "unexpected content-types error: {content_types_error}"
        );

        let relationships = flooded_root("Relationships", REL_NS);
        let relationships_error = parse_rels(relationships.as_bytes())
            .expect_err("namespace declaration flood must be rejected")
            .to_string();
        assert!(
            relationships_error.contains("namespace bindings"),
            "unexpected relationships error: {relationships_error}"
        );
    }

    #[test]
    fn opc_xml_depth_cap_accepts_exact_limit_and_rejects_next_level() {
        let content_types_at_cap = nested_metadata("Types", CT_NS, MAX_OPC_XML_DEPTH);
        let content_types_at_cap_result = parse_content_types(content_types_at_cap.as_bytes());
        assert!(
            content_types_at_cap_result.is_ok(),
            "[Content_Types].xml at the exact depth cap must parse: {content_types_at_cap_result:?}"
        );
        let content_types_over_cap = nested_metadata("Types", CT_NS, MAX_OPC_XML_DEPTH + 1);
        let content_types_error = parse_content_types(content_types_over_cap.as_bytes())
            .expect_err("[Content_Types].xml over the depth cap must be rejected")
            .to_string();
        assert!(
            content_types_error.contains("XML nesting exceeds 128 elements"),
            "unexpected content-types depth error: {content_types_error}"
        );

        let relationships_at_cap = nested_metadata("Relationships", REL_NS, MAX_OPC_XML_DEPTH);
        let relationships_at_cap_result = parse_rels(relationships_at_cap.as_bytes());
        assert!(
            relationships_at_cap_result.is_ok(),
            ".rels at the exact depth cap must parse: {relationships_at_cap_result:?}"
        );
        let relationships_over_cap =
            nested_metadata("Relationships", REL_NS, MAX_OPC_XML_DEPTH + 1);
        let relationships_error = parse_rels(relationships_over_cap.as_bytes())
            .expect_err(".rels over the depth cap must be rejected")
            .to_string();
        assert!(
            relationships_error.contains("XML nesting exceeds 128 elements"),
            "unexpected relationships depth error: {relationships_error}"
        );
    }

    #[test]
    fn over_depth_metadata_is_read_only_and_no_op_preserved() {
        let content_types = nested_metadata("Types", CT_NS, MAX_OPC_XML_DEPTH + 1);
        let relationships = nested_metadata("Relationships", REL_NS, MAX_OPC_XML_DEPTH + 1);
        let zip = build_zip(&[
            (CONTENT_TYPES, content_types.as_bytes()),
            ("_rels/.rels", relationships.as_bytes()),
        ]);

        let package = Package::from_zip(&zip)
            .expect("over-depth metadata must remain readable for raw passthrough");
        assert!(
            package.is_meta_lossy(),
            "over-depth metadata must make the package read-only"
        );
        let saved = package
            .to_zip()
            .expect("a no-op save must preserve over-depth metadata");
        let saved_parts = part_set(&saved);
        assert_eq!(
            saved_parts.get(CONTENT_TYPES).map(Vec::as_slice),
            Some(content_types.as_bytes()),
            "no-op save changed raw [Content_Types].xml bytes"
        );
        assert_eq!(
            saved_parts.get("_rels/.rels").map(Vec::as_slice),
            Some(relationships.as_bytes()),
            "no-op save changed raw .rels bytes"
        );
    }

    /// A `.rels` / `[Content_Types].xml` packing more records than the cap
    /// is rejected rather than amplified into the heap. (Cap lowered for the test;
    /// production uses `MAX_META_RECORDS`.)
    #[test]
    fn oversized_opc_metadata_is_rejected() {
        set_test_max_meta(4);
        let mut rels = String::from(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        for i in 0..6 {
            rels.push_str(&format!(
                r#"<Relationship Id="rId{i}" Type="t" Target="x"/>"#
            ));
        }
        rels.push_str("</Relationships>");
        let rels_res = parse_rels(rels.as_bytes());

        let mut ct = String::from(
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">"#,
        );
        for i in 0..6 {
            ct.push_str(&format!(r#"<Override PartName="/p{i}" ContentType="x"/>"#));
        }
        ct.push_str("</Types>");
        let ct_res = parse_content_types(ct.as_bytes());

        // Exactly the cap (4) must parse — only cap+1 is rejected.
        let mut exact = String::from(
            r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">"#,
        );
        for i in 0..4 {
            exact.push_str(&format!(
                r#"<Relationship Id="rId{i}" Type="t" Target="x"/>"#
            ));
        }
        exact.push_str("</Relationships>");
        let exact_res = parse_rels(exact.as_bytes());
        set_test_max_meta(MAX_META_RECORDS);

        assert!(rels_res.is_err(), "oversized .rels not rejected");
        assert!(ct_res.is_err(), "oversized [Content_Types] not rejected");
        assert_eq!(
            exact_res.map(|v| v.len()).ok(),
            Some(4),
            "exactly-the-cap records must parse"
        );
    }

    /// A real package from the crate's own writer, used as a round-trip fixture.
    fn sample_docx() -> Vec<u8> {
        use crate::model::{Block, DocModel, ParaProps, Paragraph, Run};
        let model = DocModel {
            blocks: vec![Block::Paragraph(Paragraph {
                props: ParaProps {
                    heading_level: Some(1),
                    ..ParaProps::default()
                },
                runs: vec![Run {
                    text: "본문 hello".to_string(),
                    ..Run::default()
                }],
            })],
            ..DocModel::default()
        };
        crate::write_docx(&model)
    }

    fn part_set(bytes: &[u8]) -> std::collections::BTreeMap<String, Vec<u8>> {
        let mut z = ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut m = std::collections::BTreeMap::new();
        for i in 0..z.len() {
            let mut f = z.by_index(i).unwrap();
            let n = f.name().to_string();
            let mut b = Vec::new();
            f.read_to_end(&mut b).unwrap();
            m.insert(n, b);
        }
        m
    }

    #[test]
    fn roundtrip_preserves_every_part_byte_for_byte() {
        let orig = sample_docx();
        let pkg = Package::from_zip(&orig).expect("open");
        let out = pkg.to_zip().expect("save");
        let a = part_set(&orig);
        let b = part_set(&out);
        assert_eq!(
            a.keys().collect::<Vec<_>>(),
            b.keys().collect::<Vec<_>>(),
            "part set changed"
        );
        for (name, abytes) in &a {
            assert_eq!(
                abytes, &b[name],
                "part {name} not byte-stable on no-op round-trip"
            );
        }
    }

    #[test]
    fn parses_content_types_and_rels_and_seeds_allocator() {
        let pkg = Package::from_zip(&sample_docx()).unwrap();
        assert!(pkg.part("word/document.xml").is_some());
        assert!(pkg.part("[Content_Types].xml").is_some());
        // The document part is related to the package root.
        assert!(
            !pkg.rels_for("").is_empty() || !pkg.rels_for("word/document.xml").is_empty(),
            "no rels parsed"
        );
        // Allocator never collides with an existing id.
        let mut pkg = pkg;
        let existing: std::collections::HashSet<String> = pkg
            .rels
            .values()
            .flat_map(|v| v.iter().map(|r| r.id.clone()))
            .collect();
        for _ in 0..5 {
            let id = pkg.alloc_rid();
            assert!(!existing.contains(&id), "alloc collided with {id}");
        }
    }

    #[test]
    fn set_part_reuses_matching_default_and_survives_roundtrip() {
        let mut pkg = Package::from_zip(&sample_docx()).unwrap();
        pkg.set_part("word/custom.xml", b"<x/>".to_vec(), Some("application/xml"));
        let out = pkg.to_zip().unwrap();
        let reopened = Package::from_zip(&out).unwrap();
        assert_eq!(
            reopened.part("word/custom.xml").as_deref(),
            Some(&b"<x/>"[..])
        );
        assert!(reopened.part_has_content_type("word/custom.xml"));
        assert!(
            !reopened
                .part("[Content_Types].xml")
                .map(|b| String::from_utf8_lossy(&b).contains("/word/custom.xml"))
                .unwrap_or(false),
            "matching Default should avoid a redundant Override"
        );
    }

    #[test]
    fn add_related_part_allocates_rid_and_rels() {
        let mut pkg = Package::from_zip(&sample_docx()).unwrap();
        let before = pkg.rels_for("word/document.xml").len();
        let rid = pkg.add_related_part(
            "word/document.xml",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
            "word/media/imageZ.png",
            Some("image/png"),
            vec![1, 2, 3],
        );
        assert!(rid.starts_with("rId"));
        let after = pkg.rels_for("word/document.xml");
        assert_eq!(after.len(), before + 1);
        assert!(after
            .iter()
            .any(|r| r.id == rid && r.target == "media/imageZ.png"));
    }

    #[test]
    fn garbage_bytes_error_not_panic() {
        assert!(Package::from_zip(&[1, 2, 3, 4]).is_err());
        assert!(Package::from_zip(b"PK\x03\x04not a zip").is_err());
    }

    /// Malformed OPC metadata attributes (duplicate names) are rejected,
    /// not silently parsed as a partial graph.
    #[test]
    fn malformed_metadata_attributes_rejected() {
        let dup_ct = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" Extension="xml" ContentType="application/xml"/></Types>"#;
        assert!(parse_content_types(dup_ct.as_bytes()).is_err());
        let dup_rel = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Id="r1" Type="t" Target="x"/></Relationships>"#;
        assert!(parse_rels(dup_rel.as_bytes()).is_err());
        // The well-formed controls still parse (with the mandatory `rels` Default present).
        assert!(parse_content_types(
            br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/></Types>"#
        )
        .is_ok());
        assert!(parse_rels(
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="r1" Type="t" Target="x"/></Relationships>"#
        )
        .is_ok());
    }

    #[test]
    fn metadata_record_elements_must_be_empty() {
        let ct_child = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/xml"><Foreign/></Override></Types>"#;
        assert!(
            parse_content_types(ct_child).is_err(),
            "non-empty Override must make content types read-only"
        );
        let rel_child = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="t" Target="x"><Foreign/></Relationship></Relationships>"#;
        assert!(
            parse_rels(rel_child).is_err(),
            "non-empty Relationship must make rels read-only"
        );
        let ct_decl = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"><?xml version="1.0"?></Default></Types>"#;
        assert!(
            parse_content_types(ct_decl).is_err(),
            "XML declaration inside Default must make content types read-only"
        );
        let rel_decl = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="t" Target="x"><?xml version="1.0"?></Relationship></Relationships>"#;
        assert!(
            parse_rels(rel_decl).is_err(),
            "XML declaration inside Relationship must make rels read-only"
        );
        assert!(parse_content_types(
            br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/xml"/></Types>"#
        )
        .is_ok());
        assert!(parse_rels(
            br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="t" Target="x"/></Relationships>"#
        )
        .is_ok());
    }

    /// CRC-32 (IEEE) for hand-building ZIP fixtures.
    fn zip_crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xEDB8_8320 & (!(crc & 1)).wrapping_add(1));
            }
        }
        !crc
    }

    /// Build a minimal stored-method ZIP from `(name, data)` entries, in order, with
    /// correct CRCs/sizes — including duplicate names, which `ZipWriter` refuses to emit.
    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let (mut local, mut cd, mut offs) = (Vec::new(), Vec::new(), Vec::new());
        for (name, data) in entries {
            offs.push(local.len() as u32);
            let crc = zip_crc32(data);
            local.extend_from_slice(&[0x50, 0x4b, 0x03, 0x04]);
            local.extend_from_slice(&20u16.to_le_bytes());
            local.extend_from_slice(&[0u8; 8]); // flags, method, time, date
            local.extend_from_slice(&crc.to_le_bytes());
            local.extend_from_slice(&(data.len() as u32).to_le_bytes());
            local.extend_from_slice(&(data.len() as u32).to_le_bytes());
            local.extend_from_slice(&(name.len() as u16).to_le_bytes());
            local.extend_from_slice(&0u16.to_le_bytes());
            local.extend_from_slice(name.as_bytes());
            local.extend_from_slice(data);
        }
        let cd_off = local.len() as u32;
        for ((name, data), off) in entries.iter().zip(&offs) {
            cd.extend_from_slice(&[0x50, 0x4b, 0x01, 0x02]);
            cd.extend_from_slice(&20u16.to_le_bytes());
            cd.extend_from_slice(&20u16.to_le_bytes());
            cd.extend_from_slice(&[0u8; 8]); // flags, method, time, date
            cd.extend_from_slice(&zip_crc32(data).to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(data.len() as u32).to_le_bytes());
            cd.extend_from_slice(&(name.len() as u16).to_le_bytes());
            cd.extend_from_slice(&[0u8; 8]); // extra, comment, disk, internal-attrs
            cd.extend_from_slice(&0u32.to_le_bytes()); // external attrs
            cd.extend_from_slice(&off.to_le_bytes());
            cd.extend_from_slice(name.as_bytes());
        }
        let (cd_size, n) = (cd.len() as u32, entries.len() as u16);
        let mut out = local;
        out.extend_from_slice(&cd);
        out.extend_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        out.extend_from_slice(&[0u8; 4]); // disk numbers
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // comment len
        out
    }

    /// Duplicate part names (invalid OPC, not constructible via ZipWriter)
    /// collapse to a single deterministic entry — the one the ZIP reader exposes (the
    /// `zip` crate keeps the last value for a duplicate name) — both on open and re-emit.
    #[test]
    fn duplicate_part_names_collapse_deterministically() {
        let ct = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/></Types>"#;
        let zip = build_zip(&[
            ("[Content_Types].xml", ct),
            ("word/document.xml", b"FIRST"),
            ("word/document.xml", b"SECOND"),
        ]);
        let pkg = Package::from_zip(&zip).unwrap();
        assert!(!pkg.is_meta_lossy());
        // The `zip` crate collapses a duplicate name to its LAST central-directory value;
        // the package keeps exactly that one and re-emits a single matching entry. (Pinned
        // to current `zip` semantics so a future change in dedup behavior is caught.)
        let kept = pkg.part("word/document.xml").unwrap();
        assert_eq!(
            kept, b"SECOND",
            "duplicate name should resolve to the last value"
        );
        let out = pkg.to_zip().unwrap();
        let parts = part_set(&out);
        assert_eq!(
            parts.get("word/document.xml").map(|v| v.as_slice()),
            Some(kept.as_slice()),
            "re-emit must keep exactly the opened occurrence"
        );
        // And it is a SINGLE entry (the duplicate is collapsed, not re-emitted twice).
        let mut z = ZipArchive::new(std::io::Cursor::new(out)).unwrap();
        let count = (0..z.len())
            .filter(|&i| z.by_index(i).unwrap().name() == "word/document.xml")
            .count();
        assert_eq!(count, 1, "duplicate part re-emitted more than once");
    }

    /// Semantically-malformed metadata (conflicting duplicate content-type
    /// records, a `rels` Default with the wrong type, duplicate relationship Ids) is
    /// rejected by the parser (→ `meta_lossy`, read-only) — not treated as editable.
    #[test]
    fn conflicting_or_duplicate_metadata_rejected() {
        const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
        const RL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
        // Conflicting Default (same extension, different content type).
        assert!(parse_content_types(
            format!(r#"<Types xmlns="{CT}"><Default Extension="png" ContentType="image/png"/><Default Extension="png" ContentType="image/jpeg"/></Types>"#).as_bytes()
        ).is_err());
        // Conflicting Override (same part, different content type).
        assert!(parse_content_types(
            format!(r#"<Types xmlns="{CT}"><Override PartName="/a.xml" ContentType="application/xml"/><Override PartName="/a.xml" ContentType="text/xml"/></Types>"#).as_bytes()
        ).is_err());
        // `rels` Default with the wrong content type.
        assert!(parse_content_types(
            format!(r#"<Types xmlns="{CT}"><Default Extension="rels" ContentType="text/plain"/></Types>"#).as_bytes()
        ).is_err());
        // Duplicate relationship Id.
        assert!(parse_rels(
            format!(r#"<Relationships xmlns="{RL}"><Relationship Id="rId1" Type="t" Target="a"/><Relationship Id="rId1" Type="t" Target="b"/></Relationships>"#).as_bytes()
        ).is_err());
        // Identical-duplicate Default (same ext AND type) is harmless — still parses
        // (with the mandatory `rels` Default present).
        assert!(parse_content_types(
            format!(r#"<Types xmlns="{CT}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="png" ContentType="image/png"/><Default Extension="png" ContentType="image/png"/></Types>"#).as_bytes()
        ).is_ok());
    }

    #[test]
    fn content_type_part_and_extension_identity_is_ascii_case_insensitive() {
        const MAIN: &str =
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
        let ct = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><Override PartName="/word/Document.xml" ContentType="{MAIN}"/></Types>"#
        );
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/document.xml", br#"<w:document/>"#),
            ("word/media/Image1.png", b"png"),
        ]);

        let mut pkg = Package::from_zip(&zip).unwrap();
        pkg.ensure_content_type("word/document.xml", MAIN);
        let out = String::from_utf8(pkg.part(CONTENT_TYPES).unwrap()).unwrap();
        assert_eq!(
            out.to_ascii_lowercase()
                .matches(r#"partname="/word/document.xml""#)
                .count(),
            1,
            "case-variant Override was duplicated: {out}"
        );
        assert!(
            pkg.has_part("word/media/image1.png"),
            "case-variant media part should collide"
        );

        let mut pkg = Package::from_zip(&zip).unwrap();
        pkg.set_part(
            "word/document.xml",
            b"<w:document/>".to_vec(),
            Some("application/xml"),
        );
        let out = String::from_utf8(pkg.part(CONTENT_TYPES).unwrap()).unwrap();
        assert_eq!(
            out.to_ascii_lowercase()
                .matches(r#"partname="/word/document.xml""#)
                .count(),
            1,
            "set_part duplicated a case-variant Override: {out}"
        );
        assert!(
            out.contains(r#"ContentType="application/xml""#),
            "set_part did not update the existing Override: {out}"
        );

        let conflicting = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><Override PartName="/word/Document.xml" ContentType="{MAIN}"/><Override PartName="/word/document.xml" ContentType="application/xml"/></Types>"#
        );
        assert!(
            parse_content_types(conflicting.as_bytes()).is_err(),
            "case-variant conflicting Overrides must be read-only metadata"
        );
    }

    #[test]
    fn identical_default_extensions_dedup_case_insensitively() {
        let ct = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><Default Extension="PNG" ContentType="image/png"/><Default Extension="png" ContentType="image/png"/></Types>"#
        );

        let parsed = parse_content_types(ct.as_bytes()).unwrap();

        assert_eq!(
            parsed
                .defaults
                .iter()
                .filter(|d| d.extension.eq_ignore_ascii_case("png") && d.content_type == "image/png")
                .count(),
            1,
            "case-variant identical Defaults must collapse"
        );
    }

    #[test]
    fn metadata_records_must_be_direct_children() {
        let nested_ct = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><wrap><Override PartName="/word/document.xml" ContentType="application/xml"/></wrap></Types>"#
        );
        assert!(
            parse_content_types(nested_ct.as_bytes()).is_err(),
            "nested Override must not be folded as a real content type"
        );

        let nested_rel = format!(
            r#"<Relationships xmlns="{REL_NS}"><wrap><Relationship Id="rId1" Type="t" Target="x"/></wrap></Relationships>"#
        );
        assert!(
            parse_rels(nested_rel.as_bytes()).is_err(),
            "nested Relationship must not be folded as a real relationship"
        );

        let foreign = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><x:wrap xmlns:x="urn:x"><x:Future Value="ok"/></x:wrap></Types>"#
        );
        assert!(
            parse_content_types(foreign.as_bytes()).is_ok(),
            "foreign non-record extension elements remain forward-compatible"
        );
    }

    #[test]
    fn metadata_attribute_cap_uses_test_budget() {
        set_test_max_meta(2);
        let ct = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}" Extra="x"/></Types>"#
        );
        let res = parse_content_types(ct.as_bytes());
        set_test_max_meta(MAX_META_RECORDS);

        assert!(res.is_err(), "metadata attributes ignored the lowered cap");
    }

    #[test]
    fn to_zip_rechecks_regenerated_metadata_record_cap() {
        let ct = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/xml"/></Types>"#
        );
        set_test_max_meta(3);
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
        ]);
        let mut pkg = Package::from_zip(&zip).unwrap();
        pkg.set_part("word/media/image1.png", b"png".to_vec(), Some("image/png"));
        let ct_res = pkg.to_zip();
        set_test_max_meta(MAX_META_RECORDS);
        assert!(
            ct_res.is_err(),
            "save emitted [Content_Types].xml past the reopen cap"
        );

        let mut rels = format!(r#"<Relationships xmlns="{REL_NS}">"#);
        for i in 0..10 {
            rels.push_str(&format!(
                r#"<Relationship Id="rId{i}" Type="t" Target="x{i}"/>"#
            ));
        }
        rels.push_str("</Relationships>");
        let ct = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/xml"/></Types>"#
        );
        set_test_max_meta(10);
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
            ("word/_rels/document.xml.rels", rels.as_bytes()),
        ]);
        let mut pkg = Package::from_zip(&zip).unwrap();
        pkg.add_related_part(
            "word/document.xml",
            "urn:image",
            "word/media/image1.png",
            None,
            b"png".to_vec(),
        );
        let rels_res = pkg.to_zip();
        set_test_max_meta(MAX_META_RECORDS);
        assert!(rels_res.is_err(), "save emitted .rels past the reopen cap");
    }

    #[test]
    fn to_zip_rejects_touched_internal_relationship_to_missing_part() {
        let ct = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/xml"/></Types>"#
        );
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
        ]);
        let mut pkg = Package::from_zip(&zip).unwrap();

        pkg.ensure_relationship("word/document.xml", "urn:missing", "word/missing.xml");

        let save = pkg.to_zip();
        assert!(
            save.is_err(),
            "save emitted a touched .rels part targeting a missing package part"
        );
    }

    #[test]
    fn to_zip_rejects_touched_root_relationship_to_missing_part() {
        let ct = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/xml"/></Types>"#
        );
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
        ]);
        let mut pkg = Package::from_zip(&zip).unwrap();

        pkg.ensure_relationship("", "urn:missing-root", "docProps/missing.xml");

        let save = pkg.to_zip();
        assert!(
            save.is_err(),
            "save emitted a touched root .rels part targeting a missing package part"
        );
    }

    /// `[Content_Types].xml` missing the mandatory `rels` Default is injected
    /// into the in-memory view (not refused — real `.docx` files omit it), AND an edit that
    /// writes a new `.rels` part regenerates `[Content_Types].xml` so the injected default
    /// is serialized. The trap this guards: a *stale image Override* means `set_part` skips
    /// regenerating content types for the media part, so without the `regen_rels` hook the
    /// new `.rels` would be emitted untyped (no `Default Extension="rels"` on disk).
    #[test]
    fn missing_rels_default_typed_via_regen_on_edit() {
        const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
        // No `Default Extension="rels"`, but an `xml` Default and a stale image Override.
        let ct = format!(
            r#"<Types xmlns="{CT}"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/media/image1.png" ContentType="image/png"/></Types>"#
        );
        let zip = build_zip(&[
            ("[Content_Types].xml", ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
        ]);
        let mut pkg = Package::from_zip(&zip).expect("missing rels Default is injected, not fatal");
        // Editable (not meta_lossy): the missing default is injected + tracked, not refused.
        assert!(!pkg.is_meta_lossy());
        // An edit that writes a NEW `.rels` part (the source had none), while the stale
        // image Override means the media part itself is already typed (the trap).
        pkg.add_related_part(
            "word/document.xml",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image",
            "word/media/image1.png",
            Some("image/png"),
            b"PNGDATA".to_vec(),
        );
        let out = pkg.to_zip().unwrap();
        let reopened = Package::from_zip(&out).unwrap();
        let ct_out = reopened.part(CONTENT_TYPES).unwrap();
        assert!(
            String::from_utf8_lossy(&ct_out).contains(r#"Extension="rels""#),
            "edit writing a .rels must regenerate [Content_Types].xml with the rels Default"
        );
        // The new `.rels` part exists and now resolves to the relationships content type.
        assert!(reopened.part("word/_rels/document.xml.rels").is_some());
        assert!(reopened.part_has_content_type("word/_rels/document.xml.rels"));
    }

    /// OPC metadata SHAPE is validated, not just namespaced children. A
    /// wrong-namespace (here: unnamespaced) `[Content_Types].xml` reads as an empty graph
    /// under the namespace-strict parser; it must be `meta_lossy` (read-only) so an edit
    /// can't regenerate it and silently drop the real styles/numbering/etc. types — and a
    /// no-op save must preserve the original raw bytes verbatim.
    #[test]
    fn wrong_namespace_metadata_is_read_only_not_regenerated() {
        let ct = r#"<Types><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/><Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/></Types>"#;
        let zip = build_zip(&[
            ("[Content_Types].xml", ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
        ]);
        let pkg = Package::from_zip(&zip).expect("malformed metadata must still open (lenient)");
        assert!(
            pkg.is_meta_lossy(),
            "wrong-namespace [Content_Types].xml must be read-only, not an editable empty graph"
        );
        let out = pkg.to_zip().unwrap();
        let reopened_ct = Package::from_zip(&out)
            .unwrap()
            .part(CONTENT_TYPES)
            .unwrap();
        assert_eq!(
            reopened_ct,
            ct.as_bytes(),
            "no-op save must preserve the raw [Content_Types].xml, never regenerate it"
        );
    }

    /// Identical-duplicate `<Override>` records (accepted as non-conflicting)
    /// must not strand a stale duplicate when an edit repairs the part's type. They are
    /// deduplicated at parse, so a content-type repair rewrites the single record and the
    /// saved package reopens cleanly (no conflicting duplicates). FAILS without the dedup.
    #[test]
    fn identical_duplicate_overrides_deduped_so_edit_does_not_strand_one() {
        const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
        const RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";
        const WML_MAIN: &str =
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
        // Two byte-identical stale overrides for the same part.
        let ct = format!(
            r#"<Types xmlns="{CT}"><Default Extension="rels" ContentType="{RELS}"/><Override PartName="/word/document.xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/xml"/></Types>"#
        );
        let zip = build_zip(&[
            ("[Content_Types].xml", ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
        ]);
        let mut pkg = Package::from_zip(&zip).expect("identical duplicates are not a conflict");
        assert!(!pkg.is_meta_lossy());
        // The edit's content-type repair (as `replace_body_text`/`add_image_png` perform).
        pkg.ensure_content_type("word/document.xml", WML_MAIN);
        let out = pkg.to_zip().unwrap();
        // Must reopen WITHOUT tripping the conflicting-duplicate check.
        let reopened =
            Package::from_zip(&out).expect("saved package must reopen (no conflicting overrides)");
        assert!(
            !reopened.is_meta_lossy(),
            "stranded a duplicate override → saved [Content_Types].xml is self-conflicting"
        );
        let ct_out = String::from_utf8_lossy(&reopened.part(CONTENT_TYPES).unwrap()).into_owned();
        assert_eq!(
            ct_out.matches(r#"PartName="/word/document.xml""#).count(),
            1,
            "exactly one override should remain for the part"
        );
        assert!(
            ct_out.contains(WML_MAIN),
            "the override must carry the repaired type"
        );
    }

    /// The metadata parsers reject every malformed *shape* — wrong/absent
    /// root namespace, wrong root element name, a correct-namespace record missing required
    /// attributes, and no root at all — while still accepting a correct (even empty) root.
    #[test]
    fn malformed_opc_metadata_shape_rejected() {
        const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
        const RL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
        // Wrong/absent root namespace.
        assert!(parse_content_types(
            br#"<Types><Default Extension="rels" ContentType="x"/></Types>"#
        )
        .is_err());
        assert!(parse_rels(
            br#"<Relationships><Relationship Id="r" Type="t" Target="x"/></Relationships>"#
        )
        .is_err());
        // Right namespace, wrong root element name.
        assert!(parse_content_types(format!(r#"<NotTypes xmlns="{CT}"/>"#).as_bytes()).is_err());
        assert!(parse_rels(format!(r#"<NotRels xmlns="{RL}"/>"#).as_bytes()).is_err());
        // Correct-namespace record missing a required attribute.
        assert!(parse_content_types(
            format!(r#"<Types xmlns="{CT}"><Default Extension="rels"/></Types>"#).as_bytes()
        )
        .is_err());
        assert!(parse_content_types(
            format!(
                r#"<Types xmlns="{CT}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/a"/></Types>"#
            )
            .as_bytes()
        )
        .is_err());
        // Correct-namespace Override with malformed PartName values. These must be
        // read-only metadata, not editable partial graphs.
        for bad_part_name in [
            "",
            "word/document.xml",
            "/",
            "/word//document.xml",
            "/word/../document.xml",
        ] {
            assert!(
                parse_content_types(
                    format!(
                        r#"<Types xmlns="{CT}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="{bad_part_name}" ContentType="application/xml"/></Types>"#
                    )
                    .as_bytes()
                )
                .is_err(),
                "malformed Override PartName {bad_part_name:?} must be rejected"
            );
        }
        assert!(parse_rels(
            format!(
                r#"<Relationships xmlns="{RL}"><Relationship Id="r" Type="t"/></Relationships>"#
            )
            .as_bytes()
        )
        .is_err());
        // No root element at all.
        assert!(parse_content_types(br#"<?xml version="1.0"?>"#).is_err());
        assert!(parse_rels(br#"<?xml version="1.0"?>"#).is_err());
        // More than one root element.
        assert!(parse_content_types(
            format!(r#"<Types xmlns="{CT}"/><Types xmlns="{CT}"/>"#).as_bytes()
        )
        .is_err());
        assert!(parse_rels(
            format!(r#"<Relationships xmlns="{RL}"/><Relationships xmlns="{RL}"/>"#).as_bytes()
        )
        .is_err());
        // Non-whitespace character data / CDATA outside the root, and doctypes, make the
        // metadata lossy to regenerate. They must be rejected rather than parsed as editable
        // metadata whose regeneration silently drops the extra content.
        for ct in [
            format!(r#"junk<Types xmlns="{CT}"/>"#),
            format!(r#"<Types xmlns="{CT}"/>junk"#),
            format!(r#"<![CDATA[junk]]><Types xmlns="{CT}"/>"#),
            format!(r#"<!DOCTYPE Types><Types xmlns="{CT}"/>"#),
        ] {
            assert!(
                parse_content_types(ct.as_bytes()).is_err(),
                "malformed top-level content type metadata was accepted: {ct}"
            );
        }
        for rels in [
            format!(r#"junk<Relationships xmlns="{RL}"/>"#),
            format!(r#"<Relationships xmlns="{RL}"/>junk"#),
            format!(r#"<![CDATA[junk]]><Relationships xmlns="{RL}"/>"#),
            format!(r#"<!DOCTYPE Relationships><Relationships xmlns="{RL}"/>"#),
        ] {
            assert!(
                parse_rels(rels.as_bytes()).is_err(),
                "malformed top-level relationship metadata was accepted: {rels}"
            );
        }
        assert!(
            parse_content_types(
                format!(
                    "\n<Types xmlns=\"{CT}\"><Default Extension=\"rels\" ContentType=\"{CT_RELS}\"/></Types>\n"
                )
                .as_bytes()
            )
            .is_ok(),
            "top-level whitespace around metadata root must remain accepted"
        );
        assert!(
            parse_rels(format!("\n<Relationships xmlns=\"{RL}\"/>\n").as_bytes()).is_ok(),
            "top-level whitespace around rels root must remain accepted"
        );
        // A correct-namespace record name in NO namespace (xmlns="") is rejected, not dropped.
        assert!(parse_content_types(
            format!(r#"<Types xmlns="{CT}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override xmlns="" PartName="/a.xml" ContentType="application/xml"/></Types>"#).as_bytes()
        )
        .is_err());
        assert!(parse_rels(
            format!(r#"<Relationships xmlns="{RL}"><Relationship xmlns="" Id="r" Type="t" Target="x"/></Relationships>"#).as_bytes()
        )
        .is_err());
        // An invalid `TargetMode` (closed OPC enum) is rejected, not coerced to internal and
        // then regenerated without TargetMode — `Internal`/`External`/absent are accepted.
        assert!(parse_rels(
            format!(r#"<Relationships xmlns="{RL}"><Relationship Id="r" Type="t" Target="http://x" TargetMode="external"/></Relationships>"#).as_bytes()
        )
        .is_err());
        assert!(parse_rels(
            format!(r#"<Relationships xmlns="{RL}"><Relationship Id="r" Type="t" Target="http://x" TargetMode="External"/></Relationships>"#).as_bytes()
        )
        .is_ok());
        assert!(parse_rels(
            format!(r#"<Relationships xmlns="{RL}"><Relationship Id="r" Type="t" Target="x" TargetMode="Internal"/></Relationships>"#).as_bytes()
        )
        .is_ok());
        // A correct, even self-closing/empty, root is accepted.
        assert!(parse_content_types(
            format!(r#"<Types xmlns="{CT}"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/></Types>"#).as_bytes()
        )
        .is_ok());
        assert!(parse_rels(format!(r#"<Relationships xmlns="{RL}"/>"#).as_bytes()).is_ok());
    }

    #[test]
    fn malformed_content_type_values_are_read_only() {
        const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
        const RELS_DEFAULT: &str = r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#;

        for bad in [
            "",
            "  ",
            "garbage",
            "application/",
            "/xml",
            "application/x/y",
        ] {
            assert!(
                parse_content_types(
                    format!(
                        r#"<Types xmlns="{CT}">{RELS_DEFAULT}<Override PartName="/word/document.xml" ContentType="{bad}"/></Types>"#
                    )
                    .as_bytes()
                )
                .is_err(),
                "malformed Override ContentType {bad:?} must be rejected"
            );
            assert!(
                parse_content_types(
                    format!(
                        r#"<Types xmlns="{CT}"><Default Extension="xml" ContentType="{bad}"/>{RELS_DEFAULT}</Types>"#
                    )
                    .as_bytes()
                )
                .is_err(),
                "malformed Default ContentType {bad:?} must be rejected"
            );
        }

        assert!(
            parse_content_types(
                format!(
                    r#"<Types xmlns="{CT}">{RELS_DEFAULT}<Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml; charset=UTF-8"/></Types>"#
                )
                .as_bytes()
            )
            .is_ok(),
            "valid +xml media type with parameters must stay accepted"
        );

        let ct = format!(
            r#"<Types xmlns="{CT}">{RELS_DEFAULT}<Override PartName="/word/document.xml" ContentType="application/xml"/><Override PartName="/word/styles.xml" ContentType=""/></Types>"#
        );
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
            ("word/styles.xml", b"<w:styles/>"),
        ]);
        let pkg = Package::from_zip(&zip).expect("malformed metadata still opens for passthrough");
        assert!(
            pkg.is_meta_lossy(),
            "empty content type must make package read-only"
        );
        assert_eq!(
            pkg.part(CONTENT_TYPES).as_deref(),
            Some(ct.as_bytes()),
            "no-op save must preserve malformed [Content_Types].xml"
        );
    }

    #[test]
    fn malformed_default_extensions_are_read_only() {
        const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
        const RELS_DEFAULT: &str = r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#;

        for bad in ["", "x/y", r"x\y", "x.y", "x y", "x\t"] {
            assert!(
                parse_content_types(
                    format!(
                        r#"<Types xmlns="{CT}"><Default Extension="{bad}" ContentType="application/xml"/>{RELS_DEFAULT}</Types>"#
                    )
                    .as_bytes()
                )
                .is_err(),
                "malformed Default Extension {bad:?} must be rejected"
            );
        }

        let ct = format!(
            r#"<Types xmlns="{CT}"><Default Extension="x/y" ContentType="application/xml"/>{RELS_DEFAULT}</Types>"#
        );
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
        ]);
        let pkg = Package::from_zip(&zip).expect("malformed metadata still opens for passthrough");
        assert!(
            pkg.is_meta_lossy(),
            "invalid Default Extension must make package read-only"
        );
        assert_eq!(pkg.part(CONTENT_TYPES).as_deref(), Some(ct.as_bytes()));
    }

    #[test]
    fn malformed_override_part_name_edges_are_read_only() {
        const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
        const RELS_DEFAULT: &str = r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#;

        for bad_part_name in [
            "/[Content_Types].xml",
            "/[content_types].xml",
            "/word/a%zz.xml",
            "/word/a%.xml",
            "/word/a%2.xml",
        ] {
            assert!(
                parse_content_types(
                    format!(
                        r#"<Types xmlns="{CT}">{RELS_DEFAULT}<Override PartName="{bad_part_name}" ContentType="application/xml"/></Types>"#
                    )
                    .as_bytes()
                )
                .is_err(),
                "malformed Override PartName {bad_part_name:?} must be rejected"
            );
        }

        assert!(
            parse_content_types(
                format!(
                    r#"<Types xmlns="{CT}">{RELS_DEFAULT}<Override PartName="/word/a%20b.xml" ContentType="application/xml"/></Types>"#
                )
                .as_bytes()
            )
            .is_ok(),
            "well-formed percent escapes must stay accepted"
        );
    }

    #[test]
    fn malformed_relationship_values_are_rejected() {
        const RL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

        for bad_id in ["", "1abc", "r d", "r:id"] {
            assert!(
                parse_rels(
                    format!(
                        r#"<Relationships xmlns="{RL}"><Relationship Id="{bad_id}" Type="urn:t" Target="x.xml"/></Relationships>"#
                    )
                    .as_bytes()
                )
                .is_err(),
                "malformed Relationship Id {bad_id:?} must be rejected"
            );
        }
        assert!(
            parse_rels(
                format!(
                    r#"<Relationships xmlns="{RL}"><Relationship Id="rId007" Type="urn:t" Target="x.xml"/></Relationships>"#
                )
                .as_bytes()
            )
            .is_ok(),
            "valid NCName ids with leading-zero suffixes must stay accepted"
        );

        for rel in [
            r#"<Relationship Id="rId1" Type="" Target="x.xml"/>"#,
            r#"<Relationship Id="rId1" Type="  " Target="x.xml"/>"#,
            r#"<Relationship Id="rId1" Type="urn:t" Target=""/>"#,
            r#"<Relationship Id="rId1" Type="urn:t" Target="" TargetMode="External"/>"#,
        ] {
            assert!(
                parse_rels(
                    format!(r#"<Relationships xmlns="{RL}">{rel}</Relationships>"#).as_bytes()
                )
                .is_err(),
                "empty Relationship Type/Target must be rejected: {rel}"
            );
        }
    }

    #[test]
    fn illegal_xml_controls_in_metadata_values_are_rejected() {
        const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
        const RL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";

        assert!(
            parse_content_types(
                format!(
                    r#"<Types xmlns="{CT}"><Default Extension="rels" ContentType="{CT_RELS}"/><Override PartName="/word/styles.xml" ContentType="application/xml&#1;"/></Types>"#
                )
                .as_bytes()
            )
            .is_err(),
            "character references to XML-illegal controls must not become editable metadata"
        );
        assert!(
            parse_rels(
                format!(
                    r#"<Relationships xmlns="{RL}"><Relationship Id="rId1" Type="urn:t" Target="x&#1;.xml"/></Relationships>"#
                )
                .as_bytes()
            )
            .is_err(),
            "Relationship attrs with XML-illegal controls must be rejected"
        );
        assert!(
            parse_content_types(
                format!(
                    r#"<Types xmlns="{CT}"><Default Extension="x{}" ContentType="application/xml"/><Default Extension="rels" ContentType="{CT_RELS}"/></Types>"#,
                    '\u{FFFF}'
                )
                .as_bytes()
            )
            .is_err(),
            "raw XML-forbidden scalars in metadata attrs must be rejected"
        );
        assert!(
            parse_rels(
                format!(
                    r#"<Relationships xmlns="{RL}"><Relationship Id="rId1" Type="urn:t" Target="x{}.xml"/></Relationships>"#,
                    '\u{FFFF}'
                )
                .as_bytes()
            )
            .is_err(),
            "raw XML-forbidden scalars in relationship attrs must be rejected"
        );
    }

    #[test]
    fn case_colliding_zip_part_names_are_read_only() {
        let ct = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/xml"/></Types>"#;
        let zip = build_zip(&[
            (CONTENT_TYPES, ct),
            ("word/document.xml", b"A"),
            ("word/Document.xml", b"B"),
        ]);

        let pkg = Package::from_zip(&zip).expect("case-colliding package still opens");

        assert!(
            pkg.is_meta_lossy(),
            "case-colliding part identities must make package read-only"
        );
        let out = pkg.to_zip().unwrap();
        let parts = part_set(&out);
        assert_eq!(
            parts.get("word/document.xml").map(Vec::as_slice),
            Some(&b"A"[..])
        );
        assert_eq!(
            parts.get("word/Document.xml").map(Vec::as_slice),
            Some(&b"B"[..])
        );
    }

    fn assert_meta_lossy_noop_preserves(zip: &[u8], expected: &[(&str, &[u8])]) {
        let pkg = Package::from_zip(zip).expect("lossy metadata package still opens");
        assert!(
            pkg.is_meta_lossy(),
            "foreign/unmodeled metadata attributes must make the package read-only"
        );
        let saved = pkg.to_zip().expect("no-op save should still be allowed");
        let parts = part_set(&saved);
        for (name, bytes) in expected {
            assert_eq!(
                parts.get(*name).map(Vec::as_slice),
                Some(*bytes),
                "no-op save changed raw bytes for {name}"
            );
        }
    }

    #[test]
    fn foreign_metadata_record_attributes_make_package_read_only() {
        let ct = format!(
            r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><Default Extension="xml" ContentType="application/xml" data-keep="ct"/><Override PartName="/word/document.xml" ContentType="application/xml" data-keep="override"/></Types>"#
        );
        let rels = format!(
            r#"<Relationships xmlns="{REL_NS}"><Relationship Id="rId1" Type="urn:styles" Target="styles.xml" data-keep="rel"/></Relationships>"#
        );
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
            ("word/_rels/document.xml.rels", rels.as_bytes()),
        ]);

        assert_meta_lossy_noop_preserves(
            &zip,
            &[
                (CONTENT_TYPES, ct.as_bytes()),
                ("word/_rels/document.xml.rels", rels.as_bytes()),
            ],
        );
    }

    #[test]
    fn foreign_metadata_root_attributes_make_package_read_only() {
        let ct = format!(
            r#"<Types xmlns="{CT_NS}" data-root="ct"><Default Extension="rels" ContentType="{CT_RELS}"/><Default Extension="xml" ContentType="application/xml"/></Types>"#
        );
        let rels = format!(
            r#"<Relationships xmlns="{REL_NS}" xmlns:x="urn:x"><Relationship Id="rId1" Type="urn:styles" Target="styles.xml"/></Relationships>"#
        );
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
            ("word/_rels/document.xml.rels", rels.as_bytes()),
        ]);

        assert_meta_lossy_noop_preserves(
            &zip,
            &[
                (CONTENT_TYPES, ct.as_bytes()),
                ("word/_rels/document.xml.rels", rels.as_bytes()),
            ],
        );
    }

    #[test]
    fn invalid_metadata_attribute_names_make_package_read_only() {
        for bad_attr in ["a/b", "1bad", ".x"] {
            let ct = format!(
                r#"<Types xmlns="{CT_NS}"><Default Extension="rels" ContentType="{CT_RELS}"/><Default Extension="xml" ContentType="application/xml" {bad_attr}="ct"/></Types>"#
            );
            let zip = build_zip(&[
                (CONTENT_TYPES, ct.as_bytes()),
                ("word/document.xml", b"<w:document/>"),
            ]);

            assert_meta_lossy_noop_preserves(&zip, &[(CONTENT_TYPES, ct.as_bytes())]);
        }
    }

    #[test]
    fn malformed_override_part_name_makes_package_read_only() {
        const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
        let ct = format!(
            r#"<Types xmlns="{CT}"><Default Extension="rels" ContentType="{CT_RELS}"/><Override PartName="word/document.xml" ContentType="application/xml"/></Types>"#
        );
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
        ]);

        let pkg = Package::from_zip(&zip).expect("malformed metadata still opens for passthrough");

        assert!(
            pkg.is_meta_lossy(),
            "bad Override PartName must make metadata read-only"
        );
        assert_eq!(
            pkg.part(CONTENT_TYPES).as_deref(),
            Some(ct.as_bytes()),
            "no-op save path must preserve malformed metadata bytes"
        );
    }

    /// A package with up to the entry cap opens AND no-op-saves (to_zip must
    /// count emitted entries, not the overlapping `order` + `parts` which would double it);
    /// cap+1 is rejected on open.
    #[test]
    fn entry_count_open_save_no_double_count() {
        let ct = br#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/></Types>"#;
        set_test_max_entries(3);
        let zip3 = build_zip(&[
            ("[Content_Types].xml", ct),
            ("a.xml", b"A"),
            ("b.xml", b"B"),
        ]);
        let pkg = Package::from_zip(&zip3).expect("at-cap package should open");
        assert!(
            pkg.to_zip().is_ok(),
            "no-op save must not double-count entries"
        );
        let zip4 = build_zip(&[
            ("[Content_Types].xml", ct),
            ("a.xml", b"A"),
            ("b.xml", b"B"),
            ("c.xml", b"C"),
        ]);
        let over = Package::from_zip(&zip4);
        reset_test_max_entries();
        assert!(over.is_err(), "over-cap package must be rejected on open");
    }

    /// OPC metadata is namespace-aware — a foreign-namespace
    /// `<x:Override>`/`<x:Relationship>` is NOT treated as real metadata (else a foreign
    /// override could fool `ensure_content_type` into leaving a part mistyped).
    #[test]
    fn foreign_namespace_opc_records_are_rejected() {
        const CT: &str = "http://schemas.openxmlformats.org/package/2006/content-types";
        const RL: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
        const RELS_DEFAULT: &str = r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>"#;
        // A foreign-namespace `<x:Override>` has the OPC *local* name but a non-OPC namespace.
        // It must NOT be silently dropped (which would leave the media part untyped and let a
        // later edit regenerate `[Content_Types].xml` without it) — it is rejected, so the
        // package is `meta_lossy` (read-only). Round-20's security intent (a foreign override
        // never fools `ensure_content_type`) holds the same way: a read-only package is never edited.
        assert!(parse_content_types(
            format!(r#"<Types xmlns="{CT}">{RELS_DEFAULT}<x:Override xmlns:x="urn:foo" PartName="/word/media/image1.png" ContentType="image/png"/></Types>"#).as_bytes()
        ).is_err());
        // Correct-namespace Override: accepted (and resolves).
        let ct_ok = parse_content_types(
            format!(r#"<Types xmlns="{CT}">{RELS_DEFAULT}<Override PartName="/word/media/image1.png" ContentType="image/png"/></Types>"#).as_bytes()
        ).unwrap();
        assert!(ct_ok.resolves("word/media/image1.png"));
        // A genuinely foreign element (a DIFFERENT local name) is still ignored — real OPC
        // forward-compatibility is preserved; only an OPC-record local name in the wrong
        // namespace is rejected.
        assert!(parse_content_types(
            format!(r#"<Types xmlns="{CT}">{RELS_DEFAULT}<x:Whatever xmlns:x="urn:foo" foo="bar"/></Types>"#).as_bytes()
        ).is_ok());
        // Foreign-namespace Relationship: rejected; correct-namespace: parsed.
        assert!(parse_rels(
            format!(r#"<Relationships xmlns="{RL}"><x:Relationship xmlns:x="urn:foo" Id="rIdX" Type="t" Target="x"/></Relationships>"#).as_bytes()
        ).is_err());
        let rels_ok = parse_rels(
            format!(r#"<Relationships xmlns="{RL}"><Relationship Id="rId1" Type="t" Target="x"/></Relationships>"#).as_bytes()
        ).unwrap();
        assert_eq!(rels_ok.len(), 1);
    }

    /// A ZIP64 EOCD declaring a huge entry count is rejected by the cheap
    /// preflight before `ZipArchive::new` allocates per-entry state.
    #[test]
    fn eocd_preflight_rejects_huge_zip64_entry_count() {
        // A real archive's count reads back small.
        let real = eocd_entry_count(&sample_docx()).unwrap();
        assert!(real > 0 && real <= MAX_ENTRIES as u64);

        // Minimal forged structure: ZIP64 EOCD (huge total) + locator + EOCD (0xFFFF).
        let mut b = vec![0u8; 56]; // ZIP64 EOCD at offset 0
        b[0..4].copy_from_slice(&[0x50, 0x4b, 0x06, 0x06]);
        b[32..40].copy_from_slice(&u64::MAX.to_le_bytes()); // total entries
        let mut loc = vec![0u8; 20];
        loc[0..4].copy_from_slice(&[0x50, 0x4b, 0x06, 0x07]);
        loc[8..16].copy_from_slice(&0u64.to_le_bytes()); // ZIP64 EOCD at offset 0
        b.extend_from_slice(&loc);
        let mut eocd = vec![0u8; 22];
        eocd[0..4].copy_from_slice(&[0x50, 0x4b, 0x05, 0x06]);
        eocd[10..12].copy_from_slice(&0xFFFFu16.to_le_bytes());
        b.extend_from_slice(&eocd);

        assert_eq!(eocd_entry_count(&b), Some(u64::MAX));
        assert!(Package::from_zip(&b).is_err());
    }

    /// OPC metadata that is malformed beyond duplicate attrs — an unclosed
    /// root or a bad entity reference in a value — is also rejected (not parsed partially).
    #[test]
    fn malformed_metadata_unclosed_or_bad_entity_rejected() {
        const CT_NS_DECL: &str =
            r#"xmlns="http://schemas.openxmlformats.org/package/2006/content-types""#;
        const REL_NS_DECL: &str =
            r#"xmlns="http://schemas.openxmlformats.org/package/2006/relationships""#;
        // Unclosed root element.
        assert!(parse_content_types(format!("<Types {CT_NS_DECL}>").as_bytes()).is_err());
        assert!(parse_rels(format!("<Relationships {REL_NS_DECL}>").as_bytes()).is_err());
        // Bad entity reference in an attribute value.
        assert!(parse_content_types(
            format!(
                r#"<Types {CT_NS_DECL}><Override PartName="/a" ContentType="x&bogus;"/></Types>"#
            )
            .as_bytes()
        )
        .is_err());
        assert!(parse_rels(
            format!(r#"<Relationships {REL_NS_DECL}><Relationship Id="r" Type="t" Target="x&bogus;"/></Relationships>"#).as_bytes()
        )
        .is_err());
    }

    /// A fully-readable package is `complete`; one missing an entry is not
    /// (and `Document::save` refuses the latter — wired via `is_complete`).
    #[test]
    fn completeness_tracks_retained_parts() {
        let pkg = Package::from_zip(&sample_docx()).unwrap();
        assert!(pkg.is_complete(), "clean package should be complete");
        // Simulate an open that had to drop an unreadable entry.
        let mut incomplete = pkg;
        incomplete.complete = false;
        assert!(!incomplete.is_complete());
    }

    #[test]
    fn namespace_prefixed_opc_roots_make_package_read_only() {
        let ct = format!(
            r#"<ct:Types xmlns:ct="{CT_NS}"><ct:Default Extension="rels" ContentType="{CT_RELS}"/><ct:Override PartName="/word/document.xml" ContentType="application/xml"/></ct:Types>"#
        );
        let rels = format!(
            r#"<pr:Relationships xmlns:pr="{REL_NS}"><pr:Relationship Id="rId7" Type="urn:styles" Target="styles.xml"/></pr:Relationships>"#
        );
        let zip = build_zip(&[
            (CONTENT_TYPES, ct.as_bytes()),
            ("word/_rels/document.xml.rels", rels.as_bytes()),
            ("word/document.xml", b"<w:document/>"),
        ]);

        assert_meta_lossy_noop_preserves(
            &zip,
            &[
                (CONTENT_TYPES, ct.as_bytes()),
                ("word/_rels/document.xml.rels", rels.as_bytes()),
            ],
        );
    }

    // Note: a duplicate-part-name `.docx` (invalid OPC, e.g. the corpus'
    // `unicode-path.docx`) is normalized first-occurrence-wins by `from_zip`. It
    // can't be unit-constructed because `ZipWriter::start_file` itself dedups names;
    // the behavior is covered by the corpus passthrough validation.
}
