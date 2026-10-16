//! `.docx` tracked-change marker parsing.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::{HashMap, HashSet};

use crate::annotation::{
    legacy_form_field_syntax, FieldKind, Revision, RevisionKind, RevisionView,
};
use crate::text;

use super::fields::{
    computed_contextless_result, computed_run_symbol_char, is_section_field_instruction,
    is_style_ref_field_instruction, ContextlessFieldState, FieldDocumentProperties,
    FieldResolutionContext, LegacyFormContext, NoteRefContext, SectionContext, StyleRefContext,
    TocEntry,
};
use super::xml_text::{
    inline_marker_text, read_text, skip_alternate_content_branch, skip_subtree,
    AlternateContentBranchState,
};
use super::{attr_local_trimmed, field_char_type, local};

type Xml<'a> = Reader<&'a [u8]>;

pub(crate) fn parse(xml: &str, ctx: &FieldResolutionContext<'_>) -> Vec<Revision> {
    let mut r = Reader::from_str(xml);
    let mut revisions = Vec::new();
    let mut alternate_content_stack = Vec::new();
    let mut field_cursor = RevisionFieldCursor::default();
    let ctx = RevisionContext {
        properties: ctx.properties,
        document_bookmarks: ctx.document_bookmarks,
        note_refs: ctx.note_refs,
        sections: ctx.sections,
        style_refs: ctx.style_refs,
        legacy_forms: ctx.legacy_forms,
        toc_entries: ctx.toc_entries,
        bookmark_names: ctx.bookmark_names,
    };
    loop {
        match r.read_event() {
            Ok(Event::Start(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) =>
            {
                skip_subtree(&mut r);
            }
            Ok(Event::Empty(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) => {}
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.push(AlternateContentBranchState::default());
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"pPrChange" => {
                skip_subtree(&mut r);
            }
            Ok(Event::Start(e)) => {
                if let Some(kind) = revision_kind(local(e.name().as_ref())) {
                    revisions.push(read_revision(&mut r, &e, kind, &ctx, &mut field_cursor));
                } else {
                    field_cursor.apply_start(&mut r, &e);
                }
            }
            Ok(Event::Empty(e)) => {
                if let Some(kind) = revision_kind(local(e.name().as_ref())) {
                    revisions.push(revision_shell(&e, kind, String::new()));
                } else {
                    field_cursor.apply_empty(&e);
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    revisions
}

pub(crate) fn main_text_with_view(
    xml: &str,
    view: RevisionView,
    ctx: Option<&FieldResolutionContext<'_>>,
) -> String {
    // The field-resolution context is all-or-nothing at the call site: either every
    // family context is present or none is. Spread it back into the per-field options
    // the text reader threads.
    let properties = ctx.map(|c| c.properties);
    let document_bookmarks = ctx.map(|c| c.document_bookmarks);
    let note_refs = ctx.map(|c| c.note_refs);
    let sections = ctx.map(|c| c.sections);
    let style_refs = ctx.map(|c| c.style_refs);
    let legacy_forms = ctx.map(|c| c.legacy_forms);
    let toc_entries = ctx.map(|c| c.toc_entries);
    let bookmark_names = ctx.map(|c| c.bookmark_names);
    let mut r = Reader::from_str(xml);
    let mut out = String::new();
    let mut alternate_content_stack = Vec::new();
    let mut inline_continuation = false;
    let mut field_cursor = RevisionFieldCursor::default();
    loop {
        match r.read_event() {
            Ok(Event::Start(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) =>
            {
                skip_subtree(&mut r);
            }
            Ok(Event::Empty(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) => {}
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.push(AlternateContentBranchState::default());
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"pPrChange" => {
                skip_subtree(&mut r);
            }
            Ok(Event::Start(e)) => {
                if let Some(kind) = revision_kind(local(e.name().as_ref())) {
                    let use_sections = sections.filter(|_| revision_kind_is_current(kind));
                    let use_style_refs = style_refs.filter(|_| revision_kind_is_current(kind));
                    let use_legacy_forms = legacy_forms.filter(|_| revision_kind_is_current(kind));
                    let (
                        rev_text,
                        next_section_field_index,
                        next_style_ref_field_index,
                        next_form_field_index,
                    ) = read_revision_text(
                        &mut r,
                        local(e.name().as_ref()),
                        RevisionTextContext {
                            properties,
                            document_bookmarks,
                            note_refs,
                            sections: use_sections,
                            section_field_index: field_cursor.section_field_index,
                            style_refs: use_style_refs,
                            style_ref_field_index: field_cursor.style_ref_field_index,
                            legacy_forms: use_legacy_forms,
                            form_field_index: field_cursor.form_field_index,
                            toc_entries,
                            bookmark_names,
                        },
                    );
                    if use_sections.is_some() {
                        field_cursor.section_field_index = next_section_field_index;
                    }
                    if use_style_refs.is_some() {
                        field_cursor.style_ref_field_index = next_style_ref_field_index;
                    }
                    if use_legacy_forms.is_some() {
                        field_cursor.form_field_index = next_form_field_index;
                    }
                    push_revision_text(&mut out, view, kind, &rev_text);
                    inline_continuation = false;
                } else if matches!(local(e.name().as_ref()), b"t" | b"delText") {
                    push_text_segment(&mut out, &read_text(&mut r), &mut inline_continuation);
                } else if (sections.is_some() || legacy_forms.is_some())
                    && field_cursor.apply_start(&mut r, &e)
                {
                    inline_continuation = false;
                } else if let Some(marker) = inline_marker_text(&e) {
                    push_inline_segment(&mut out, marker, &mut inline_continuation);
                    skip_subtree(&mut r);
                } else if local(e.name().as_ref()) == b"sym" {
                    if let Some(ch) = revision_symbol_char(&e) {
                        let text = ch.to_string();
                        push_segment(&mut out, &text);
                        inline_continuation = false;
                    }
                    skip_subtree(&mut r);
                }
            }
            Ok(Event::Empty(e)) => {
                if let Some(kind) = revision_kind(local(e.name().as_ref())) {
                    push_revision_text(&mut out, view, kind, "");
                    inline_continuation = false;
                } else if (sections.is_some() || legacy_forms.is_some())
                    && field_cursor.apply_empty(&e)
                {
                    inline_continuation = false;
                } else if let Some(marker) = inline_marker_text(&e) {
                    push_inline_segment(&mut out, marker, &mut inline_continuation);
                } else if let Some(ch) = revision_symbol_char(&e) {
                    let text = ch.to_string();
                    push_segment(&mut out, &text);
                    inline_continuation = false;
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text::finalize(&out)
}

fn revision_kind(name: &[u8]) -> Option<RevisionKind> {
    match name {
        b"ins" => Some(RevisionKind::Insertion),
        b"del" => Some(RevisionKind::Deletion),
        b"moveFrom" => Some(RevisionKind::MoveFrom),
        b"moveTo" => Some(RevisionKind::MoveTo),
        _ => None,
    }
}

fn revision_shell(e: &BytesStart<'_>, kind: RevisionKind, text: String) -> Revision {
    Revision {
        kind,
        id: attr_local_trimmed(e, b"id"),
        author: attr_local_trimmed(e, b"author"),
        date: attr_local_trimmed(e, b"date"),
        text,
    }
}

/// Non-optional field-evaluation context for a single revision element parsed by
/// [`read_revision`]. Bundled to keep the reader's argument list small; the mutable
/// [`RevisionFieldCursor`] stays a separate parameter because it is updated in place.
struct RevisionContext<'a> {
    properties: FieldDocumentProperties<'a>,
    document_bookmarks: &'a HashMap<String, String>,
    note_refs: &'a NoteRefContext,
    sections: &'a SectionContext,
    style_refs: &'a StyleRefContext,
    legacy_forms: &'a LegacyFormContext,
    toc_entries: &'a [TocEntry],
    bookmark_names: &'a HashSet<String>,
}

fn read_revision(
    r: &mut Xml<'_>,
    start: &BytesStart<'_>,
    kind: RevisionKind,
    ctx: &RevisionContext<'_>,
    field_cursor: &mut RevisionFieldCursor,
) -> Revision {
    let end_name = local(start.name().as_ref()).to_vec();
    let use_sections = revision_kind_is_current(kind).then_some(ctx.sections);
    let use_style_refs = revision_kind_is_current(kind).then_some(ctx.style_refs);
    let use_legacy_forms = revision_kind_is_current(kind).then_some(ctx.legacy_forms);
    let (text, next_section_field_index, next_style_ref_field_index, next_form_field_index) =
        read_revision_text(
            r,
            &end_name,
            RevisionTextContext {
                properties: Some(ctx.properties),
                document_bookmarks: Some(ctx.document_bookmarks),
                note_refs: Some(ctx.note_refs),
                sections: use_sections,
                section_field_index: field_cursor.section_field_index,
                style_refs: use_style_refs,
                style_ref_field_index: field_cursor.style_ref_field_index,
                legacy_forms: use_legacy_forms,
                form_field_index: field_cursor.form_field_index,
                toc_entries: Some(ctx.toc_entries),
                bookmark_names: Some(ctx.bookmark_names),
            },
        );
    if use_sections.is_some() {
        field_cursor.section_field_index = next_section_field_index;
    }
    if use_style_refs.is_some() {
        field_cursor.style_ref_field_index = next_style_ref_field_index;
    }
    if use_legacy_forms.is_some() {
        field_cursor.form_field_index = next_form_field_index;
    }
    revision_shell(start, kind, text)
}

/// Optional field-evaluation context threaded into [`read_revision_text`]: the
/// document/note/section/form contexts plus the running section and form field
/// cursors. Bundled so the reader takes one argument instead of nine parallel ones.
struct RevisionTextContext<'a> {
    properties: Option<FieldDocumentProperties<'a>>,
    document_bookmarks: Option<&'a HashMap<String, String>>,
    note_refs: Option<&'a NoteRefContext>,
    sections: Option<&'a SectionContext>,
    section_field_index: usize,
    style_refs: Option<&'a StyleRefContext>,
    style_ref_field_index: usize,
    legacy_forms: Option<&'a LegacyFormContext>,
    form_field_index: usize,
    toc_entries: Option<&'a [TocEntry]>,
    bookmark_names: Option<&'a HashSet<String>>,
}

fn read_revision_text(
    r: &mut Xml<'_>,
    end_name: &[u8],
    ctx: RevisionTextContext<'_>,
) -> (String, usize, usize, usize) {
    let RevisionTextContext {
        properties,
        document_bookmarks,
        note_refs,
        sections,
        section_field_index,
        style_refs,
        style_ref_field_index,
        legacy_forms,
        form_field_index,
        toc_entries,
        bookmark_names,
    } = ctx;
    let mut depth = 1usize;
    let mut text = String::new();
    let mut complex_field = RevisionComplexField::default();
    let mut field_state = match (properties, document_bookmarks, note_refs) {
        (Some(properties), Some(document_bookmarks), Some(note_refs)) => {
            ContextlessFieldState::with_document_and_note_context(
                properties,
                document_bookmarks,
                note_refs,
            )
        }
        (Some(properties), Some(document_bookmarks), None) => {
            ContextlessFieldState::with_document_context(properties, document_bookmarks)
        }
        (Some(properties), None, _) => ContextlessFieldState::with_document_properties(properties),
        _ => ContextlessFieldState::default(),
    };
    if let (Some(toc_entries), Some(bookmark_names)) = (toc_entries, bookmark_names) {
        field_state = field_state.with_toc_context(toc_entries, bookmark_names);
    }
    if let Some(sections) = sections {
        field_state = field_state.with_section_context_from(sections, section_field_index);
    }
    if let Some(style_refs) = style_refs {
        field_state = field_state.with_style_ref_context_from(style_refs, style_ref_field_index);
    }
    if let Some(legacy_forms) = legacy_forms {
        field_state = field_state.with_legacy_form_context_from(legacy_forms, form_field_index);
    }
    let mut embedded_body_depth = 0usize;
    let mut alternate_content_stack = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) =>
            {
                skip_subtree(r);
            }
            Ok(Event::Empty(e))
                if skip_alternate_content_branch(
                    &mut alternate_content_stack,
                    local(e.name().as_ref()),
                ) => {}
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.push(AlternateContentBranchState::default());
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == end_name => {
                depth = depth.saturating_add(1);
            }
            Ok(Event::Start(e)) if is_revision_embedded_body(local(e.name().as_ref())) => {
                embedded_body_depth += 1;
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"fldChar" => {
                if let Some(computed) = complex_field.apply_field_char(&e, &mut field_state) {
                    text.push_str(&computed);
                }
                skip_subtree(r);
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"fldChar" => {
                if let Some(computed) = complex_field.apply_field_char(&e, &mut field_state) {
                    text.push_str(&computed);
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"instrText" => {
                let instruction = read_text(r);
                complex_field.append_instruction_text(&instruction);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"p" && embedded_body_depth == 0 => {
                push_revision_paragraph_boundary(&mut text);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"fldSimple" => {
                if let Some(instruction) = attr_local_trimmed(&e, b"instr") {
                    if is_revision_text_form_field_instruction(&instruction) {
                        if let Some(computed) = computed_revision_simple_text_form_field_text(
                            r,
                            &instruction,
                            &mut field_state,
                        ) {
                            text.push_str(&computed);
                        }
                    } else if let Some(computed) =
                        computed_revision_field_text(&instruction, &mut field_state)
                    {
                        text.push_str(&computed);
                        skip_subtree(r);
                    }
                }
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"fldSimple" => {
                if let Some(computed) = computed_revision_simple_field_text(&e, &mut field_state) {
                    text.push_str(&computed);
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"t" => {
                let value = read_text(r);
                complex_field.append_result_text(&value);
                if !complex_field.suppresses_result() {
                    text.push_str(&value);
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"delText" => {
                let value = read_text(r);
                complex_field.append_result_text(&value);
                if !complex_field.suppresses_result() {
                    text.push_str(&value);
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"sym" => {
                if let Some(ch) = revision_symbol_char(&e) {
                    complex_field.append_result_char(ch);
                }
                if !complex_field.suppresses_result() {
                    if let Some(ch) = revision_symbol_char(&e) {
                        text.push(ch);
                    }
                }
                skip_subtree(r);
            }
            Ok(Event::Start(e)) => {
                if let Some(marker) = inline_marker_text(&e) {
                    complex_field.append_result_text(marker);
                }
                if !complex_field.suppresses_result() {
                    if let Some(marker) = inline_marker_text(&e) {
                        text.push_str(marker);
                        skip_subtree(r);
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                if let Some(marker) = inline_marker_text(&e) {
                    complex_field.append_result_text(marker);
                } else if let Some(ch) = revision_symbol_char(&e) {
                    complex_field.append_result_char(ch);
                }
                if !complex_field.suppresses_result() {
                    if let Some(marker) = inline_marker_text(&e) {
                        text.push_str(marker);
                    } else if let Some(ch) = revision_symbol_char(&e) {
                        text.push(ch);
                    }
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == end_name => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::End(e)) if is_revision_embedded_body(local(e.name().as_ref())) => {
                embedded_body_depth = embedded_body_depth.saturating_sub(1);
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (
        text,
        field_state.section_field_index(),
        field_state.style_ref_field_index(),
        field_state.form_field_index(),
    )
}

fn is_revision_embedded_body(name: &[u8]) -> bool {
    matches!(name, b"drawing" | b"pict" | b"object" | b"txbxContent")
}

fn revision_kind_is_current(kind: RevisionKind) -> bool {
    matches!(kind, RevisionKind::Insertion | RevisionKind::MoveTo)
}

#[derive(Default)]
struct RevisionFieldCursor {
    section_field_index: usize,
    style_ref_field_index: usize,
    form_field_index: usize,
    complex_depth: usize,
    complex_instruction: String,
    complex_phase: Option<RevisionComplexFieldPhase>,
}

impl RevisionFieldCursor {
    fn apply_start(&mut self, r: &mut Xml<'_>, e: &BytesStart<'_>) -> bool {
        match local(e.name().as_ref()) {
            b"fldSimple" => self.apply_simple_field(e),
            b"fldChar" => {
                self.apply_field_char(e);
                false
            }
            b"instrText" => {
                let text = read_text(r);
                self.append_instruction_text(&text);
                true
            }
            _ => false,
        }
    }

    fn apply_empty(&mut self, e: &BytesStart<'_>) -> bool {
        match local(e.name().as_ref()) {
            b"fldSimple" => self.apply_simple_field(e),
            b"fldChar" => {
                self.apply_field_char(e);
                false
            }
            _ => false,
        }
    }

    fn apply_simple_field(&mut self, e: &BytesStart<'_>) -> bool {
        let Some(instruction) = attr_local_trimmed(e, b"instr") else {
            return false;
        };
        if is_section_field_instruction(&instruction) {
            self.section_field_index += 1;
        }
        if is_style_ref_field_instruction(&instruction) {
            self.style_ref_field_index += 1;
        }
        if legacy_form_field_syntax(&instruction).is_some() {
            self.form_field_index += 1;
        }
        false
    }

    fn apply_field_char(&mut self, e: &BytesStart<'_>) {
        match field_char_type(e).as_deref() {
            Some("begin") => {
                if self.complex_depth == 0 {
                    self.complex_instruction.clear();
                    self.complex_phase = Some(RevisionComplexFieldPhase::Instruction);
                }
                self.complex_depth += 1;
            }
            Some("separate")
                if self.complex_depth == 1
                    && self.complex_phase == Some(RevisionComplexFieldPhase::Instruction) =>
            {
                self.complex_phase = Some(RevisionComplexFieldPhase::Result);
            }
            Some("end") if self.complex_depth > 0 => {
                self.complex_depth -= 1;
                if self.complex_depth == 0 {
                    if is_section_field_instruction(&self.complex_instruction) {
                        self.section_field_index += 1;
                    }
                    if is_style_ref_field_instruction(&self.complex_instruction) {
                        self.style_ref_field_index += 1;
                    }
                    if legacy_form_field_syntax(&self.complex_instruction).is_some() {
                        self.form_field_index += 1;
                    }
                    self.complex_instruction.clear();
                    self.complex_phase = None;
                }
            }
            _ => {}
        }
    }

    fn append_instruction_text(&mut self, text: &str) {
        if self.complex_depth == 1
            && self.complex_phase == Some(RevisionComplexFieldPhase::Instruction)
        {
            self.complex_instruction.push_str(text);
        }
    }
}

fn push_revision_paragraph_boundary(text: &mut String) {
    if !text.is_empty() {
        text.push('\n');
    }
}

fn revision_symbol_char(e: &BytesStart<'_>) -> Option<char> {
    let value = attr_local_trimmed(e, b"char")?;
    let font = attr_local_trimmed(e, b"font");
    computed_run_symbol_char(font.as_deref(), &value)
}

fn computed_revision_simple_field_text(
    e: &BytesStart<'_>,
    field_state: &mut ContextlessFieldState<'_>,
) -> Option<String> {
    let instruction = attr_local_trimmed(e, b"instr")?;
    computed_revision_field_text(&instruction, field_state)
}

fn is_revision_text_form_field_instruction(instruction: &str) -> bool {
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::FormField(kind) if kind == "FORMTEXT"
    )
}

fn computed_revision_simple_text_form_field_text(
    r: &mut Xml<'_>,
    instruction: &str,
    field_state: &mut ContextlessFieldState<'_>,
) -> Option<String> {
    let current_result = read_revision_simple_field_current_result(r);
    field_state
        .computed_legacy_text_form_current_result(instruction, &current_result)
        .or_else(|| (!current_result.is_empty()).then_some(current_result))
}

fn read_revision_simple_field_current_result(r: &mut Xml<'_>) -> String {
    let mut result = String::new();
    let mut depth = 1usize;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if matches!(name, b"t" | b"delText") {
                    result.push_str(&read_text(r));
                } else if let Some(marker) = inline_marker_text(&e) {
                    result.push_str(marker);
                    skip_subtree(r);
                } else if let Some(ch) = revision_symbol_char(&e) {
                    result.push(ch);
                    skip_subtree(r);
                } else {
                    depth += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                if let Some(marker) = inline_marker_text(&e) {
                    result.push_str(marker);
                } else if let Some(ch) = revision_symbol_char(&e) {
                    result.push(ch);
                }
            }
            Ok(Event::End(_)) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    result
}

fn computed_revision_field_text(
    instruction: &str,
    field_state: &mut ContextlessFieldState<'_>,
) -> Option<String> {
    computed_contextless_result(instruction, field_state)
}

#[derive(Default)]
struct RevisionComplexField {
    depth: usize,
    instruction: String,
    phase: Option<RevisionComplexFieldPhase>,
    computed_result: Option<String>,
    result_text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RevisionComplexFieldPhase {
    Instruction,
    Result,
}

impl RevisionComplexField {
    fn apply_field_char(
        &mut self,
        e: &BytesStart<'_>,
        field_state: &mut ContextlessFieldState<'_>,
    ) -> Option<String> {
        match field_char_type(e).as_deref() {
            Some("begin") => {
                if self.depth == 0 {
                    self.instruction.clear();
                    self.result_text.clear();
                    self.phase = Some(RevisionComplexFieldPhase::Instruction);
                    self.computed_result = None;
                }
                self.depth += 1;
                None
            }
            Some("separate")
                if self.depth == 1
                    && self.phase == Some(RevisionComplexFieldPhase::Instruction) =>
            {
                self.phase = Some(RevisionComplexFieldPhase::Result);
                if !is_revision_text_form_field_instruction(&self.instruction) {
                    self.computed_result =
                        computed_revision_field_text(&self.instruction, field_state);
                }
                self.computed_result.clone()
            }
            Some("end") => {
                let computed_text_form = if self.depth == 1
                    && self.phase == Some(RevisionComplexFieldPhase::Result)
                    && self.computed_result.is_none()
                    && is_revision_text_form_field_instruction(&self.instruction)
                {
                    field_state
                        .computed_legacy_text_form_current_result(
                            &self.instruction,
                            &self.result_text,
                        )
                        .or_else(|| {
                            (!self.result_text.is_empty()).then_some(self.result_text.clone())
                        })
                } else {
                    None
                };
                if self.depth > 0 {
                    self.depth -= 1;
                    if self.depth == 0 {
                        self.instruction.clear();
                        self.result_text.clear();
                        self.phase = None;
                        self.computed_result = None;
                    }
                }
                computed_text_form
            }
            _ => None,
        }
    }

    fn append_instruction_text(&mut self, text: &str) {
        if self.depth == 1 && self.phase == Some(RevisionComplexFieldPhase::Instruction) {
            self.instruction.push_str(text);
        }
    }

    fn suppresses_result(&self) -> bool {
        self.depth > 0
            && self.phase == Some(RevisionComplexFieldPhase::Result)
            && (self.computed_result.is_some()
                || is_revision_text_form_field_instruction(&self.instruction))
    }

    fn append_result_text(&mut self, text: &str) {
        if self.collects_result_text() {
            self.result_text.push_str(text);
        }
    }

    fn append_result_char(&mut self, ch: char) {
        if self.collects_result_text() {
            self.result_text.push(ch);
        }
    }

    fn collects_result_text(&self) -> bool {
        self.depth > 0
            && self.phase == Some(RevisionComplexFieldPhase::Result)
            && self.computed_result.is_none()
            && is_revision_text_form_field_instruction(&self.instruction)
    }
}

fn push_revision_text(out: &mut String, view: RevisionView, kind: RevisionKind, value: &str) {
    match (view, kind) {
        (RevisionView::Accepted, RevisionKind::Insertion | RevisionKind::MoveTo)
        | (RevisionView::Original, RevisionKind::Deletion | RevisionKind::MoveFrom) => {
            push_segment(out, value);
        }
        (RevisionView::Annotated, RevisionKind::Insertion) => {
            push_marker(out, "[+", value, "]");
        }
        (RevisionView::Annotated, RevisionKind::Deletion) => {
            push_marker(out, "[-", value, "]");
        }
        (RevisionView::Annotated, RevisionKind::MoveFrom) => {
            push_marker(out, "[~", value, "->]");
        }
        (RevisionView::Annotated, RevisionKind::MoveTo) => {
            push_marker(out, "[~->", value, "]");
        }
        _ => {}
    }
}

fn push_marker(out: &mut String, prefix: &str, value: &str, suffix: &str) {
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(prefix);
    out.push_str(value);
    out.push_str(suffix);
}

fn push_text_segment(out: &mut String, value: &str, inline_continuation: &mut bool) {
    if value.is_empty() {
        return;
    }
    if !out.is_empty() && !*inline_continuation {
        out.push(' ');
    }
    out.push_str(value);
    *inline_continuation = false;
}

fn push_inline_segment(out: &mut String, value: &str, inline_continuation: &mut bool) {
    if value.is_empty() {
        return;
    }
    out.push_str(value);
    *inline_continuation = true;
}

fn push_segment(out: &mut String, value: &str) {
    if value.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(value);
}
