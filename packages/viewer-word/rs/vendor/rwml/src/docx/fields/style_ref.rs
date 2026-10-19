use super::reference::{
    computed_ref_bookmark_text_result, direct_bookmark_ref_instruction,
    is_ref_position_field_instruction, ref_instruction, ref_or_unknown_direct_bookmark_instruction,
};
use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct StyleRefContext {
    entries: Vec<StyleRefEntry>,
    field_positions: Vec<StyleRefFieldPosition>,
}

impl StyleRefContext {
    pub(crate) fn field_position(&self, index: usize) -> Option<StyleRefFieldPosition> {
        self.field_positions.get(index).cloned()
    }

    fn entry_for_style(
        &self,
        style_identifier: &str,
        field_order: usize,
    ) -> Option<&StyleRefEntry> {
        self.entries
            .iter()
            .rev()
            .find(|entry| {
                entry.order < field_order && style_ref_entry_matches(entry, style_identifier)
            })
            .or_else(|| {
                self.entries.iter().find(|entry| {
                    entry.order > field_order && style_ref_entry_matches(entry, style_identifier)
                })
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StyleRefEntry {
    style_id: Option<String>,
    style_name: Option<String>,
    text: String,
    number_text: Option<String>,
    number_numeric: Option<String>,
    number_full_context: Option<String>,
    order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StyleRefFieldPosition {
    order: usize,
    number_context: Option<String>,
}

#[derive(Debug, Clone)]
struct StyleRefScanField {
    instruction: String,
    phase: FieldPhase,
    paragraph_result_start: Option<usize>,
    entry_result_start: Option<usize>,
}

#[derive(Debug, Clone)]
struct StyleRefParagraphNumber {
    text: String,
    numeric: Option<String>,
    full_context: Option<String>,
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn style_ref_context(
    xml: &str,
    styles: &Styles,
    numbering: &Numbering,
    document_bookmarks: &HashMap<String, String>,
) -> StyleRefContext {
    let core_properties = CoreProperties::default();
    let empty_properties = HashMap::new();
    let note_refs = NoteRefContext::empty();
    let sections = SectionContext::empty();
    style_ref_context_with_properties(
        xml,
        styles,
        numbering,
        &StyleRefResolutionSources {
            document_bookmarks,
            note_refs: &note_refs,
            sections: &sections,
        },
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

/// Read-only field-resolution sources threaded into the style-ref scan. Bundled to
/// keep [`style_ref_context_with_properties`] within argument limits; this scan runs
/// *before* any `StyleRefContext` exists, so it cannot take the full
/// [`FieldResolutionContext`](super::FieldResolutionContext).
pub(crate) struct StyleRefResolutionSources<'a> {
    pub(crate) document_bookmarks: &'a HashMap<String, String>,
    pub(crate) note_refs: &'a NoteRefContext,
    pub(crate) sections: &'a SectionContext,
}

pub(crate) fn style_ref_context_with_properties(
    xml: &str,
    styles: &Styles,
    numbering: &Numbering,
    sources: &StyleRefResolutionSources<'_>,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
) -> StyleRefContext {
    let document_bookmarks = sources.document_bookmarks;
    let note_refs = sources.note_refs;
    let sections = sources.sections;
    let legacy_forms = legacy_form_context(xml, preserve_legacy_form_cache);
    let mut r = Reader::from_str(xml);
    let mut entries = Vec::new();
    let mut field_positions = Vec::new();
    let mut counters: HashMap<String, [u32; 9]> = HashMap::new();
    let mut sequence_counters = HashMap::new();
    let mut autonum_counter = 0i64;
    let mut listnum_counter = 0i64;
    let mut field_bookmarks = HashMap::new();
    let mut form_field_index = 0usize;
    let mut ref_field_index = 0usize;
    let mut note_ref_field_index = 0usize;
    let mut section_field_index = 0usize;
    let mut next_order = 0usize;
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
                        read_style_ref_paragraph(
                            &mut r,
                            styles,
                            numbering,
                            &mut counters,
                            &mut StyleRefEmit {
                                entries: &mut entries,
                                field_positions: &mut field_positions,
                                next_order: &mut next_order,
                            },
                            &mut StyleRefSourceCtx {
                                document_bookmarks,
                                note_refs,
                                sections,
                                legacy_forms: &legacy_forms,
                                properties,
                                sequence_counters: &mut sequence_counters,
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
    StyleRefContext {
        entries,
        field_positions,
    }
}

fn read_style_ref_paragraph(
    r: &mut Xml<'_>,
    styles: &Styles,
    numbering: &Numbering,
    counters: &mut HashMap<String, [u32; 9]>,
    emit: &mut StyleRefEmit<'_>,
    src: &mut StyleRefSourceCtx<'_>,
) {
    let entries = &mut *emit.entries;
    let field_positions = &mut *emit.field_positions;
    let next_order = &mut *emit.next_order;
    let document_bookmarks = src.document_bookmarks;
    let note_refs = src.note_refs;
    let sections = src.sections;
    let legacy_forms = src.legacy_forms;
    let properties = src.properties;
    let sequence_counters = &mut *src.sequence_counters;
    let autonum_counter = &mut *src.autonum_counter;
    let listnum_counter = &mut *src.listnum_counter;
    let field_bookmarks = &mut *src.field_bookmarks;
    let form_field_index = &mut *src.form_field_index;
    let ref_field_index = &mut *src.ref_field_index;
    let note_ref_field_index = &mut *src.note_ref_field_index;
    let section_field_index = &mut *src.section_field_index;
    let mut style_id = None;
    let mut num_id = None;
    let mut ilvl = 0u8;
    let mut number = None;
    let mut text = String::new();
    let mut current = Vec::new();
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
                        read_style_ref_ppr(r, &mut style_id, &mut num_id, &mut ilvl);
                        consumed_element = true;
                    }
                    b"r" => {
                        ensure_style_ref_paragraph_number(
                            &mut number,
                            numbering,
                            counters,
                            num_id.as_deref(),
                            ilvl,
                        );
                        read_style_ref_run(
                            r,
                            StyleRefRunScan {
                                styles,
                                entries,
                                field_positions,
                                next_order,
                                paragraph_number: &number,
                                current: &mut current,
                                paragraph_text: &mut text,
                                document_bookmarks,
                                note_refs,
                                sections,
                                sequence_counters,
                                autonum_counter,
                                listnum_counter,
                                field_bookmarks,
                                legacy_forms,
                                form_field_index,
                                ref_field_index,
                                note_ref_field_index,
                                section_field_index,
                                properties,
                            },
                        );
                        consumed_element = true;
                    }
                    b"fldSimple" => {
                        ensure_style_ref_paragraph_number(
                            &mut number,
                            numbering,
                            counters,
                            num_id.as_deref(),
                            ilvl,
                        );
                        let instruction = attr_local(&e, b"instr");
                        let is_style_ref = instruction
                            .as_deref()
                            .is_some_and(is_style_ref_field_instruction);
                        record_style_ref_field(
                            instruction.as_deref(),
                            field_positions,
                            next_order,
                            &number,
                        );
                        if is_style_ref {
                            skip_element(r, b"fldSimple");
                        } else {
                            text.push_str(&read_style_ref_simple_field_result(
                                r,
                                instruction.as_deref(),
                                styles,
                                &number,
                                &mut StyleRefEmit {
                                    entries,
                                    field_positions,
                                    next_order,
                                },
                                &mut StyleRefSourceCtx {
                                    document_bookmarks,
                                    note_refs,
                                    sections,
                                    legacy_forms,
                                    properties,
                                    sequence_counters,
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
                    b"t" => {
                        let run_text = read_text(r);
                        consumed_element = true;
                        if !style_ref_suppresses_source_text(&current) {
                            text.push_str(&run_text);
                        }
                    }
                    b"sym" => {
                        if !style_ref_suppresses_source_text(&current) {
                            if let Some(symbol) = style_ref_symbol_text(&e) {
                                text.push_str(&symbol);
                            }
                        }
                        skip_subtree(r);
                        consumed_element = true;
                    }
                    _ => {
                        if let Some(marker) = inline_marker_text(&e) {
                            if !style_ref_suppresses_source_text(&current) {
                                text.push_str(marker);
                            }
                        }
                    }
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
                        ensure_style_ref_paragraph_number(
                            &mut number,
                            numbering,
                            counters,
                            num_id.as_deref(),
                            ilvl,
                        );
                        let instruction = attr_local(&e, b"instr");
                        record_style_ref_field(
                            instruction.as_deref(),
                            field_positions,
                            next_order,
                            &number,
                        );
                        if !instruction
                            .as_deref()
                            .is_some_and(is_style_ref_field_instruction)
                        {
                            if let Some(computed) = computed_style_ref_source_field_result(
                                instruction.as_deref(),
                                &mut StyleRefSourceCtx {
                                    document_bookmarks,
                                    note_refs,
                                    sections,
                                    legacy_forms,
                                    properties,
                                    sequence_counters,
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
                    b"sym" => {
                        if !style_ref_suppresses_source_text(&current) {
                            if let Some(symbol) = style_ref_symbol_text(&e) {
                                text.push_str(&symbol);
                            }
                        }
                    }
                    _ => {
                        if let Some(marker) = inline_marker_text(&e) {
                            if !style_ref_suppresses_source_text(&current) {
                                text.push_str(marker);
                            }
                        }
                    }
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
    ensure_style_ref_paragraph_number(&mut number, numbering, counters, num_id.as_deref(), ilvl);
    if !text.is_empty() && (style_id.is_some() || style_name.is_some()) {
        entries.push(StyleRefEntry {
            style_id,
            style_name,
            text,
            number_text: number.as_ref().map(|number| number.text.clone()),
            number_numeric: number.as_ref().and_then(|number| number.numeric.clone()),
            number_full_context: number.and_then(|number| number.full_context),
            order: take_style_ref_order(next_order),
        });
    }
}

struct StyleRefRunScan<'a> {
    styles: &'a Styles,
    entries: &'a mut Vec<StyleRefEntry>,
    field_positions: &'a mut Vec<StyleRefFieldPosition>,
    next_order: &'a mut usize,
    paragraph_number: &'a Option<StyleRefParagraphNumber>,
    current: &'a mut Vec<StyleRefScanField>,
    paragraph_text: &'a mut String,
    document_bookmarks: &'a HashMap<String, String>,
    note_refs: &'a NoteRefContext,
    sections: &'a SectionContext,
    sequence_counters: &'a mut HashMap<String, i64>,
    autonum_counter: &'a mut i64,
    listnum_counter: &'a mut i64,
    field_bookmarks: &'a mut HashMap<String, String>,
    legacy_forms: &'a LegacyFormContext,
    form_field_index: &'a mut usize,
    ref_field_index: &'a mut usize,
    note_ref_field_index: &'a mut usize,
    section_field_index: &'a mut usize,
    properties: FieldDocumentProperties<'a>,
}

/// Style-ref entry/position accumulators threaded across the paragraph scan.
struct StyleRefEmit<'a> {
    entries: &'a mut Vec<StyleRefEntry>,
    field_positions: &'a mut Vec<StyleRefFieldPosition>,
    next_order: &'a mut usize,
}

/// Threaded source-field evaluation state: read-only document context plus the
/// mutable sequence/index accumulators that travel together through the scan.
struct StyleRefSourceCtx<'a> {
    document_bookmarks: &'a HashMap<String, String>,
    note_refs: &'a NoteRefContext,
    sections: &'a SectionContext,
    legacy_forms: &'a LegacyFormContext,
    properties: FieldDocumentProperties<'a>,
    sequence_counters: &'a mut HashMap<String, i64>,
    autonum_counter: &'a mut i64,
    listnum_counter: &'a mut i64,
    field_bookmarks: &'a mut HashMap<String, String>,
    form_field_index: &'a mut usize,
    ref_field_index: &'a mut usize,
    note_ref_field_index: &'a mut usize,
    section_field_index: &'a mut usize,
}

fn read_style_ref_run(r: &mut Xml<'_>, scan: StyleRefRunScan<'_>) {
    let StyleRefRunScan {
        styles,
        entries,
        field_positions,
        next_order,
        paragraph_number,
        current,
        paragraph_text,
        document_bookmarks,
        note_refs,
        sections,
        sequence_counters,
        autonum_counter,
        listnum_counter,
        field_bookmarks,
        legacy_forms,
        form_field_index,
        ref_field_index,
        note_ref_field_index,
        section_field_index,
        properties,
    } = scan;
    let mut run_style_id = None;
    let mut run_text = String::new();
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
                if matches!(name, b"del" | b"moveFrom" | b"rPrChange") {
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
                    b"rStyle" => {
                        run_style_id = attr_local_trimmed(&e, b"val");
                    }
                    b"fldChar" => {
                        apply_style_ref_scan_fld_char(
                            &e,
                            current,
                            paragraph_number,
                            paragraph_text,
                            &mut StyleRefEmit {
                                entries,
                                field_positions,
                                next_order,
                            },
                            &mut StyleRefSourceCtx {
                                document_bookmarks,
                                note_refs,
                                sections,
                                legacy_forms,
                                properties,
                                sequence_counters,
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
                    b"fldSimple"
                        if attr_local(&e, b"instr")
                            .as_deref()
                            .is_some_and(is_style_ref_field_instruction) =>
                    {
                        record_style_ref_field(
                            attr_local(&e, b"instr").as_deref(),
                            field_positions,
                            next_order,
                            paragraph_number,
                        );
                        skip_element(r, b"fldSimple");
                        consumed_element = true;
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
                        let text = read_text(r);
                        consumed_element = true;
                        if !style_ref_suppresses_source_text(current) {
                            paragraph_text.push_str(&text);
                            run_text.push_str(&text);
                        }
                    }
                    b"sym" => {
                        if !style_ref_suppresses_source_text(current) {
                            if let Some(symbol) = style_ref_symbol_text(&e) {
                                paragraph_text.push_str(&symbol);
                                run_text.push_str(&symbol);
                            }
                        }
                        skip_subtree(r);
                        consumed_element = true;
                    }
                    _ => {
                        if let Some(marker) = inline_marker_text(&e) {
                            if !style_ref_suppresses_source_text(current) {
                                paragraph_text.push_str(marker);
                                run_text.push_str(marker);
                            }
                        }
                    }
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
                    b"rStyle" => {
                        run_style_id = attr_local_trimmed(&e, b"val");
                    }
                    b"fldChar" => {
                        apply_style_ref_scan_fld_char(
                            &e,
                            current,
                            paragraph_number,
                            paragraph_text,
                            &mut StyleRefEmit {
                                entries,
                                field_positions,
                                next_order,
                            },
                            &mut StyleRefSourceCtx {
                                document_bookmarks,
                                note_refs,
                                sections,
                                legacy_forms,
                                properties,
                                sequence_counters,
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
                    b"fldSimple" => {
                        record_style_ref_field(
                            attr_local(&e, b"instr").as_deref(),
                            field_positions,
                            next_order,
                            paragraph_number,
                        );
                    }
                    b"sym" => {
                        if !style_ref_suppresses_source_text(current) {
                            if let Some(symbol) = style_ref_symbol_text(&e) {
                                paragraph_text.push_str(&symbol);
                                run_text.push_str(&symbol);
                            }
                        }
                    }
                    _ => {
                        if let Some(marker) = inline_marker_text(&e) {
                            if !style_ref_suppresses_source_text(current) {
                                paragraph_text.push_str(marker);
                                run_text.push_str(marker);
                            }
                        }
                    }
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"r" => break,
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
    let text = normalize_toc_text(&run_text);
    if !text.is_empty() && run_style_id.is_some() {
        let style_name = run_style_id
            .as_deref()
            .and_then(|style_id| styles.name(style_id))
            .map(str::to_string);
        entries.push(StyleRefEntry {
            style_id: run_style_id,
            style_name,
            text,
            number_text: paragraph_number.as_ref().map(|number| number.text.clone()),
            number_numeric: paragraph_number
                .as_ref()
                .and_then(|number| number.numeric.clone()),
            number_full_context: paragraph_number
                .as_ref()
                .and_then(|number| number.full_context.clone()),
            order: take_style_ref_order(next_order),
        });
    }
}

fn read_style_ref_simple_field_result(
    r: &mut Xml<'_>,
    instruction: Option<&str>,
    styles: &Styles,
    paragraph_number: &Option<StyleRefParagraphNumber>,
    emit: &mut StyleRefEmit<'_>,
    src: &mut StyleRefSourceCtx<'_>,
) -> String {
    let entries = &mut *emit.entries;
    let field_positions = &mut *emit.field_positions;
    let next_order = &mut *emit.next_order;
    let document_bookmarks = src.document_bookmarks;
    let note_refs = src.note_refs;
    let sections = src.sections;
    let legacy_forms = src.legacy_forms;
    let properties = src.properties;
    let sequence_counters = &mut *src.sequence_counters;
    let autonum_counter = &mut *src.autonum_counter;
    let listnum_counter = &mut *src.listnum_counter;
    let field_bookmarks = &mut *src.field_bookmarks;
    let form_field_index = &mut *src.form_field_index;
    let ref_field_index = &mut *src.ref_field_index;
    let note_ref_field_index = &mut *src.note_ref_field_index;
    let section_field_index = &mut *src.section_field_index;
    let mut text = String::new();
    let mut current = Vec::new();
    let entry_start = entries.len();
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
                    b"r" => {
                        read_style_ref_run(
                            r,
                            StyleRefRunScan {
                                styles,
                                entries,
                                field_positions,
                                next_order,
                                paragraph_number,
                                current: &mut current,
                                paragraph_text: &mut text,
                                document_bookmarks,
                                note_refs,
                                sections,
                                sequence_counters,
                                autonum_counter,
                                listnum_counter,
                                field_bookmarks,
                                legacy_forms,
                                form_field_index,
                                ref_field_index,
                                note_ref_field_index,
                                section_field_index,
                                properties,
                            },
                        );
                        consumed_element = true;
                    }
                    b"fldSimple"
                        if attr_local(&e, b"instr")
                            .as_deref()
                            .is_some_and(is_style_ref_field_instruction) =>
                    {
                        skip_element(r, b"fldSimple");
                        consumed_element = true;
                    }
                    b"fldSimple" => {
                        let nested_instruction = attr_local(&e, b"instr");
                        text.push_str(&read_style_ref_simple_field_result(
                            r,
                            nested_instruction.as_deref(),
                            styles,
                            paragraph_number,
                            &mut StyleRefEmit {
                                entries,
                                field_positions,
                                next_order,
                            },
                            &mut StyleRefSourceCtx {
                                document_bookmarks,
                                note_refs,
                                sections,
                                legacy_forms,
                                properties,
                                sequence_counters,
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
                    b"instrText" => {
                        read_text(r);
                        consumed_element = true;
                    }
                    b"t" => {
                        text.push_str(&read_text(r));
                        consumed_element = true;
                    }
                    b"sym" => {
                        if let Some(symbol) = style_ref_symbol_text(&e) {
                            text.push_str(&symbol);
                        }
                        skip_subtree(r);
                        consumed_element = true;
                    }
                    _ => {
                        if let Some(marker) = inline_marker_text(&e) {
                            text.push_str(marker);
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
                    b"fldSimple"
                        if attr_local(&e, b"instr")
                            .as_deref()
                            .is_some_and(is_style_ref_field_instruction) => {}
                    b"fldSimple" => {
                        if let Some(computed) = computed_style_ref_source_field_result(
                            attr_local(&e, b"instr").as_deref(),
                            &mut StyleRefSourceCtx {
                                document_bookmarks,
                                note_refs,
                                sections,
                                legacy_forms,
                                properties,
                                sequence_counters,
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
                    b"sym" => {
                        if let Some(symbol) = style_ref_symbol_text(&e) {
                            text.push_str(&symbol);
                        }
                    }
                    _ => {
                        if let Some(marker) = inline_marker_text(&e) {
                            text.push_str(marker);
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
    if let Some(computed) = computed_style_ref_source_field_result(
        instruction,
        &mut StyleRefSourceCtx {
            document_bookmarks,
            note_refs,
            sections,
            legacy_forms,
            properties,
            sequence_counters,
            autonum_counter,
            listnum_counter,
            field_bookmarks,
            form_field_index,
            ref_field_index,
            note_ref_field_index,
            section_field_index,
        },
        &cached_text,
    ) {
        replace_style_ref_source_entries_with_computed(
            entries,
            entry_start,
            &cached_text,
            &computed,
        );
        return computed;
    }
    collapse_style_ref_source_entries_for_cached_text(entries, entry_start, &cached_text);
    text
}

fn replace_style_ref_source_entries_with_computed(
    entries: &mut Vec<StyleRefEntry>,
    entry_start: usize,
    cached_text: &str,
    computed: &str,
) {
    let Some(created) = entries.get(entry_start..) else {
        return;
    };
    if created.is_empty() {
        return;
    }
    if created.len() == 1 {
        if created[0].text == cached_text {
            entries[entry_start].text = computed.to_string();
        }
        return;
    }
    let first = &created[0];
    if !created
        .iter()
        .all(|entry| style_ref_entries_share_source_style(entry, first))
    {
        return;
    }
    let joined = created
        .iter()
        .map(|entry| entry.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if normalize_toc_text(&joined) != cached_text {
        return;
    }
    let mut replacement = first.clone();
    replacement.text = computed.to_string();
    entries.splice(entry_start.., [replacement]);
}

fn collapse_style_ref_source_entries_for_cached_text(
    entries: &mut Vec<StyleRefEntry>,
    entry_start: usize,
    cached_text: &str,
) {
    let Some(created) = entries.get(entry_start..) else {
        return;
    };
    if created.len() <= 1 {
        return;
    }
    let first = &created[0];
    if !created
        .iter()
        .all(|entry| style_ref_entries_share_source_style(entry, first))
    {
        return;
    }
    let joined = created
        .iter()
        .map(|entry| entry.text.as_str())
        .collect::<String>();
    if normalize_toc_text(&joined) != cached_text {
        return;
    }
    let mut replacement = first.clone();
    replacement.text = cached_text.to_string();
    entries.splice(entry_start.., [replacement]);
}

fn style_ref_entries_share_source_style(left: &StyleRefEntry, right: &StyleRefEntry) -> bool {
    left.style_id == right.style_id
        && left.style_name == right.style_name
        && left.number_text == right.number_text
        && left.number_numeric == right.number_numeric
        && left.number_full_context == right.number_full_context
}

fn computed_style_ref_source_field_result(
    instruction: Option<&str>,
    src: &mut StyleRefSourceCtx<'_>,
    current_result: &str,
) -> Option<String> {
    let instruction = normalize_instruction(instruction?);
    if is_style_ref_field_instruction(&instruction) {
        return None;
    }
    let document_bookmarks = src.document_bookmarks;
    let note_refs = src.note_refs;
    let sections = src.sections;
    let legacy_forms = src.legacy_forms;
    let properties = src.properties;
    let sequence_counters = &mut *src.sequence_counters;
    let autonum_counter = &mut *src.autonum_counter;
    let listnum_counter = &mut *src.listnum_counter;
    let field_bookmarks = &mut *src.field_bookmarks;
    let form_field_index = &mut *src.form_field_index;
    let ref_field_index = &mut *src.ref_field_index;
    let note_ref_field_index = &mut *src.note_ref_field_index;
    let section_field_index = &mut *src.section_field_index;
    match FieldKind::from_instruction(&instruction) {
        FieldKind::Dynamic(kind) if kind == "ASK" => {
            return computed_ask_result(&instruction, field_bookmarks);
        }
        FieldKind::Dynamic(kind) if kind == "SET" => {
            return computed_set_result(&instruction, field_bookmarks);
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
        style_ref_source_note_ref_field_position(&instruction, note_refs, ref_field_index);
    let direct_note_ref_field_position =
        note_ref_source_field_position(&instruction, note_refs, note_ref_field_index);
    if let Some(text) =
        computed_style_ref_source_ref_result(&instruction, document_bookmarks, field_bookmarks)
    {
        return Some(text);
    }
    computed_style_ref_source_ref_note_reference_result(
        &instruction,
        note_refs,
        note_ref_field_position,
    )
    .or_else(|| computed_note_ref_result(&instruction, note_refs, direct_note_ref_field_position))
    .or_else(|| computed_numbering_result(&instruction, autonum_counter))
    .or_else(|| computed_listnum_result(&instruction, listnum_counter))
    .or_else(|| computed_sequence_result(&instruction, sequence_counters))
    .or_else(|| computed_toc_entry_result(&instruction))
    .or_else(|| {
        computed_formula_result_with_bookmark_context(
            &instruction,
            document_bookmarks,
            field_bookmarks,
        )
    })
    .or_else(|| super::computed_dynamic_result_with_bookmarks(&instruction, field_bookmarks))
    .or_else(|| {
        computed_if_compare_result_with_bookmark_context(
            &instruction,
            document_bookmarks,
            field_bookmarks,
        )
    })
    .or_else(|| {
        computed_merge_control_result_with_bookmark_context(
            &instruction,
            document_bookmarks,
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

fn computed_style_ref_source_ref_note_reference_result(
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

fn style_ref_source_note_ref_field_position(
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

fn computed_style_ref_source_ref_result(
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

fn ensure_style_ref_paragraph_number(
    number: &mut Option<StyleRefParagraphNumber>,
    numbering: &Numbering,
    counters: &mut HashMap<String, [u32; 9]>,
    num_id: Option<&str>,
    ilvl: u8,
) {
    if number.is_none() {
        *number = style_ref_paragraph_number(numbering, counters, num_id, ilvl);
    }
}

fn take_style_ref_order(next_order: &mut usize) -> usize {
    let order = *next_order;
    *next_order += 1;
    order
}

fn read_style_ref_ppr(
    r: &mut Xml<'_>,
    style_id: &mut Option<String>,
    num_id: &mut Option<String>,
    ilvl: &mut u8,
) {
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"pPrChange" => skip_subtree(r),
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"pStyle" => *style_id = attr_local_trimmed(&e, b"val"),
                b"ilvl" => {
                    if let Some(value) = attr_u8(&e, b"val") {
                        *ilvl = value;
                    }
                }
                b"numId" => *num_id = attr_local_trimmed(&e, b"val"),
                _ => {}
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"pStyle" => *style_id = attr_local_trimmed(&e, b"val"),
                b"ilvl" => {
                    if let Some(value) = attr_u8(&e, b"val") {
                        *ilvl = value;
                    }
                }
                b"numId" => *num_id = attr_local_trimmed(&e, b"val"),
                _ => {}
            },
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"pPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn style_ref_paragraph_number(
    numbering: &Numbering,
    counters: &mut HashMap<String, [u32; 9]>,
    num_id: Option<&str>,
    ilvl: u8,
) -> Option<StyleRefParagraphNumber> {
    let num_id = num_id.filter(|num_id| *num_id != "0")?;
    let counter = counters.entry(num_id.to_string()).or_insert([0; 9]);
    let label = numbering.label(num_id, ilvl, counter)?;
    let text = ref_paragraph_number(&label)?;
    let full_context = numbering
        .full_context_label(num_id, ilvl, counter)
        .and_then(|label| ref_paragraph_number(&label));
    Some(StyleRefParagraphNumber {
        numeric: ref_numeric_paragraph_number(&text),
        text,
        full_context,
    })
}

fn style_ref_suppresses_source_text(current: &[StyleRefScanField]) -> bool {
    current.iter().any(|field| {
        field.phase == FieldPhase::Result && is_style_ref_field_instruction(&field.instruction)
    })
}

fn style_ref_symbol_text(e: &BytesStart<'_>) -> Option<String> {
    let value = attr_local_trimmed(e, b"char")?;
    let font = attr_local_trimmed(e, b"font");
    computed_run_symbol_char(font.as_deref(), &value).map(|ch| ch.to_string())
}

fn record_style_ref_field(
    instruction: Option<&str>,
    field_positions: &mut Vec<StyleRefFieldPosition>,
    next_order: &mut usize,
    paragraph_number: &Option<StyleRefParagraphNumber>,
) {
    if instruction.is_some_and(is_style_ref_field_instruction) {
        field_positions.push(StyleRefFieldPosition {
            order: take_style_ref_order(next_order),
            number_context: paragraph_number
                .as_ref()
                .and_then(|number| number.full_context.clone()),
        });
    }
}

fn apply_style_ref_scan_fld_char(
    e: &BytesStart<'_>,
    current: &mut Vec<StyleRefScanField>,
    paragraph_number: &Option<StyleRefParagraphNumber>,
    paragraph_text: &mut String,
    emit: &mut StyleRefEmit<'_>,
    src: &mut StyleRefSourceCtx<'_>,
) {
    let field_positions = &mut *emit.field_positions;
    let next_order = &mut *emit.next_order;
    let entries = &mut *emit.entries;
    match field_char_type(e).as_deref() {
        Some("begin") => {
            current.push(StyleRefScanField {
                instruction: String::new(),
                phase: FieldPhase::Instruction,
                paragraph_result_start: None,
                entry_result_start: None,
            });
        }
        Some("separate") => {
            if let Some(field) = current.last_mut() {
                field.phase = FieldPhase::Result;
                field.paragraph_result_start = Some(paragraph_text.len());
                field.entry_result_start = Some(entries.len());
            }
        }
        Some("end") => {
            if let Some(field) = current.pop() {
                if let (Some(start), Some(computed)) = (field.paragraph_result_start, {
                    let current_result = field
                        .paragraph_result_start
                        .and_then(|start| paragraph_text.get(start..))
                        .map(normalize_toc_text)
                        .unwrap_or_default();
                    computed_style_ref_source_field_result(
                        Some(&field.instruction),
                        src,
                        &current_result,
                    )
                }) {
                    let cached_text = normalize_toc_text(&paragraph_text[start..]);
                    if let Some(entry_start) = field.entry_result_start {
                        replace_style_ref_source_entries_with_computed(
                            entries,
                            entry_start,
                            &cached_text,
                            &computed,
                        );
                    }
                    paragraph_text.replace_range(start.., &computed);
                }
                record_style_ref_field(
                    Some(&field.instruction),
                    field_positions,
                    next_order,
                    paragraph_number,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn is_style_ref_field_instruction(instruction: &str) -> bool {
    instruction_parts(instruction)
        .first()
        .is_some_and(|kind| kind.eq_ignore_ascii_case("STYLEREF"))
}

fn style_ref_entry_matches(entry: &StyleRefEntry, style_identifier: &str) -> bool {
    entry
        .style_id
        .as_ref()
        .is_some_and(|style_id| style_id.eq_ignore_ascii_case(style_identifier))
        || entry
            .style_name
            .as_ref()
            .is_some_and(|style_name| style_name.eq_ignore_ascii_case(style_identifier))
}

pub(crate) fn computed_style_ref_result(
    instruction: &str,
    style_refs: &StyleRefContext,
    field_position: Option<StyleRefFieldPosition>,
) -> Option<String> {
    let spec = style_ref_instruction(instruction)?;
    let field_position = field_position?;
    let entry = style_refs.entry_for_style(&spec.style_identifier, field_position.order)?;
    let text = match spec.result {
        StyleRefResult::Text => entry.text.clone(),
        StyleRefResult::ParagraphNumber if spec.suppress_non_numeric => {
            entry.number_numeric.clone()?
        }
        StyleRefResult::ParagraphNumber => entry.number_text.clone()?,
        StyleRefResult::RelativeContextNumber => {
            let target = entry.number_full_context.as_deref()?;
            let field = field_position.number_context.as_deref()?;
            relative_context_ref_number(target, field)
        }
        StyleRefResult::FullContextNumber => entry.number_full_context.clone()?,
        StyleRefResult::RelativePosition => {
            if entry.order < field_position.order {
                "above".to_string()
            } else if entry.order > field_position.order {
                "below".to_string()
            } else {
                return None;
            }
        }
    };
    Some(apply_field_text_format(text, spec.text_format))
}

pub(crate) fn preserved_note_local_style_ref_target(instruction: &str) -> Option<String> {
    let spec = style_ref_instruction(instruction)?;
    matches!(
        spec.result,
        StyleRefResult::Text | StyleRefResult::RelativePosition
    )
    .then_some(spec.style_identifier)
}

pub(crate) fn computed_preserved_note_local_style_ref_result(
    instruction: &str,
    source_text: &str,
    source_before_field: bool,
) -> Option<String> {
    let spec = style_ref_instruction(instruction)?;
    let source_text = normalize_toc_text(source_text);
    if source_text.is_empty() {
        return None;
    }
    let text = match spec.result {
        StyleRefResult::Text => source_text,
        StyleRefResult::RelativePosition => {
            if source_before_field {
                "above".to_string()
            } else {
                "below".to_string()
            }
        }
        _ => return None,
    };
    Some(apply_field_text_format(text, spec.text_format))
}

pub(crate) fn supports_style_ref_field_syntax(instruction: &str) -> bool {
    style_ref_instruction(instruction).is_some()
}

pub(super) fn style_ref_instruction(instruction: &str) -> Option<StyleRefInstruction> {
    style_ref_field_syntax(instruction)
}

pub(super) type StyleRefInstruction = StyleRefFieldSyntax;
