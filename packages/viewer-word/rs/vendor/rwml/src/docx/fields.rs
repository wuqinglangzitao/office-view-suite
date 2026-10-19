//! `.docx` field marker parsing.

use std::collections::{HashMap, HashSet};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::annotation::{
    accept_field_number_format_switch,
    accept_field_text_format_switch as accept_field_format_switch, accept_general_format_switch,
    action_field_syntax, advance_field_syntax, compare_field_syntax, direct_ref_field_syntax,
    document_property_key, eq_enclosed_operand as take_eq_enclosed_operand,
    eq_fraction_operands as split_eq_fraction_operands, eq_list_operands as split_eq_list_operands,
    eq_numeric_prefix_option as consume_eq_numeric_prefix_option,
    eq_parenthesized_operand as take_eq_parenthesized_operand,
    eq_prefix_switch_tail as consume_eq_prefix_switch,
    eq_radical_operands as split_eq_radical_operands, field_identifier_token, field_literal_token,
    field_name_operand, field_name_token, field_non_empty_non_switch_literal_token,
    field_quoted_literal_token, filename_field_syntax, if_field_syntax, instruction_parts,
    legacy_form_field_syntax, merge_control_field_syntax, merge_field_name, merge_field_syntax,
    note_ref_field_syntax, numbering_field_syntax, page_field_format_syntax_tail,
    page_ref_field_syntax, prompt_field_syntax, quote_field_syntax, ref_field_syntax,
    reference_index_category_token, reference_index_literal_operand, reference_index_literal_token,
    reference_index_plain_value_token, revision_number_field_text_format, sequence_field_syntax,
    set_field_syntax, strip_ascii_switch_prefix, style_ref_field_syntax, symbol_field_syntax,
    toc_entry_field_syntax, toc_field_syntax, Field, FieldContext, FieldKind, FieldNumberFormat,
    FieldTextFormat, PromptFieldSyntax, StyleRefFieldSyntax, StyleRefResult,
    TocFieldSyntax as TocSpec, TocSequenceFilter, TocTcFilter as TcFilter,
};
use crate::CoreProperties;

use super::numbering::Numbering;
use super::styles::Styles;
use super::xml_text::{read_text, skip_subtree};
use super::{
    attr_local, attr_local_trimmed, attr_local_trimmed_preserve_empty, attr_u8, attr_usize,
    field_char_type, is_page_break_type, local, toggle_on,
};

type Xml<'a> = Reader<&'a [u8]>;

mod display;
mod document_info;
mod formula;
mod legacy_form;
mod note_ref;
mod page_ref;
mod reference;
mod section;
mod style_ref;
mod table_formula;
mod toc;

pub(crate) use self::display::computed_run_symbol_char;
use self::display::unquote_field_text;
pub(crate) use self::display::{
    computed_display_result, supports_computed_symbol_field_syntax,
    supports_context_free_display_field_syntax, supports_display_field_syntax,
};
#[cfg(test)]
use self::document_info::document_info_instruction;
pub(crate) use self::document_info::{
    computed_context_document_info_result, computed_document_info_result,
    computed_preserved_document_info_result, computed_preserved_revision_number_result,
    computed_revision_number_result, supports_document_info_field_syntax,
    supports_preserved_document_info_field_syntax, supports_revision_number_field_syntax,
};
#[cfg(test)]
use self::formula::computed_formula_result;
use self::formula::computed_formula_result_with_bookmarks;
pub(crate) use self::formula::{
    supports_context_free_formula_field_syntax, supports_formula_field_syntax,
};
pub(crate) use self::legacy_form::{
    computed_legacy_form_result, legacy_form_context, LegacyFormContext,
};
#[cfg(test)]
use self::note_ref::note_ref_context;
#[cfg(test)]
use self::note_ref::note_ref_instruction;
pub(crate) use self::note_ref::{
    computed_note_ref_result, note_numbering_from_settings, note_ref_context_with_numbering,
    note_ref_context_with_properties, note_ref_source_field_position, note_ref_target_names,
    NoteNumbering, NoteRefContext, NoteRefFieldPosition,
};
use self::page_ref::{
    accept_page_field_format_switch_for_tail, format_page_number, page_after_section_break,
    page_number_format_from_field_format, page_ref_on_off_enabled, page_ref_section_break,
    PageNumberFormat, PageRefSectionBreak,
};
#[cfg(test)]
use self::page_ref::{cardinal_page_number_text, ordinal_page_number_text, page_ref_instruction};
#[allow(unused_imports)]
pub(crate) use self::page_ref::{
    computed_page_ref_result, computed_page_result, page_ref_context,
    page_ref_context_with_properties, supports_page_field_syntax, PageRefContext, PageRefPosition,
};
pub(crate) use self::reference::{
    computed_direct_bookmark_ref_result, computed_preserved_note_local_ref_result,
    computed_ref_result, is_direct_bookmark_ref_field_instruction,
    is_ref_position_field_instruction, preserved_note_local_ref_target, ref_number_context,
    ref_position_context, ref_targets, ref_targets_with_note_context, ref_targets_with_properties,
    RefFieldPosition, RefNumberContext, RefPositionContext, RefResultContext,
};
use self::reference::{
    computed_ref_instruction_result, direct_bookmark_ref_instruction, ref_instruction,
    ref_instruction_target_known, ref_note_field_target, ref_numeric_paragraph_number,
    ref_paragraph_number, relative_context_ref_number,
};
pub(crate) use self::section::{
    computed_section_result, is_section_field_instruction, section_context_with_properties,
    SectionContext, SectionFieldPosition,
};
#[cfg(test)]
use self::style_ref::style_ref_instruction;
#[allow(unused_imports)]
pub(crate) use self::style_ref::{
    computed_preserved_note_local_style_ref_result, computed_style_ref_result,
    is_style_ref_field_instruction, preserved_note_local_style_ref_target,
    style_ref_context_with_properties, supports_style_ref_field_syntax, StyleRefContext,
    StyleRefFieldPosition, StyleRefResolutionSources,
};
#[cfg(test)]
use self::table_formula::table_formula_context;
pub(crate) use self::table_formula::{
    computed_span_free_table_formula_result, table_formula_context_with_properties,
    TableFormulaContext,
};
#[cfg(test)]
use self::toc::toc_entries;
pub(crate) use self::toc::{
    computed_toc_entry_result, computed_toc_result, supports_toc_entry_field_syntax,
    toc_entries_with_properties, TocEntry,
};
#[cfg(test)]
use self::toc::{seq_identifier_from_instruction, toc_spec, TocEntrySource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FieldPhase {
    Instruction,
    Result,
}

#[derive(Debug, Clone)]
struct ComplexField {
    instruction: String,
    result: String,
    phase: FieldPhase,
}

// Fill computed results for volatile fields the caller-supplied context makes
// deterministic. Fields the default evaluation already computed are left
// untouched, so context values never override document-derived results.
pub(crate) fn apply_context_results(fields: &mut [Field], context: &FieldContext) {
    for field in fields {
        if field.computed_result.is_some() {
            continue;
        }
        if let Some(text) = computed_context_field_result(field, context) {
            field.computed_result = Some(text);
        }
    }
}

fn computed_context_field_result(field: &Field, context: &FieldContext) -> Option<String> {
    match &field.kind {
        FieldKind::DocumentInfo(kind) => {
            computed_context_document_info_result(kind, &field.instruction, context)
        }
        FieldKind::MergeField => computed_context_merge_field_result(&field.instruction, context),
        FieldKind::InsertedContent(kind) if kind.eq_ignore_ascii_case("INCLUDETEXT") => {
            computed_context_include_text_result(&field.instruction, context)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MergeFieldContextOptions {
    before: Option<String>,
    after: Option<String>,
    text_format: Option<FieldTextFormat>,
}

fn computed_context_merge_field_result(
    instruction: &str,
    context: &FieldContext,
) -> Option<String> {
    if !merge_field_syntax(instruction) {
        return None;
    }
    let name = merge_field_name(instruction)?;
    let options = merge_field_context_options(instruction)?;
    let mut text = context
        .merge_values
        .get(&document_property_key(&name))?
        .to_string();
    if !text.is_empty() {
        if let Some(before) = options.before {
            text.insert_str(0, &before);
        }
        if let Some(after) = options.after {
            text.push_str(&after);
        }
    }
    Some(apply_field_text_format(text, options.text_format))
}

fn merge_field_context_options(instruction: &str) -> Option<MergeFieldContextOptions> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("MERGEFIELD") {
        return None;
    }
    let first = parts.next()?;
    field_name_operand(first, &mut parts)?;
    let mut options = MergeFieldContextOptions::default();
    while let Some(part) = parts.next() {
        if accept_field_format_switch_for_tail(part, &mut parts, &mut options.text_format)? {
            continue;
        }
        if part.eq_ignore_ascii_case("\\b") {
            options.before = Some(merge_field_text_switch_value(parts.next()?, &mut parts)?);
            continue;
        }
        if part.eq_ignore_ascii_case("\\f") {
            options.after = Some(merge_field_text_switch_value(parts.next()?, &mut parts)?);
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\b") {
            options.before = Some(merge_field_compact_text_switch_value(value)?);
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\f") {
            options.after = Some(merge_field_compact_text_switch_value(value)?);
            continue;
        }
        return None;
    }
    Some(options)
}

fn merge_field_text_switch_value<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    let first_value = field_non_empty_non_switch_literal_token(first)?;
    if field_quoted_literal_token(first).is_some() {
        return Some(first_value.to_string());
    }
    let mut value = first_value.to_string();
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        if field_quoted_literal_token(part).is_some() {
            return None;
        }
        let part = field_non_empty_non_switch_literal_token(parts.next()?)?;
        value.push(' ');
        value.push_str(part);
    }
    Some(value)
}

fn merge_field_compact_text_switch_value(value: &str) -> Option<String> {
    Some(field_non_empty_non_switch_literal_token(value)?.to_string())
}

fn computed_context_include_text_result(
    instruction: &str,
    context: &FieldContext,
) -> Option<String> {
    let source = include_text_source(instruction)?;
    context.include_text.get(&source).cloned()
}

fn include_text_source(instruction: &str) -> Option<String> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("INCLUDETEXT") {
        return None;
    }
    Some(field_non_empty_non_switch_literal_token(parts.next()?)?.to_string())
}

pub(crate) fn parse(
    xml: &str,
    styles: &Styles,
    toc_entries: &[TocEntry],
    numbering: &Numbering,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
) -> Vec<Field> {
    parse_with_note_numbering(
        xml,
        styles,
        toc_entries,
        numbering,
        properties,
        preserve_legacy_form_cache,
        NoteNumbering::default(),
    )
}

pub(crate) fn parse_with_note_numbering(
    xml: &str,
    styles: &Styles,
    toc_entries: &[TocEntry],
    numbering: &Numbering,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
    note_numbering: NoteNumbering,
) -> Vec<Field> {
    let all_bookmark_names = bookmark_names(xml);
    let ref_positions = ref_position_context(xml, numbering);
    let ref_numbers = ref_number_context(xml, numbering);
    let bookmarks = ref_targets_with_properties(xml, properties, preserve_legacy_form_cache);
    let note_refs = note_ref_context_with_numbering(
        xml,
        &bookmarks,
        properties,
        preserve_legacy_form_cache,
        note_numbering,
    );
    let bookmarks =
        ref_targets_with_note_context(xml, properties, preserve_legacy_form_cache, &note_refs);
    let page_refs =
        page_ref_context_with_properties(xml, &bookmarks, properties, preserve_legacy_form_cache);
    let sections =
        section_context_with_properties(xml, &bookmarks, properties, preserve_legacy_form_cache);
    let style_refs = style_ref_context_with_properties(
        xml,
        styles,
        numbering,
        &StyleRefResolutionSources {
            document_bookmarks: &bookmarks,
            note_refs: &note_refs,
            sections: &sections,
        },
        properties,
        preserve_legacy_form_cache,
    );
    let legacy_forms = legacy_form_context(xml, preserve_legacy_form_cache);
    let table_formulas = table_formula_context_with_properties(
        xml,
        &bookmarks,
        &note_refs,
        &sections,
        properties,
        preserve_legacy_form_cache,
    );
    let sequence_headings = sequence_heading_context(xml, styles);
    let mut r = Reader::from_str(xml);
    let mut fields = Vec::new();
    let mut current = Vec::new();
    let mut xml_depth = 0usize;
    let mut run_depth = 0usize;
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
                if matches!(name, b"del" | b"moveFrom" | b"pPrChange") {
                    skip_subtree(&mut r);
                    continue;
                }
                if name == b"r" {
                    run_depth = run_depth.saturating_add(1);
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
                        record_simple_field(
                            read_simple_field(&mut r, &e),
                            &mut current,
                            &mut fields,
                        );
                        consumed_element = true;
                    }
                    b"fldChar" => {
                        apply_fld_char(&e, &mut current, &mut fields);
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
                    b"t" => {
                        let text = read_text(&mut r);
                        consumed_element = true;
                        if run_depth > 0 {
                            if let Some(field) = current.last_mut() {
                                if field.phase == FieldPhase::Result {
                                    field.result.push_str(&text);
                                }
                            }
                        }
                    }
                    _ => {
                        if run_depth > 0 {
                            if let Some(field) = current.last_mut() {
                                if field.phase == FieldPhase::Result {
                                    append_field_result_inline(&mut field.result, &e);
                                }
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
                        record_simple_field(
                            make_field(attr_local(&e, b"instr").unwrap_or_default(), String::new()),
                            &mut current,
                            &mut fields,
                        );
                    }
                    b"fldChar" => {
                        apply_fld_char(&e, &mut current, &mut fields);
                    }
                    _ => {
                        if run_depth > 0 {
                            if let Some(field) = current.last_mut() {
                                if field.phase == FieldPhase::Result {
                                    append_field_result_inline(&mut field.result, &e);
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                if name == b"r" {
                    run_depth = run_depth.saturating_sub(1);
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    apply_computed_results(
        &mut fields,
        ComputedResultContexts {
            bookmarks: &bookmarks,
            ref_positions: &ref_positions,
            ref_numbers: &ref_numbers,
            page_refs: &page_refs,
            note_refs: &note_refs,
            sections: &sections,
            style_refs: &style_refs,
            legacy_forms: &legacy_forms,
            table_formulas: &table_formulas,
            sequence_headings: &sequence_headings,
            toc_entries,
            bookmark_names: &all_bookmark_names,
            core_properties: properties.core,
            custom_properties: properties.custom,
            document_variables: properties.variables,
            extended_properties: properties.extended,
            file_size_bytes: properties.file_size_bytes,
        },
    );
    fields
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FieldDocumentProperties<'a> {
    pub(crate) core: &'a CoreProperties,
    pub(crate) custom: &'a HashMap<String, String>,
    pub(crate) variables: &'a HashMap<String, String>,
    pub(crate) extended: &'a HashMap<String, String>,
    pub(crate) file_size_bytes: Option<usize>,
}

/// Fully-assembled field-resolution context threaded into the marker/anchor
/// scanners (`comments::parse`, `comments::parse_anchors`, `revisions::parse`,
/// `body::scan_note_ref_anchors`). Bundles the document properties and every
/// field-family context so those `pub(crate)` boundary functions take one borrow
/// instead of eight parallel arguments. All fields are `Copy` refs, so the whole
/// context is `Copy`; scanners that do not consult `style_refs` simply ignore it.
#[derive(Clone, Copy)]
pub(crate) struct FieldResolutionContext<'a> {
    pub(crate) properties: FieldDocumentProperties<'a>,
    pub(crate) document_bookmarks: &'a HashMap<String, String>,
    pub(crate) note_refs: &'a NoteRefContext,
    pub(crate) sections: &'a SectionContext,
    pub(crate) style_refs: &'a StyleRefContext,
    pub(crate) legacy_forms: &'a LegacyFormContext,
    pub(crate) toc_entries: &'a [TocEntry],
    pub(crate) bookmark_names: &'a HashSet<String>,
}

fn record_simple_field(field: Field, current: &mut [ComplexField], fields: &mut Vec<Field>) {
    if let Some(parent) = current.last_mut() {
        if parent.phase == FieldPhase::Result {
            parent.result.push_str(&field.result);
        }
    }
    fields.push(field);
}

fn read_simple_field(r: &mut Xml<'_>, start: &BytesStart<'_>) -> Field {
    let (instruction, result) = read_simple_field_result(r, start, |_, _| None);
    make_field(instruction, result)
}

fn read_simple_field_result(
    r: &mut Xml<'_>,
    start: &BytesStart<'_>,
    mut nested_field_result: impl FnMut(&str, &str) -> Option<String>,
) -> (String, String) {
    read_simple_field_result_with_nested(r, start, &mut nested_field_result)
}

fn read_simple_field_result_with_nested<F>(
    r: &mut Xml<'_>,
    start: &BytesStart<'_>,
    nested_field_result: &mut F,
) -> (String, String)
where
    F: FnMut(&str, &str) -> Option<String>,
{
    let instruction = attr_local(start, b"instr").unwrap_or_default();
    let mut result = String::new();
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
                        apply_simple_result_fld_char(
                            &e,
                            &mut current,
                            &mut result,
                            nested_field_result,
                        );
                    }
                    b"instrText" => {
                        let text = read_text(r);
                        if let Some(field) = current.last_mut() {
                            if field.phase == FieldPhase::Instruction {
                                field.instruction.push_str(&text);
                            }
                        }
                        consumed_element = true;
                    }
                    b"fldSimple" => {
                        let (nested_instruction, nested_result) =
                            read_simple_field_result_with_nested(r, &e, nested_field_result);
                        let text = nested_field_result(&nested_instruction, &nested_result)
                            .unwrap_or(nested_result);
                        append_simple_result_text(&mut result, &mut current, &text);
                        consumed_element = true;
                    }
                    b"t" => {
                        let text = read_text(r);
                        append_simple_result_text(&mut result, &mut current, &text);
                        consumed_element = true;
                    }
                    _ => {
                        append_simple_result_inline(&mut result, &mut current, &e);
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
                        apply_simple_result_fld_char(
                            &e,
                            &mut current,
                            &mut result,
                            nested_field_result,
                        );
                    }
                    b"fldSimple" => {
                        let instruction = attr_local(&e, b"instr").unwrap_or_default();
                        if let Some(text) = nested_field_result(&instruction, "") {
                            append_simple_result_text(&mut result, &mut current, &text);
                        } else {
                            append_simple_result_inline(&mut result, &mut current, &e);
                        }
                    }
                    _ => {
                        append_simple_result_inline(&mut result, &mut current, &e);
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
    (instruction, result)
}

fn apply_simple_result_fld_char<F>(
    e: &BytesStart<'_>,
    current: &mut Vec<ComplexField>,
    result: &mut String,
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
            let text =
                nested_field_result(&field.instruction, &field.result).unwrap_or(field.result);
            append_simple_result_text(result, current, &text);
        }
        _ => {}
    }
}

fn append_simple_result_text(result: &mut String, current: &mut [ComplexField], text: &str) {
    if let Some(field) = current.last_mut() {
        if field.phase == FieldPhase::Result {
            field.result.push_str(text);
        }
    } else {
        result.push_str(text);
    }
}

fn append_simple_result_inline(
    result: &mut String,
    current: &mut [ComplexField],
    e: &BytesStart<'_>,
) {
    let mut text = String::new();
    append_field_result_inline(&mut text, e);
    append_simple_result_text(result, current, &text);
}

fn apply_fld_char(e: &BytesStart<'_>, current: &mut Vec<ComplexField>, out: &mut Vec<Field>) {
    apply_complex_field_scan_fld_char(e, current, |field| {
        out.push(make_field(field.instruction, field.result));
    });
}

fn apply_complex_field_scan_fld_char(
    e: &BytesStart<'_>,
    current: &mut Vec<ComplexField>,
    mut finish: impl FnMut(ComplexField),
) {
    match field_char_type(e).as_deref() {
        Some("begin") => {
            current.push(ComplexField {
                instruction: String::new(),
                result: String::new(),
                phase: FieldPhase::Instruction,
            });
        }
        Some("separate") => {
            if let Some(field) = current.last_mut() {
                field.phase = FieldPhase::Result;
            }
        }
        Some("end") => {
            if let Some(field) = current.pop() {
                if let Some(parent) = current.last_mut() {
                    if parent.phase == FieldPhase::Result {
                        parent.result.push_str(&field.result);
                    }
                }
                finish(field);
            }
        }
        _ => {}
    }
}

fn make_field(instruction: String, result: String) -> Field {
    let instruction = normalize_instruction(&instruction);
    Field {
        kind: field_kind(&instruction),
        instruction,
        result,
        computed_result: None,
    }
}

#[derive(Debug, Clone, Copy)]
struct AlternateContentBranchState {
    branch_depth: usize,
    took_branch: bool,
}

fn should_skip_alternate_branch(
    stack: &mut [AlternateContentBranchState],
    xml_depth: usize,
    name: &[u8],
) -> bool {
    if !matches!(name, b"Choice" | b"Fallback") {
        return false;
    }
    let Some(state) = stack.last_mut() else {
        return false;
    };
    if state.branch_depth != xml_depth {
        return false;
    }
    if state.took_branch {
        true
    } else {
        state.took_branch = true;
        false
    }
}

fn append_ref_text(active: &[(String, String)], out: &mut HashMap<String, String>, text: &str) {
    for (_, name) in active {
        out.entry(name.clone()).or_default().push_str(text);
    }
}

fn append_ref_paragraph_breaks(active: &[(String, String)], out: &mut HashMap<String, String>) {
    for (_, name) in active {
        let text = out.entry(name.clone()).or_default();
        if !text.is_empty() {
            text.push('\n');
        }
    }
}

fn bookmark_start(e: &BytesStart<'_>) -> Option<(String, String)> {
    Some((
        attr_local_trimmed(e, b"id")?,
        attr_local_trimmed(e, b"name")?,
    ))
}

fn bookmark_end_id(e: &BytesStart<'_>) -> Option<String> {
    attr_local_trimmed(e, b"id")
}

fn bookmark_name(e: &BytesStart<'_>) -> Option<String> {
    attr_local_trimmed(e, b"name")
}

pub(crate) fn bookmark_names(xml: &str) -> HashSet<String> {
    let mut r = Reader::from_str(xml);
    let mut names = HashSet::new();
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
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.push(AlternateContentBranchState {
                            branch_depth: xml_depth + 1,
                            took_branch: false,
                        });
                    }
                    b"bookmarkStart" => {
                        if let Some(name) = bookmark_name(&e) {
                            names.insert(name);
                        }
                    }
                    _ => {}
                }
                xml_depth = xml_depth.saturating_add(1);
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    continue;
                }
                if name == b"bookmarkStart" {
                    if let Some(name) = bookmark_name(&e) {
                        names.insert(name);
                    }
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
    names
}

fn skip_element(r: &mut Xml<'_>, element: &[u8]) {
    let mut depth = 1usize;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == element => {
                depth += 1;
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == element => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
}

fn push_unique(names: &mut Vec<String>, name: String) {
    if !names.iter().any(|existing| existing == &name) {
        names.push(name);
    }
}

fn normalize_toc_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn append_field_result_inline(result: &mut String, e: &BytesStart<'_>) {
    if let Some(text) = inline_marker_text(e) {
        result.push_str(text);
    } else if let Some(ch) = field_result_symbol_char(e) {
        result.push(ch);
    }
}

fn field_result_symbol_char(e: &BytesStart<'_>) -> Option<char> {
    let value = attr_local_trimmed(e, b"char")?;
    let font = attr_local_trimmed(e, b"font");
    computed_run_symbol_char(font.as_deref(), &value)
}

fn is_supported_run_symbol(e: &BytesStart<'_>) -> bool {
    local(e.name().as_ref()) == b"sym" && field_result_symbol_char(e).is_some()
}

fn is_visible_reference_mark(name: &[u8]) -> bool {
    matches!(
        name,
        b"footnoteReference" | b"endnoteReference" | b"commentReference"
    )
}

fn inline_marker_text(e: &BytesStart<'_>) -> Option<&'static str> {
    match local(e.name().as_ref()) {
        b"tab" => Some("\t"),
        b"br" => {
            if is_page_break_type(e) {
                Some("\u{000C}")
            } else {
                Some("\n")
            }
        }
        b"cr" => Some("\n"),
        b"noBreakHyphen" => Some("-"),
        b"softHyphen" => Some("\u{00ad}"),
        _ => None,
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SequenceHeadingContext {
    field_scopes: Vec<[u32; 9]>,
}

impl SequenceHeadingContext {
    pub(crate) fn field_scope(&self, index: usize) -> Option<[u32; 9]> {
        self.field_scopes.get(index).copied()
    }
}

#[derive(Debug, Default)]
struct SequenceHeadingParagraph {
    depth: usize,
    properties_depth: usize,
    style_id: Option<String>,
    outline: Option<u8>,
    heading_applied: bool,
}

impl SequenceHeadingParagraph {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn active(&self) -> bool {
        self.depth > 0
    }
}

pub(crate) fn sequence_heading_context(xml: &str, styles: &Styles) -> SequenceHeadingContext {
    let mut r = Reader::from_str(xml);
    let mut context = SequenceHeadingContext::default();
    let mut current = Vec::new();
    let mut paragraph = SequenceHeadingParagraph::default();
    let mut heading_counts = [0u32; 9];
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
                if matches!(name, b"del" | b"moveFrom" | b"pPrChange") {
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
                        if paragraph.active() {
                            paragraph.depth += 1;
                        } else {
                            paragraph.reset();
                            paragraph.depth = 1;
                        }
                    }
                    b"pPr" if paragraph.active() => {
                        paragraph.properties_depth += 1;
                    }
                    b"pStyle" if paragraph.properties_depth > 0 => {
                        paragraph.style_id = attr_local_trimmed(&e, b"val");
                    }
                    b"outlineLvl" if paragraph.properties_depth > 0 => {
                        paragraph.outline = attr_u8(&e, b"val");
                    }
                    b"fldSimple" => {
                        record_sequence_heading_scope(
                            attr_local(&e, b"instr").as_deref(),
                            &heading_counts,
                            &mut context,
                        );
                        skip_subtree(&mut r);
                        consumed_element = true;
                    }
                    b"fldChar" => {
                        apply_sequence_heading_scan_fld_char(
                            &e,
                            &mut current,
                            heading_counts,
                            &mut context,
                        );
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
                    b"p" => {
                        paragraph.reset();
                    }
                    b"pPr" if paragraph.active() => {}
                    b"pStyle" if paragraph.properties_depth > 0 => {
                        paragraph.style_id = attr_local_trimmed(&e, b"val");
                    }
                    b"outlineLvl" if paragraph.properties_depth > 0 => {
                        paragraph.outline = attr_u8(&e, b"val");
                    }
                    b"fldSimple" => {
                        record_sequence_heading_scope(
                            attr_local(&e, b"instr").as_deref(),
                            &heading_counts,
                            &mut context,
                        );
                    }
                    b"fldChar" => {
                        apply_sequence_heading_scan_fld_char(
                            &e,
                            &mut current,
                            heading_counts,
                            &mut context,
                        );
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"pPr" if paragraph.properties_depth > 0 => {
                        paragraph.properties_depth -= 1;
                        if paragraph.properties_depth == 0 {
                            apply_sequence_paragraph_heading(
                                &mut paragraph,
                                styles,
                                &mut heading_counts,
                            );
                        }
                    }
                    b"p" if paragraph.depth > 1 => {
                        paragraph.depth -= 1;
                    }
                    b"p" if paragraph.active() => {
                        apply_sequence_paragraph_heading(
                            &mut paragraph,
                            styles,
                            &mut heading_counts,
                        );
                        paragraph.reset();
                    }
                    b"AlternateContent" => {
                        alternate_content_stack.pop();
                    }
                    _ => {}
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    context
}

fn apply_sequence_paragraph_heading(
    paragraph: &mut SequenceHeadingParagraph,
    styles: &Styles,
    heading_counts: &mut [u32; 9],
) {
    if paragraph.heading_applied {
        return;
    }
    let Some(level) = sequence_paragraph_heading_level(paragraph, styles) else {
        paragraph.heading_applied = true;
        return;
    };
    let index = usize::from(level - 1);
    heading_counts[index] = heading_counts[index].saturating_add(1);
    paragraph.heading_applied = true;
}

fn sequence_paragraph_heading_level(
    paragraph: &SequenceHeadingParagraph,
    styles: &Styles,
) -> Option<u8> {
    paragraph
        .outline
        .filter(|&level| level <= 8)
        .map(|level| level + 1)
        .or_else(|| {
            paragraph
                .style_id
                .as_deref()
                .and_then(|style_id| styles.heading_level(style_id))
        })
}

fn apply_sequence_heading_scan_fld_char(
    e: &BytesStart<'_>,
    current: &mut Vec<ComplexField>,
    heading_counts: [u32; 9],
    context: &mut SequenceHeadingContext,
) {
    apply_complex_field_scan_fld_char(e, current, |field| {
        record_sequence_heading_scope(Some(&field.instruction), &heading_counts, context);
    });
}

fn record_sequence_heading_scope(
    instruction: Option<&str>,
    heading_counts: &[u32; 9],
    context: &mut SequenceHeadingContext,
) {
    if instruction.is_some_and(is_sequence_field_instruction) {
        context.field_scopes.push(*heading_counts);
    }
}

fn is_sequence_field_instruction(instruction: &str) -> bool {
    matches!(
        field_kind(&normalize_instruction(instruction)),
        FieldKind::Sequence
    )
}

struct ComputedResultContexts<'a> {
    bookmarks: &'a HashMap<String, String>,
    ref_positions: &'a RefPositionContext,
    ref_numbers: &'a RefNumberContext,
    page_refs: &'a PageRefContext,
    note_refs: &'a NoteRefContext,
    sections: &'a SectionContext,
    style_refs: &'a StyleRefContext,
    legacy_forms: &'a LegacyFormContext,
    table_formulas: &'a TableFormulaContext,
    sequence_headings: &'a SequenceHeadingContext,
    toc_entries: &'a [TocEntry],
    bookmark_names: &'a HashSet<String>,
    core_properties: &'a CoreProperties,
    custom_properties: &'a HashMap<String, String>,
    document_variables: &'a HashMap<String, String>,
    extended_properties: &'a HashMap<String, String>,
    file_size_bytes: Option<usize>,
}

fn apply_computed_results(fields: &mut [Field], ctx: ComputedResultContexts<'_>) {
    let mut ref_field_index = 0usize;
    let mut page_field_index = 0usize;
    let mut page_ref_field_index = 0usize;
    let mut note_ref_field_index = 0usize;
    let mut section_field_index = 0usize;
    let mut style_ref_field_index = 0usize;
    let mut form_field_index = 0usize;
    let mut formula_field_index = 0usize;
    let mut sequence_field_index = 0usize;
    let mut sequence_counters = HashMap::new();
    let mut sequence_heading_scopes = HashMap::new();
    let mut autonum_counter = 0i64;
    let mut listnum_counter = 0i64;
    let mut field_bookmarks = HashMap::new();
    for field in fields {
        field.computed_result = match field.kind.clone() {
            FieldKind::Ref => {
                let position = ctx.ref_positions.field_position(ref_field_index);
                let note_ref_position = ctx.note_refs.ref_field_position(ref_field_index);
                ref_field_index += 1;
                let ref_ctx = RefResultContext {
                    bookmarks: ctx.bookmarks,
                    ref_positions: ctx.ref_positions,
                    ref_numbers: ctx.ref_numbers,
                    note_refs: ctx.note_refs,
                    field_bookmarks: &field_bookmarks,
                };
                computed_ref_result(&field.instruction, &ref_ctx, position, note_ref_position)
            }
            FieldKind::Page => {
                let position = ctx.page_refs.page_field_position(page_field_index);
                page_field_index += 1;
                computed_page_result(&field.instruction, position)
            }
            FieldKind::PageRef => {
                let position = ctx.page_refs.field_position(page_ref_field_index);
                let order = ctx.page_refs.field_order(page_ref_field_index);
                page_ref_field_index += 1;
                computed_page_ref_result(&field.instruction, ctx.page_refs, position, order)
            }
            FieldKind::NoteRef => {
                let position = ctx.note_refs.field_position(note_ref_field_index);
                note_ref_field_index += 1;
                computed_note_ref_result(&field.instruction, ctx.note_refs, position)
            }
            FieldKind::Sequence => {
                let heading_scope = ctx.sequence_headings.field_scope(sequence_field_index);
                sequence_field_index += 1;
                computed_sequence_result_with_heading_scope(
                    &field.instruction,
                    &mut sequence_counters,
                    heading_scope,
                    &mut sequence_heading_scopes,
                )
            }
            FieldKind::TocEntry => computed_toc_entry_result(&field.instruction),
            FieldKind::Toc => {
                computed_toc_result(&field.instruction, ctx.toc_entries, ctx.bookmark_names)
            }
            FieldKind::DocumentStructure(kind) if kind == "REVNUM" => {
                computed_revision_number_result(&field.instruction, ctx.core_properties)
            }
            FieldKind::DocumentStructure(kind) if kind == "SECTION" || kind == "SECTIONPAGES" => {
                if is_section_field_instruction(&field.instruction) {
                    let position = ctx.sections.field_position(section_field_index);
                    section_field_index += 1;
                    computed_section_result(&field.instruction, position)
                } else {
                    None
                }
            }
            FieldKind::DocumentStructure(kind) if kind == "STYLEREF" => {
                let position = ctx.style_refs.field_position(style_ref_field_index);
                style_ref_field_index += 1;
                computed_style_ref_result(&field.instruction, ctx.style_refs, position)
            }
            FieldKind::Dynamic(kind) if kind == "=" => {
                let result = ctx.table_formulas.field_result(formula_field_index);
                formula_field_index += 1;
                result.or_else(|| {
                    computed_formula_result_with_bookmark_context(
                        &field.instruction,
                        ctx.bookmarks,
                        &field_bookmarks,
                    )
                })
            }
            FieldKind::Dynamic(kind) if kind == "QUOTE" || kind == "FILLIN" || kind == "NEXT" => {
                computed_dynamic_result_with_bookmarks(&field.instruction, &field_bookmarks)
            }
            FieldKind::Dynamic(kind) if kind == "IF" || kind == "COMPARE" => {
                computed_if_compare_result_with_bookmark_context(
                    &field.instruction,
                    ctx.bookmarks,
                    &field_bookmarks,
                )
            }
            FieldKind::Dynamic(kind) if kind == "NEXTIF" || kind == "SKIPIF" => {
                computed_merge_control_result_with_bookmark_context(
                    &field.instruction,
                    ctx.bookmarks,
                    &field_bookmarks,
                )
            }
            FieldKind::Dynamic(kind) if kind == "ASK" => {
                computed_ask_result(&field.instruction, &mut field_bookmarks)
            }
            FieldKind::Dynamic(kind) if kind == "SET" => {
                computed_set_result(&field.instruction, &mut field_bookmarks)
            }
            FieldKind::DocumentInfo(_) => computed_document_info_result(
                &field.instruction,
                ctx.core_properties,
                ctx.custom_properties,
                ctx.document_variables,
                ctx.extended_properties,
                ctx.file_size_bytes,
            ),
            FieldKind::ReferenceIndex(kind) if kind == "RD" || kind == "TA" || kind == "XE" => {
                computed_reference_index_result(&field.instruction)
            }
            FieldKind::Display(_) => computed_display_result(&field.instruction),
            FieldKind::Action(kind)
                if kind == "GOTOBUTTON" || kind == "MACROBUTTON" || kind == "PRINT" =>
            {
                computed_action_result(&field.instruction)
            }
            FieldKind::FormField(_) => {
                let result = computed_legacy_form_result(
                    &field.instruction,
                    &field.result,
                    ctx.legacy_forms,
                    form_field_index,
                );
                form_field_index += 1;
                result
            }
            FieldKind::Numbering(kind)
                if kind == "AUTONUM"
                    || kind == "AUTONUMLGL"
                    || kind == "AUTONUMOUT"
                    || kind == "BIDIOUTLINE" =>
            {
                computed_numbering_result(&field.instruction, &mut autonum_counter)
            }
            FieldKind::Numbering(kind) if kind == "LISTNUM" => {
                computed_listnum_result(&field.instruction, &mut listnum_counter)
            }
            FieldKind::Unknown(_) => {
                let position = ctx.ref_positions.field_position(ref_field_index);
                let note_ref_position = ctx.note_refs.ref_field_position(ref_field_index);
                let is_ref_position = is_ref_position_field_instruction(&field.instruction);
                if is_ref_position {
                    ref_field_index += 1;
                }
                let spec = direct_bookmark_ref_instruction(&field.instruction);
                let ref_ctx = RefResultContext {
                    bookmarks: ctx.bookmarks,
                    ref_positions: ctx.ref_positions,
                    ref_numbers: ctx.ref_numbers,
                    note_refs: ctx.note_refs,
                    field_bookmarks: &field_bookmarks,
                };
                let result = spec.as_ref().and_then(|spec| {
                    let text = computed_ref_instruction_result(
                        spec,
                        &ref_ctx,
                        position.clone(),
                        note_ref_position,
                    )?;
                    Some(apply_field_text_format(text, spec.text_format))
                });
                if result.is_some()
                    || spec
                        .as_ref()
                        .is_some_and(|spec| ref_instruction_target_known(spec, &ref_ctx))
                {
                    field.kind = FieldKind::Ref;
                }
                result
            }
            _ => None,
        };
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FillInInstruction {
    default: Option<String>,
    text_format: Option<FieldTextFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AskInstruction {
    bookmark: String,
    default: Option<String>,
}

#[cfg(test)]
pub(crate) fn computed_dynamic_result(instruction: &str) -> Option<String> {
    computed_formula_result(instruction)
        .or_else(|| computed_quote_result(instruction))
        .or_else(|| computed_fill_in_result(instruction))
        .or_else(|| computed_if_result(instruction))
        .or_else(|| computed_compare_result(instruction))
        .or_else(|| computed_merge_control_result(instruction))
}

pub(crate) fn computed_dynamic_result_with_bookmarks(
    instruction: &str,
    field_bookmarks: &HashMap<String, String>,
) -> Option<String> {
    computed_formula_result_with_bookmarks(instruction, Some(field_bookmarks))
        .or_else(|| computed_quote_result(instruction))
        .or_else(|| computed_fill_in_result(instruction))
        .or_else(|| computed_if_result_with_bookmarks(instruction, field_bookmarks))
        .or_else(|| computed_compare_result_with_bookmarks(instruction, field_bookmarks))
        .or_else(|| computed_merge_control_result_with_bookmarks(instruction, field_bookmarks))
}

#[derive(Debug, Default)]
pub(crate) struct ContextlessFieldState<'a> {
    document_properties: Option<FieldDocumentProperties<'a>>,
    document_bookmarks: Option<&'a HashMap<String, String>>,
    note_refs: Option<&'a NoteRefContext>,
    sections: Option<&'a SectionContext>,
    style_refs: Option<&'a StyleRefContext>,
    legacy_forms: Option<&'a LegacyFormContext>,
    toc_entries: Option<&'a [TocEntry]>,
    bookmark_names: Option<&'a HashSet<String>>,
    field_bookmarks: HashMap<String, String>,
    section_field_index: usize,
    style_ref_field_index: usize,
    form_field_index: usize,
    sequence_counters: HashMap<String, i64>,
    autonum_counter: i64,
    listnum_counter: i64,
}

impl<'a> ContextlessFieldState<'a> {
    pub(crate) fn with_document_properties(properties: FieldDocumentProperties<'a>) -> Self {
        Self {
            document_properties: Some(properties),
            ..Self::default()
        }
    }

    pub(crate) fn with_document_context(
        properties: FieldDocumentProperties<'a>,
        document_bookmarks: &'a HashMap<String, String>,
    ) -> Self {
        Self {
            document_properties: Some(properties),
            document_bookmarks: Some(document_bookmarks),
            ..Self::default()
        }
    }

    pub(crate) fn with_document_and_note_context(
        properties: FieldDocumentProperties<'a>,
        document_bookmarks: &'a HashMap<String, String>,
        note_refs: &'a NoteRefContext,
    ) -> Self {
        Self {
            document_properties: Some(properties),
            document_bookmarks: Some(document_bookmarks),
            note_refs: Some(note_refs),
            ..Self::default()
        }
    }

    pub(crate) fn with_toc_context(
        mut self,
        toc_entries: &'a [TocEntry],
        bookmark_names: &'a HashSet<String>,
    ) -> Self {
        self.toc_entries = Some(toc_entries);
        self.bookmark_names = Some(bookmark_names);
        self
    }

    pub(crate) fn with_section_context(self, sections: &'a SectionContext) -> Self {
        self.with_section_context_from(sections, 0)
    }

    pub(crate) fn with_section_context_from(
        mut self,
        sections: &'a SectionContext,
        section_field_index: usize,
    ) -> Self {
        self.sections = Some(sections);
        self.section_field_index = section_field_index;
        self
    }

    pub(crate) fn section_field_index(&self) -> usize {
        self.section_field_index
    }

    pub(crate) fn with_style_ref_context_from(
        mut self,
        style_refs: &'a StyleRefContext,
        style_ref_field_index: usize,
    ) -> Self {
        self.style_refs = Some(style_refs);
        self.style_ref_field_index = style_ref_field_index;
        self
    }

    pub(crate) fn style_ref_field_index(&self) -> usize {
        self.style_ref_field_index
    }

    pub(crate) fn with_legacy_form_context_from(
        mut self,
        legacy_forms: &'a LegacyFormContext,
        form_field_index: usize,
    ) -> Self {
        self.legacy_forms = Some(legacy_forms);
        self.form_field_index = form_field_index;
        self
    }

    pub(crate) fn form_field_index(&self) -> usize {
        self.form_field_index
    }

    pub(crate) fn computed_legacy_text_form_current_result(
        &mut self,
        instruction: &str,
        current_result: &str,
    ) -> Option<String> {
        let legacy_forms = self.legacy_forms?;
        let spec = legacy_form_field_syntax(instruction)?;
        if spec.kind != "FORMTEXT" {
            return None;
        }
        let field_index = self.form_field_index;
        self.form_field_index += 1;
        computed_legacy_form_result(instruction, current_result, legacy_forms, field_index)
    }

    pub(crate) fn clear(&mut self) {
        self.field_bookmarks.clear();
        self.sequence_counters.clear();
        self.autonum_counter = 0;
        self.listnum_counter = 0;
    }
}

pub(crate) fn computed_contextless_result(
    instruction: &str,
    state: &mut ContextlessFieldState<'_>,
) -> Option<String> {
    if let Some(text) = computed_set_result(instruction, &mut state.field_bookmarks) {
        return Some(text);
    }
    if let Some(text) = computed_ask_result(instruction, &mut state.field_bookmarks) {
        return Some(text);
    }
    if let Some(document_bookmarks) = state.document_bookmarks {
        if let Some(text) = computed_formula_result_with_bookmark_context(
            instruction,
            document_bookmarks,
            &state.field_bookmarks,
        ) {
            return Some(text);
        }
        if let Some(text) = computed_if_compare_result_with_bookmark_context(
            instruction,
            document_bookmarks,
            &state.field_bookmarks,
        ) {
            return Some(text);
        }
        if let Some(text) = computed_merge_control_result_with_bookmark_context(
            instruction,
            document_bookmarks,
            &state.field_bookmarks,
        ) {
            return Some(text);
        }
        if let Some(text) = computed_contextless_ref_result(instruction, document_bookmarks, state)
        {
            return Some(text);
        }
    }
    if let Some(note_refs) = state.note_refs {
        if let Some(text) = computed_note_ref_result(instruction, note_refs, None) {
            return Some(text);
        }
    }
    if let Some(sections) = state.sections {
        if is_section_field_instruction(instruction) {
            let position = sections.field_position(state.section_field_index);
            state.section_field_index += 1;
            if let Some(text) = computed_section_result(instruction, position) {
                return Some(text);
            }
        }
    }
    if let Some(style_refs) = state.style_refs {
        if is_style_ref_field_instruction(instruction) {
            let position = style_refs.field_position(state.style_ref_field_index);
            state.style_ref_field_index += 1;
            if let Some(text) = computed_style_ref_result(instruction, style_refs, position) {
                return Some(text);
            }
        }
    }
    if let Some(legacy_forms) = state.legacy_forms {
        if let Some(spec) = legacy_form_field_syntax(instruction) {
            let field_index = state.form_field_index;
            state.form_field_index += 1;
            if spec.kind != "FORMTEXT" {
                if let Some(text) =
                    computed_legacy_form_result(instruction, "", legacy_forms, field_index)
                {
                    return Some(text);
                }
            }
        }
    }
    computed_dynamic_result_with_bookmarks(instruction, &state.field_bookmarks)
        .or_else(|| {
            if FieldKind::from_instruction(instruction) == FieldKind::Sequence {
                computed_sequence_result(instruction, &mut state.sequence_counters)
            } else {
                None
            }
        })
        .or_else(|| computed_toc_entry_result(instruction))
        .or_else(|| computed_toc_result(instruction, state.toc_entries?, state.bookmark_names?))
        .or_else(|| {
            if matches!(
                FieldKind::from_instruction(instruction),
                FieldKind::Numbering(kind)
                    if kind == "AUTONUM"
                        || kind == "AUTONUMLGL"
                        || kind == "AUTONUMOUT"
                        || kind == "BIDIOUTLINE"
            ) {
                computed_numbering_result(instruction, &mut state.autonum_counter)
            } else {
                None
            }
        })
        .or_else(|| {
            if matches!(
                FieldKind::from_instruction(instruction),
                FieldKind::Numbering(kind) if kind == "LISTNUM"
            ) {
                computed_listnum_result(instruction, &mut state.listnum_counter)
            } else {
                None
            }
        })
        .or_else(|| {
            let properties = state.document_properties?;
            computed_document_info_result(
                instruction,
                properties.core,
                properties.custom,
                properties.variables,
                properties.extended,
                properties.file_size_bytes,
            )
        })
        .or_else(|| {
            let properties = state.document_properties?;
            computed_revision_number_result(instruction, properties.core)
        })
        .or_else(|| computed_display_result(instruction))
        .or_else(|| computed_action_result(instruction))
        .or_else(|| computed_reference_index_result(instruction))
}

fn computed_contextless_ref_result(
    instruction: &str,
    document_bookmarks: &HashMap<String, String>,
    state: &ContextlessFieldState<'_>,
) -> Option<String> {
    let ref_positions = RefPositionContext::default();
    let ref_numbers = RefNumberContext::empty();
    let empty_note_refs = NoteRefContext::empty();
    let note_refs = state.note_refs.unwrap_or(&empty_note_refs);
    let ctx = RefResultContext {
        bookmarks: document_bookmarks,
        ref_positions: &ref_positions,
        ref_numbers: &ref_numbers,
        note_refs,
        field_bookmarks: &state.field_bookmarks,
    };
    computed_ref_result(instruction, &ctx, None, None)
        .or_else(|| computed_direct_bookmark_ref_result(instruction, &ctx, None, None))
        .or_else(|| computed_contextless_ref_note_reference_result(instruction, note_refs))
}

fn computed_contextless_ref_note_reference_result(
    instruction: &str,
    note_refs: &NoteRefContext,
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
    let number = note_refs.target_reference_number(&spec.target)?;
    let text = format_page_number(number, spec.number_format)?;
    Some(apply_field_text_format(text, spec.text_format))
}

pub(crate) fn computed_formula_result_with_bookmark_context(
    instruction: &str,
    document_bookmarks: &HashMap<String, String>,
    field_bookmarks: &HashMap<String, String>,
) -> Option<String> {
    let Some(formula_bookmarks) = merged_bookmark_context(document_bookmarks, field_bookmarks)
    else {
        return computed_formula_result_with_bookmarks(instruction, Some(field_bookmarks));
    };
    computed_formula_result_with_bookmarks(instruction, Some(&formula_bookmarks))
}

pub(crate) fn computed_if_compare_result_with_bookmark_context(
    instruction: &str,
    document_bookmarks: &HashMap<String, String>,
    field_bookmarks: &HashMap<String, String>,
) -> Option<String> {
    let Some(comparison_bookmarks) = merged_bookmark_context(document_bookmarks, field_bookmarks)
    else {
        return computed_if_result_with_bookmarks(instruction, field_bookmarks)
            .or_else(|| computed_compare_result_with_bookmarks(instruction, field_bookmarks));
    };
    computed_if_result_with_bookmarks(instruction, &comparison_bookmarks)
        .or_else(|| computed_compare_result_with_bookmarks(instruction, &comparison_bookmarks))
}

pub(crate) fn computed_merge_control_result_with_bookmark_context(
    instruction: &str,
    document_bookmarks: &HashMap<String, String>,
    field_bookmarks: &HashMap<String, String>,
) -> Option<String> {
    let Some(merge_control_bookmarks) =
        merged_bookmark_context(document_bookmarks, field_bookmarks)
    else {
        return computed_merge_control_result_with_bookmarks(instruction, field_bookmarks);
    };
    computed_merge_control_result_with_bookmarks(instruction, &merge_control_bookmarks)
}

fn merged_bookmark_context(
    document_bookmarks: &HashMap<String, String>,
    field_bookmarks: &HashMap<String, String>,
) -> Option<HashMap<String, String>> {
    if document_bookmarks.is_empty() {
        return None;
    }
    let mut bookmarks = document_bookmarks.clone();
    bookmarks.extend(
        field_bookmarks
            .iter()
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    Some(bookmarks)
}

#[cfg(test)]
fn computed_merge_control_result(instruction: &str) -> Option<String> {
    computed_merge_control_result_with_bookmarks(instruction, &HashMap::new())
}

fn computed_merge_control_result_with_bookmarks(
    instruction: &str,
    field_bookmarks: &HashMap<String, String>,
) -> Option<String> {
    merge_control_instruction(instruction, field_bookmarks)?;
    Some(String::new())
}

pub(crate) fn supports_filename_field_syntax(instruction: &str) -> bool {
    filename_field_syntax(instruction)
}

fn merge_control_instruction(
    instruction: &str,
    field_bookmarks: &HashMap<String, String>,
) -> Option<()> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if kind.eq_ignore_ascii_case("NEXT") {
        accept_field_format_tail(&mut parts)?;
        return Some(());
    }
    if kind.eq_ignore_ascii_case("NEXTIF") || kind.eq_ignore_ascii_case("SKIPIF") {
        let first = parts.next()?;
        let (left, operator, right) = comparison_operands(first, &mut parts, field_bookmarks)?;
        compare_if_operands(&left, operator, &right)?;
        accept_field_format_tail(&mut parts)?;
        return Some(());
    }
    None
}

pub(crate) fn supports_merge_control_field_syntax(instruction: &str) -> bool {
    merge_control_field_syntax(instruction)
}

fn field_format_tail<'a, I>(parts: &mut I) -> Option<Option<FieldTextFormat>>
where
    I: Iterator<Item = &'a str>,
{
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if accept_field_format_switch_for_tail(part, parts, &mut text_format)? {
            continue;
        }
        return None;
    }
    Some(text_format)
}

fn accept_field_format_tail<'a, I>(parts: &mut I) -> Option<()>
where
    I: Iterator<Item = &'a str>,
{
    field_format_tail(parts).map(|_| ())
}

fn accept_field_format_switch_for_tail<'a>(
    part: &'a str,
    parts: &mut impl Iterator<Item = &'a str>,
    text_format: &mut Option<FieldTextFormat>,
) -> Option<bool> {
    accept_general_format_switch(part, parts, |format| {
        accept_field_format_switch(format, text_format)
    })
}

fn accept_field_format_or_number_switch_for_tail<'a>(
    part: &'a str,
    parts: &mut impl Iterator<Item = &'a str>,
    text_format: &mut Option<FieldTextFormat>,
    number_format: &mut Option<FieldNumberFormat>,
) -> Option<bool> {
    accept_general_format_switch(part, parts, |format| {
        accept_field_format_switch(format, text_format)
            || accept_field_number_format_switch(format, number_format)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SetInstruction {
    name: String,
    value: String,
}

pub(crate) fn computed_set_result(
    instruction: &str,
    field_bookmarks: &mut HashMap<String, String>,
) -> Option<String> {
    let spec = set_instruction(instruction)?;
    field_bookmarks.insert(spec.name, spec.value);
    Some(String::new())
}

pub(crate) fn supports_set_field_syntax(instruction: &str) -> bool {
    set_field_syntax(instruction)
}

fn set_instruction(instruction: &str) -> Option<SetInstruction> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("SET") {
        return None;
    }
    let name = field_identifier_token(parts.next()?)?;
    let value = set_value_literal(&tokens[2..])?;
    Some(SetInstruction {
        name: name.to_string(),
        value,
    })
}

fn set_value_literal(tokens: &[String]) -> Option<String> {
    let first = tokens.first()?.as_str();
    if let Some(value) = quoted_literal_text(first) {
        let mut tail = tokens[1..].iter().map(String::as_str);
        accept_field_format_tail(&mut tail)?;
        return Some(value);
    }
    let mut values = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        let token = token.as_str();
        if is_field_format_start(token) {
            let mut tail = tokens[index..].iter().map(String::as_str);
            accept_field_format_tail(&mut tail)?;
            break;
        }
        if token.is_empty() || token.starts_with('\\') || token.contains('"') {
            return None;
        }
        values.push(token);
    }
    (!values.is_empty()).then(|| values.join(" "))
}

pub(crate) fn computed_quote_result(instruction: &str) -> Option<String> {
    let spec = quote_field_syntax(instruction)?;
    Some(apply_field_text_format(spec.text, spec.text_format))
}

pub(crate) fn supports_quote_field_syntax(instruction: &str) -> bool {
    quote_field_syntax(instruction).is_some()
}

fn computed_fill_in_result(instruction: &str) -> Option<String> {
    let spec = fill_in_instruction(instruction)?;
    let text = spec.default?;
    Some(apply_field_text_format(text, spec.text_format))
}

fn fill_in_instruction(instruction: &str) -> Option<FillInInstruction> {
    match prompt_field_syntax(instruction)? {
        PromptFieldSyntax::FillIn {
            default,
            text_format,
        } => Some(FillInInstruction {
            default,
            text_format,
        }),
        PromptFieldSyntax::Ask { .. } => None,
    }
}

pub(crate) fn supports_prompt_field_syntax(instruction: &str) -> bool {
    fill_in_instruction(instruction).is_some() || ask_instruction(instruction).is_some()
}

pub(crate) fn supports_context_free_fill_in_field_syntax(instruction: &str) -> bool {
    computed_fill_in_result(instruction).is_some()
}

pub(crate) fn computed_ask_result(
    instruction: &str,
    field_bookmarks: &mut HashMap<String, String>,
) -> Option<String> {
    let spec = ask_instruction(instruction)?;
    let value = spec.default?;
    field_bookmarks.insert(spec.bookmark, value);
    Some(String::new())
}

fn ask_instruction(instruction: &str) -> Option<AskInstruction> {
    match prompt_field_syntax(instruction)? {
        PromptFieldSyntax::Ask { bookmark, default } => Some(AskInstruction { bookmark, default }),
        PromptFieldSyntax::FillIn { .. } => None,
    }
}

pub(super) fn quoted_literal_text(token: &str) -> Option<String> {
    let bytes = token.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'"' || *bytes.last()? != b'"' {
        return None;
    }
    let text = &token[1..token.len() - 1];
    (!text.contains('"')).then(|| text.to_string())
}

#[derive(Debug, Clone, PartialEq)]
enum IfOperand {
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IfOperator {
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

#[derive(Debug, Clone, PartialEq)]
struct IfInstruction {
    left: IfOperand,
    operator: IfOperator,
    right: IfOperand,
    true_text: String,
    false_text: String,
    text_format: Option<FieldTextFormat>,
}

fn computed_if_result(instruction: &str) -> Option<String> {
    computed_if_result_with_bookmarks(instruction, &HashMap::new())
}

fn computed_if_result_with_bookmarks(
    instruction: &str,
    field_bookmarks: &HashMap<String, String>,
) -> Option<String> {
    let spec = if_instruction(instruction, field_bookmarks)?;
    let selected = if compare_if_operands(&spec.left, spec.operator, &spec.right)? {
        spec.true_text
    } else {
        spec.false_text
    };
    Some(apply_field_text_format(selected, spec.text_format))
}

fn if_instruction(
    instruction: &str,
    field_bookmarks: &HashMap<String, String>,
) -> Option<IfInstruction> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("IF") {
        return None;
    }
    let first = parts.next()?;
    let (left, operator, right) = comparison_operands(first, &mut parts, field_bookmarks)?;
    let true_text = if_result_text(parts.next()?)?;
    let mut false_text = String::new();
    let mut text_format = None;
    if let Some(part) = parts.next() {
        if !accept_if_format_switch(part, &mut parts, &mut text_format)? {
            false_text = if_result_text(part)?;
        }
    }
    while let Some(part) = parts.next() {
        if !accept_if_format_switch(part, &mut parts, &mut text_format)? {
            return None;
        }
    }
    Some(IfInstruction {
        left,
        operator,
        right,
        true_text,
        false_text,
        text_format,
    })
}

pub(crate) fn supports_if_field_syntax(instruction: &str) -> bool {
    if_field_syntax(instruction)
}

fn computed_compare_result(instruction: &str) -> Option<String> {
    computed_compare_result_with_bookmarks(instruction, &HashMap::new())
}

fn computed_compare_result_with_bookmarks(
    instruction: &str,
    field_bookmarks: &HashMap<String, String>,
) -> Option<String> {
    let spec = compare_instruction(instruction, field_bookmarks)?;
    let result = if compare_if_operands(&spec.left, spec.operator, &spec.right)? {
        "1"
    } else {
        "0"
    };
    Some(apply_field_text_format(
        result.to_string(),
        spec.text_format,
    ))
}

pub(crate) fn supports_context_free_if_compare_field_syntax(instruction: &str) -> bool {
    computed_if_result(instruction)
        .or_else(|| computed_compare_result(instruction))
        .is_some()
}

pub(crate) fn supports_compare_field_syntax(instruction: &str) -> bool {
    compare_field_syntax(instruction)
}

fn compare_instruction(
    instruction: &str,
    field_bookmarks: &HashMap<String, String>,
) -> Option<IfInstruction> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("COMPARE") {
        return None;
    }
    let first = parts.next()?;
    let (left, operator, right) = comparison_operands(first, &mut parts, field_bookmarks)?;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if !accept_if_format_switch(part, &mut parts, &mut text_format)? {
            return None;
        }
    }
    Some(IfInstruction {
        left,
        operator,
        right,
        true_text: "1".to_string(),
        false_text: "0".to_string(),
        text_format,
    })
}

fn comparison_operands<'a, I>(
    first: &str,
    parts: &mut I,
    field_bookmarks: &HashMap<String, String>,
) -> Option<(IfOperand, IfOperator, IfOperand)>
where
    I: Iterator<Item = &'a str>,
{
    if let Some(comparison) = compact_if_comparison(first, field_bookmarks) {
        Some(comparison)
    } else {
        Some((
            if_operand(first, field_bookmarks)?,
            if_operator(parts.next()?)?,
            if_operand(parts.next()?, field_bookmarks)?,
        ))
    }
}

fn if_operand(token: &str, field_bookmarks: &HashMap<String, String>) -> Option<IfOperand> {
    if let Some(text) = quoted_literal_text(token) {
        return Some(IfOperand::Text(text));
    }
    if let Some(value) = token.parse::<f64>().ok().filter(|value| value.is_finite()) {
        return Some(IfOperand::Number(value));
    }
    let name = field_name_token(token)?;
    field_bookmarks
        .get(name)
        .map(|value| bookmark_if_operand(value))
}

fn bookmark_if_operand(value: &str) -> IfOperand {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .map(IfOperand::Number)
        .unwrap_or_else(|| IfOperand::Text(value.to_string()))
}

fn compact_if_comparison(
    token: &str,
    field_bookmarks: &HashMap<String, String>,
) -> Option<(IfOperand, IfOperator, IfOperand)> {
    for operator in [">=", "<=", "<>", "=", ">", "<"] {
        let Some(index) = find_unquoted_operator(token, operator) else {
            continue;
        };
        let (left, right_with_operator) = token.split_at(index);
        let right = &right_with_operator[operator.len()..];
        if left.is_empty() || right.is_empty() {
            return None;
        }
        return Some((
            if_operand(left, field_bookmarks)?,
            if_operator(operator)?,
            if_operand(right, field_bookmarks)?,
        ));
    }
    None
}

fn find_unquoted_operator(token: &str, operator: &str) -> Option<usize> {
    let mut in_quotes = false;
    for (index, ch) in token.char_indices() {
        if ch == '"' {
            in_quotes = !in_quotes;
        } else if !in_quotes && token[index..].starts_with(operator) {
            return Some(index);
        }
    }
    None
}

fn if_result_text(token: &str) -> Option<String> {
    (!token.starts_with('\\')).then(|| field_literal_token(token).map(str::to_string))?
}

fn if_operator(token: &str) -> Option<IfOperator> {
    match token {
        "=" => Some(IfOperator::Eq),
        "<>" => Some(IfOperator::Ne),
        ">" => Some(IfOperator::Gt),
        "<" => Some(IfOperator::Lt),
        ">=" => Some(IfOperator::Ge),
        "<=" => Some(IfOperator::Le),
        _ => None,
    }
}

fn compare_if_operands(left: &IfOperand, operator: IfOperator, right: &IfOperand) -> Option<bool> {
    match (left, right) {
        (IfOperand::Number(left), IfOperand::Number(right)) => match operator {
            IfOperator::Eq => Some(left == right),
            IfOperator::Ne => Some(left != right),
            IfOperator::Gt => Some(left > right),
            IfOperator::Lt => Some(left < right),
            IfOperator::Ge => Some(left >= right),
            IfOperator::Le => Some(left <= right),
        },
        (IfOperand::Text(left), IfOperand::Text(right)) => match operator {
            IfOperator::Eq => Some(compare_text_operands(left, right)),
            IfOperator::Ne => Some(!compare_text_operands(left, right)),
            _ => None,
        },
        _ => None,
    }
}

fn compare_text_operands(left: &str, right: &str) -> bool {
    if right.contains(['*', '?']) {
        wildcard_match(left, right)
    } else if left.contains(['*', '?']) {
        wildcard_match(right, left)
    } else {
        left == right
    }
}

pub(crate) fn computed_reference_index_result(instruction: &str) -> Option<String> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if kind.eq_ignore_ascii_case("TA") {
        reference_index_ta_instruction(parts)?;
        return Some(String::new());
    }
    if kind.eq_ignore_ascii_case("XE") {
        reference_index_xe_instruction(parts)?;
        return Some(String::new());
    }
    if kind.eq_ignore_ascii_case("RD") {
        reference_index_rd_instruction(parts)?;
        return Some(String::new());
    }
    None
}

pub(crate) fn supports_reference_index_marker_syntax(instruction: &str) -> bool {
    computed_reference_index_result(instruction).is_some()
}

fn reference_index_rd_instruction<'a>(mut parts: impl Iterator<Item = &'a str>) -> Option<()> {
    reference_index_literal_token(parts.next()?)?;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("\\f") {
            continue;
        }
        if accept_reference_index_field_format(part, &mut parts, &mut text_format).is_some() {
            continue;
        }
        return None;
    }
    Some(())
}

fn reference_index_ta_instruction<'a>(parts: impl Iterator<Item = &'a str>) -> Option<()> {
    let mut parts = parts.peekable();
    let mut has_entry_text = false;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("\\l") || part.eq_ignore_ascii_case("\\s") {
            reference_index_literal_operand(parts.next()?, &mut parts)?;
            has_entry_text = true;
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\l")
            .or_else(|| strip_ascii_switch_prefix(part, "\\s"))
        {
            if value.is_empty() {
                return None;
            }
            reference_index_literal_token(value)?;
            has_entry_text = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\c") {
            reference_index_category_token(parts.next()?)?;
            continue;
        }
        if let Some(category) = strip_ascii_switch_prefix(part, "\\c") {
            if category.is_empty() {
                return None;
            }
            reference_index_category_token(category)?;
            continue;
        }
        if accept_reference_index_field_format(part, &mut parts, &mut text_format).is_some() {
            continue;
        }
        return None;
    }
    has_entry_text.then_some(())
}

fn reference_index_xe_instruction<'a>(parts: impl Iterator<Item = &'a str>) -> Option<()> {
    let mut parts = parts.peekable();
    reference_index_literal_token(parts.next()?)?;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if part.eq_ignore_ascii_case("\\b") || part.eq_ignore_ascii_case("\\i") {
            continue;
        }
        if part.eq_ignore_ascii_case("\\f") || part.eq_ignore_ascii_case("\\r") {
            reference_index_plain_value_token(parts.next()?)?;
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\f")
            .or_else(|| strip_ascii_switch_prefix(part, "\\r"))
        {
            if value.is_empty() {
                return None;
            }
            reference_index_plain_value_token(value)?;
            continue;
        }
        if part.eq_ignore_ascii_case("\\t") {
            reference_index_literal_operand(parts.next()?, &mut parts)?;
            continue;
        }
        if let Some(value) = strip_ascii_switch_prefix(part, "\\t") {
            if value.is_empty() {
                return None;
            }
            reference_index_literal_token(value)?;
            continue;
        }
        if accept_reference_index_field_format(part, &mut parts, &mut text_format).is_some() {
            continue;
        }
        return None;
    }
    Some(())
}

fn accept_reference_index_field_format<'a>(
    part: &'a str,
    parts: &mut impl Iterator<Item = &'a str>,
    text_format: &mut Option<FieldTextFormat>,
) -> Option<()> {
    accept_field_format_switch_for_tail(part, parts, text_format)
        .and_then(|accepted| accepted.then_some(()))
}

fn wildcard_match(text: &str, pattern: &str) -> bool {
    let text: Vec<char> = text.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    let mut matches = vec![vec![false; pattern.len() + 1]; text.len() + 1];
    matches[0][0] = true;
    for pattern_index in 1..=pattern.len() {
        if pattern[pattern_index - 1] == '*' {
            matches[0][pattern_index] = matches[0][pattern_index - 1];
        }
    }
    for text_index in 1..=text.len() {
        for pattern_index in 1..=pattern.len() {
            matches[text_index][pattern_index] = match pattern[pattern_index - 1] {
                '*' => {
                    matches[text_index][pattern_index - 1] || matches[text_index - 1][pattern_index]
                }
                '?' => matches[text_index - 1][pattern_index - 1],
                ch => matches[text_index - 1][pattern_index - 1] && text[text_index - 1] == ch,
            };
        }
    }
    matches[text.len()][pattern.len()]
}

pub(super) fn is_field_format_start(part: &str) -> bool {
    part == "\\*" || part.starts_with("\\*")
}

fn accept_if_format_switch<'a, I>(
    part: &'a str,
    parts: &mut I,
    text_format: &mut Option<FieldTextFormat>,
) -> Option<bool>
where
    I: Iterator<Item = &'a str>,
{
    accept_field_format_switch_for_tail(part, parts, text_format)
}

pub(crate) fn supports_action_field_syntax(instruction: &str) -> bool {
    action_field_syntax(instruction).is_some()
}

pub(crate) fn computed_action_result(instruction: &str) -> Option<String> {
    let spec = action_field_syntax(instruction)?;
    let text = spec.computed_text?;
    Some(apply_field_text_format(text, spec.text_format))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceAction {
    Next,
    Current,
    Reset(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequenceInstruction {
    identifier: String,
    action: SequenceAction,
    heading_reset: Option<u8>,
    hidden: bool,
    number_format: Option<PageNumberFormat>,
    text_format: Option<FieldTextFormat>,
}

fn sequence_instruction(instruction: &str) -> Option<SequenceInstruction> {
    sequence_instruction_with_reset_policy(instruction, false, false)
}

fn sequence_instruction_with_reset_policy(
    instruction: &str,
    allow_negative_reset: bool,
    allow_heading_reset: bool,
) -> Option<SequenceInstruction> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("SEQ") {
        return None;
    }
    let identifier = field_identifier_token(parts.next()?)?.to_string();
    let mut action = SequenceAction::Next;
    let mut action_seen = false;
    let mut heading_reset_seen = false;
    let mut heading_reset = None;
    let mut hidden = false;
    let mut number_format = None;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if accept_page_field_format_switch_for_tail(
            part,
            &mut parts,
            &mut number_format,
            &mut text_format,
        )? {
            continue;
        }
        if part.eq_ignore_ascii_case("\\n") {
            if action_seen {
                return None;
            }
            action_seen = true;
            action = SequenceAction::Next;
            continue;
        }
        if part.eq_ignore_ascii_case("\\c") {
            if action_seen {
                return None;
            }
            action_seen = true;
            action = SequenceAction::Current;
            continue;
        }
        if part.eq_ignore_ascii_case("\\h") {
            if hidden {
                return None;
            }
            hidden = true;
            continue;
        }
        if part.eq_ignore_ascii_case("\\s") {
            let level = parts.next()?;
            heading_reset = Some(accept_sequence_heading_reset(
                level,
                &mut heading_reset_seen,
                allow_heading_reset,
            )?);
            continue;
        }
        if let Some(level) = strip_ascii_switch_prefix(part, "\\s") {
            if level.is_empty() {
                return None;
            }
            heading_reset = Some(accept_sequence_heading_reset(
                level,
                &mut heading_reset_seen,
                allow_heading_reset,
            )?);
            continue;
        }
        if part.eq_ignore_ascii_case("\\r") {
            if action_seen {
                return None;
            }
            let reset = field_name_token(parts.next()?)?.parse::<i64>().ok()?;
            action_seen = true;
            action = SequenceAction::Reset(sequence_reset_value(reset, allow_negative_reset)?);
            continue;
        }
        if let Some(reset) = strip_ascii_switch_prefix(part, "\\r") {
            if reset.is_empty() || action_seen {
                return None;
            }
            action_seen = true;
            let reset = field_name_token(reset)?.parse::<i64>().ok()?;
            action = SequenceAction::Reset(sequence_reset_value(reset, allow_negative_reset)?);
            continue;
        }
        return None;
    }
    Some(SequenceInstruction {
        identifier,
        action,
        heading_reset,
        hidden,
        number_format,
        text_format,
    })
}

fn accept_sequence_heading_reset(
    value: &str,
    heading_reset_seen: &mut bool,
    allow_heading_reset: bool,
) -> Option<u8> {
    if !allow_heading_reset || *heading_reset_seen {
        return None;
    }
    let level = field_name_token(value)?.parse::<u8>().ok()?;
    if !(1..=9).contains(&level) {
        return None;
    }
    *heading_reset_seen = true;
    Some(level)
}

fn sequence_reset_value(value: i64, allow_negative: bool) -> Option<i64> {
    (allow_negative || value >= 0).then_some(value)
}

pub(crate) fn supports_sequence_field_syntax(instruction: &str) -> bool {
    sequence_field_syntax(instruction)
}

pub(crate) fn computed_sequence_result(
    instruction: &str,
    counters: &mut HashMap<String, i64>,
) -> Option<String> {
    let instruction = sequence_instruction(instruction)?;
    computed_sequence_instruction_result(instruction, counters)
}

pub(crate) fn computed_preserved_sequence_reset_result(instruction: &str) -> Option<String> {
    let instruction = sequence_instruction(instruction)?;
    if instruction.heading_reset.is_some()
        || instruction.hidden
        || !matches!(instruction.action, SequenceAction::Reset(_))
    {
        return None;
    }
    computed_sequence_instruction_result(instruction, &mut HashMap::new())
}

pub(crate) fn computed_sequence_result_with_heading_scope(
    instruction: &str,
    counters: &mut HashMap<String, i64>,
    heading_scope: Option<[u32; 9]>,
    heading_scopes: &mut HashMap<(String, u8), u32>,
) -> Option<String> {
    let instruction = sequence_instruction_with_reset_policy(instruction, false, true)?;
    if let Some(level) = instruction.heading_reset {
        let scope = heading_scope?[usize::from(level - 1)];
        let key = (instruction.identifier.clone(), level);
        if heading_scopes.get(&key).copied() != Some(scope) {
            counters.insert(instruction.identifier.clone(), 0);
            heading_scopes.insert(key, scope);
        }
    }
    computed_sequence_instruction_result(instruction, counters)
}

fn computed_sequence_instruction_result(
    instruction: SequenceInstruction,
    counters: &mut HashMap<String, i64>,
) -> Option<String> {
    let value = match instruction.action {
        SequenceAction::Next => counters.get(&instruction.identifier).copied().unwrap_or(0) + 1,
        SequenceAction::Current => counters.get(&instruction.identifier).copied()?,
        SequenceAction::Reset(value) => value,
    };
    let text = if instruction.hidden {
        if value < 0 {
            return None;
        }
        String::new()
    } else {
        format_sequence_number(value, instruction.number_format)?
    };
    if !matches!(instruction.action, SequenceAction::Current) {
        counters.insert(instruction.identifier, value);
    }
    Some(apply_field_text_format(text, instruction.text_format))
}

fn format_sequence_number(value: i64, format: Option<PageNumberFormat>) -> Option<String> {
    let value = usize::try_from(value).ok()?;
    format_page_number(value, format)
}

pub(crate) fn computed_numbering_result(
    instruction: &str,
    autonum_counter: &mut i64,
) -> Option<String> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("AUTONUM")
        && !kind.eq_ignore_ascii_case("AUTONUMLGL")
        && !kind.eq_ignore_ascii_case("AUTONUMOUT")
        && !kind.eq_ignore_ascii_case("BIDIOUTLINE")
    {
        return None;
    }
    let accepts_separator_switch = kind.eq_ignore_ascii_case("AUTONUM");
    let mut number_format = None;
    let mut text_format = None;
    let mut separator = None;
    while let Some(part) = parts.next() {
        if accept_page_field_format_switch_for_tail(
            part,
            &mut parts,
            &mut number_format,
            &mut text_format,
        )? {
            continue;
        }
        if accepts_separator_switch && part.eq_ignore_ascii_case("\\s") {
            accept_autonum_separator_switch(parts.next()?, &mut separator)?;
            continue;
        }
        if accepts_separator_switch {
            if let Some(value) = strip_ascii_switch_prefix(part, "\\s") {
                accept_autonum_separator_switch(value, &mut separator)?;
                continue;
            }
        }
        if strip_ascii_switch_prefix(part, "\\s").is_some() || part.eq_ignore_ascii_case("\\s") {
            return None;
        }
        return None;
    }
    let value = *autonum_counter + 1;
    let mut text = format_sequence_number(value, number_format)?;
    if let Some(separator) = separator {
        text.push(separator);
    }
    *autonum_counter = value;
    Some(apply_field_text_format(text, text_format))
}

pub(crate) fn supports_numbering_field_syntax(instruction: &str) -> bool {
    numbering_field_syntax(instruction)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ListNumInstruction {
    reset_start: Option<i64>,
    number_format: Option<PageNumberFormat>,
    text_format: Option<FieldTextFormat>,
}

fn listnum_instruction(instruction: &str) -> Option<ListNumInstruction> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("LISTNUM") {
        return None;
    }
    let mut list_name_seen = false;
    let mut level_seen = false;
    let mut reset_start = None;
    let mut number_format = None;
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if accept_page_field_format_switch_for_tail(
            part,
            &mut parts,
            &mut number_format,
            &mut text_format,
        )? {
            continue;
        }
        if part.eq_ignore_ascii_case("\\l") {
            accept_listnum_level_switch(parts.next()?, &mut level_seen)?;
            continue;
        }
        if let Some(level) = strip_ascii_switch_prefix(part, "\\l") {
            if level.is_empty() {
                return None;
            }
            accept_listnum_level_switch(level, &mut level_seen)?;
            continue;
        }
        if part.eq_ignore_ascii_case("\\s") {
            accept_listnum_start_switch(parts.next()?, &mut reset_start)?;
            continue;
        }
        if let Some(start) = strip_ascii_switch_prefix(part, "\\s") {
            if start.is_empty() {
                return None;
            }
            accept_listnum_start_switch(start, &mut reset_start)?;
            continue;
        }
        if part.starts_with('\\') || list_name_seen {
            return None;
        }
        let list_name = unquote_field_text(part);
        if !list_name.eq_ignore_ascii_case("NumberDefault")
            && !list_name.eq_ignore_ascii_case("LegalDefault")
        {
            return None;
        }
        list_name_seen = true;
    }
    Some(ListNumInstruction {
        reset_start,
        number_format,
        text_format,
    })
}

fn computed_listnum_instruction_result(
    instruction: ListNumInstruction,
    listnum_counter: &mut i64,
) -> Option<String> {
    let value = match instruction.reset_start {
        Some(value) => value,
        None => listnum_counter.checked_add(1)?,
    };
    let text = format_sequence_number(value, instruction.number_format)?;
    *listnum_counter = value;
    Some(apply_field_text_format(text, instruction.text_format))
}

pub(crate) fn computed_listnum_result(
    instruction: &str,
    listnum_counter: &mut i64,
) -> Option<String> {
    computed_listnum_instruction_result(listnum_instruction(instruction)?, listnum_counter)
}

pub(crate) fn computed_preserved_listnum_start_result(instruction: &str) -> Option<String> {
    let instruction = listnum_instruction(instruction)?;
    instruction.reset_start?;
    computed_listnum_instruction_result(instruction, &mut 0)
}

fn accept_listnum_level_switch(part: &str, level_seen: &mut bool) -> Option<()> {
    if *level_seen {
        return None;
    }
    let level = field_name_token(part)?.parse::<u8>().ok()?;
    if level != 1 {
        return None;
    }
    *level_seen = true;
    Some(())
}

fn accept_listnum_start_switch(part: &str, reset_start: &mut Option<i64>) -> Option<()> {
    let start = field_name_token(part)?.parse::<i64>().ok()?;
    if start < 0 || reset_start.replace(start).is_some() {
        return None;
    }
    Some(())
}

fn accept_autonum_separator_switch(part: &str, separator: &mut Option<char>) -> Option<()> {
    let value = field_literal_token(part)?;
    let mut chars = value.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || separator.replace(ch).is_some() {
        return None;
    }
    Some(())
}

fn capitalize_first_word(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut changed = false;
    for ch in text.chars() {
        if !changed && ch.is_alphabetic() {
            out.extend(ch.to_uppercase());
            changed = true;
        } else {
            out.push(ch);
        }
    }
    out
}

fn capitalize_words(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut at_word_start = true;
    for ch in text.chars() {
        if ch.is_alphabetic() {
            if at_word_start {
                out.extend(ch.to_uppercase());
            } else {
                out.push(ch);
            }
            at_word_start = false;
        } else {
            out.push(ch);
            at_word_start = !ch.is_alphanumeric();
        }
    }
    out
}

fn apply_field_text_format(text: String, format: Option<FieldTextFormat>) -> String {
    match format {
        Some(FieldTextFormat::Upper) => text.to_uppercase(),
        Some(FieldTextFormat::Lower) => text.to_lowercase(),
        Some(FieldTextFormat::Caps) => capitalize_words(&text),
        Some(FieldTextFormat::FirstCap) => capitalize_first_word(&text),
        None => text,
    }
}

fn normalize_instruction(s: &str) -> String {
    instruction_parts(s).join(" ")
}

fn field_kind(instruction: &str) -> FieldKind {
    FieldKind::from_instruction(instruction)
}

#[cfg(test)]
mod tests {
    use super::{
        cardinal_page_number_text, computed_action_result, computed_ask_result,
        computed_display_result, computed_dynamic_result, computed_listnum_result,
        computed_numbering_result, computed_preserved_document_info_result,
        computed_preserved_listnum_start_result, computed_preserved_note_local_ref_result,
        computed_preserved_note_local_style_ref_result, computed_preserved_revision_number_result,
        computed_preserved_sequence_reset_result, computed_reference_index_result,
        computed_sequence_result, computed_set_result, computed_toc_entry_result,
        direct_bookmark_ref_instruction, document_info_instruction, format_page_number,
        note_ref_context, note_ref_instruction, ordinal_page_number_text, page_ref_context,
        page_ref_instruction, preserved_note_local_ref_target,
        preserved_note_local_style_ref_target, ref_instruction, ref_position_context, ref_targets,
        seq_identifier_from_instruction, style_ref_instruction, supports_action_field_syntax,
        supports_compare_field_syntax, supports_computed_symbol_field_syntax,
        supports_context_free_display_field_syntax, supports_context_free_fill_in_field_syntax,
        supports_context_free_formula_field_syntax, supports_context_free_if_compare_field_syntax,
        supports_formula_field_syntax, supports_if_field_syntax,
        supports_merge_control_field_syntax, supports_preserved_document_info_field_syntax,
        supports_prompt_field_syntax, supports_reference_index_marker_syntax,
        supports_revision_number_field_syntax, supports_sequence_field_syntax,
        supports_toc_entry_field_syntax, table_formula_context, toc_entries, toc_spec,
        PageNumberFormat, TocEntrySource,
    };
    use crate::docx::numbering::Numbering;
    use crate::docx::styles::Styles;
    use crate::CoreProperties;
    use std::collections::HashMap;

    #[test]
    fn page_number_text_formats_cover_compound_values() {
        assert_eq!(
            cardinal_page_number_text(342).as_deref(),
            Some("three hundred forty-two")
        );
        assert_eq!(
            ordinal_page_number_text(321).as_deref(),
            Some("three hundred twenty-first")
        );
        assert_eq!(
            format_page_number(21, Some(PageNumberFormat::OrdText)).as_deref(),
            Some("twenty-first")
        );
        assert_eq!(
            format_page_number(21, Some(PageNumberFormat::ArabicDash)).as_deref(),
            Some("- 21 -")
        );
        assert_eq!(
            format_page_number(4, Some(PageNumberFormat::DecimalZero)).as_deref(),
            Some("04")
        );
        assert_eq!(
            format_page_number(1_005, Some(PageNumberFormat::CardText)).as_deref(),
            Some("one thousand five")
        );
    }

    #[test]
    fn formula_numeric_pictures_use_common_format_tail() {
        assert_eq!(
            computed_dynamic_result(r#"= 10 / 4 \# "0.00" \* MERGEFORMAT"#).as_deref(),
            Some("2.50")
        );
        assert_eq!(
            computed_dynamic_result(r#"= 10 / 4 \#"0.0" \*CHARFORMAT"#).as_deref(),
            Some("2.5")
        );
        assert_eq!(
            computed_dynamic_result(r#"= 10 / 4 \# "0.00" \* MERGEFORMATINET"#).as_deref(),
            Some("2.50")
        );
        assert_eq!(
            computed_dynamic_result(r#"= 10 / 4 \# "0.00" \* Upper"#).as_deref(),
            Some("2.50")
        );
        assert_eq!(
            computed_dynamic_result(r#"= 5 \# "0 units" \* Upper"#).as_deref(),
            Some("5 UNITS")
        );
    }

    #[test]
    fn formula_general_number_switches_format_literal_results() {
        assert_eq!(
            computed_dynamic_result(r#"= 10.25 \* DollarText"#).as_deref(),
            Some("ten and 25/100")
        );
        assert_eq!(
            computed_dynamic_result(r#"= 31 \* Hex"#).as_deref(),
            Some("1F")
        );
        assert_eq!(
            computed_dynamic_result(r#"= 21 \* OrdText"#).as_deref(),
            Some("twenty-first")
        );
        assert_eq!(computed_dynamic_result(r#"= 10.25 \* Hex"#), None);
    }

    #[test]
    fn formula_syntax_accepts_data_identifiers_and_rejects_malformed_bodies() {
        assert!(supports_formula_field_syntax(
            r#"= CustomerTotal \# "0.00""#
        ));
        assert!(supports_formula_field_syntax("= CustomerTotal + TaxTotal"));
        assert!(!supports_formula_field_syntax("= 1 +"));
        assert!(!supports_formula_field_syntax("= (1 + 2"));
        assert!(!supports_formula_field_syntax("= 1e+"));
    }

    #[test]
    fn context_free_formula_ownership_requires_context_independent_finite_results() {
        for instruction in [
            r#"= 10 / 4 \# "0.00""#,
            r#"= ROUND(AVERAGE(2; 4; 7); 1) \# "0.0""#,
            "= IF(1, 7, Missing + 1)",
        ] {
            assert!(supports_context_free_formula_field_syntax(instruction));
        }
        for instruction in [
            "= Amount + 1",
            "= SUM(LEFT)",
            "= A1 + 1",
            "= DEFINED(Known)",
            "= defined (Known)",
            "= IF(1, 7, DEFINED(Known))",
            "= 1 / 0",
            "= 1e309 + 1",
            "= 1 +",
            r#"QUOTE "not a formula""#,
        ] {
            assert!(!supports_context_free_formula_field_syntax(instruction));
        }
    }

    #[test]
    fn ask_default_result_populates_field_bookmark() {
        let mut field_bookmarks = HashMap::new();

        assert_eq!(
            computed_ask_result(
                r#"ASK ClientCode "Client code?" \d "ac-42" \o"#,
                &mut field_bookmarks,
            )
            .as_deref(),
            Some("")
        );
        assert_eq!(
            field_bookmarks.get("ClientCode").map(String::as_str),
            Some("ac-42")
        );
        assert_eq!(
            computed_dynamic_result(r#"ASK ClientCode "Client code?" \d "ac-42" \o"#),
            None
        );
    }

    #[test]
    fn ask_accepts_field_text_format_switches_without_formatting_bookmark() {
        let mut field_bookmarks = HashMap::new();

        assert_eq!(
            computed_ask_result(
                r#"ASK ClientCode "Client code?" \d "ac-42" \* Upper"#,
                &mut field_bookmarks,
            )
            .as_deref(),
            Some("")
        );
        assert_eq!(
            field_bookmarks.get("ClientCode").map(String::as_str),
            Some("ac-42")
        );
    }

    #[test]
    fn prompt_defaults_accept_unquoted_literal_tokens() {
        assert_eq!(
            computed_dynamic_result(r#"FILLIN "Client?" \d Acme"#).as_deref(),
            Some("Acme")
        );
        assert_eq!(
            computed_dynamic_result(r#"FILLIN "Department?" \d ops \* Upper"#).as_deref(),
            Some("OPS")
        );

        let mut field_bookmarks = HashMap::new();
        assert_eq!(
            computed_ask_result(
                r#"ASK ClientCode "Client code?" \d ac-42"#,
                &mut field_bookmarks,
            )
            .as_deref(),
            Some("")
        );
        assert_eq!(
            field_bookmarks.get("ClientCode").map(String::as_str),
            Some("ac-42")
        );

        assert_eq!(
            computed_dynamic_result(r#"FILLIN "Project?" \d Client 42 \* Upper"#).as_deref(),
            Some("CLIENT 42")
        );
        assert_eq!(
            computed_ask_result(
                r#"ASK ClientName "Client name?" \d Client 42"#,
                &mut field_bookmarks,
            )
            .as_deref(),
            Some("")
        );
        assert_eq!(
            field_bookmarks.get("ClientName").map(String::as_str),
            Some("Client 42")
        );
    }

    #[test]
    fn context_free_fill_in_ownership_requires_an_explicit_literal_default() {
        for instruction in [
            r#"FILLIN "Client?" \d "Acme""#,
            r#"FILLIN "Department?" \dops \* Upper"#,
            r#"FILLIN Project display prompt \d Client 42 \o \* Caps"#,
        ] {
            assert!(supports_context_free_fill_in_field_syntax(instruction));
        }
        for instruction in [
            r#"FILLIN "Client?""#,
            r#"FILLIN "Client?" \d \o"#,
            r#"FILLIN "broken prompt \d Acme"#,
            r#"ASK ClientCode "Client code?" \d "ac-42""#,
            r#"QUOTE "not a prompt""#,
        ] {
            assert!(!supports_context_free_fill_in_field_syntax(instruction));
        }
    }

    #[test]
    fn context_free_display_ownership_requires_computed_eq_or_advance_syntax() {
        for instruction in [
            r#"EQ \f(1,2)"#,
            r#"EQ \d \fo10 \li()"#,
            r#"ADVANCE \r"2" \d4 \* MERGEFORMAT"#,
        ] {
            assert!(supports_context_free_display_field_syntax(instruction));
        }
        for instruction in [
            r#"EQ \f(1,"#,
            "EQ plain text",
            r#"ADVANCE \z 2"#,
            r#"SYMBOL 183 \f Symbol"#,
            r#"MACROBUTTON RunReport "Run""#,
        ] {
            assert!(!supports_context_free_display_field_syntax(instruction));
        }
    }

    #[test]
    fn prompt_defaults_reject_switch_like_unquoted_tokens() {
        assert_eq!(computed_dynamic_result(r#"FILLIN "Client?" \d \o"#), None);
        assert!(!supports_prompt_field_syntax(r#"FILLIN "Client?" \d \o"#));

        let mut field_bookmarks = HashMap::new();
        assert_eq!(
            computed_ask_result(
                r#"ASK ClientCode "Client code?" \d \o"#,
                &mut field_bookmarks,
            ),
            None
        );
        assert!(field_bookmarks.is_empty());
    }

    #[test]
    fn prompt_text_accepts_unquoted_single_tokens() {
        assert_eq!(
            computed_dynamic_result(r#"FILLIN Client? \d Acme"#).as_deref(),
            Some("Acme")
        );

        let mut field_bookmarks = HashMap::new();
        assert_eq!(
            computed_ask_result(r#"ASK ClientCode Client? \d ac-42"#, &mut field_bookmarks)
                .as_deref(),
            Some("")
        );
        assert_eq!(
            field_bookmarks.get("ClientCode").map(String::as_str),
            Some("ac-42")
        );
    }

    #[test]
    fn prompt_text_accepts_unquoted_multi_token_prompts_without_defaults() {
        assert!(supports_prompt_field_syntax("FILLIN Client display prompt"));
        assert_eq!(
            computed_dynamic_result("FILLIN Client display prompt"),
            None
        );

        let mut field_bookmarks = HashMap::new();
        assert!(supports_prompt_field_syntax(
            "ASK ClientName Client name prompt"
        ));
        assert_eq!(
            computed_ask_result("ASK ClientName Client name prompt", &mut field_bookmarks),
            None
        );
        assert!(field_bookmarks.is_empty());
    }

    #[test]
    fn prompt_text_rejects_switch_like_unquoted_tokens() {
        assert_eq!(computed_dynamic_result(r#"FILLIN \z \d Acme"#), None);
        assert!(!supports_prompt_field_syntax(r#"FILLIN \z \d Acme"#));

        let mut field_bookmarks = HashMap::new();
        assert_eq!(
            computed_ask_result(r#"ASK ClientCode \o \d ac-42"#, &mut field_bookmarks),
            None
        );
        assert!(field_bookmarks.is_empty());
    }

    #[test]
    fn ask_bookmarks_reject_unbalanced_and_embedded_quotes() {
        let mut field_bookmarks = HashMap::new();

        assert_eq!(
            computed_ask_result(
                r#"ASK Client"Code" "Client code?" \d "ac-42""#,
                &mut field_bookmarks,
            ),
            None
        );
        assert_eq!(
            computed_ask_result(
                r#"ASK "ClientCode"x "Client code?" \d "ac-42""#,
                &mut field_bookmarks,
            ),
            None
        );
        assert!(field_bookmarks.is_empty());
    }

    #[test]
    fn quoted_field_literals_reject_embedded_quotes() {
        assert_eq!(
            computed_dynamic_result(r#"FILLIN "Client?" \d "Acme""#).as_deref(),
            Some("Acme")
        );
        assert_eq!(
            computed_action_result(r#"PRINT "page \p""#).as_deref(),
            Some("")
        );
        assert_eq!(
            computed_dynamic_result(r#"FILLIN "Cli"ent?" \d "Acme""#),
            None
        );
        assert_eq!(
            computed_dynamic_result(r#"FILLIN "Client?" \d "Ac"me""#),
            None
        );
        assert_eq!(computed_action_result(r#"PRINT "page "p""#), None);
    }

    #[test]
    fn print_accepts_field_text_format_switches() {
        assert_eq!(
            computed_action_result(r#"PRINT "page \p" \* Upper"#).as_deref(),
            Some("")
        );
        assert_eq!(
            computed_action_result(r#"PRINT \p ReportBox "0 0 moveto" \*Lower"#).as_deref(),
            Some("")
        );
        assert_eq!(
            computed_action_result(r#"PRINT \p ReportBox "\p literal code""#).as_deref(),
            Some("")
        );
    }

    #[test]
    fn merge_controls_accept_field_text_format_switches() {
        assert_eq!(
            computed_dynamic_result(r#"NEXT \* Upper"#).as_deref(),
            Some("")
        );
        assert_eq!(
            computed_dynamic_result(r#"NEXTIF 1 = 1 \* FirstCap"#).as_deref(),
            Some("")
        );
        assert_eq!(
            computed_dynamic_result(r#"SKIPIF 1 = 0 \*Lower"#).as_deref(),
            Some("")
        );
    }

    #[test]
    fn set_accepts_field_text_format_switches_without_formatting_bookmark() {
        let mut field_bookmarks = HashMap::new();

        assert_eq!(
            computed_set_result(
                r#"SET ClientName "Acme Launch" \* Upper"#,
                &mut field_bookmarks,
            )
            .as_deref(),
            Some("")
        );
        assert_eq!(
            field_bookmarks.get("ClientName").map(String::as_str),
            Some("Acme Launch")
        );
    }

    #[test]
    fn set_bookmarks_reject_quoted_switch_names() {
        let mut field_bookmarks = HashMap::new();

        assert_eq!(
            computed_set_result(r#"SET " \r" "Acme""#, &mut field_bookmarks),
            None
        );
        assert!(field_bookmarks.is_empty());
    }

    #[test]
    fn set_values_reject_unbalanced_quotes() {
        let mut field_bookmarks = HashMap::new();

        assert_eq!(
            computed_set_result(r#"SET ClientName "Acme"#, &mut field_bookmarks),
            None
        );
        assert!(field_bookmarks.is_empty());
    }

    #[test]
    fn document_info_names_reject_malformed_quotes() {
        assert!(document_info_instruction(r#"DOCPROPERTY "Client Name""#).is_some());
        assert!(document_info_instruction(r#"DOCPROPERTY Client Name \* Caps"#).is_some());
        assert!(document_info_instruction(r#"DOCVARIABLE Client Code \* Upper"#).is_some());
        assert!(document_info_instruction(r#"DOCPROPERTY "Client Name"#).is_none());
        assert!(document_info_instruction(r#"INFO "Title"#).is_none());
        assert!(document_info_instruction(r#"DOCVARIABLE Client"Code""#).is_none());
    }

    #[test]
    fn document_info_date_formats_reject_malformed_quotes() {
        assert!(document_info_instruction(r#"CREATEDATE \@ "yyyy-MM-dd""#).is_some());
        assert!(document_info_instruction(r#"CREATEDATE \@"yyyy-MM-dd""#).is_some());
        assert!(document_info_instruction(r#"CREATEDATE \@ yyyy-MM-dd"#).is_some());
        assert!(document_info_instruction(r#"CREATEDATE \@ "yyyy-MM-dd"#).is_none());
        assert!(document_info_instruction(r#"CREATEDATE \@ yyyy-MM-dd""#).is_none());
        assert!(document_info_instruction(r#"CREATEDATE \@"yyyy-MM-dd"#).is_none());
    }

    #[test]
    fn preserved_document_info_fields_require_fresh_writer_stable_sources() {
        let core = CoreProperties {
            title: Some("Quarter Plan".to_string()),
            subject: Some("release train".to_string()),
            created: Some("2026-08-24T13:45:00Z".to_string()),
            revision: Some("12".to_string()),
            ..CoreProperties::default()
        };
        let custom = HashMap::from([
            ("CLIENTNAME".to_string(), "acme launch".to_string()),
            ("EMPTYVALUE".to_string(), String::new()),
        ]);

        for (instruction, expected) in [
            (r#"TITLE \* Upper"#, "QUARTER PLAN"),
            (r#"DOCPROPERTY Subject \* Caps"#, "Release Train"),
            (r#"INFO Title"#, "Quarter Plan"),
            (r#"CREATEDATE \@ "yyyy-MM-dd""#, "2026-08-24"),
            (r#"DOCPROPERTY RevisionNumber"#, "12"),
            (r#"DOCPROPERTY "Client Name" \* Caps"#, "Acme Launch"),
            (r#"DOCPROPERTY "Empty Value""#, ""),
        ] {
            assert!(
                supports_preserved_document_info_field_syntax(instruction),
                "{instruction}"
            );
            assert_eq!(
                computed_preserved_document_info_result(instruction, &core, &custom).as_deref(),
                Some(expected),
                "{instruction}"
            );
        }

        for instruction in [
            r#"NUMPAGES \* ROMAN"#,
            "DOCPROPERTY Pages",
            r#"DOCVARIABLE ClientCode \* Upper"#,
            "FILESIZE",
            r#"DATE \@ "yyyy-MM-dd""#,
            "USERNAME",
            "REVNUM",
            r#"DOCPROPERTY "Broken Name"#,
        ] {
            assert!(
                !supports_preserved_document_info_field_syntax(instruction),
                "{instruction}"
            );
            assert_eq!(
                computed_preserved_document_info_result(instruction, &core, &custom),
                None,
                "{instruction}"
            );
        }

        let normalized_by_writer = CoreProperties {
            title: Some(" Quarter Plan ".to_string()),
            ..CoreProperties::default()
        };
        assert!(supports_preserved_document_info_field_syntax("TITLE"));
        assert_eq!(
            computed_preserved_document_info_result("TITLE", &normalized_by_writer, &custom),
            None
        );
        assert_eq!(
            computed_preserved_document_info_result(
                "DOCPROPERTY Subject",
                &CoreProperties::default(),
                &custom,
            ),
            None
        );
    }

    #[test]
    fn preserved_revision_number_fields_require_fresh_writer_stable_sources() {
        let core = CoreProperties {
            revision: Some("draft revision".to_string()),
            ..CoreProperties::default()
        };

        for (instruction, expected) in [
            ("REVNUM", "draft revision"),
            (r#"REVNUM \* Upper"#, "DRAFT REVISION"),
            (r#"REVNUM \* Caps"#, "Draft Revision"),
        ] {
            assert!(
                supports_revision_number_field_syntax(instruction),
                "{instruction}"
            );
            assert_eq!(
                computed_preserved_revision_number_result(instruction, &core).as_deref(),
                Some(expected),
                "{instruction}"
            );
        }
        assert!(!supports_revision_number_field_syntax(r#"REVNUM \x"#));
        assert_eq!(
            computed_preserved_revision_number_result(r#"REVNUM \x"#, &core),
            None
        );
        for revision in [
            None,
            Some(String::new()),
            Some(" draft revision ".to_string()),
        ] {
            assert_eq!(
                computed_preserved_revision_number_result(
                    "REVNUM",
                    &CoreProperties {
                        revision,
                        ..CoreProperties::default()
                    },
                ),
                None
            );
        }
    }

    #[test]
    fn preserved_note_local_ref_fields_use_only_bookmark_text() {
        let bookmarks = HashMap::from([
            ("FootLocal".to_string(), "alpha launch".to_string()),
            ("Count".to_string(), "12".to_string()),
            ("Amount".to_string(), "1250.5".to_string()),
        ]);
        for (instruction, target, expected) in [
            ("REF FootLocal", "FootLocal", "alpha launch"),
            (
                r#"REF FootLocal \h \! \* Caps"#,
                "FootLocal",
                "Alpha Launch",
            ),
            (r#"REF Count \* ROMAN"#, "Count", "XII"),
            (r##"REF Amount \# "$#,##0.00""##, "Amount", "$1,250.50"),
        ] {
            assert_eq!(
                preserved_note_local_ref_target(instruction).as_deref(),
                Some(target),
                "{instruction}"
            );
            assert_eq!(
                computed_preserved_note_local_ref_result(instruction, &bookmarks).as_deref(),
                Some(expected),
                "{instruction}"
            );
        }

        for instruction in [
            r#"REF FootLocal \f"#,
            r#"REF FootLocal \p"#,
            r#"REF FootLocal \n"#,
            r#"REF FootLocal \w"#,
            r#"REF FootLocal \r"#,
            r#"REF FootLocal \d ".""#,
            r#"REF FootLocal \x"#,
            "FootLocal",
        ] {
            assert_eq!(preserved_note_local_ref_target(instruction), None);
            assert_eq!(
                computed_preserved_note_local_ref_result(instruction, &bookmarks),
                None
            );
        }
        assert_eq!(
            computed_preserved_note_local_ref_result("REF Missing", &bookmarks),
            None
        );
        assert_eq!(
            computed_preserved_note_local_ref_result(r#"REF FootLocal \# "0.00""#, &bookmarks,),
            None
        );
    }

    #[test]
    fn style_ref_names_reject_malformed_quotes() {
        assert!(style_ref_instruction(r#"STYLEREF "Heading 1""#).is_some());
        assert!(style_ref_instruction(r#"STYLEREF "Heading 1"#).is_none());
        assert!(style_ref_instruction(r#"STYLEREF Heading"1""#).is_none());
        assert!(style_ref_instruction(r#"STYLEREF "\Heading 1""#).is_none());
    }

    #[test]
    fn preserved_note_local_style_refs_use_plain_paragraph_text_or_position() {
        for (instruction, target, before, expected) in [
            ("STYLEREF Heading1", "Heading1", true, "Alpha heading"),
            (
                r#"STYLEREF "Heading 1" \* Upper"#,
                "Heading 1",
                true,
                "ALPHA HEADING",
            ),
            (r#"STYLEREF \p Callout \* Caps"#, "Callout", false, "Below"),
            (r#"STYLEREF Callout \p"#, "Callout", true, "above"),
        ] {
            assert_eq!(
                preserved_note_local_style_ref_target(instruction).as_deref(),
                Some(target),
                "{instruction}"
            );
            assert_eq!(
                computed_preserved_note_local_style_ref_result(
                    instruction,
                    "  Alpha\t heading  ",
                    before,
                )
                .as_deref(),
                Some(expected),
                "{instruction}"
            );
        }

        for instruction in [
            r#"STYLEREF NumberedBody \n"#,
            r#"STYLEREF NumberedBody \r"#,
            r#"STYLEREF NumberedBody \w"#,
            r#"STYLEREF NumberedBody \n\t"#,
            r#"STYLEREF "Broken Style"#,
            r#"STYLEREF Heading1 \x"#,
            "Heading1",
        ] {
            assert_eq!(
                preserved_note_local_style_ref_target(instruction),
                None,
                "{instruction}"
            );
            assert_eq!(
                computed_preserved_note_local_style_ref_result(instruction, "Alpha heading", true,),
                None,
                "{instruction}"
            );
        }
        assert_eq!(
            computed_preserved_note_local_style_ref_result("STYLEREF Heading1", " \t\n ", true,),
            None
        );
    }

    #[test]
    fn compact_autonum_separator_switches_are_case_insensitive() {
        let mut counter = 0;

        assert_eq!(
            computed_numbering_result(r#"AUTONUM \S":" "#, &mut counter).as_deref(),
            Some("1:")
        );
        assert_eq!(counter, 1);
    }

    #[test]
    fn autonum_separator_switches_reject_malformed_quotes() {
        let mut counter = 0;
        assert_eq!(
            computed_numbering_result(r#"AUTONUM \s ")""#, &mut counter).as_deref(),
            Some("1)")
        );

        let mut counter = 0;
        assert_eq!(
            computed_numbering_result(r#"AUTONUM \s ""#, &mut counter),
            None
        );
        assert_eq!(counter, 0);

        let mut counter = 0;
        assert_eq!(
            computed_numbering_result(r#"AUTONUM \s"""#, &mut counter),
            None
        );
        assert_eq!(counter, 0);
    }

    #[test]
    fn listnum_numeric_switches_reject_malformed_quotes() {
        let mut counter = 0;
        assert_eq!(
            computed_listnum_result(r#"LISTNUM NumberDefault \l "1""#, &mut counter).as_deref(),
            Some("1")
        );

        let mut counter = 0;
        assert_eq!(
            computed_listnum_result(r#"LISTNUM NumberDefault \l1"#, &mut counter).as_deref(),
            Some("1")
        );

        let mut counter = 0;
        assert_eq!(
            computed_listnum_result(r#"LISTNUM NumberDefault \s "4""#, &mut counter).as_deref(),
            Some("4")
        );

        let mut counter = 0;
        assert_eq!(
            computed_listnum_result(r#"LISTNUM NumberDefault \s4"#, &mut counter).as_deref(),
            Some("4")
        );

        let mut counter = 0;
        assert_eq!(
            computed_listnum_result(r#"LISTNUM NumberDefault \l "1"#, &mut counter),
            None
        );
        assert_eq!(counter, 0);

        let mut counter = 0;
        assert_eq!(
            computed_listnum_result(r#"LISTNUM NumberDefault \s 4""#, &mut counter),
            None
        );
        assert_eq!(counter, 0);

        let mut counter = i64::MAX;
        assert_eq!(
            computed_listnum_result("LISTNUM NumberDefault", &mut counter),
            None
        );
        assert_eq!(counter, i64::MAX);
    }

    #[test]
    fn table_formula_scan_keeps_outer_field_across_nested_result_field() {
        let xml = r#"<w:document>
            <w:body>
                <w:tbl>
                    <w:tr>
                        <w:tc><w:p><w:r><w:t>5</w:t></w:r></w:p></w:tc>
                    </w:tr>
                    <w:tr>
                        <w:tc><w:p>
                            <w:r><w:fldChar w:fldCharType="begin"/></w:r>
                            <w:r><w:instrText>= SUM(ABOVE)</w:instrText></w:r>
                            <w:r><w:fldChar w:fldCharType="separate"/></w:r>
                            <w:r><w:fldChar w:fldCharType="begin"/></w:r>
                            <w:r><w:instrText>PAGE</w:instrText></w:r>
                            <w:r><w:fldChar w:fldCharType="separate"/></w:r>
                            <w:r><w:t>1</w:t></w:r>
                            <w:r><w:fldChar w:fldCharType="end"/></w:r>
                            <w:r><w:fldChar w:fldCharType="end"/></w:r>
                        </w:p></w:tc>
                    </w:tr>
                </w:tbl>
            </w:body>
        </w:document>"#;

        assert_eq!(
            table_formula_context(xml, &HashMap::new())
                .field_result(0)
                .as_deref(),
            Some("5")
        );
    }

    #[test]
    fn table_formula_scan_trims_unit_gridspan() {
        let xml = r#"<w:document>
            <w:body>
                <w:tbl>
                    <w:tr>
                        <w:tc>
                            <w:tcPr><w:gridSpan w:val=" 1 "/></w:tcPr>
                            <w:p><w:r><w:t>5</w:t></w:r></w:p>
                        </w:tc>
                        <w:tc><w:p><w:fldSimple w:instr=" = SUM(LEFT) ">
                            <w:r><w:t>stale sum</w:t></w:r>
                        </w:fldSimple></w:p></w:tc>
                    </w:tr>
                </w:tbl>
            </w:body>
        </w:document>"#;

        assert_eq!(
            table_formula_context(xml, &HashMap::new())
                .field_result(0)
                .as_deref(),
            Some("5")
        );
    }

    #[test]
    fn toc_sequence_scan_keeps_outer_field_across_nested_result_field() {
        let xml = r#"<w:document>
            <w:body>
                <w:p>
                    <w:r><w:t>Figure </w:t></w:r>
                    <w:r><w:fldChar w:fldCharType="begin"/></w:r>
                    <w:r><w:instrText>SEQ Figure</w:instrText></w:r>
                    <w:r><w:fldChar w:fldCharType="separate"/></w:r>
                    <w:r><w:fldChar w:fldCharType="begin"/></w:r>
                    <w:r><w:instrText>PAGE</w:instrText></w:r>
                    <w:r><w:fldChar w:fldCharType="separate"/></w:r>
                    <w:r><w:t>1</w:t></w:r>
                    <w:r><w:fldChar w:fldCharType="end"/></w:r>
                    <w:r><w:fldChar w:fldCharType="end"/></w:r>
                    <w:r><w:t>: Nested</w:t></w:r>
                </w:p>
            </w:body>
        </w:document>"#;

        let entries = toc_entries(xml, &Styles::default(), &HashMap::new());
        let sequence = entries
            .iter()
            .find(|entry| entry.source == TocEntrySource::SequenceField)
            .expect("sequence entry");

        assert_eq!(sequence.sequence_identifier.as_deref(), Some("Figure"));
        assert_eq!(sequence.text, "Figure 1: Nested");
        assert_eq!(sequence.sequence_caption_text.as_deref(), Some("Nested"));
    }

    #[test]
    fn bookmark_scanners_trim_ids_and_names() {
        let xml = r#"<w:document>
            <w:body>
                <w:p><w:r><w:br w:type="page"/></w:r></w:p>
                <w:p>
                    <w:bookmarkStart w:id=" 7 " w:name=" Figure1 "/>
                    <w:r><w:t>Figure 1</w:t></w:r>
                    <w:bookmarkEnd w:id="7"/>
                </w:p>
                <w:p><w:r><w:t>Outside</w:t></w:r></w:p>
                <w:p>
                    <w:bookmarkStart w:id=" 8 " w:name=" ScopedToc "/>
                    <w:pPr><w:pStyle w:val="Heading1"/></w:pPr>
                    <w:r><w:t>Scoped Heading</w:t></w:r>
                    <w:bookmarkEnd w:id="8"/>
                </w:p>
                <w:p>
                    <w:bookmarkStart w:id=" 9 " w:name=" FootOne "/>
                    <w:r><w:footnoteReference w:id="1"/></w:r>
                    <w:bookmarkEnd w:id="9"/>
                </w:p>
            </w:body>
        </w:document>"#;

        assert_eq!(
            ref_targets(xml).get("Figure1").map(String::as_str),
            Some("Figure 1")
        );
        assert!(ref_position_context(xml, &Numbering::default())
            .target_position("Figure1")
            .is_some());
        assert!(page_ref_context(xml, &ref_targets(xml))
            .target_position("Figure1")
            .is_some());
        assert!(note_ref_context(xml, &ref_targets(xml))
            .targets
            .contains_key("FootOne"));
        assert!(toc_entries(xml, &Styles::default(), &HashMap::new())
            .iter()
            .any(|entry| {
                entry.text == "Scoped Heading"
                    && entry.bookmarks.len() == 1
                    && entry.bookmarks[0] == "ScopedToc"
            }));
    }

    #[test]
    fn sequence_identifiers_reject_quoted_switch_names() {
        let mut counters = HashMap::new();

        assert_eq!(
            computed_sequence_result(r#"SEQ " \r""#, &mut counters),
            None
        );
        assert!(counters.is_empty());
        assert_eq!(seq_identifier_from_instruction(Some(r#"SEQ " \r""#)), None);
    }

    #[test]
    fn sequence_identifiers_reject_whitespace_names() {
        let mut counters = HashMap::new();

        assert_eq!(
            computed_sequence_result(r#"SEQ "Figure List""#, &mut counters),
            None
        );
        assert!(counters.is_empty());
        assert_eq!(
            seq_identifier_from_instruction(Some(r#"SEQ "Figure List""#)),
            None
        );
    }

    #[test]
    fn sequence_identifiers_reject_unbalanced_quotes() {
        let mut counters = HashMap::new();

        assert_eq!(
            computed_sequence_result(r##"SEQ "Figure"##, &mut counters),
            None
        );
        assert!(counters.is_empty());
        assert_eq!(
            seq_identifier_from_instruction(Some(r##"SEQ "Figure"##)),
            None
        );
    }

    #[test]
    fn sequence_identifiers_reject_unsupported_tail_switches() {
        assert_eq!(
            seq_identifier_from_instruction(Some(r#"SEQ Figure \x"#)),
            None
        );
        assert_eq!(
            seq_identifier_from_instruction(Some(r#"SEQ Figure \* roman"#)).as_deref(),
            Some("Figure")
        );
    }

    #[test]
    fn sequence_identifiers_reject_unsupported_negative_resets() {
        assert!(!supports_sequence_field_syntax(r#"SEQ Figure \r -1"#));
        assert_eq!(
            seq_identifier_from_instruction(Some(r#"SEQ Figure \r -1"#)),
            None
        );
    }

    #[test]
    fn sequence_resets_reject_malformed_quotes() {
        let mut counters = HashMap::new();
        assert_eq!(
            computed_sequence_result(r#"SEQ Figure \r "7""#, &mut counters).as_deref(),
            Some("7")
        );

        let mut counters = HashMap::new();
        assert_eq!(
            computed_sequence_result(r#"SEQ Figure \r 7"#, &mut counters).as_deref(),
            Some("7")
        );

        let mut counters = HashMap::new();
        assert_eq!(
            computed_sequence_result(r#"SEQ Figure \r "7"#, &mut counters),
            None
        );
        assert!(counters.is_empty());

        let mut counters = HashMap::new();
        assert_eq!(
            computed_sequence_result(r#"SEQ Figure \r 7""#, &mut counters),
            None
        );
        assert!(counters.is_empty());
    }

    #[test]
    fn preserved_sequence_resets_are_visible_and_context_free() {
        for (instruction, result) in [
            (r#"SEQ Figure \r 7"#, "7"),
            (r#"SEQ Appendix \r31 \* Hex"#, "1F"),
            (
                r#"SEQ Invoice \r "21" \* DollarText \* Upper"#,
                "TWENTY-ONE AND 00/100",
            ),
        ] {
            assert_eq!(
                computed_preserved_sequence_reset_result(instruction).as_deref(),
                Some(result),
                "{instruction}"
            );
        }

        for instruction in [
            "SEQ Figure",
            r#"SEQ Figure \n"#,
            r#"SEQ Figure \c"#,
            r#"SEQ Figure \s 1"#,
            r#"SEQ Figure \r 7 \s 1"#,
            r#"SEQ Figure \r 7 \h"#,
            r#"SEQ Figure \r -1"#,
            r#"SEQ Figure \r"#,
            r#"SEQ Figure \r 7 \x"#,
        ] {
            assert_eq!(
                computed_preserved_sequence_reset_result(instruction),
                None,
                "{instruction}"
            );
        }
    }

    #[test]
    fn preserved_listnum_starts_are_explicit_and_context_free() {
        for (instruction, result) in [
            (r#"LISTNUM \s 7"#, "7"),
            (r#"LISTNUM NumberDefault \s "4""#, "4"),
            (r#"LISTNUM NumberDefault \s4 \* ROMAN"#, "IV"),
            (
                r#"LISTNUM NumberDefault \l "1" \s 21 \* DollarText \* Upper"#,
                "TWENTY-ONE AND 00/100",
            ),
            (r#"LISTNUM LegalDefault \l1 \s31 \* Hex"#, "1F"),
        ] {
            assert_eq!(
                computed_preserved_listnum_start_result(instruction).as_deref(),
                Some(result),
                "{instruction}"
            );
        }

        for instruction in [
            "LISTNUM",
            "LISTNUM NumberDefault",
            "AUTONUM",
            r#"LISTNUM CustomList \s 4"#,
            r#"LISTNUM NumberDefault \l 2 \s 4"#,
            r#"LISTNUM NumberDefault \s 4 \s 5"#,
            r#"LISTNUM NumberDefault \s -1"#,
            r#"LISTNUM NumberDefault \s"#,
            r#"LISTNUM NumberDefault \s 4 \x"#,
        ] {
            assert_eq!(
                computed_preserved_listnum_start_result(instruction),
                None,
                "{instruction}"
            );
        }
    }

    #[test]
    fn reference_targets_reject_quoted_switch_names() {
        assert!(ref_instruction(r#"REF " \p""#).is_none());
        assert!(direct_bookmark_ref_instruction(r#"" \p""#).is_none());
        assert!(page_ref_instruction(r#"PAGEREF " \p""#).is_none());
        assert!(note_ref_instruction(r#"NOTEREF " \p""#).is_none());
    }

    #[test]
    fn reference_targets_reject_switch_first_names() {
        assert!(ref_instruction(r#"REF \h Figure1"#).is_none());
        assert!(direct_bookmark_ref_instruction(r#"\h Figure1"#).is_none());
        assert!(page_ref_instruction(r#"PAGEREF \p Figure1"#).is_none());
        assert!(note_ref_instruction(r#"NOTEREF \p FootOne"#).is_none());
    }

    #[test]
    fn reference_targets_reject_whitespace_names() {
        assert!(ref_instruction(r#"REF "Figure List""#).is_none());
        assert!(direct_bookmark_ref_instruction(r#""Figure List""#).is_none());
        assert!(page_ref_instruction(r#"PAGEREF "Figure List""#).is_none());
        assert!(note_ref_instruction(r#"NOTEREF "Figure List""#).is_none());
    }

    #[test]
    fn reference_targets_reject_unbalanced_quotes() {
        assert!(ref_instruction(r#"REF "Figure1"#).is_none());
        assert!(direct_bookmark_ref_instruction(r#""Figure1"#).is_none());
        assert!(page_ref_instruction(r#"PAGEREF "Figure1"#).is_none());
        assert!(note_ref_instruction(r#"NOTEREF "Figure1"#).is_none());
    }

    #[test]
    fn ref_sequence_separators_accept_compact_values() {
        assert_eq!(
            ref_instruction(r#"REF Figure1 \d-"#)
                .expect("compact separator")
                .sequence_separator_value
                .as_deref(),
            Some("-")
        );
        assert_eq!(
            direct_bookmark_ref_instruction(r#"Figure1 \d-"#)
                .expect("compact direct separator")
                .sequence_separator_value
                .as_deref(),
            Some("-")
        );
        assert!(ref_instruction(r#"REF Figure1 \d\p"#).is_none());
    }

    #[test]
    fn ref_sequence_separators_reject_malformed_quotes() {
        assert_eq!(
            ref_instruction(r#"REF Figure1 \d "-""#)
                .expect("quoted separator")
                .sequence_separator_value
                .as_deref(),
            Some("-")
        );
        assert_eq!(
            direct_bookmark_ref_instruction(r#"Figure1 \d-"#)
                .expect("compact direct separator")
                .sequence_separator_value
                .as_deref(),
            Some("-")
        );
        assert!(ref_instruction(r#"REF Figure1 \d "-"#).is_none());
        assert!(ref_instruction(r#"REF Figure1 \d -""#).is_none());
        assert!(direct_bookmark_ref_instruction(r#"Figure1 \d"-"#).is_none());
    }

    #[test]
    fn reference_index_categories_reject_malformed_quotes() {
        assert!(computed_reference_index_result(r#"TA \l "Case" \c "1""#).is_some());
        assert!(computed_reference_index_result(r#"TA \l "Case" \c "1"#).is_none());
        assert!(computed_reference_index_result(r#"TA \l "Case" \c"1"#).is_none());
    }

    #[test]
    fn reference_index_markers_accept_field_text_format_switches() {
        assert_eq!(
            computed_reference_index_result(r#"RD "chapter2.docx" \* Upper"#).as_deref(),
            Some("")
        );
        assert_eq!(
            computed_reference_index_result(r#"TA \l "Case" \*Lower"#).as_deref(),
            Some("")
        );
        assert_eq!(
            computed_reference_index_result(r#"XE "Mercury" \t "See planets" \* FirstCap"#)
                .as_deref(),
            Some("")
        );
    }

    #[test]
    fn reference_index_syntax_support_matches_marker_forms() {
        assert!(supports_reference_index_marker_syntax(
            r#"RD "chapter2.docx" \* Upper"#
        ));
        assert!(supports_reference_index_marker_syntax(
            r#"TA \l "Case" \*Lower"#
        ));
        assert!(supports_reference_index_marker_syntax(
            r#"XE "Mercury" \t "See planets" \* FirstCap"#
        ));
        assert!(!supports_reference_index_marker_syntax(
            r#"TA \l "Case" \c"1"#
        ));
    }

    #[test]
    fn toc_bookmark_targets_reject_empty_and_quoted_switch_names() {
        assert!(toc_spec(r#"TOC \b """#).is_none());
        assert!(toc_spec(r#"TOC \b " \o""#).is_none());
        assert!(toc_spec(r#"TOC \b" \o""#).is_none());
    }

    #[test]
    fn toc_bookmark_targets_reject_unbalanced_quotes() {
        assert!(toc_spec(r#"TOC \b "ChapterList"#).is_none());
        assert!(toc_spec(r#"TOC \b"ChapterList"#).is_none());
    }

    #[test]
    fn toc_type_identifiers_reject_quoted_switch_names() {
        assert_eq!(computed_toc_entry_result(r#"TC "Entry" \f " \l""#), None);
        assert!(toc_spec(r#"TOC \f " \l""#).is_none());
        assert!(toc_spec(r#"TOC \f" \l""#).is_none());
    }

    #[test]
    fn toc_entry_text_rejects_malformed_quotes() {
        assert!(computed_toc_entry_result(r#"TC "Entry""#).is_some());
        assert!(computed_toc_entry_result(r#"TC Entry"#).is_some());
        assert!(computed_toc_entry_result(r#"TC "Entry"#).is_none());
        assert!(computed_toc_entry_result(r#"TC Entry""#).is_none());
        assert!(computed_toc_entry_result(r#"TC En"try""#).is_none());
    }

    #[test]
    fn toc_entry_syntax_support_matches_tc_forms() {
        assert!(supports_toc_entry_field_syntax(r#"TC "Entry" \f A \l 2"#));
        assert!(supports_toc_entry_field_syntax(
            r#"TC "Entry" \f A \l 2 \* Upper"#
        ));
        assert!(supports_toc_entry_field_syntax(r#"TC Entry \n"#));
        assert!(!supports_toc_entry_field_syntax(r#"TC "Entry" \l "2"#));
    }

    #[test]
    fn toc_sequence_filters_reject_invalid_identifiers() {
        assert!(toc_spec(r#"TOC \c "Figure List""#).is_none());
        assert!(toc_spec(r#"TOC \a " \o""#).is_none());
        assert!(toc_spec(r#"TOC \c" \o""#).is_none());
    }

    #[test]
    fn toc_page_number_sequence_prefixes_reject_invalid_identifiers() {
        assert!(toc_spec(r#"TOC \o "1-2" \s "\o""#).is_none());
        assert!(toc_spec(r#"TOC \o "1-2" \s"\o""#).is_none());
    }

    #[test]
    fn toc_numeric_levels_reject_malformed_quotes() {
        assert!(computed_toc_entry_result(r#"TC "Entry" \l "2"#).is_none());
        assert!(toc_spec(r#"TOC \o "1-2"#).is_none());
        assert!(toc_spec(r#"TOC \o "1-2" \l "2-3"#).is_none());
    }

    #[test]
    fn toc_separator_switches_reject_malformed_quotes() {
        assert!(toc_spec(r#"TOC \o "1-2" \p """#).is_some());
        assert!(toc_spec(r#"TOC \o "1-2" \p ""#).is_none());
        assert!(toc_spec(r#"TOC \o "1-2" \d"-"#).is_none());
    }

    #[test]
    fn toc_style_specs_reject_malformed_style_names() {
        assert!(toc_spec(r#"TOC \o "1-2" \t "Custom Heading,2""#).is_some());
        assert!(toc_spec(r#"TOC \o "1-2" \t "Custom Heading,2"#).is_none());
        assert!(toc_spec(r#"TOC \o "1-2" \t "Custom"Heading,2""#).is_none());
        assert!(toc_spec(r#"TOC \o "1-2" \t "\Bad,2""#).is_none());
    }

    #[test]
    fn compact_if_comparisons_ignore_operators_inside_quotes() {
        assert_eq!(
            computed_dynamic_result(r#"IF "A=B"="A=B" "yes" "no""#).as_deref(),
            Some("yes")
        );
        assert_eq!(
            computed_dynamic_result(r#"COMPARE "A>B"<>"A?B""#).as_deref(),
            Some("0")
        );
    }

    #[test]
    fn text_comparisons_accept_wildcards_on_either_operand() {
        assert_eq!(
            computed_dynamic_result(r#"COMPARE "A*" = "AB""#).as_deref(),
            Some("1")
        );
    }

    #[test]
    fn if_and_compare_use_common_general_format_switches() {
        assert_eq!(
            computed_dynamic_result(r#"IF 1 = 1 "ship" "hold" \* Upper"#).as_deref(),
            Some("SHIP")
        );
        assert_eq!(
            computed_dynamic_result(r#"IF 1 = 1 "ship" \*Lower"#).as_deref(),
            Some("ship")
        );
        assert_eq!(
            computed_dynamic_result(r#"COMPARE "A*" = "AB" \* MERGEFORMAT"#).as_deref(),
            Some("1")
        );
    }

    #[test]
    fn context_free_if_compare_ownership_requires_literal_operands() {
        for instruction in [
            r#"IF 1 = 1 "ship" "hold" \* Upper"#,
            r#"IF "A=B"="A=B" yes no"#,
            r#"COMPARE "A*" = "AB" \* MERGEFORMAT"#,
            "COMPARE 5 > 3",
        ] {
            assert!(supports_context_free_if_compare_field_syntax(instruction));
        }
        for instruction in [
            r#"IF Gate = "Ready" "ship" "hold""#,
            r#"COMPARE CustomerTier = "Gold""#,
            "COMPARE 1e309 > 0",
            r#"IF 1 = 1 "broken"#,
            r#"QUOTE "not a comparison""#,
        ] {
            assert!(!supports_context_free_if_compare_field_syntax(instruction));
        }
    }

    #[test]
    fn compare_syntax_accepts_data_operands_without_computing_them() {
        assert!(supports_compare_field_syntax(
            r#"COMPARE CustomerTier = "Gold""#
        ));
        assert!(supports_compare_field_syntax(
            r#"COMPARE CustomerTier="Gold""#
        ));
        assert_eq!(
            computed_dynamic_result(r#"COMPARE CustomerTier = "Gold""#),
            None
        );
        assert!(!supports_compare_field_syntax(r#"COMPARE 1e309 > 0"#));
    }

    #[test]
    fn comparison_syntax_rejects_switch_like_unquoted_operands() {
        assert!(!supports_compare_field_syntax(r#"COMPARE \o = "Gold""#));
        assert!(!supports_if_field_syntax(r#"IF \o = "Gold" "ship" "hold""#));
        assert!(!supports_merge_control_field_syntax(
            r#"NEXTIF \o = "Gold""#
        ));
        assert!(supports_compare_field_syntax(r#"COMPARE "\o" = "Gold""#));
    }

    #[test]
    fn if_result_text_rejects_malformed_quotes() {
        assert_eq!(
            computed_dynamic_result(r#"IF 1 = 1 "yes" "no""#).as_deref(),
            Some("yes")
        );
        assert_eq!(
            computed_dynamic_result(r#"IF 1 = 1 yes no"#).as_deref(),
            Some("yes")
        );
        assert_eq!(computed_dynamic_result(r#"IF 1 = 1 "yes no"#), None);
        assert_eq!(computed_dynamic_result(r#"IF 1 = 0 yes "no"#), None);
    }

    #[test]
    fn quote_text_rejects_malformed_quotes() {
        assert_eq!(
            computed_dynamic_result(r#"QUOTE "literal text""#).as_deref(),
            Some("literal text")
        );
        assert_eq!(
            computed_dynamic_result(r#"QUOTE plain words"#).as_deref(),
            Some("plain words")
        );
        assert_eq!(computed_dynamic_result(r#"QUOTE "literal text"#), None);
        assert_eq!(computed_dynamic_result(r#"QUOTE literal text""#), None);
    }

    #[test]
    fn eq_script_segments_accept_multiple_visible_and_empty_options() {
        assert_eq!(
            computed_display_result(r#"EQ \s\up8(UB)\ai4()\do8(2)\di3()"#).as_deref(),
            Some("^{UB}_2")
        );
    }

    #[test]
    fn eq_enclosed_operands_reject_malformed_quotes() {
        assert_eq!(
            computed_display_result(r#"EQ \b("Chapter One")"#).as_deref(),
            Some("(Chapter One)")
        );
        assert!(computed_display_result(r#"EQ \b("A"B")"#).is_none());
        assert!(computed_display_result(r#"EQ \b("Chapter One)"#).is_none());
        assert!(computed_display_result(r#"EQ \x \to("Chapter One)"#).is_none());
    }

    #[test]
    fn eq_literal_operands_accept_escaped_closing_parentheses() {
        assert_eq!(
            computed_display_result(r#"EQ \f(A\),B)"#).as_deref(),
            Some("A)/B")
        );
    }

    #[test]
    fn eq_literal_operands_accept_escaped_semicolons() {
        assert_eq!(
            computed_display_result(r#"EQ \f(A\;B;C)"#).as_deref(),
            Some("A;B/C")
        );
    }

    #[test]
    fn eq_displacement_controls_preserve_literal_operand_text() {
        assert_eq!(
            computed_display_result(r#"EQ \d \fo10(A)"#).as_deref(),
            Some("A")
        );
        assert_eq!(
            computed_display_result(r#"EQ \d \ba2(\f(1,2))"#).as_deref(),
            Some("(1/2)")
        );
        assert_eq!(computed_display_result(r#"EQ \d(A)"#), None);
    }

    #[test]
    fn advance_points_reject_malformed_quotes() {
        assert_eq!(
            computed_display_result(r#"ADVANCE \r "2""#).as_deref(),
            Some("")
        );
        assert_eq!(
            computed_display_result(r#"ADVANCE \r2"#).as_deref(),
            Some("")
        );
        assert!(computed_display_result(r#"ADVANCE \r "2"#).is_none());
        assert!(computed_display_result(r#"ADVANCE \r2""#).is_none());
    }

    #[test]
    fn advance_accepts_field_text_format_switches() {
        assert_eq!(
            computed_display_result(r#"ADVANCE \r 2 \* Upper"#).as_deref(),
            Some("")
        );
    }

    #[test]
    fn symbol_values_reject_malformed_quotes() {
        assert!(supports_computed_symbol_field_syntax(
            r#"SYMBOL 183 \f Symbol"#
        ));
        assert!(!supports_computed_symbol_field_syntax(
            r#"SYMBOL 66 \f Wingdings"#
        ));
        assert!(!supports_computed_symbol_field_syntax(r#"EQ \f(1,2)"#));
        assert_eq!(
            computed_display_result(r#"SYMBOL "65""#).as_deref(),
            Some("A")
        );
        assert!(computed_display_result(r#"SYMBOL "65"#).is_none());
        assert_eq!(
            computed_display_result(r#"SYMBOL 65 \s "12""#).as_deref(),
            Some("A")
        );
        assert!(computed_display_result(r#"SYMBOL 65 \s "12"#).is_none());
        assert!(computed_display_result(r#"SYMBOL 183 \f "Symbol""#).is_some());
        assert!(computed_display_result(r#"SYMBOL 183 \f "Symbol"#).is_none());
    }

    #[test]
    fn symbol_values_accept_field_text_format_switches() {
        assert_eq!(
            computed_display_result(r#"SYMBOL 0x0063 \u \* Upper"#).as_deref(),
            Some("C")
        );
        assert_eq!(
            computed_display_result(r#"SYMBOL 0x0064 \u \*Lower"#).as_deref(),
            Some("d")
        );
    }

    #[test]
    fn action_targets_reject_quoted_backslash_names() {
        assert_eq!(
            computed_action_result(r#"GOTOBUTTON "\BadTarget" "Jump""#),
            None
        );
        assert_eq!(
            computed_action_result(r#"GOTOBUTTON " \BadTarget" "Jump""#),
            None
        );
    }

    #[test]
    fn action_targets_reject_whitespace_names() {
        assert_eq!(
            computed_action_result(r#"GOTOBUTTON "Bad Target" "Jump""#),
            None
        );
        assert_eq!(
            computed_action_result(r#"MACROBUTTON "Run Report" "Run""#),
            None
        );
    }

    #[test]
    fn action_targets_reject_embedded_quotes() {
        assert_eq!(
            computed_action_result(r#"GOTOBUTTON ""BadTarget"" "Jump""#),
            None
        );
        assert_eq!(
            computed_action_result(r#"MACROBUTTON ""RunReport"" "Run""#),
            None
        );
    }

    #[test]
    fn action_display_text_rejects_malformed_quotes() {
        assert_eq!(
            computed_action_result(r#"GOTOBUTTON TargetBookmark "Jump Now""#).as_deref(),
            Some("Jump Now")
        );
        assert_eq!(
            computed_action_result(r#"MACROBUTTON RunReport Run Now"#).as_deref(),
            Some("Run Now")
        );
        assert_eq!(
            computed_action_result(r#"GOTOBUTTON TargetBookmark "Jump Now"#),
            None
        );
        assert_eq!(
            computed_action_result(r#"MACROBUTTON RunReport Run Now""#),
            None
        );
    }

    #[test]
    fn action_syntax_support_matches_computed_action_forms() {
        assert!(supports_action_field_syntax(
            r#"GOTOBUTTON TargetBookmark "Jump Now""#
        ));
        assert!(supports_action_field_syntax(
            r#"PRINT \p ReportBox "0 0 moveto""#
        ));
        assert!(supports_action_field_syntax(
            r#"MACROBUTTON RunReport \* MERGEFORMAT"#
        ));
        assert_eq!(
            computed_action_result(r#"MACROBUTTON RunReport \* MERGEFORMAT"#),
            None
        );
        assert!(!supports_action_field_syntax(
            r#"MACROBUTTON RunReport Run \* Upper Again"#
        ));
    }

    #[test]
    fn print_groups_reject_quoted_backslash_names() {
        assert_eq!(
            computed_action_result(r#"PRINT \p " \BadGroup" "0 0 moveto""#),
            None
        );
        assert_eq!(
            computed_action_result(r#"PRINT \p" \BadGroup" "0 0 moveto""#),
            None
        );
    }

    #[test]
    fn print_groups_reject_embedded_quotes() {
        assert_eq!(
            computed_action_result(r#"PRINT \p ""BadGroup"" "0 0 moveto""#),
            None
        );
        assert_eq!(
            computed_action_result(r#"PRINT \p""BadGroup"" "0 0 moveto""#),
            None
        );
    }

    #[test]
    fn print_groups_reject_whitespace_names() {
        assert_eq!(
            computed_action_result(r#"PRINT \p "Bad Group" "0 0 moveto""#),
            None
        );
        assert_eq!(
            computed_action_result(r#"PRINT \p"Bad Group" "0 0 moveto""#),
            None
        );
    }
}
