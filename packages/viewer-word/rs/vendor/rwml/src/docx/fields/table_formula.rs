use std::collections::HashMap;

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::annotation::FieldKind;

use super::super::xml_text::{read_text, skip_subtree};
use super::super::{attr_local, field_char_type, local, toggle_on};
use super::formula::{
    eval_formula_function, format_formula_general_number, format_formula_number,
    formula_instruction, formula_number_text, formula_truthy, FormulaNumberFormat, FormulaParser,
};
use super::page_ref::format_page_number;
use super::reference::{
    computed_ref_bookmark_text_result, direct_bookmark_ref_instruction,
    is_ref_position_field_instruction, ref_instruction, ref_or_unknown_direct_bookmark_instruction,
};
use super::{
    apply_complex_field_scan_fld_char, apply_field_text_format, computed_action_result,
    computed_ask_result, computed_display_result, computed_document_info_result,
    computed_fill_in_result, computed_formula_result_with_bookmark_context,
    computed_if_compare_result_with_bookmark_context, computed_legacy_form_result,
    computed_listnum_result, computed_merge_control_result_with_bookmark_context,
    computed_note_ref_result, computed_numbering_result, computed_quote_result,
    computed_reference_index_result, computed_revision_number_result, computed_run_symbol_char,
    computed_section_result, computed_sequence_result, computed_set_result,
    computed_toc_entry_result, inline_marker_text, is_section_field_instruction,
    legacy_form_context, normalize_instruction, note_ref_source_field_position,
    should_skip_alternate_branch, skip_element, AlternateContentBranchState, ComplexField,
    FieldDocumentProperties, FieldPhase, LegacyFormContext, NoteRefContext, NoteRefFieldPosition,
    SectionContext,
};

type Xml<'a> = Reader<&'a [u8]>;

#[derive(Debug, Clone, Default)]
pub(crate) struct TableFormulaContext {
    results: Vec<Option<String>>,
}

impl TableFormulaContext {
    pub(crate) fn field_result(&self, index: usize) -> Option<String> {
        self.results.get(index).and_then(Clone::clone)
    }
}

/// Threaded source-field evaluation state: read-only document context plus the
/// mutable sequence/index accumulators that travel together through the scan.
struct SourceFieldCtx<'a> {
    document_bookmarks: &'a HashMap<String, String>,
    note_refs: &'a NoteRefContext,
    sections: &'a SectionContext,
    legacy_forms: &'a LegacyFormContext,
    properties: FieldDocumentProperties<'a>,
    sequence_counters: HashMap<String, i64>,
    autonum_counter: i64,
    listnum_counter: i64,
    field_bookmarks: HashMap<String, String>,
    form_field_index: usize,
    ref_field_index: usize,
    note_ref_field_index: usize,
    section_field_index: usize,
}

impl<'a> SourceFieldCtx<'a> {
    fn new(
        document_bookmarks: &'a HashMap<String, String>,
        note_refs: &'a NoteRefContext,
        sections: &'a SectionContext,
        legacy_forms: &'a LegacyFormContext,
        properties: FieldDocumentProperties<'a>,
    ) -> Self {
        Self {
            document_bookmarks,
            note_refs,
            sections,
            legacy_forms,
            properties,
            sequence_counters: HashMap::new(),
            autonum_counter: 0,
            listnum_counter: 0,
            field_bookmarks: HashMap::new(),
            form_field_index: 0,
            ref_field_index: 0,
            note_ref_field_index: 0,
            section_field_index: 0,
        }
    }
}

#[cfg(test)]
pub(crate) fn table_formula_context(
    xml: &str,
    document_bookmarks: &HashMap<String, String>,
) -> TableFormulaContext {
    let core_properties = crate::CoreProperties::default();
    let empty_properties = HashMap::new();
    let note_refs = NoteRefContext::empty();
    let sections = SectionContext::empty();
    table_formula_context_with_properties(
        xml,
        document_bookmarks,
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

pub(crate) fn table_formula_context_with_properties(
    xml: &str,
    document_bookmarks: &HashMap<String, String>,
    note_refs: &NoteRefContext,
    sections: &SectionContext,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
) -> TableFormulaContext {
    let legacy_forms = legacy_form_context(xml, preserve_legacy_form_cache);
    let mut r = Reader::from_str(xml);
    let mut results = Vec::new();
    let mut current = Vec::new();
    let mut ctx = SourceFieldCtx::new(
        document_bookmarks,
        note_refs,
        sections,
        &legacy_forms,
        properties,
    );
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
                    b"tbl" => {
                        results.extend(read_table_formula_table(&mut r, &mut ctx));
                        consumed_element = true;
                    }
                    b"fldSimple" => {
                        let instruction =
                            attr_local(&e, b"instr").map(|value| normalize_instruction(&value));
                        if is_formula_instruction(instruction.as_deref()) {
                            results.push(None);
                            skip_element(&mut r, b"fldSimple");
                        } else {
                            let result_text = read_field_result_text(&mut r);
                            let _ = computed_table_formula_source_field_result(
                                instruction.as_deref(),
                                &mut ctx,
                                &result_text,
                            )
                            .unwrap_or(result_text);
                        }
                        consumed_element = true;
                    }
                    b"fldChar" => {
                        apply_table_formula_scan_fld_char(&e, &mut current, &mut ctx, |_, _| {
                            results.push(None)
                        });
                    }
                    b"instrText" => {
                        let text = read_text(&mut r);
                        consumed_element = true;
                        if let Some(field) = current.last_mut() {
                            if field.phase == FieldPhase::Instruction {
                                field.instruction.push_str(&text);
                            }
                        }
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
                    b"fldSimple" => {
                        let instruction =
                            attr_local(&e, b"instr").map(|value| normalize_instruction(&value));
                        if is_formula_instruction(instruction.as_deref()) {
                            results.push(None);
                        } else {
                            let _ = computed_table_formula_source_field_result(
                                instruction.as_deref(),
                                &mut ctx,
                                "",
                            );
                        }
                    }
                    b"fldChar" => {
                        apply_table_formula_scan_fld_char(&e, &mut current, &mut ctx, |_, _| {
                            results.push(None)
                        });
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                if local(e.name().as_ref()) == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    TableFormulaContext { results }
}

#[derive(Debug, Clone)]
enum TableFormulaRecord {
    Nested(Option<String>),
    Local {
        row: usize,
        col: usize,
        instruction: String,
        cached_result: String,
    },
}

#[derive(Debug, Clone, Default)]
struct TableFormulaCell {
    text: String,
    contains_formula: bool,
    has_span: bool,
    is_header_row: bool,
    records: Vec<TableFormulaRecord>,
}

fn read_table_formula_table(r: &mut Xml<'_>, ctx: &mut SourceFieldCtx<'_>) -> Vec<Option<String>> {
    let mut rows = Vec::new();
    let mut records = Vec::new();
    let mut xml_depth = 1usize;
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
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.push(AlternateContentBranchState {
                            branch_depth: xml_depth + 1,
                            took_branch: false,
                        });
                        xml_depth = xml_depth.saturating_add(1);
                    }
                    b"Choice" | b"Fallback" => {
                        xml_depth = xml_depth.saturating_add(1);
                    }
                    b"tr" => {
                        let row_index = rows.len();
                        let mut row = read_table_formula_row(r, row_index, ctx);
                        for cell in &mut row {
                            records.append(&mut cell.records);
                        }
                        rows.push(row);
                    }
                    name if is_current_table_formula_structural_wrapper(name) => {
                        xml_depth = xml_depth.saturating_add(1);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"tbl" {
                    break;
                }
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let has_spans = rows.iter().flatten().any(|cell| cell.has_span);
    let mut results = Vec::with_capacity(records.len());
    for record in records {
        let result = match record {
            TableFormulaRecord::Nested(result) => result,
            TableFormulaRecord::Local {
                row,
                col,
                instruction,
                cached_result,
            } => {
                let result = if !has_spans || span_safe_formula(&instruction, &rows, row, col) {
                    computed_table_formula_result(&instruction, &rows, row, col)
                } else {
                    None
                };
                if let Some(text) = result.as_deref() {
                    promote_table_formula_cell_source_value(
                        &mut rows,
                        row,
                        col,
                        &cached_result,
                        text,
                    );
                }
                result
            }
        };
        results.push(result);
    }
    results
}

fn promote_table_formula_cell_source_value(
    rows: &mut [Vec<TableFormulaCell>],
    row: usize,
    col: usize,
    cached_result: &str,
    computed_result: &str,
) {
    let Some(cell) = rows
        .get_mut(row)
        .and_then(|table_row| table_row.get_mut(col))
    else {
        return;
    };
    if !cached_result.is_empty() && cell.contains_formula && cell.text == cached_result {
        cell.text = computed_result.to_string();
        cell.contains_formula = false;
    }
}

fn read_table_formula_row(
    r: &mut Xml<'_>,
    row_index: usize,
    ctx: &mut SourceFieldCtx<'_>,
) -> Vec<TableFormulaCell> {
    let mut row: Vec<TableFormulaCell> = Vec::new();
    let mut is_header_row = false;
    let mut xml_depth = 1usize;
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
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.push(AlternateContentBranchState {
                            branch_depth: xml_depth + 1,
                            took_branch: false,
                        });
                        xml_depth = xml_depth.saturating_add(1);
                    }
                    b"Choice" | b"Fallback" => {
                        xml_depth = xml_depth.saturating_add(1);
                    }
                    b"trPr" => {
                        is_header_row = read_table_formula_row_props(r);
                        for cell in &mut row {
                            cell.is_header_row = is_header_row;
                        }
                    }
                    b"tc" => {
                        let col_index = row.len();
                        let mut cell = read_table_formula_cell(r, row_index, col_index, ctx);
                        cell.is_header_row = is_header_row;
                        row.push(cell);
                    }
                    name if is_current_table_formula_structural_wrapper(name) => {
                        xml_depth = xml_depth.saturating_add(1);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::Empty(e)) if local(e.name().as_ref()) == b"tc" => {
                row.push(TableFormulaCell {
                    is_header_row,
                    ..TableFormulaCell::default()
                });
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"tr" {
                    break;
                }
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    row
}

fn read_table_formula_row_props(r: &mut Xml<'_>) -> bool {
    let mut header = false;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"trPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if let Some(value) = read_table_formula_row_props_alternate_content(r) {
                    header = value;
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"tblHeader" =>
            {
                header = toggle_on(attr_local(&e, b"val"));
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"trPr" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    header
}

fn read_table_formula_row_props_alternate_content(r: &mut Xml<'_>) -> Option<bool> {
    let mut took = false;
    let mut header = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"Choice" | b"Fallback" if !took => {
                        took = true;
                        header = read_table_formula_row_props_alternate_content_branch(r, name);
                    }
                    _ => skip_subtree(r),
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"AlternateContent" => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    header
}

fn read_table_formula_row_props_alternate_content_branch(
    r: &mut Xml<'_>,
    branch: &[u8],
) -> Option<bool> {
    let mut header = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"trPrChange" => {
                skip_subtree(r);
            }
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"AlternateContent" => {
                if let Some(value) = read_table_formula_row_props_alternate_content(r) {
                    header = Some(value);
                }
            }
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"tblHeader" =>
            {
                header = Some(toggle_on(attr_local(&e, b"val")));
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == branch => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    header
}

fn is_current_table_formula_structural_wrapper(name: &[u8]) -> bool {
    matches!(
        name,
        b"sdt" | b"sdtContent" | b"customXml" | b"smartTag" | b"ins" | b"moveTo"
    )
}

fn read_table_formula_cell(
    r: &mut Xml<'_>,
    row: usize,
    col: usize,
    ctx: &mut SourceFieldCtx<'_>,
) -> TableFormulaCell {
    let mut cell = TableFormulaCell::default();
    let mut current = Vec::new();
    let mut xml_depth = 1usize;
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
                        skip_subtree(r);
                        consumed_element = true;
                    }
                    b"tcPr" => {
                        cell.has_span |= read_table_formula_cell_props(r);
                        consumed_element = true;
                    }
                    b"tbl" => {
                        for result in read_table_formula_table(r, ctx) {
                            cell.records.push(TableFormulaRecord::Nested(result));
                        }
                        if !cell.text.is_empty() {
                            cell.text.push('\n');
                        }
                        consumed_element = true;
                    }
                    b"fldSimple" => {
                        let instruction =
                            attr_local(&e, b"instr").map(|value| normalize_instruction(&value));
                        let is_local_formula = instruction.as_deref().is_some_and(|value| {
                            FieldKind::from_instruction(value)
                                == FieldKind::Dynamic("=".to_string())
                        });
                        let result_text = if is_local_formula {
                            read_field_result_text(r)
                        } else {
                            read_table_formula_source_field_result_text(r, ctx)
                        };
                        if is_local_formula && current.is_empty() {
                            cell.contains_formula = true;
                            cell.records.push(TableFormulaRecord::Local {
                                row,
                                col,
                                instruction: instruction.clone().unwrap_or_default(),
                                cached_result: result_text.clone(),
                            });
                        }
                        let text = if is_local_formula {
                            result_text
                        } else {
                            computed_table_formula_source_field_result(
                                instruction.as_deref(),
                                ctx,
                                &result_text,
                            )
                            .unwrap_or(result_text)
                        };
                        append_table_formula_cell_text(&mut cell.text, &mut current, &text);
                        consumed_element = true;
                    }
                    b"fldChar" => {
                        apply_table_formula_cell_fld_char(
                            &e,
                            &mut current,
                            &mut cell,
                            row,
                            col,
                            ctx,
                        );
                    }
                    b"instrText" => {
                        let text = read_text(r);
                        consumed_element = true;
                        if let Some(field) = current.last_mut() {
                            if field.phase == FieldPhase::Instruction {
                                field.instruction.push_str(&text);
                            }
                        }
                    }
                    b"t" => {
                        let text = read_text(r);
                        append_table_formula_cell_text(&mut cell.text, &mut current, &text);
                        consumed_element = true;
                    }
                    _ => {
                        append_table_formula_cell_inline(&mut cell.text, &mut current, &e);
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
                        let instruction =
                            attr_local(&e, b"instr").map(|value| normalize_instruction(&value));
                        let is_local_formula = instruction.as_deref().is_some_and(|value| {
                            FieldKind::from_instruction(value)
                                == FieldKind::Dynamic("=".to_string())
                        });
                        if current.is_empty() && is_local_formula {
                            cell.contains_formula = true;
                            cell.records.push(TableFormulaRecord::Local {
                                row,
                                col,
                                instruction: instruction.unwrap_or_default(),
                                cached_result: String::new(),
                            });
                        } else if !is_local_formula {
                            let text = computed_table_formula_source_field_result(
                                instruction.as_deref(),
                                ctx,
                                "",
                            )
                            .unwrap_or_default();
                            append_table_formula_cell_text(&mut cell.text, &mut current, &text);
                        }
                    }
                    b"fldChar" => {
                        apply_table_formula_cell_fld_char(
                            &e,
                            &mut current,
                            &mut cell,
                            row,
                            col,
                            ctx,
                        );
                    }
                    _ => {
                        append_table_formula_cell_inline(&mut cell.text, &mut current, &e);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"tc" {
                    break;
                }
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    cell
}

fn read_table_formula_cell_props(r: &mut Xml<'_>) -> bool {
    let mut has_span = false;
    let mut xml_depth = 1usize;
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
                if name == b"tcPrChange" {
                    skip_subtree(r);
                    continue;
                }
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.push(AlternateContentBranchState {
                            branch_depth: xml_depth + 1,
                            took_branch: false,
                        });
                        xml_depth = xml_depth.saturating_add(1);
                    }
                    b"Choice" | b"Fallback" => {
                        xml_depth = xml_depth.saturating_add(1);
                    }
                    b"gridSpan" => {
                        has_span |= grid_span_exceeds_one(&e);
                        skip_subtree(r);
                    }
                    b"vMerge" => {
                        has_span = true;
                        skip_subtree(r);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    continue;
                }
                match name {
                    b"gridSpan" => {
                        has_span |= grid_span_exceeds_one(&e);
                    }
                    b"vMerge" => has_span = true,
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"tcPr" {
                    break;
                }
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    has_span
}

fn grid_span_exceeds_one(e: &BytesStart<'_>) -> bool {
    attr_local(e, b"val").is_some_and(|value| value.trim() != "1")
}

fn read_field_result_text(r: &mut Xml<'_>) -> String {
    read_field_result_text_with_nested_fields(r, |_, _| None)
}

fn read_table_formula_source_field_result_text(
    r: &mut Xml<'_>,
    ctx: &mut SourceFieldCtx<'_>,
) -> String {
    read_field_result_text_with_nested_fields(r, |instruction, current_result| {
        let instruction = normalize_instruction(instruction);
        computed_table_formula_source_field_result(Some(&instruction), ctx, current_result)
    })
}

fn read_field_result_text_with_nested_fields(
    r: &mut Xml<'_>,
    mut nested_field_result: impl FnMut(&str, &str) -> Option<String>,
) -> String {
    read_field_result_text_with_nested_fields_inner(r, &mut nested_field_result)
}

fn read_field_result_text_with_nested_fields_inner<F>(
    r: &mut Xml<'_>,
    nested_field_result: &mut F,
) -> String
where
    F: FnMut(&str, &str) -> Option<String>,
{
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
                    b"fldChar" => {
                        apply_field_result_fld_char(
                            &e,
                            &mut current,
                            &mut text,
                            nested_field_result,
                        );
                    }
                    b"instrText" => {
                        let result = read_text(r);
                        if let Some(field) = current.last_mut() {
                            if field.phase == FieldPhase::Instruction {
                                field.instruction.push_str(&result);
                            }
                        }
                        consumed_element = true;
                    }
                    b"fldSimple" => {
                        let instruction = attr_local(&e, b"instr").unwrap_or_default();
                        let nested_result =
                            read_field_result_text_with_nested_fields_inner(r, nested_field_result);
                        let result = nested_field_result(&instruction, &nested_result)
                            .unwrap_or(nested_result);
                        append_field_result_text(&mut text, &mut current, &result);
                        consumed_element = true;
                    }
                    b"t" => {
                        let result = read_text(r);
                        append_field_result_text(&mut text, &mut current, &result);
                        consumed_element = true;
                    }
                    _ => {
                        append_field_result_inline_text(&mut text, &mut current, &e);
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
                    b"fldChar" => {
                        apply_field_result_fld_char(
                            &e,
                            &mut current,
                            &mut text,
                            nested_field_result,
                        );
                    }
                    b"fldSimple" => {
                        let instruction = attr_local(&e, b"instr").unwrap_or_default();
                        if let Some(computed) = nested_field_result(&instruction, "") {
                            append_field_result_text(&mut text, &mut current, &computed);
                        } else {
                            append_field_result_inline_text(&mut text, &mut current, &e);
                        }
                    }
                    _ => {
                        append_field_result_inline_text(&mut text, &mut current, &e);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"fldSimple" && xml_depth == 0 {
                    break;
                }
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    text
}

fn apply_field_result_fld_char<F>(
    e: &BytesStart<'_>,
    current: &mut Vec<ComplexField>,
    text: &mut String,
    nested_field_result: &mut F,
) where
    F: FnMut(&str, &str) -> Option<String>,
{
    match field_char_type(e).as_deref() {
        Some("begin") => current.push(ComplexField {
            instruction: String::new(),
            result: String::new(),
            phase: FieldPhase::Instruction,
        }),
        Some("separate") => {
            if let Some(field) = current.last_mut() {
                field.phase = FieldPhase::Result;
            }
        }
        Some("end") => {
            let Some(field) = current.pop() else {
                return;
            };
            let result =
                nested_field_result(&field.instruction, &field.result).unwrap_or(field.result);
            append_field_result_text(text, current, &result);
        }
        _ => {}
    }
}

fn append_field_result_text(text: &mut String, current: &mut [ComplexField], value: &str) {
    if let Some(field) = current.last_mut() {
        if field.phase == FieldPhase::Result {
            field.result.push_str(value);
        }
    } else {
        text.push_str(value);
    }
}

fn append_field_result_inline_text(
    text: &mut String,
    current: &mut [ComplexField],
    e: &BytesStart<'_>,
) {
    let mut value = String::new();
    append_table_formula_result_inline(&mut value, e);
    append_field_result_text(text, current, &value);
}

fn apply_table_formula_cell_fld_char(
    e: &BytesStart<'_>,
    current: &mut Vec<ComplexField>,
    cell: &mut TableFormulaCell,
    row: usize,
    col: usize,
    ctx: &mut SourceFieldCtx<'_>,
) {
    match field_char_type(e).as_deref() {
        Some("begin") => current.push(ComplexField {
            instruction: String::new(),
            result: String::new(),
            phase: FieldPhase::Instruction,
        }),
        Some("separate") => {
            if let Some(field) = current.last_mut() {
                field.phase = FieldPhase::Result;
            }
        }
        Some("end") => {
            let Some(field) = current.pop() else {
                return;
            };
            let instruction = normalize_instruction(&field.instruction);
            let is_local_formula =
                FieldKind::from_instruction(&instruction) == FieldKind::Dynamic("=".to_string());
            if is_local_formula && current.is_empty() {
                cell.contains_formula = true;
                cell.records.push(TableFormulaRecord::Local {
                    row,
                    col,
                    instruction: instruction.clone(),
                    cached_result: field.result.clone(),
                });
            }
            let text = if is_local_formula {
                field.result
            } else {
                computed_table_formula_source_field_result(Some(&instruction), ctx, &field.result)
                    .unwrap_or(field.result)
            };
            append_table_formula_cell_text(&mut cell.text, current, &text);
        }
        _ => {}
    }
}

fn append_table_formula_cell_text(
    cell_text: &mut String,
    current: &mut [ComplexField],
    text: &str,
) {
    if let Some(field) = current.last_mut() {
        if field.phase == FieldPhase::Result {
            field.result.push_str(text);
        }
    } else {
        cell_text.push_str(text);
    }
}

fn append_table_formula_cell_inline(
    cell_text: &mut String,
    current: &mut [ComplexField],
    e: &BytesStart<'_>,
) {
    let mut text = String::new();
    append_table_formula_result_inline(&mut text, e);
    append_table_formula_cell_text(cell_text, current, &text);
}

fn computed_table_formula_source_field_result(
    instruction: Option<&str>,
    ctx: &mut SourceFieldCtx<'_>,
    current_result: &str,
) -> Option<String> {
    let instruction = instruction?;
    let document_bookmarks = ctx.document_bookmarks;
    let note_refs = ctx.note_refs;
    let sections = ctx.sections;
    let legacy_forms = ctx.legacy_forms;
    let properties = ctx.properties;
    let sequence_counters = &mut ctx.sequence_counters;
    let autonum_counter = &mut ctx.autonum_counter;
    let listnum_counter = &mut ctx.listnum_counter;
    let field_bookmarks = &mut ctx.field_bookmarks;
    let form_field_index = &mut ctx.form_field_index;
    let ref_field_index = &mut ctx.ref_field_index;
    let note_ref_field_index = &mut ctx.note_ref_field_index;
    let section_field_index = &mut ctx.section_field_index;
    match FieldKind::from_instruction(instruction) {
        FieldKind::Dynamic(kind) if kind == "=" => return None,
        FieldKind::Dynamic(kind) if kind == "ASK" => {
            return computed_ask_result(instruction, field_bookmarks);
        }
        FieldKind::Dynamic(kind) if kind == "SET" => {
            return computed_set_result(instruction, field_bookmarks);
        }
        FieldKind::FormField(_) => {
            let index = *form_field_index;
            *form_field_index += 1;
            return computed_legacy_form_result(instruction, current_result, legacy_forms, index);
        }
        FieldKind::DocumentStructure(kind)
            if (kind == "SECTION" || kind == "SECTIONPAGES")
                && is_section_field_instruction(instruction) =>
        {
            let position = sections.field_position(*section_field_index);
            *section_field_index += 1;
            return computed_section_result(instruction, position);
        }
        _ => {}
    }
    let note_ref_field_position =
        table_formula_source_note_ref_field_position(instruction, note_refs, ref_field_index);
    let direct_note_ref_field_position =
        note_ref_source_field_position(instruction, note_refs, note_ref_field_index);
    if let Some(text) =
        computed_table_formula_source_ref_result(instruction, document_bookmarks, field_bookmarks)
    {
        return Some(text);
    }
    computed_table_formula_source_ref_note_reference_result(
        instruction,
        note_refs,
        note_ref_field_position,
    )
    .or_else(|| computed_note_ref_result(instruction, note_refs, direct_note_ref_field_position))
    .or_else(|| computed_numbering_result(instruction, autonum_counter))
    .or_else(|| computed_listnum_result(instruction, listnum_counter))
    .or_else(|| computed_sequence_result(instruction, sequence_counters))
    .or_else(|| computed_toc_entry_result(instruction))
    .or_else(|| {
        computed_formula_result_with_bookmark_context(
            instruction,
            document_bookmarks,
            field_bookmarks,
        )
    })
    .or_else(|| computed_quote_result(instruction))
    .or_else(|| computed_fill_in_result(instruction))
    .or_else(|| {
        computed_if_compare_result_with_bookmark_context(
            instruction,
            document_bookmarks,
            field_bookmarks,
        )
    })
    .or_else(|| {
        computed_merge_control_result_with_bookmark_context(
            instruction,
            document_bookmarks,
            field_bookmarks,
        )
    })
    .or_else(|| computed_display_result(instruction))
    .or_else(|| computed_action_result(instruction))
    .or_else(|| computed_revision_number_result(instruction, properties.core))
    .or_else(|| {
        computed_document_info_result(
            instruction,
            properties.core,
            properties.custom,
            properties.variables,
            properties.extended,
            properties.file_size_bytes,
        )
    })
    .or_else(|| computed_reference_index_result(instruction))
}

fn computed_table_formula_source_ref_note_reference_result(
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

fn table_formula_source_note_ref_field_position(
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

fn computed_table_formula_source_ref_result(
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

fn append_table_formula_result_inline(text: &mut String, e: &BytesStart<'_>) {
    if let Some(marker) = inline_marker_text(e) {
        text.push_str(marker);
    } else if let Some(ch) = table_formula_symbol_char(e) {
        text.push(ch);
    }
}

fn table_formula_symbol_char(e: &BytesStart<'_>) -> Option<char> {
    if local(e.name().as_ref()) != b"sym" {
        return None;
    }
    let value = attr_local(e, b"char")?;
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let font = attr_local(e, b"font");
    computed_run_symbol_char(font.as_deref(), value)
}

fn apply_table_formula_scan_fld_char(
    e: &BytesStart<'_>,
    current: &mut Vec<ComplexField>,
    ctx: &mut SourceFieldCtx<'_>,
    mut record: impl FnMut(&str, &str),
) {
    apply_complex_field_scan_fld_char(e, current, |field| {
        let instruction = normalize_instruction(&field.instruction);
        if is_formula_instruction(Some(&instruction)) {
            record(&instruction, &field.result);
        } else {
            let _ =
                computed_table_formula_source_field_result(Some(&instruction), ctx, &field.result)
                    .unwrap_or(field.result);
        }
    });
}

fn is_formula_instruction(instruction: Option<&str>) -> bool {
    instruction
        .map(normalize_instruction)
        .as_deref()
        .is_some_and(|instruction| {
            FieldKind::from_instruction(instruction) == FieldKind::Dynamic("=".to_string())
        })
}

fn computed_table_formula_result(
    instruction: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> Option<String> {
    let spec = formula_instruction(instruction)?;
    if spec.expression.is_empty() || spec.expression.contains(['\\', '"']) {
        return None;
    }
    let expression = resolved_table_formula_expression(&spec.expression, rows, row, col)?;
    let mut parser = FormulaParser::new(&expression, None);
    let value = parser.parse()?;
    let text = match spec.number_format {
        Some(FormulaNumberFormat::Picture(format)) => format_formula_number(value, &format),
        Some(FormulaNumberFormat::General(format)) => format_formula_general_number(value, format),
        None => formula_number_text(value),
    }?;
    Some(apply_field_text_format(text, spec.text_format))
}

pub(crate) fn computed_span_free_table_formula_result(
    instruction: &str,
    rows: &[Vec<String>],
    row: usize,
    col: usize,
) -> Option<String> {
    let mut cells = rows
        .iter()
        .map(|table_row| {
            table_row
                .iter()
                .map(|text| TableFormulaCell {
                    text: text.clone(),
                    ..TableFormulaCell::default()
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    cells.get_mut(row)?.get_mut(col)?.contains_formula = true;
    computed_table_formula_result(instruction, &cells, row, col)
}

fn span_safe_formula(
    instruction: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> bool {
    if !table_formula_row_is_span_free(rows, row) {
        return false;
    }
    let Some(spec) = formula_instruction(instruction) else {
        return false;
    };
    table_formula_expression_is_span_safe(&spec.expression, rows, row, col)
}

fn table_formula_expression_is_span_safe(
    expression: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> bool {
    let chars: Vec<_> = expression.chars().collect();
    let mut pos = 0usize;
    while pos < chars.len() {
        if let Some((end, target_row, target_col)) = table_formula_cell_reference_at(&chars, pos) {
            if !table_formula_cell_reference_is_span_safe(rows, row, col, target_row, target_col) {
                return false;
            }
            pos = end;
            continue;
        }
        if chars[pos].is_ascii_alphabetic() {
            let start = pos;
            while pos < chars.len() && chars[pos].is_ascii_alphabetic() {
                pos += 1;
            }
            let name: String = chars[start..pos].iter().collect();
            if table_formula_cell_reference(&name).is_some() {
                return false;
            }
            let mut after_name = pos;
            while after_name < chars.len() && chars[after_name].is_whitespace() {
                after_name += 1;
            }
            if after_name < chars.len() && chars[after_name] == '(' {
                let Some(close) = matching_formula_paren(&chars, after_name) else {
                    return false;
                };
                let inner: String = chars[after_name + 1..close].iter().collect();
                if name.eq_ignore_ascii_case("IF") {
                    if let Some(safe) =
                        table_formula_if_expression_is_span_safe(&inner, rows, row, col)
                    {
                        if !safe {
                            return false;
                        }
                        pos = close + 1;
                        continue;
                    }
                }
                if let Some(safe) =
                    table_formula_call_arguments_are_span_safe(&inner, rows, row, col)
                {
                    if !safe {
                        return false;
                    }
                } else if !table_formula_expression_is_span_safe(&inner, rows, row, col) {
                    return false;
                }
                pos = close + 1;
            }
            continue;
        }
        pos += 1;
    }
    true
}

fn table_formula_if_expression_is_span_safe(
    expression: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> Option<bool> {
    let arguments = table_formula_top_level_argument_expressions(expression)?;
    if arguments.len() != 3 {
        return None;
    }
    if !table_formula_expression_is_span_safe(arguments[0], rows, row, col) {
        return Some(false);
    }
    let condition = table_formula_expression_value(arguments[0], rows, row, col)?;
    let selected = if formula_truthy(condition) {
        arguments[1]
    } else {
        arguments[2]
    };
    Some(table_formula_expression_is_span_safe(
        selected, rows, row, col,
    ))
}

fn table_formula_cell_reference_is_span_safe(
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
    target_row: usize,
    target_col: usize,
) -> bool {
    !(target_row == row && target_col == col) && table_formula_row_is_span_free(rows, target_row)
}

fn table_formula_argument_is_span_safe(
    argument: TableFormulaArgument,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
) -> bool {
    match argument {
        TableFormulaArgument::Direction(
            TableFormulaDirection::Left | TableFormulaDirection::Right,
        )
        | TableFormulaArgument::CurrentRow => true,
        TableFormulaArgument::Direction(TableFormulaDirection::Above) => {
            table_formula_rows_above_are_span_free(rows, row)
        }
        TableFormulaArgument::Direction(TableFormulaDirection::Below) => {
            table_formula_rows_below_until_formula_are_span_free(rows, row)
        }
        TableFormulaArgument::Range {
            start_row, end_row, ..
        } => table_formula_rows_are_span_free(rows, start_row, end_row),
        TableFormulaArgument::CurrentColumn => false,
    }
}

fn table_formula_row_is_span_free(rows: &[Vec<TableFormulaCell>], row: usize) -> bool {
    table_formula_rows_are_span_free(rows, row, row)
}

fn table_formula_rows_are_span_free(
    rows: &[Vec<TableFormulaCell>],
    start_row: usize,
    end_row: usize,
) -> bool {
    rows.get(start_row..=end_row).is_some_and(|table_rows| {
        table_rows
            .iter()
            .all(|table_row| !table_row.iter().any(|cell| cell.has_span))
    })
}

fn table_formula_rows_above_are_span_free(rows: &[Vec<TableFormulaCell>], row: usize) -> bool {
    match row.checked_sub(1) {
        Some(end_row) => table_formula_directional_rows_are_span_free(rows, 0, end_row),
        None => true,
    }
}

fn table_formula_rows_below_until_formula_are_span_free(
    rows: &[Vec<TableFormulaCell>],
    row: usize,
) -> bool {
    let Some(start_row) = row.checked_add(1) else {
        return false;
    };
    let Some(table_rows) = rows.get(start_row..) else {
        return false;
    };
    for table_row in table_rows {
        if table_formula_row_is_header(table_row) {
            continue;
        }
        if table_row.iter().any(|cell| cell.contains_formula) {
            break;
        }
        if table_row.iter().any(|cell| cell.has_span) {
            return false;
        }
    }
    true
}

fn table_formula_directional_rows_are_span_free(
    rows: &[Vec<TableFormulaCell>],
    start_row: usize,
    end_row: usize,
) -> bool {
    rows.get(start_row..=end_row).is_some_and(|table_rows| {
        table_rows.iter().all(|table_row| {
            table_formula_row_is_header(table_row) || !table_row.iter().any(|cell| cell.has_span)
        })
    })
}

fn table_formula_row_is_header(row: &[TableFormulaCell]) -> bool {
    row.iter().any(|cell| cell.is_header_row)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableFormulaDirection {
    Left,
    Right,
    Above,
    Below,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TableFormulaArgument {
    Direction(TableFormulaDirection),
    CurrentRow,
    CurrentColumn,
    Range {
        start_row: usize,
        start_col: usize,
        end_row: usize,
        end_col: usize,
    },
}

fn resolved_table_formula_expression(
    expression: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> Option<String> {
    let chars: Vec<_> = expression.chars().collect();
    let mut output = String::new();
    let mut pos = 0usize;
    let mut changed = false;
    while pos < chars.len() {
        if let Some((end, target_row, target_col)) = table_formula_cell_reference_at(&chars, pos) {
            let value = table_formula_direct_cell_value(rows, row, col, target_row, target_col)?;
            output.push_str(&formula_number_text(value)?);
            pos = end;
            changed = true;
            continue;
        }
        if chars[pos].is_ascii_alphabetic() {
            let start = pos;
            while pos < chars.len() && chars[pos].is_ascii_alphabetic() {
                pos += 1;
            }
            let name: String = chars[start..pos].iter().collect();
            let mut after_name = pos;
            while after_name < chars.len() && chars[after_name].is_whitespace() {
                after_name += 1;
            }
            if after_name < chars.len() && chars[after_name] == '(' {
                if let Some(close) = matching_formula_paren(&chars, after_name) {
                    let inner: String = chars[after_name + 1..close].iter().collect();
                    if name.eq_ignore_ascii_case("IF") {
                        if let Some(value) = computed_table_formula_if_value(&inner, rows, row, col)
                        {
                            output.push_str(&formula_number_text(value)?);
                            pos = close + 1;
                            changed = true;
                            continue;
                        }
                    }
                    if let Some(value) =
                        computed_table_formula_call_value(&name, &inner, rows, row, col)
                    {
                        output.push_str(&formula_number_text(value)?);
                        pos = close + 1;
                        changed = true;
                        continue;
                    }
                    if let Some(inner) = resolved_table_formula_expression(&inner, rows, row, col) {
                        output.push_str(&name);
                        output.push('(');
                        output.push_str(&inner);
                        output.push(')');
                        pos = close + 1;
                        changed = true;
                        continue;
                    }
                }
            }
            output.push_str(&name);
            continue;
        }
        output.push(chars[pos]);
        pos += 1;
    }
    changed.then_some(output)
}

fn computed_table_formula_if_value(
    expression: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> Option<f64> {
    let arguments = table_formula_top_level_argument_expressions(expression)?;
    if arguments.len() != 3 {
        return None;
    }
    let condition = table_formula_expression_value(arguments[0], rows, row, col)?;
    let selected = if formula_truthy(condition) {
        arguments[1]
    } else {
        arguments[2]
    };
    table_formula_expression_value(selected, rows, row, col)
}

fn table_formula_expression_value(
    expression: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> Option<f64> {
    let expression = resolved_table_formula_expression_or_original(expression, rows, row, col);
    let mut parser = FormulaParser::new(&expression, None);
    parser.parse()
}

fn resolved_table_formula_expression_or_original(
    expression: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> String {
    resolved_table_formula_expression(expression, rows, row, col)
        .unwrap_or_else(|| expression.trim().to_string())
}

fn table_formula_top_level_argument_expressions(expression: &str) -> Option<Vec<&str>> {
    let mut arguments = Vec::new();
    let mut separator = None;
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in expression.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return None,
            ')' => depth = depth.checked_sub(1)?,
            ',' | ';' if depth == 0 => {
                if separator.replace(ch).is_some_and(|seen| seen != ch) {
                    return None;
                }
                let argument = expression[start..index].trim();
                if argument.is_empty() {
                    return None;
                }
                arguments.push(argument);
                start = index + ch.len_utf8();
            }
            _ => {}
        }
    }
    if depth != 0 {
        return None;
    }
    let argument = expression[start..].trim();
    if argument.is_empty() {
        return None;
    }
    arguments.push(argument);
    Some(arguments)
}

fn table_formula_cell_reference_at(chars: &[char], pos: usize) -> Option<(usize, usize, usize)> {
    if pos > 0 && chars.get(pos - 1) == Some(&':') {
        return None;
    }
    let mut end = pos;
    while chars
        .get(end)
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
    {
        end += 1;
    }
    if end == pos || chars.get(end) == Some(&':') {
        return None;
    }
    let mut after = end;
    while chars.get(after).is_some_and(|ch| ch.is_whitespace()) {
        after += 1;
    }
    if chars.get(after) == Some(&'(') {
        return None;
    }
    let token: String = chars[pos..end].iter().collect();
    let (row, col) = table_formula_cell_reference(&token)?;
    Some((end, row, col))
}

fn matching_formula_paren(chars: &[char], open: usize) -> Option<usize> {
    if chars.get(open) != Some(&'(') {
        return None;
    }
    let mut depth = 0usize;
    for (index, ch) in chars.iter().enumerate().skip(open + 1) {
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(index),
            ')' => depth = depth.checked_sub(1)?,
            _ => {}
        }
    }
    None
}

fn computed_table_formula_call_value(
    function: &str,
    expression: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> Option<f64> {
    let values = table_formula_call_argument_values(expression, rows, row, col)?;
    if values.is_empty() {
        return None;
    }
    eval_formula_function(&function.to_ascii_uppercase(), &values)
}

fn table_formula_call_arguments_are_span_safe(
    expression: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> Option<bool> {
    let mut table_arguments = Vec::new();
    for raw_argument in table_formula_top_level_argument_expressions(expression)? {
        if let Some(argument) = table_formula_argument(raw_argument) {
            if table_arguments.contains(&argument) {
                return None;
            }
            table_arguments.push(argument);
            if !table_formula_argument_is_span_safe(argument, rows, row) {
                return Some(false);
            }
        } else if !table_formula_expression_is_span_safe(raw_argument, rows, row, col) {
            return Some(false);
        }
    }
    Some(true)
}

fn table_formula_call_argument_values(
    expression: &str,
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
) -> Option<Vec<f64>> {
    let mut values = Vec::new();
    let mut table_arguments = Vec::new();
    for raw_argument in table_formula_top_level_argument_expressions(expression)? {
        if let Some(argument) = table_formula_argument(raw_argument) {
            if table_arguments.contains(&argument) {
                return None;
            }
            table_arguments.push(argument);
            push_table_formula_argument_values(rows, row, col, argument, &mut values)?;
        } else {
            let value = table_formula_expression_value(raw_argument, rows, row, col)?;
            if !value.is_finite() {
                return None;
            }
            values.push(value);
        }
    }
    Some(values)
}

fn table_formula_argument(value: &str) -> Option<TableFormulaArgument> {
    if let Some(direction) = table_formula_direction(value) {
        return Some(TableFormulaArgument::Direction(direction));
    }
    if value.eq_ignore_ascii_case("R") {
        return Some(TableFormulaArgument::CurrentRow);
    }
    if value.eq_ignore_ascii_case("C") {
        return Some(TableFormulaArgument::CurrentColumn);
    }
    table_formula_range_reference(value)
}

fn table_formula_direction(value: &str) -> Option<TableFormulaDirection> {
    match value.to_ascii_uppercase().as_str() {
        "LEFT" => Some(TableFormulaDirection::Left),
        "RIGHT" => Some(TableFormulaDirection::Right),
        "ABOVE" => Some(TableFormulaDirection::Above),
        "BELOW" => Some(TableFormulaDirection::Below),
        _ => None,
    }
}

fn table_formula_range_reference(value: &str) -> Option<TableFormulaArgument> {
    let (start, end) = match value.split_once(':') {
        Some((start, end)) if !start.is_empty() && !end.is_empty() => (
            table_formula_cell_reference(start)?,
            table_formula_cell_reference(end)?,
        ),
        Some(_) => return None,
        None => {
            let cell = table_formula_cell_reference(value)?;
            (cell, cell)
        }
    };
    Some(TableFormulaArgument::Range {
        start_row: start.0.min(end.0),
        start_col: start.1.min(end.1),
        end_row: start.0.max(end.0),
        end_col: start.1.max(end.1),
    })
}

fn table_formula_cell_reference(value: &str) -> Option<(usize, usize)> {
    parse_rncn_cell_reference(value).or_else(|| parse_a1_cell_reference(value))
}

fn parse_rncn_cell_reference(value: &str) -> Option<(usize, usize)> {
    let value = value.trim();
    let rest = value
        .strip_prefix('R')
        .or_else(|| value.strip_prefix('r'))?;
    let c_index = rest.find(['C', 'c'])?;
    let (row, col_with_c) = rest.split_at(c_index);
    let col = &col_with_c[1..];
    if row.is_empty() || col.is_empty() {
        return None;
    }
    let row = row.parse::<usize>().ok()?.checked_sub(1)?;
    let col = col.parse::<usize>().ok()?.checked_sub(1)?;
    Some((row, col))
}

fn parse_a1_cell_reference(value: &str) -> Option<(usize, usize)> {
    let value = value.trim();
    let split = value.find(|ch: char| ch.is_ascii_digit())?;
    let (col, row) = value.split_at(split);
    if col.is_empty()
        || row.is_empty()
        || !col.chars().all(|ch| ch.is_ascii_alphabetic())
        || !row.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let mut col_index = 0usize;
    for ch in col.chars() {
        let digit = ch.to_ascii_uppercase() as u8 - b'A' + 1;
        col_index = col_index.checked_mul(26)?.checked_add(digit as usize)?;
    }
    let row_index = row.parse::<usize>().ok()?.checked_sub(1)?;
    Some((row_index, col_index.checked_sub(1)?))
}

fn push_table_formula_argument_values(
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
    argument: TableFormulaArgument,
    values: &mut Vec<f64>,
) -> Option<()> {
    match argument {
        TableFormulaArgument::Direction(TableFormulaDirection::Left) => {
            for cell in rows.get(row)?.get(..col)? {
                push_table_formula_directional_cell_number(cell, values)?;
            }
        }
        TableFormulaArgument::Direction(TableFormulaDirection::Right) => {
            for cell in rows.get(row)?.get(col + 1..)? {
                push_table_formula_directional_cell_number(cell, values)?;
            }
        }
        TableFormulaArgument::Direction(TableFormulaDirection::Above) => {
            for table_row in rows.get(..row)? {
                if let Some(cell) = table_row.get(col) {
                    push_table_formula_directional_cell_number(cell, values)?;
                }
            }
        }
        TableFormulaArgument::Direction(TableFormulaDirection::Below) => {
            for table_row in rows.get(row + 1..)? {
                if table_formula_row_is_header(table_row) {
                    continue;
                }
                if table_row.iter().any(|cell| cell.contains_formula) {
                    break;
                }
                if let Some(cell) = table_row.get(col) {
                    push_table_formula_directional_cell_number(cell, values)?;
                }
            }
        }
        TableFormulaArgument::CurrentRow => {
            for (cell_index, cell) in rows.get(row)?.iter().enumerate() {
                if cell_index != col {
                    push_table_formula_directional_cell_number(cell, values)?;
                }
            }
        }
        TableFormulaArgument::CurrentColumn => {
            for (row_index, table_row) in rows.iter().enumerate() {
                if row_index != row {
                    if let Some(cell) = table_row.get(col) {
                        push_table_formula_directional_cell_number(cell, values)?;
                    }
                }
            }
        }
        TableFormulaArgument::Range {
            start_row,
            start_col,
            end_row,
            end_col,
        } => {
            for row_index in start_row..=end_row {
                let table_row = rows.get(row_index)?;
                for col_index in start_col..=end_col {
                    if row_index == row && col_index == col {
                        continue;
                    }
                    if let Some(cell) = table_row.get(col_index) {
                        push_table_formula_cell_number(cell, values)?;
                    }
                }
            }
        }
    }
    Some(())
}

fn push_table_formula_directional_cell_number(
    cell: &TableFormulaCell,
    values: &mut Vec<f64>,
) -> Option<()> {
    if cell.is_header_row {
        return Some(());
    }
    push_table_formula_cell_number(cell, values)
}

fn push_table_formula_cell_number(cell: &TableFormulaCell, values: &mut Vec<f64>) -> Option<()> {
    if cell.contains_formula {
        return None;
    }
    let text = cell.text.trim();
    if text.is_empty() {
        return None;
    }
    values.push(text.parse::<f64>().ok()?);
    Some(())
}

fn table_formula_direct_cell_value(
    rows: &[Vec<TableFormulaCell>],
    row: usize,
    col: usize,
    target_row: usize,
    target_col: usize,
) -> Option<f64> {
    if row == target_row && col == target_col {
        return None;
    }
    let cell = rows.get(target_row)?.get(target_col)?;
    if cell.contains_formula {
        return None;
    }
    let value = cell.text.trim().parse::<f64>().ok()?;
    value.is_finite().then_some(value)
}
