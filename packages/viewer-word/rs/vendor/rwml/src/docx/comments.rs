//! `.docx` comments part parsing.

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use std::collections::HashMap;

use crate::annotation::{Comment, FieldKind, TextAnchor};

use super::fields::{
    computed_contextless_result, computed_run_symbol_char, ContextlessFieldState,
    FieldResolutionContext,
};
use super::xml_text::{
    inline_marker_text, read_text, skip_alternate_content_branch, skip_subtree,
    AlternateContentBranchState,
};
use super::{attr_local_trimmed, field_char_type, local};

type Xml<'a> = Reader<&'a [u8]>;

pub(crate) fn parse(xml: &str, ctx: &FieldResolutionContext<'_>) -> Vec<Comment> {
    let mut r = Reader::from_str(xml);
    let mut comments = Vec::new();
    let mut alternate_content_stack = Vec::new();
    let mut cursor = CommentFieldCursor::default();
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
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"comment" => {
                if let Some(comment) = read_comment(&mut r, &e, ctx, &mut cursor) {
                    comments.push(comment);
                }
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"comment" => {
                if let Some(comment) = comment_shell(&e) {
                    comments.push(comment);
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    comments
}

pub(crate) fn apply_extended_parent_ids(
    comments: &mut [Comment],
    comments_xml: &str,
    ex_xml: &str,
) {
    let id_to_para = comment_para_ids(comments_xml);
    if id_to_para.is_empty() {
        return;
    }
    let para_to_id: HashMap<String, String> = id_to_para
        .iter()
        .map(|(id, para_id)| (para_id.clone(), id.clone()))
        .collect();
    let metadata = extended_comment_metadata(ex_xml);
    for comment in comments {
        let Some(child_para_id) = id_to_para.get(&comment.id) else {
            continue;
        };
        let Some(meta) = metadata.get(child_para_id) else {
            continue;
        };
        if let Some(done) = meta.done {
            comment.resolved = Some(done);
        }
        if comment.parent_comment_id.is_some() {
            continue;
        }
        let Some(parent_para_id) = meta.parent_para_id.as_ref() else {
            continue;
        };
        if let Some(parent_id) = para_to_id.get(parent_para_id) {
            comment.parent_comment_id = Some(parent_id.clone());
        }
    }
}

fn comment_shell(e: &BytesStart<'_>) -> Option<Comment> {
    Some(Comment {
        id: attr_local_trimmed(e, b"id")?,
        author: attr_local_trimmed(e, b"author"),
        initials: attr_local_trimmed(e, b"initials"),
        date: attr_local_trimmed(e, b"date"),
        parent_comment_id: attr_local_trimmed(e, b"parentId"),
        text: String::new(),
        anchor: None,
        resolved: None,
    })
}

fn comment_para_ids(xml: &str) -> HashMap<String, String> {
    let mut r = Reader::from_str(xml);
    let mut current_comment_id: Option<String> = None;
    let mut ids = HashMap::new();
    let mut alternate_content_stack = Vec::new();
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
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"comment" => {
                current_comment_id = attr_local_trimmed(&e, b"id");
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"comment" => {
                current_comment_id = None;
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"p" => {
                if let (Some(comment_id), Some(para_id)) = (
                    current_comment_id.as_ref(),
                    attr_local_trimmed(&e, b"paraId"),
                ) {
                    ids.insert(comment_id.clone(), para_id);
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"comment" => {
                current_comment_id = None;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    ids
}

#[derive(Default)]
struct CommentExMetadata {
    parent_para_id: Option<String>,
    done: Option<bool>,
}

fn extended_comment_metadata(xml: &str) -> HashMap<String, CommentExMetadata> {
    let mut r = Reader::from_str(xml);
    let mut ids: HashMap<String, CommentExMetadata> = HashMap::new();
    let mut alternate_content_stack = Vec::new();
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
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"commentEx" =>
            {
                if let Some(para_id) = attr_local_trimmed(&e, b"paraId") {
                    ids.insert(
                        para_id,
                        CommentExMetadata {
                            parent_para_id: attr_local_trimmed(&e, b"paraIdParent"),
                            done: attr_local_trimmed(&e, b"done")
                                .map(|v| super::toggle_on(Some(v))),
                        },
                    );
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    ids
}

pub(crate) fn parse_anchors(
    xml: &str,
    ctx: &FieldResolutionContext<'_>,
) -> HashMap<String, TextAnchor> {
    let mut r = Reader::from_str(xml);
    let mut anchors: HashMap<String, TextAnchor> = HashMap::new();
    let mut active: Vec<(String, bool)> = Vec::new();
    let mut complex_field = CommentComplexField::default();
    let mut field_state = ContextlessFieldState::with_document_and_note_context(
        ctx.properties,
        ctx.document_bookmarks,
        ctx.note_refs,
    )
    .with_toc_context(ctx.toc_entries, ctx.bookmark_names)
    .with_section_context(ctx.sections)
    .with_style_ref_context_from(ctx.style_refs, 0)
    .with_legacy_form_context_from(ctx.legacy_forms, 0);
    let mut old_content_depth = 0usize;
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
            Ok(Event::Start(e)) if matches!(local(e.name().as_ref()), b"del" | b"moveFrom") => {
                old_content_depth += 1;
            }
            Ok(Event::Start(e)) if is_comment_anchor_embedded_body(local(e.name().as_ref())) => {
                embedded_body_depth += 1;
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"fldChar" => {
                if old_content_depth == 0 && (!active.is_empty() || complex_field.is_active()) {
                    if let Some(text) = complex_field.apply_field_char(&e, &mut field_state) {
                        push_anchor_text(&active, &mut anchors, &text);
                    }
                }
                skip_subtree(&mut r);
            }
            Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"fldChar"
                    && old_content_depth == 0
                    && (!active.is_empty() || complex_field.is_active()) =>
            {
                if let Some(text) = complex_field.apply_field_char(&e, &mut field_state) {
                    push_anchor_text(&active, &mut anchors, &text);
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"instrText" => {
                let text = read_text(&mut r);
                if old_content_depth == 0 && (!active.is_empty() || complex_field.is_active()) {
                    complex_field.append_instruction_text(&text);
                }
            }
            Ok(Event::Start(e))
                if local(e.name().as_ref()) == b"p"
                    && old_content_depth == 0
                    && embedded_body_depth == 0 =>
            {
                push_anchor_paragraph_boundary(&active, &mut anchors);
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"commentRangeStart" && old_content_depth == 0 =>
            {
                if let Some(id) = attr_local_trimmed(&e, b"id") {
                    anchors.entry(id.clone()).or_insert_with(|| TextAnchor {
                        id: id.clone(),
                        text: String::new(),
                    });
                    active.push((id, true));
                }
            }
            Ok(Event::Start(e))
                if local(e.name().as_ref()) == b"fldSimple" && old_content_depth == 0 =>
            {
                if let Some(instruction) = attr_local_trimmed(&e, b"instr") {
                    if is_comment_text_form_field_instruction(&instruction) {
                        if let Some(text) = computed_comment_simple_text_form_field_text(
                            &mut r,
                            &instruction,
                            &mut field_state,
                        ) {
                            push_anchor_text(&active, &mut anchors, &text);
                        }
                    } else if let Some(text) =
                        computed_comment_field_text(&instruction, &mut field_state)
                    {
                        push_anchor_text(&active, &mut anchors, &text);
                        skip_subtree(&mut r);
                    }
                }
            }
            Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"fldSimple" && old_content_depth == 0 =>
            {
                if let Some(text) = computed_comment_simple_field_text(&e, &mut field_state) {
                    push_anchor_text(&active, &mut anchors, &text);
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"t" => {
                let text = read_text(&mut r);
                if old_content_depth == 0 {
                    complex_field.append_result_text(&text);
                    if !complex_field.suppresses_result() {
                        push_anchor_text(&active, &mut anchors, &text);
                    }
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"delText" => {
                let text = read_text(&mut r);
                if old_content_depth == 0 {
                    complex_field.append_result_text(&text);
                    if !complex_field.suppresses_result() {
                        push_anchor_text(&active, &mut anchors, &text);
                    }
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"commentRangeEnd" && old_content_depth == 0 =>
            {
                if let Some(id) = attr_local_trimmed(&e, b"id") {
                    if let Some(pos) = active.iter().rposition(|(active_id, _)| active_id == &id) {
                        active.remove(pos);
                    }
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if old_content_depth == 0 => {
                if let Some(text) = inline_marker_text(&e) {
                    complex_field.append_result_text(text);
                    if !complex_field.suppresses_result() {
                        push_anchor_text(&active, &mut anchors, text);
                    }
                } else if let Some(ch) = comment_symbol_char(&e) {
                    complex_field.append_result_char(ch);
                    if !complex_field.suppresses_result() {
                        let text = ch.to_string();
                        push_anchor_text(&active, &mut anchors, &text);
                    }
                }
            }
            Ok(Event::End(e)) if matches!(local(e.name().as_ref()), b"del" | b"moveFrom") => {
                old_content_depth = old_content_depth.saturating_sub(1);
            }
            Ok(Event::End(e)) if is_comment_anchor_embedded_body(local(e.name().as_ref())) => {
                embedded_body_depth = embedded_body_depth.saturating_sub(1);
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    anchors
}

/// Running source-order field cursors carried across comments in one part, so a
/// section/style-ref/form field keeps advancing its index from comment to comment.
#[derive(Default)]
struct CommentFieldCursor {
    section_field_index: usize,
    style_ref_field_index: usize,
    form_field_index: usize,
}

fn read_comment(
    r: &mut Xml<'_>,
    start: &BytesStart<'_>,
    ctx: &FieldResolutionContext<'_>,
    cursor: &mut CommentFieldCursor,
) -> Option<Comment> {
    let mut c = comment_shell(start);
    let mut complex_field = CommentComplexField::default();
    let mut field_state = ContextlessFieldState::with_document_and_note_context(
        ctx.properties,
        ctx.document_bookmarks,
        ctx.note_refs,
    )
    .with_toc_context(ctx.toc_entries, ctx.bookmark_names)
    .with_section_context_from(ctx.sections, cursor.section_field_index)
    .with_style_ref_context_from(ctx.style_refs, cursor.style_ref_field_index)
    .with_legacy_form_context_from(ctx.legacy_forms, cursor.form_field_index);
    let mut old_content_depth = 0usize;
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
            Ok(Event::Start(e)) if matches!(local(e.name().as_ref()), b"del" | b"moveFrom") => {
                old_content_depth += 1;
            }
            Ok(Event::Start(e)) if is_comment_anchor_embedded_body(local(e.name().as_ref())) => {
                embedded_body_depth += 1;
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"fldChar" => {
                if old_content_depth == 0 {
                    if let Some(text) = complex_field.apply_field_char(&e, &mut field_state) {
                        if let Some(c) = c.as_mut() {
                            c.text.push_str(&text);
                        }
                    }
                }
                skip_subtree(r);
            }
            Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"fldChar" && old_content_depth == 0 =>
            {
                if let Some(text) = complex_field.apply_field_char(&e, &mut field_state) {
                    if let Some(c) = c.as_mut() {
                        c.text.push_str(&text);
                    }
                }
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"instrText" => {
                let text = read_text(r);
                if old_content_depth == 0 {
                    complex_field.append_instruction_text(&text);
                }
            }
            Ok(Event::Start(e))
                if local(e.name().as_ref()) == b"p"
                    && old_content_depth == 0
                    && embedded_body_depth == 0 =>
            {
                if let Some(c) = c.as_mut() {
                    push_comment_paragraph_boundary(&mut c.text);
                }
            }
            Ok(Event::Start(e))
                if local(e.name().as_ref()) == b"fldSimple" && old_content_depth == 0 =>
            {
                if let Some(instruction) = attr_local_trimmed(&e, b"instr") {
                    if is_comment_text_form_field_instruction(&instruction) {
                        if let Some(text) = computed_comment_simple_text_form_field_text(
                            r,
                            &instruction,
                            &mut field_state,
                        ) {
                            if let Some(c) = c.as_mut() {
                                c.text.push_str(&text);
                            }
                        }
                    } else if let Some(text) =
                        computed_comment_field_text(&instruction, &mut field_state)
                    {
                        if let Some(c) = c.as_mut() {
                            c.text.push_str(&text);
                        }
                        skip_subtree(r);
                    }
                }
            }
            Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"fldSimple" && old_content_depth == 0 =>
            {
                if let Some(text) = computed_comment_simple_field_text(&e, &mut field_state) {
                    if let Some(c) = c.as_mut() {
                        c.text.push_str(&text);
                    }
                }
            }
            Ok(Event::Start(e)) if matches!(local(e.name().as_ref()), b"t" | b"delText") => {
                let text = read_text(r);
                if old_content_depth == 0 {
                    complex_field.append_result_text(&text);
                    if !complex_field.suppresses_result() {
                        if let Some(c) = c.as_mut() {
                            c.text.push_str(&text);
                        }
                    }
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if old_content_depth == 0 => {
                if let Some(text) = inline_marker_text(&e) {
                    complex_field.append_result_text(text);
                    if !complex_field.suppresses_result() {
                        if let Some(c) = c.as_mut() {
                            c.text.push_str(text);
                        }
                    }
                } else if let Some(ch) = comment_symbol_char(&e) {
                    complex_field.append_result_char(ch);
                    if !complex_field.suppresses_result() {
                        if let Some(c) = c.as_mut() {
                            c.text.push(ch);
                        }
                    }
                }
            }
            Ok(Event::End(e)) if matches!(local(e.name().as_ref()), b"del" | b"moveFrom") => {
                old_content_depth = old_content_depth.saturating_sub(1);
            }
            Ok(Event::End(e)) if is_comment_anchor_embedded_body(local(e.name().as_ref())) => {
                embedded_body_depth = embedded_body_depth.saturating_sub(1);
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                alternate_content_stack.pop();
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"comment" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    cursor.section_field_index = field_state.section_field_index();
    cursor.style_ref_field_index = field_state.style_ref_field_index();
    cursor.form_field_index = field_state.form_field_index();
    c
}

fn comment_symbol_char(e: &BytesStart<'_>) -> Option<char> {
    let value = attr_local_trimmed(e, b"char")?;
    let font = attr_local_trimmed(e, b"font");
    computed_run_symbol_char(font.as_deref(), &value)
}

fn computed_comment_simple_field_text(
    e: &BytesStart<'_>,
    field_state: &mut ContextlessFieldState<'_>,
) -> Option<String> {
    let instruction = attr_local_trimmed(e, b"instr")?;
    computed_comment_field_text(&instruction, field_state)
}

fn is_comment_text_form_field_instruction(instruction: &str) -> bool {
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::FormField(kind) if kind == "FORMTEXT"
    )
}

fn computed_comment_simple_text_form_field_text(
    r: &mut Xml<'_>,
    instruction: &str,
    field_state: &mut ContextlessFieldState<'_>,
) -> Option<String> {
    let current_result = read_comment_simple_field_current_result(r);
    field_state
        .computed_legacy_text_form_current_result(instruction, &current_result)
        .or_else(|| (!current_result.is_empty()).then_some(current_result))
}

fn read_comment_simple_field_current_result(r: &mut Xml<'_>) -> String {
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
                } else if let Some(ch) = comment_symbol_char(&e) {
                    result.push(ch);
                    skip_subtree(r);
                } else {
                    depth += 1;
                }
            }
            Ok(Event::Empty(e)) => {
                if let Some(marker) = inline_marker_text(&e) {
                    result.push_str(marker);
                } else if let Some(ch) = comment_symbol_char(&e) {
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

fn computed_comment_field_text(
    instruction: &str,
    field_state: &mut ContextlessFieldState<'_>,
) -> Option<String> {
    computed_contextless_result(instruction, field_state)
}

#[derive(Default)]
struct CommentComplexField {
    depth: usize,
    instruction: String,
    phase: Option<CommentComplexFieldPhase>,
    computed_result: Option<String>,
    result_text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CommentComplexFieldPhase {
    Instruction,
    Result,
}

impl CommentComplexField {
    fn is_active(&self) -> bool {
        self.depth > 0
    }

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
                    self.phase = Some(CommentComplexFieldPhase::Instruction);
                    self.computed_result = None;
                }
                self.depth += 1;
                None
            }
            Some("separate")
                if self.depth == 1 && self.phase == Some(CommentComplexFieldPhase::Instruction) =>
            {
                self.phase = Some(CommentComplexFieldPhase::Result);
                if !is_comment_text_form_field_instruction(&self.instruction) {
                    self.computed_result =
                        computed_comment_field_text(&self.instruction, field_state);
                }
                self.computed_result.clone()
            }
            Some("end") => {
                let computed_text_form = if self.depth == 1
                    && self.phase == Some(CommentComplexFieldPhase::Result)
                    && self.computed_result.is_none()
                    && is_comment_text_form_field_instruction(&self.instruction)
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
        if self.depth == 1 && self.phase == Some(CommentComplexFieldPhase::Instruction) {
            self.instruction.push_str(text);
        }
    }

    fn suppresses_result(&self) -> bool {
        self.depth > 0
            && self.phase == Some(CommentComplexFieldPhase::Result)
            && (self.computed_result.is_some()
                || is_comment_text_form_field_instruction(&self.instruction))
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
            && self.phase == Some(CommentComplexFieldPhase::Result)
            && self.computed_result.is_none()
            && is_comment_text_form_field_instruction(&self.instruction)
    }
}

fn is_comment_anchor_embedded_body(name: &[u8]) -> bool {
    matches!(name, b"drawing" | b"pict" | b"object" | b"txbxContent")
}

fn push_comment_paragraph_boundary(text: &mut String) {
    if !text.is_empty() {
        text.push('\n');
    }
}

fn push_anchor_paragraph_boundary(
    active: &[(String, bool)],
    anchors: &mut HashMap<String, TextAnchor>,
) {
    for (id, visible) in active {
        if !visible {
            continue;
        }
        if let Some(anchor) = anchors.get_mut(id) {
            push_comment_paragraph_boundary(&mut anchor.text);
        }
    }
}

fn push_anchor_text(
    active: &[(String, bool)],
    anchors: &mut HashMap<String, TextAnchor>,
    text: &str,
) {
    for (id, visible) in active {
        if !visible {
            continue;
        }
        if let Some(anchor) = anchors.get_mut(id) {
            anchor.text.push_str(text);
        }
    }
}
