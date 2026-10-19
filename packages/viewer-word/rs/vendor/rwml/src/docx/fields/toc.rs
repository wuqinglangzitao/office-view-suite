use super::reference::{
    computed_ref_bookmark_text_result, direct_bookmark_ref_instruction,
    is_ref_position_field_instruction, ref_instruction, ref_or_unknown_direct_bookmark_instruction,
};
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TocEntry {
    pub level: u8,
    pub text: String,
    pub(crate) source: TocEntrySource,
    pub(crate) tc_type: Option<String>,
    pub(crate) sequence_identifier: Option<String>,
    pub(crate) sequence_caption_text: Option<String>,
    pub(crate) bookmarks: Vec<String>,
    pub(crate) style_id: Option<String>,
    pub(crate) style_name: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TocEntrySource {
    HeadingStyle,
    OutlineLevel,
    StyledParagraph,
    TcField,
    SequenceField,
}

#[cfg(test)]
pub(crate) fn toc_entries(
    xml: &str,
    styles: &Styles,
    ref_targets: &HashMap<String, String>,
) -> Vec<TocEntry> {
    let core_properties = CoreProperties::default();
    let empty_properties = HashMap::new();
    let note_refs = NoteRefContext::empty();
    let sections = SectionContext::empty();
    toc_entries_with_properties(
        xml,
        styles,
        ref_targets,
        &note_refs,
        &sections,
        FieldDocumentProperties {
            core: &core_properties,
            custom: &empty_properties,
            variables: &empty_properties,
            extended: &empty_properties,
            file_size_bytes: None,
        },
        false,
    )
}

pub(crate) fn toc_entries_with_properties(
    xml: &str,
    styles: &Styles,
    ref_targets: &HashMap<String, String>,
    note_refs: &NoteRefContext,
    sections: &SectionContext,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
) -> Vec<TocEntry> {
    let legacy_forms = legacy_form_context(xml, preserve_legacy_form_cache);
    let mut r = Reader::from_str(xml);
    let mut entries = Vec::new();
    let mut active_bookmarks = Vec::new();
    let mut sequence_counters = HashMap::new();
    let sequence_headings = sequence_heading_context(xml, styles);
    let mut sequence_heading_scopes = HashMap::new();
    let mut sequence_field_index = 0usize;
    let mut autonum_counter = 0i64;
    let mut listnum_counter = 0i64;
    let mut field_bookmarks = HashMap::new();
    let mut form_field_index = 0usize;
    let mut ref_field_index = 0usize;
    let mut note_ref_field_index = 0usize;
    let mut section_field_index = 0usize;
    let mut xml_depth = 0usize;
    let mut alternate_content_stack = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    skip_subtree(&mut r);
                    continue;
                }
                if matches!(name, b"del" | b"moveFrom") {
                    skip_subtree(&mut r);
                    continue;
                }
                let mut consumed_element = false;
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.push(AlternateContentBranchState {
                            branch_depth: xml_depth + 1,
                            took_branch: false,
                        });
                    }
                    b"p" => {
                        read_toc_paragraph(
                            &mut r,
                            styles,
                            &mut active_bookmarks,
                            TocParagraphScan {
                                entries: &mut entries,
                                sequence_counters: &mut sequence_counters,
                                sequence_heading_scopes: &mut sequence_heading_scopes,
                                sequence_field_index: &mut sequence_field_index,
                                sequence_headings: &sequence_headings,
                            },
                            &mut TocSourceCtx {
                                ref_targets,
                                note_refs,
                                sections,
                                legacy_forms: &legacy_forms,
                                properties,
                                autonum_counter: &mut autonum_counter,
                                listnum_counter: &mut listnum_counter,
                                field_bookmarks: &mut field_bookmarks,
                                form_field_index: &mut form_field_index,
                                ref_field_index: &mut ref_field_index,
                                note_ref_field_index: &mut note_ref_field_index,
                                section_field_index: &mut section_field_index,
                            },
                        );
                        consumed_element = true;
                    }
                    b"bookmarkStart" => {
                        push_active_bookmark(&mut active_bookmarks, &e);
                    }
                    b"bookmarkEnd" => {
                        remove_active_bookmark(&mut active_bookmarks, &e);
                    }
                    _ => {}
                }
                if !consumed_element {
                    xml_depth = xml_depth.saturating_add(1);
                }
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    continue;
                }
                match name {
                    b"bookmarkStart" => {
                        push_active_bookmark(&mut active_bookmarks, &e);
                    }
                    b"bookmarkEnd" => {
                        remove_active_bookmark(&mut active_bookmarks, &e);
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    entries
}

/// Threaded TOC source-field evaluation state: read-only document context plus
/// the mutable numbering/index accumulators that travel together through the scan.
struct TocSourceCtx<'a> {
    ref_targets: &'a HashMap<String, String>,
    note_refs: &'a NoteRefContext,
    sections: &'a SectionContext,
    legacy_forms: &'a LegacyFormContext,
    properties: FieldDocumentProperties<'a>,
    autonum_counter: &'a mut i64,
    listnum_counter: &'a mut i64,
    field_bookmarks: &'a mut HashMap<String, String>,
    form_field_index: &'a mut usize,
    ref_field_index: &'a mut usize,
    note_ref_field_index: &'a mut usize,
    section_field_index: &'a mut usize,
}

/// TOC entry/sequence accumulators threaded through the paragraph scan.
struct TocEmit<'a> {
    entries: &'a mut Vec<TocEntry>,
    sequence_counters: &'a mut HashMap<String, i64>,
    sequence_heading_scopes: &'a mut HashMap<(String, u8), u32>,
    sequence_field_index: &'a mut usize,
    sequence_headings: &'a SequenceHeadingContext,
    sequence_identifiers: &'a mut Vec<String>,
}

/// The per-paragraph entry/sequence accumulators passed into `read_toc_paragraph`
/// (destructured immediately so the scan body keeps its original bindings).
struct TocParagraphScan<'a> {
    entries: &'a mut Vec<TocEntry>,
    sequence_counters: &'a mut HashMap<String, i64>,
    sequence_heading_scopes: &'a mut HashMap<(String, u8), u32>,
    sequence_field_index: &'a mut usize,
    sequence_headings: &'a SequenceHeadingContext,
}

fn read_toc_paragraph(
    r: &mut Xml<'_>,
    styles: &Styles,
    active_bookmarks: &mut Vec<(String, String)>,
    scan: TocParagraphScan<'_>,
    ctx: &mut TocSourceCtx<'_>,
) {
    let TocParagraphScan {
        entries,
        sequence_counters,
        sequence_heading_scopes,
        sequence_field_index,
        sequence_headings,
    } = scan;
    let ref_targets = ctx.ref_targets;
    let note_refs = ctx.note_refs;
    let sections = ctx.sections;
    let legacy_forms = ctx.legacy_forms;
    let properties = ctx.properties;
    let autonum_counter = &mut *ctx.autonum_counter;
    let listnum_counter = &mut *ctx.listnum_counter;
    let field_bookmarks = &mut *ctx.field_bookmarks;
    let form_field_index = &mut *ctx.form_field_index;
    let ref_field_index = &mut *ctx.ref_field_index;
    let note_ref_field_index = &mut *ctx.note_ref_field_index;
    let section_field_index = &mut *ctx.section_field_index;
    let mut style_id: Option<String> = None;
    let mut outline: Option<u8> = None;
    let mut text = String::new();
    let mut bookmarks = active_bookmark_names(active_bookmarks);
    let mut current = Vec::new();
    let mut result_starts: Vec<Option<usize>> = Vec::new();
    let mut sequence_identifiers = Vec::new();
    let mut xml_depth = 0usize;
    let mut alternate_content_stack = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    skip_subtree(r);
                    continue;
                }
                if matches!(name, b"del" | b"moveFrom") {
                    skip_subtree(r);
                    continue;
                }
                let mut consumed_element = false;
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.push(AlternateContentBranchState {
                            branch_depth: xml_depth + 1,
                            took_branch: false,
                        });
                    }
                    b"pPr" => {
                        read_toc_ppr(r, &mut style_id, &mut outline);
                        consumed_element = true;
                    }
                    b"fldSimple" => {
                        let instruction = attr_local(&e, b"instr");
                        if push_tc_entry_from_instruction(instruction.clone(), &bookmarks, entries)
                            || push_computed_toc_sequence_result(
                                instruction.as_deref(),
                                &mut TocEmit {
                                    entries,
                                    sequence_counters,
                                    sequence_heading_scopes,
                                    sequence_field_index,
                                    sequence_headings,
                                    sequence_identifiers: &mut sequence_identifiers,
                                },
                                &mut text,
                            )
                        {
                            skip_element(r, b"fldSimple");
                            consumed_element = true;
                        } else {
                            text.push_str(&read_toc_simple_field_result(
                                r,
                                instruction.as_deref(),
                                active_bookmarks,
                                &mut bookmarks,
                                &mut TocEmit {
                                    entries,
                                    sequence_counters,
                                    sequence_heading_scopes,
                                    sequence_field_index,
                                    sequence_headings,
                                    sequence_identifiers: &mut sequence_identifiers,
                                },
                                &mut TocSourceCtx {
                                    ref_targets,
                                    note_refs,
                                    sections,
                                    legacy_forms,
                                    properties,
                                    autonum_counter,
                                    listnum_counter,
                                    field_bookmarks,
                                    form_field_index,
                                    ref_field_index,
                                    note_ref_field_index,
                                    section_field_index,
                                },
                            ));
                            consumed_element = true;
                        }
                    }
                    b"fldChar" => {
                        apply_toc_fld_char(
                            &e,
                            &mut current,
                            &mut result_starts,
                            &bookmarks,
                            &mut text,
                            &mut TocEmit {
                                entries,
                                sequence_counters,
                                sequence_heading_scopes,
                                sequence_field_index,
                                sequence_headings,
                                sequence_identifiers: &mut sequence_identifiers,
                            },
                            &mut TocSourceCtx {
                                ref_targets,
                                note_refs,
                                sections,
                                legacy_forms,
                                properties,
                                autonum_counter,
                                listnum_counter,
                                field_bookmarks,
                                form_field_index,
                                ref_field_index,
                                note_ref_field_index,
                                section_field_index,
                            },
                        );
                    }
                    b"instrText" => {
                        let field_text = read_text(r);
                        consumed_element = true;
                        if let Some(field) = current.last_mut() {
                            if field.phase == FieldPhase::Instruction {
                                field.instruction.push_str(&field_text);
                            }
                        }
                    }
                    b"t" => {
                        let run_text = read_text(r);
                        consumed_element = true;
                        let hidden_result = toc_source_hidden_field_result(&current);
                        if !hidden_result {
                            text.push_str(&run_text);
                        }
                    }
                    b"sym" if !toc_source_hidden_field_result(&current) => {
                        if let Some(ch) = toc_symbol_char(&e) {
                            text.push(ch);
                        }
                    }
                    b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" => {
                        push_toc_source_marker_text(&mut text, name, &current);
                    }
                    b"bookmarkStart" => {
                        if let Some(name) = push_active_bookmark(active_bookmarks, &e) {
                            push_unique(&mut bookmarks, name);
                        }
                    }
                    b"bookmarkEnd" => remove_active_bookmark(active_bookmarks, &e),
                    _ => {}
                }
                if !consumed_element {
                    xml_depth = xml_depth.saturating_add(1);
                }
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    continue;
                }
                match name {
                    b"fldSimple" => {
                        let instruction = attr_local(&e, b"instr");
                        if !push_tc_entry_from_instruction(instruction.clone(), &bookmarks, entries)
                            && !push_computed_toc_sequence_result(
                                instruction.as_deref(),
                                &mut TocEmit {
                                    entries,
                                    sequence_counters,
                                    sequence_heading_scopes,
                                    sequence_field_index,
                                    sequence_headings,
                                    sequence_identifiers: &mut sequence_identifiers,
                                },
                                &mut text,
                            )
                        {
                            if let Some(computed) = computed_toc_source_field_result(
                                instruction.as_deref(),
                                &mut TocSourceCtx {
                                    ref_targets,
                                    note_refs,
                                    sections,
                                    legacy_forms,
                                    properties,
                                    autonum_counter,
                                    listnum_counter,
                                    field_bookmarks,
                                    form_field_index,
                                    ref_field_index,
                                    note_ref_field_index,
                                    section_field_index,
                                },
                                "",
                            ) {
                                text.push_str(&computed);
                            }
                        }
                    }
                    b"fldChar" => {
                        apply_toc_fld_char(
                            &e,
                            &mut current,
                            &mut result_starts,
                            &bookmarks,
                            &mut text,
                            &mut TocEmit {
                                entries,
                                sequence_counters,
                                sequence_heading_scopes,
                                sequence_field_index,
                                sequence_headings,
                                sequence_identifiers: &mut sequence_identifiers,
                            },
                            &mut TocSourceCtx {
                                ref_targets,
                                note_refs,
                                sections,
                                legacy_forms,
                                properties,
                                autonum_counter,
                                listnum_counter,
                                field_bookmarks,
                                form_field_index,
                                ref_field_index,
                                note_ref_field_index,
                                section_field_index,
                            },
                        );
                    }
                    b"sym" if !toc_source_hidden_field_result(&current) => {
                        if let Some(ch) = toc_symbol_char(&e) {
                            text.push(ch);
                        }
                    }
                    b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" => {
                        push_toc_source_marker_text(&mut text, name, &current);
                    }
                    b"bookmarkStart" => {
                        if let Some(name) = push_active_bookmark(active_bookmarks, &e) {
                            push_unique(&mut bookmarks, name);
                        }
                    }
                    b"bookmarkEnd" => remove_active_bookmark(active_bookmarks, &e),
                    _ => {}
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"p" => break,
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let style_name = style_id
        .as_deref()
        .and_then(|style_id| styles.name(style_id))
        .map(str::to_string);
    let text = normalize_toc_text(&text);
    if !text.is_empty() {
        for identifier in sequence_identifiers {
            entries.push(TocEntry {
                level: 1,
                sequence_caption_text: caption_text_without_label(&text, &identifier),
                text: text.clone(),
                source: TocEntrySource::SequenceField,
                tc_type: None,
                sequence_identifier: Some(identifier),
                bookmarks: bookmarks.clone(),
                style_id: None,
                style_name: None,
            });
        }
    }
    let (level, source) = match outline {
        Some(o) if o <= 8 => (o + 1, TocEntrySource::OutlineLevel),
        Some(_) => return,
        None => match style_id
            .as_deref()
            .and_then(|style_id| styles.heading_level(style_id))
        {
            Some(level) => (level, TocEntrySource::HeadingStyle),
            None if style_id.is_some() || style_name.is_some() => {
                (0, TocEntrySource::StyledParagraph)
            }
            None => return,
        },
    };
    if !text.is_empty() {
        entries.push(TocEntry {
            level,
            text,
            source,
            tc_type: None,
            sequence_identifier: None,
            sequence_caption_text: None,
            bookmarks,
            style_id,
            style_name,
        });
    }
}

fn read_toc_simple_field_result(
    r: &mut Xml<'_>,
    instruction: Option<&str>,
    active_bookmarks: &mut Vec<(String, String)>,
    bookmarks: &mut Vec<String>,
    emit: &mut TocEmit<'_>,
    ctx: &mut TocSourceCtx<'_>,
) -> String {
    let entries = &mut *emit.entries;
    let sequence_counters = &mut *emit.sequence_counters;
    let sequence_heading_scopes = &mut *emit.sequence_heading_scopes;
    let sequence_field_index = &mut *emit.sequence_field_index;
    let sequence_headings = emit.sequence_headings;
    let sequence_identifiers = &mut *emit.sequence_identifiers;
    let ref_targets = ctx.ref_targets;
    let note_refs = ctx.note_refs;
    let sections = ctx.sections;
    let legacy_forms = ctx.legacy_forms;
    let properties = ctx.properties;
    let autonum_counter = &mut *ctx.autonum_counter;
    let listnum_counter = &mut *ctx.listnum_counter;
    let field_bookmarks = &mut *ctx.field_bookmarks;
    let form_field_index = &mut *ctx.form_field_index;
    let ref_field_index = &mut *ctx.ref_field_index;
    let note_ref_field_index = &mut *ctx.note_ref_field_index;
    let section_field_index = &mut *ctx.section_field_index;
    let mut text = String::new();
    let mut current = Vec::new();
    let mut result_starts: Vec<Option<usize>> = Vec::new();
    let mut depth = 1usize;
    let mut xml_depth = 0usize;
    let mut alternate_content_stack = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    skip_subtree(r);
                    continue;
                }
                if matches!(name, b"del" | b"moveFrom") {
                    skip_subtree(r);
                    continue;
                }
                let mut consumed_element = false;
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.push(AlternateContentBranchState {
                            branch_depth: xml_depth + 1,
                            took_branch: false,
                        });
                    }
                    b"fldSimple" => {
                        let nested_instruction = attr_local(&e, b"instr");
                        if push_tc_entry_from_instruction(
                            nested_instruction.clone(),
                            bookmarks.as_slice(),
                            entries,
                        ) || push_computed_toc_sequence_result(
                            nested_instruction.as_deref(),
                            &mut TocEmit {
                                entries,
                                sequence_counters,
                                sequence_heading_scopes,
                                sequence_field_index,
                                sequence_headings,
                                sequence_identifiers,
                            },
                            &mut text,
                        ) {
                            skip_element(r, b"fldSimple");
                        } else {
                            text.push_str(&read_toc_simple_field_result(
                                r,
                                nested_instruction.as_deref(),
                                active_bookmarks,
                                bookmarks,
                                &mut TocEmit {
                                    entries,
                                    sequence_counters,
                                    sequence_heading_scopes,
                                    sequence_field_index,
                                    sequence_headings,
                                    sequence_identifiers,
                                },
                                &mut TocSourceCtx {
                                    ref_targets,
                                    note_refs,
                                    sections,
                                    legacy_forms,
                                    properties,
                                    autonum_counter,
                                    listnum_counter,
                                    field_bookmarks,
                                    form_field_index,
                                    ref_field_index,
                                    note_ref_field_index,
                                    section_field_index,
                                },
                            ));
                        }
                        consumed_element = true;
                    }
                    b"fldChar" => {
                        apply_toc_fld_char(
                            &e,
                            &mut current,
                            &mut result_starts,
                            bookmarks,
                            &mut text,
                            &mut TocEmit {
                                entries,
                                sequence_counters,
                                sequence_heading_scopes,
                                sequence_field_index,
                                sequence_headings,
                                sequence_identifiers,
                            },
                            &mut TocSourceCtx {
                                ref_targets,
                                note_refs,
                                sections,
                                legacy_forms,
                                properties,
                                autonum_counter,
                                listnum_counter,
                                field_bookmarks,
                                form_field_index,
                                ref_field_index,
                                note_ref_field_index,
                                section_field_index,
                            },
                        );
                    }
                    b"instrText" => {
                        let field_text = read_text(r);
                        consumed_element = true;
                        if let Some(field) = current.last_mut() {
                            if field.phase == FieldPhase::Instruction {
                                field.instruction.push_str(&field_text);
                            }
                        }
                    }
                    b"t" => {
                        let run_text = read_text(r);
                        consumed_element = true;
                        if !toc_source_hidden_field_result(&current) {
                            text.push_str(&run_text);
                        }
                    }
                    // Not collapsed into the arm guard: the `_` catch-all below emits
                    // inline-marker text, so failing this guard must run the body's
                    // no-op, not fall through to marker handling.
                    b"sym" => {
                        if !toc_source_hidden_field_result(&current) {
                            if let Some(ch) = toc_symbol_char(&e) {
                                text.push(ch);
                            }
                        }
                    }
                    b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" => {
                        push_toc_source_marker_text(&mut text, name, &current);
                    }
                    b"bookmarkStart" => {
                        if let Some(name) = push_active_bookmark(active_bookmarks, &e) {
                            push_unique(bookmarks, name);
                        }
                    }
                    b"bookmarkEnd" => remove_active_bookmark(active_bookmarks, &e),
                    _ => {
                        if let Some(marker) = inline_marker_text(&e) {
                            if !toc_source_hidden_field_result(&current) {
                                text.push_str(marker);
                            }
                        }
                    }
                }
                if !consumed_element {
                    depth += 1;
                    xml_depth = xml_depth.saturating_add(1);
                }
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    continue;
                }
                match name {
                    b"fldSimple" => {
                        let nested_instruction = attr_local(&e, b"instr");
                        if !push_tc_entry_from_instruction(
                            nested_instruction.clone(),
                            bookmarks.as_slice(),
                            entries,
                        ) && !push_computed_toc_sequence_result(
                            nested_instruction.as_deref(),
                            &mut TocEmit {
                                entries,
                                sequence_counters,
                                sequence_heading_scopes,
                                sequence_field_index,
                                sequence_headings,
                                sequence_identifiers,
                            },
                            &mut text,
                        ) {
                            if let Some(computed) = computed_toc_source_field_result(
                                nested_instruction.as_deref(),
                                &mut TocSourceCtx {
                                    ref_targets,
                                    note_refs,
                                    sections,
                                    legacy_forms,
                                    properties,
                                    autonum_counter,
                                    listnum_counter,
                                    field_bookmarks,
                                    form_field_index,
                                    ref_field_index,
                                    note_ref_field_index,
                                    section_field_index,
                                },
                                "",
                            ) {
                                text.push_str(&computed);
                            }
                        }
                    }
                    b"fldChar" => {
                        apply_toc_fld_char(
                            &e,
                            &mut current,
                            &mut result_starts,
                            bookmarks,
                            &mut text,
                            &mut TocEmit {
                                entries,
                                sequence_counters,
                                sequence_heading_scopes,
                                sequence_field_index,
                                sequence_headings,
                                sequence_identifiers,
                            },
                            &mut TocSourceCtx {
                                ref_targets,
                                note_refs,
                                sections,
                                legacy_forms,
                                properties,
                                autonum_counter,
                                listnum_counter,
                                field_bookmarks,
                                form_field_index,
                                ref_field_index,
                                note_ref_field_index,
                                section_field_index,
                            },
                        );
                    }
                    // Not collapsed into the arm guard: the `_` catch-all below emits
                    // inline-marker text, so failing this guard must run the body's
                    // no-op, not fall through to marker handling.
                    b"sym" => {
                        if !toc_source_hidden_field_result(&current) {
                            if let Some(ch) = toc_symbol_char(&e) {
                                text.push(ch);
                            }
                        }
                    }
                    b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" => {
                        push_toc_source_marker_text(&mut text, name, &current);
                    }
                    b"bookmarkStart" => {
                        if let Some(name) = push_active_bookmark(active_bookmarks, &e) {
                            push_unique(bookmarks, name);
                        }
                    }
                    b"bookmarkEnd" => remove_active_bookmark(active_bookmarks, &e),
                    _ => {
                        if let Some(marker) = inline_marker_text(&e) {
                            if !toc_source_hidden_field_result(&current) {
                                text.push_str(marker);
                            }
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"fldSimple" && depth == 1 {
                    break;
                }
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                depth = depth.saturating_sub(1);
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let cached_text = normalize_toc_text(&text);
    computed_toc_source_field_result(
        instruction,
        &mut TocSourceCtx {
            ref_targets,
            note_refs,
            sections,
            legacy_forms,
            properties,
            autonum_counter,
            listnum_counter,
            field_bookmarks,
            form_field_index,
            ref_field_index,
            note_ref_field_index,
            section_field_index,
        },
        &cached_text,
    )
    .unwrap_or(text)
}

fn toc_source_hidden_field_result(current: &[ComplexField]) -> bool {
    current.iter().rev().any(|field| {
        field.phase == FieldPhase::Result
            && (toc_entry_field_syntax(&field.instruction).is_some()
                || seq_identifier_from_instruction(Some(&field.instruction)).is_some())
    })
}

fn push_toc_source_marker_text(text: &mut String, name: &[u8], current: &[ComplexField]) {
    if toc_source_hidden_field_result(current) {
        return;
    }
    match name {
        b"tab" | b"br" | b"cr" => text.push(' '),
        b"noBreakHyphen" => text.push('-'),
        b"softHyphen" => text.push('\u{00ad}'),
        _ => {}
    }
}

fn toc_symbol_char(e: &BytesStart<'_>) -> Option<char> {
    let value = attr_local_trimmed(e, b"char")?;
    let font = attr_local_trimmed(e, b"font");
    computed_run_symbol_char(font.as_deref(), &value)
}

fn apply_toc_fld_char(
    e: &BytesStart<'_>,
    current: &mut Vec<ComplexField>,
    result_starts: &mut Vec<Option<usize>>,
    bookmarks: &[String],
    text: &mut String,
    emit: &mut TocEmit<'_>,
    ctx: &mut TocSourceCtx<'_>,
) {
    let entries = &mut *emit.entries;
    let sequence_counters = &mut *emit.sequence_counters;
    let sequence_heading_scopes = &mut *emit.sequence_heading_scopes;
    let sequence_field_index = &mut *emit.sequence_field_index;
    let sequence_headings = emit.sequence_headings;
    let sequence_identifiers = &mut *emit.sequence_identifiers;
    // Track where each complex field's result text begins in `text`, so a
    // deterministic source-field result can overwrite the leaked cached runs on
    // `end` — mirroring the STYLEREF/REF/table-formula complex-field sinks.
    let fld_type = field_char_type(e);
    match fld_type.as_deref() {
        Some("begin") => result_starts.push(None),
        Some("separate") => {
            if let Some(start) = result_starts.last_mut() {
                *start = Some(text.len());
            }
        }
        _ => {}
    }
    let mut computed_source = None;
    apply_complex_field_scan_fld_char(e, current, |field| {
        if push_tc_entry(&field.instruction, bookmarks, entries)
            || push_computed_toc_sequence_result(
                Some(&field.instruction),
                &mut TocEmit {
                    entries,
                    sequence_counters,
                    sequence_heading_scopes,
                    sequence_field_index,
                    sequence_headings,
                    sequence_identifiers,
                },
                text,
            )
        {
            return;
        }
        if let Some(identifier) = seq_identifier_from_instruction(Some(&field.instruction)) {
            push_unique(sequence_identifiers, identifier);
            return;
        }
        let current_result = result_starts
            .last()
            .and_then(|start| *start)
            .and_then(|start| text.get(start..))
            .map(normalize_toc_text)
            .unwrap_or_default();
        computed_source =
            computed_toc_source_field_result(Some(&field.instruction), ctx, &current_result);
    });
    if fld_type.as_deref() == Some("end") {
        if let (Some(Some(start)), Some(computed)) = (result_starts.pop(), computed_source) {
            if start <= text.len() {
                text.replace_range(start.., &computed);
            }
        }
    }
}

fn push_tc_entry_from_instruction(
    instruction: Option<String>,
    bookmarks: &[String],
    entries: &mut Vec<TocEntry>,
) -> bool {
    let Some(instruction) = instruction else {
        return false;
    };
    if !is_tc_instruction(&instruction) {
        return false;
    }
    push_tc_entry(&instruction, bookmarks, entries);
    true
}

fn push_tc_entry(instruction: &str, bookmarks: &[String], entries: &mut Vec<TocEntry>) -> bool {
    if let Some(spec) = toc_entry_field_syntax(instruction) {
        entries.push(TocEntry {
            level: spec.level,
            text: apply_field_text_format(spec.text, spec.text_format),
            source: TocEntrySource::TcField,
            tc_type: spec.entry_type,
            sequence_identifier: None,
            sequence_caption_text: None,
            bookmarks: bookmarks.to_vec(),
            style_id: None,
            style_name: None,
        });
        return true;
    }
    false
}

pub(crate) fn computed_toc_entry_result(instruction: &str) -> Option<String> {
    toc_entry_field_syntax(instruction)?;
    Some(String::new())
}

pub(crate) fn supports_toc_entry_field_syntax(instruction: &str) -> bool {
    toc_entry_field_syntax(instruction).is_some()
}

fn is_tc_instruction(instruction: &str) -> bool {
    instruction_parts(instruction)
        .first()
        .is_some_and(|kind| kind.eq_ignore_ascii_case("TC"))
}

fn push_computed_toc_sequence_result(
    instruction: Option<&str>,
    emit: &mut TocEmit<'_>,
    text: &mut String,
) -> bool {
    let Some(instruction) = instruction else {
        return false;
    };
    if FieldKind::from_instruction(instruction) != FieldKind::Sequence {
        return false;
    }
    let heading_scope = emit
        .sequence_headings
        .field_scope(*emit.sequence_field_index);
    *emit.sequence_field_index += 1;
    let Some(identifier) = seq_identifier_from_instruction(Some(instruction)) else {
        return false;
    };
    let Some(result) = computed_sequence_result_with_heading_scope(
        instruction,
        emit.sequence_counters,
        heading_scope,
        emit.sequence_heading_scopes,
    ) else {
        return false;
    };
    push_unique(emit.sequence_identifiers, identifier);
    text.push_str(&result);
    true
}

fn computed_toc_source_field_result(
    instruction: Option<&str>,
    ctx: &mut TocSourceCtx<'_>,
    current_result: &str,
) -> Option<String> {
    let instruction = normalize_instruction(instruction?);
    if is_tc_instruction(&instruction)
        || seq_identifier_from_instruction(Some(&instruction)).is_some()
    {
        return None;
    }
    let ref_targets = ctx.ref_targets;
    let note_refs = ctx.note_refs;
    let sections = ctx.sections;
    let legacy_forms = ctx.legacy_forms;
    let properties = ctx.properties;
    let autonum_counter = &mut *ctx.autonum_counter;
    let listnum_counter = &mut *ctx.listnum_counter;
    let field_bookmarks = &mut *ctx.field_bookmarks;
    let form_field_index = &mut *ctx.form_field_index;
    let ref_field_index = &mut *ctx.ref_field_index;
    let note_ref_field_index = &mut *ctx.note_ref_field_index;
    let section_field_index = &mut *ctx.section_field_index;
    match FieldKind::from_instruction(&instruction) {
        FieldKind::Dynamic(kind) if kind == "SET" => {
            return super::computed_set_result(&instruction, field_bookmarks);
        }
        FieldKind::Dynamic(kind) if kind == "ASK" => {
            return super::computed_ask_result(&instruction, field_bookmarks);
        }
        FieldKind::FormField(_) => {
            let index = *form_field_index;
            *form_field_index += 1;
            return computed_legacy_form_result(&instruction, current_result, legacy_forms, index);
        }
        FieldKind::DocumentStructure(kind)
            if (kind == "SECTION" || kind == "SECTIONPAGES")
                && is_section_field_instruction(&instruction) =>
        {
            let position = sections.field_position(*section_field_index);
            *section_field_index += 1;
            return computed_section_result(&instruction, position);
        }
        _ => {}
    }
    let note_ref_field_position =
        toc_source_note_ref_field_position(&instruction, note_refs, ref_field_index);
    let direct_note_ref_field_position =
        note_ref_source_field_position(&instruction, note_refs, note_ref_field_index);
    if let Some(text) = computed_toc_source_ref_result(&instruction, ref_targets, field_bookmarks) {
        return Some(text);
    }
    computed_toc_source_ref_note_reference_result(&instruction, note_refs, note_ref_field_position)
        .or_else(|| {
            computed_note_ref_result(&instruction, note_refs, direct_note_ref_field_position)
        })
        .or_else(|| computed_numbering_result(&instruction, autonum_counter))
        .or_else(|| computed_listnum_result(&instruction, listnum_counter))
        .or_else(|| {
            super::computed_formula_result_with_bookmark_context(
                &instruction,
                ref_targets,
                field_bookmarks,
            )
        })
        .or_else(|| super::computed_quote_result(&instruction))
        .or_else(|| super::computed_fill_in_result(&instruction))
        .or_else(|| {
            super::computed_if_compare_result_with_bookmark_context(
                &instruction,
                ref_targets,
                field_bookmarks,
            )
        })
        .or_else(|| {
            super::computed_merge_control_result_with_bookmark_context(
                &instruction,
                ref_targets,
                field_bookmarks,
            )
        })
        .or_else(|| computed_display_result(&instruction))
        .or_else(|| computed_action_result(&instruction))
        .or_else(|| computed_revision_number_result(&instruction, properties.core))
        .or_else(|| {
            computed_document_info_result(
                &instruction,
                properties.core,
                properties.custom,
                properties.variables,
                properties.extended,
                properties.file_size_bytes,
            )
        })
        .or_else(|| computed_reference_index_result(&instruction))
}

fn computed_toc_source_ref_note_reference_result(
    instruction: &str,
    note_refs: &NoteRefContext,
    note_ref_field_position: Option<NoteRefFieldPosition>,
) -> Option<String> {
    let spec =
        ref_instruction(instruction).or_else(|| direct_bookmark_ref_instruction(instruction))?;
    if !spec.note_reference
        || spec.sequence_separator
        || spec.relative
        || spec.paragraph_number
        || spec.full_context_number
        || spec.relative_context_number
    {
        return None;
    }
    let number = note_refs.ref_note_number(&spec.target, note_ref_field_position)?;
    let text = format_page_number(number, spec.number_format)?;
    Some(apply_field_text_format(text, spec.text_format))
}

fn toc_source_note_ref_field_position(
    instruction: &str,
    note_refs: &NoteRefContext,
    ref_field_index: &mut usize,
) -> Option<NoteRefFieldPosition> {
    if !is_ref_position_field_instruction(instruction) {
        return None;
    }
    let position = note_refs.ref_field_position(*ref_field_index);
    *ref_field_index += 1;
    position
}

// Plain document/field bookmark REF only, mirroring computed_style_ref_source_ref_result:
// note/relative/paragraph-number/context REFs stay cached (return None) because
// those are layout-derived and out of scope for source-text splicing.
fn computed_toc_source_ref_result(
    instruction: &str,
    document_bookmarks: &HashMap<String, String>,
    field_bookmarks: &HashMap<String, String>,
) -> Option<String> {
    let spec = ref_or_unknown_direct_bookmark_instruction(instruction)?;
    if spec.note_reference
        || spec.relative
        || spec.paragraph_number
        || spec.full_context_number
        || spec.relative_context_number
    {
        return None;
    }
    if spec.sequence_separator {
        spec.sequence_separator_value.as_deref()?;
    }
    let text = field_bookmarks
        .get(&spec.target)
        .or_else(|| document_bookmarks.get(&spec.target))?;
    let text = computed_ref_bookmark_text_result(
        text,
        spec.number_format,
        spec.number_picture.as_deref(),
    )?;
    Some(apply_field_text_format(text, spec.text_format))
}

pub(crate) fn seq_identifier_from_instruction(instruction: Option<&str>) -> Option<String> {
    sequence_instruction_with_reset_policy(instruction?, false, true)
        .map(|instruction| instruction.identifier)
}

fn caption_text_without_label(text: &str, identifier: &str) -> Option<String> {
    let rest = strip_caption_identifier_prefix(text.trim_start(), identifier)?.trim_start();
    if rest.is_empty() {
        return None;
    }
    if let Some((_, caption)) = rest.split_once(':') {
        return nonempty_caption_text(caption);
    }
    let (_, caption) = rest.split_once(char::is_whitespace)?;
    nonempty_caption_text(
        caption.trim_start_matches(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '-' | '.')),
    )
}

fn strip_caption_identifier_prefix<'a>(text: &'a str, identifier: &str) -> Option<&'a str> {
    let mut chars = text.chars();
    for expected in identifier.chars() {
        let actual = chars.next()?;
        if actual != expected && !actual.eq_ignore_ascii_case(&expected) {
            return None;
        }
    }
    Some(chars.as_str())
}

fn nonempty_caption_text(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn active_bookmark_names(active_bookmarks: &[(String, String)]) -> Vec<String> {
    let mut names = Vec::new();
    for (_, name) in active_bookmarks {
        push_unique(&mut names, name.clone());
    }
    names
}

fn push_active_bookmark(
    active_bookmarks: &mut Vec<(String, String)>,
    e: &BytesStart<'_>,
) -> Option<String> {
    let (id, name) = bookmark_start(e)?;
    if !active_bookmarks
        .iter()
        .any(|(active_id, _)| active_id == &id)
    {
        active_bookmarks.push((id, name.clone()));
    }
    Some(name)
}

fn remove_active_bookmark(active_bookmarks: &mut Vec<(String, String)>, e: &BytesStart<'_>) {
    if let Some(id) = bookmark_end_id(e) {
        active_bookmarks.retain(|(active_id, _)| active_id != &id);
    }
}

fn read_toc_ppr(r: &mut Xml<'_>, style_id: &mut Option<String>, outline: &mut Option<u8>) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"pPrChange" => skip_subtree(r),
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"pStyle" => *style_id = attr_local_trimmed(&e, b"val"),
                b"outlineLvl" => *outline = attr_u8(&e, b"val"),
                _ => {}
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"pStyle" => *style_id = attr_local_trimmed(&e, b"val"),
                b"outlineLvl" => *outline = attr_u8(&e, b"val"),
                _ => {}
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"pPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

pub(crate) fn computed_toc_result(
    instruction: &str,
    toc_entries: &[TocEntry],
    bookmark_names: &HashSet<String>,
) -> Option<String> {
    let spec = toc_spec(instruction)?;
    let lines: Vec<_> = toc_entries
        .iter()
        .filter_map(|entry| toc_entry_for_spec(entry, &spec))
        .map(|(level, text)| {
            format!(
                "{}{}",
                "  ".repeat(level.saturating_sub(spec.start) as usize),
                text
            )
        })
        .collect();
    if lines.is_empty() {
        if spec
            .bookmark
            .as_ref()
            .is_some_and(|bookmark| bookmark_names.contains(bookmark))
        {
            Some(apply_field_text_format(String::new(), spec.text_format))
        } else {
            None
        }
    } else {
        Some(apply_field_text_format(lines.join("\n"), spec.text_format))
    }
}

fn toc_entry_for_spec<'a>(entry: &'a TocEntry, spec: &TocSpec) -> Option<(u8, &'a str)> {
    if !spec
        .bookmark
        .as_ref()
        .is_none_or(|bookmark| entry.bookmarks.iter().any(|name| name == bookmark))
    {
        return None;
    }
    if entry.source == TocEntrySource::TcField {
        if spec.tc_filter.is_none() && spec.tc_level_range.is_none() {
            return None;
        }
        if let Some(TcFilter::EntryType(expected)) = &spec.tc_filter {
            if !entry
                .tc_type
                .as_ref()
                .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
            {
                return None;
            }
        }
        let (start, end) = spec.tc_level_range.unwrap_or((1, 9));
        return (start..=end)
            .contains(&entry.level)
            .then_some((entry.level, entry.text.as_str()));
    }
    if entry.source == TocEntrySource::SequenceField {
        let filter = spec.sequence_filter.as_ref()?;
        if !entry
            .sequence_identifier
            .as_ref()
            .is_some_and(|actual| actual.eq_ignore_ascii_case(filter.identifier()))
        {
            return None;
        }
        let text = match filter {
            TocSequenceFilter::FullCaption(_) => entry.text.as_str(),
            TocSequenceFilter::CaptionText(_) => entry.sequence_caption_text.as_deref()?,
        };
        return Some((1, text));
    }
    if spec.include_standard
        && (spec.start..=spec.end).contains(&entry.level)
        && (!spec.outline_only || entry.source == TocEntrySource::OutlineLevel)
    {
        return Some((entry.level, entry.text.as_str()));
    }
    spec.custom_styles
        .iter()
        .find(|style| toc_entry_matches_style(entry, &style.name))
        .map(|style| (style.level, entry.text.as_str()))
}

fn toc_entry_matches_style(entry: &TocEntry, name: &str) -> bool {
    entry
        .style_id
        .as_ref()
        .is_some_and(|style_id| style_id.eq_ignore_ascii_case(name))
        || entry
            .style_name
            .as_ref()
            .is_some_and(|style_name| style_name.eq_ignore_ascii_case(name))
}

pub(crate) fn toc_spec(instruction: &str) -> Option<TocSpec> {
    toc_field_syntax(instruction)
}
