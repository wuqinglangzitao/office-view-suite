use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct SectionContext {
    field_sections: Vec<usize>,
    section_page_counts: Vec<Option<usize>>,
}

impl SectionContext {
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn field_position(&self, index: usize) -> Option<SectionFieldPosition> {
        let section = self.field_sections.get(index).copied()?;
        Some(SectionFieldPosition {
            section,
            section_pages: self
                .section_page_counts
                .get(section.saturating_sub(1))
                .copied()
                .flatten(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SectionFieldPosition {
    section: usize,
    section_pages: Option<usize>,
}

#[derive(Debug, Clone)]
struct SectionScanField {
    instruction: String,
    phase: FieldPhase,
    suppress_result: bool,
    nested_suppressed_fields: usize,
}

#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn section_context(
    xml: &str,
    document_bookmarks: &HashMap<String, String>,
) -> SectionContext {
    let core_properties = CoreProperties::default();
    let empty_properties = HashMap::new();
    section_context_with_properties(
        xml,
        document_bookmarks,
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

pub(crate) fn section_context_with_properties(
    xml: &str,
    document_bookmarks: &HashMap<String, String>,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
) -> SectionContext {
    let mut r = Reader::from_str(xml);
    let mut field_sections = Vec::new();
    let mut section_page_counts = Vec::new();
    let mut current_section = 1usize;
    let mut current_page = 1usize;
    let mut section_start_page = 1usize;
    let mut section_has_visible_content = false;
    let mut paragraph_properties_depth = 0usize;
    let mut section_break_pending = false;
    let mut section_break_kind = Some(PageRefSectionBreak::Next);
    let mut section_type_seen = false;
    let mut section_properties_depth = 0usize;
    let mut current: Option<SectionScanField> = None;
    let mut computed_fields = SectionComputedFieldState::new(
        document_bookmarks,
        properties,
        legacy_form_context(xml, preserve_legacy_form_cache),
    );
    let mut simple_section_field_result_depth: Option<usize> = None;
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
                if suppresses_section_complex_result_scan(&current) {
                    match name {
                        b"fldChar" => apply_section_scan_fld_char(
                            &e,
                            current_section,
                            &mut current,
                            &mut field_sections,
                            &mut section_has_visible_content,
                            &mut computed_fields,
                        ),
                        b"instrText" | b"t" => {
                            let _ = read_text(&mut r);
                            continue;
                        }
                        _ => {}
                    }
                    xml_depth = xml_depth.saturating_add(1);
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
                    b"pPr" => paragraph_properties_depth += 1,
                    b"sectPr" if paragraph_properties_depth > 0 => {
                        section_properties_depth += 1;
                        section_break_pending = true;
                        section_break_kind = Some(PageRefSectionBreak::Next);
                        section_type_seen = false;
                    }
                    b"type" if section_properties_depth > 0 && !section_type_seen => {
                        section_type_seen = true;
                        section_break_kind = page_ref_section_break(&e);
                    }
                    b"pageBreakBefore"
                        if paragraph_properties_depth > 0 && page_ref_on_off_enabled(&e) =>
                    {
                        current_page += 1;
                    }
                    b"fldSimple" => {
                        let instruction = attr_local(&e, b"instr");
                        if record_section_field(
                            instruction.as_deref(),
                            current_section,
                            &mut field_sections,
                        ) {
                            simple_section_field_result_depth = Some(xml_depth + 1);
                        } else if let Some(text) = computed_section_scan_field_result(
                            instruction.as_deref(),
                            &mut computed_fields,
                        ) {
                            if !text.is_empty() {
                                section_has_visible_content = true;
                            }
                            skip_subtree(&mut r);
                            continue;
                        }
                    }
                    b"fldChar" => {
                        apply_section_scan_fld_char(
                            &e,
                            current_section,
                            &mut current,
                            &mut field_sections,
                            &mut section_has_visible_content,
                            &mut computed_fields,
                        );
                    }
                    b"instrText" => {
                        let text = read_text(&mut r);
                        consumed_element = true;
                        if let Some(field) = current.as_mut() {
                            if field.phase == FieldPhase::Instruction {
                                field.instruction.push_str(&text);
                            }
                        }
                    }
                    b"t" => {
                        consumed_element = true;
                        if !read_text(&mut r).is_empty() {
                            mark_visible_section_content(
                                &mut section_has_visible_content,
                                &current,
                                simple_section_field_result_depth,
                            );
                        }
                    }
                    b"br" if is_page_break_type(&e) => {
                        current_page += 1;
                    }
                    b"lastRenderedPageBreak" => {
                        current_page += 1;
                    }
                    b"sym" if is_supported_run_symbol(&e) => {
                        mark_visible_section_content(
                            &mut section_has_visible_content,
                            &current,
                            simple_section_field_result_depth,
                        );
                    }
                    _ if is_visible_reference_mark(name) => {
                        mark_visible_section_content(
                            &mut section_has_visible_content,
                            &current,
                            simple_section_field_result_depth,
                        );
                    }
                    b"tab" | b"cr" | b"noBreakHyphen" | b"softHyphen" | b"drawing" | b"pict"
                    | b"object" => {
                        mark_visible_section_content(
                            &mut section_has_visible_content,
                            &current,
                            simple_section_field_result_depth,
                        );
                    }
                    b"br" => {
                        mark_visible_section_content(
                            &mut section_has_visible_content,
                            &current,
                            simple_section_field_result_depth,
                        );
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
                if suppresses_section_complex_result_scan(&current) {
                    if name == b"fldChar" {
                        apply_section_scan_fld_char(
                            &e,
                            current_section,
                            &mut current,
                            &mut field_sections,
                            &mut section_has_visible_content,
                            &mut computed_fields,
                        );
                    }
                    continue;
                }
                match name {
                    b"sectPr" if paragraph_properties_depth > 0 => {
                        section_break_pending = true;
                        section_break_kind = Some(PageRefSectionBreak::Next);
                        section_type_seen = false;
                    }
                    b"type" if section_properties_depth > 0 && !section_type_seen => {
                        section_type_seen = true;
                        section_break_kind = page_ref_section_break(&e);
                    }
                    b"pageBreakBefore"
                        if paragraph_properties_depth > 0 && page_ref_on_off_enabled(&e) =>
                    {
                        current_page += 1;
                    }
                    b"fldSimple" => {
                        let instruction = attr_local(&e, b"instr");
                        if !record_section_field(
                            instruction.as_deref(),
                            current_section,
                            &mut field_sections,
                        ) {
                            if let Some(text) = computed_section_scan_field_result(
                                instruction.as_deref(),
                                &mut computed_fields,
                            ) {
                                if !text.is_empty() {
                                    section_has_visible_content = true;
                                }
                            }
                        }
                    }
                    b"fldChar" => {
                        apply_section_scan_fld_char(
                            &e,
                            current_section,
                            &mut current,
                            &mut field_sections,
                            &mut section_has_visible_content,
                            &mut computed_fields,
                        );
                    }
                    b"br" if is_page_break_type(&e) => {
                        current_page += 1;
                    }
                    b"lastRenderedPageBreak" => {
                        current_page += 1;
                    }
                    b"sym" if is_supported_run_symbol(&e) => {
                        mark_visible_section_content(
                            &mut section_has_visible_content,
                            &current,
                            simple_section_field_result_depth,
                        );
                    }
                    _ if is_visible_reference_mark(name) => {
                        mark_visible_section_content(
                            &mut section_has_visible_content,
                            &current,
                            simple_section_field_result_depth,
                        );
                    }
                    b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" | b"drawing"
                    | b"pict" | b"object" => {
                        mark_visible_section_content(
                            &mut section_has_visible_content,
                            &current,
                            simple_section_field_result_depth,
                        );
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if suppresses_section_complex_result_scan(&current) {
                    xml_depth = xml_depth.saturating_sub(1);
                    continue;
                }
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                if name == b"sectPr" {
                    section_properties_depth = section_properties_depth.saturating_sub(1);
                }
                if name == b"pPr" {
                    paragraph_properties_depth = paragraph_properties_depth.saturating_sub(1);
                }
                if name == b"fldSimple" && simple_section_field_result_depth == Some(xml_depth) {
                    simple_section_field_result_depth = None;
                }
                if name == b"p" && section_break_pending {
                    push_section_page_count(
                        &mut section_page_counts,
                        current_page,
                        section_start_page,
                        section_has_visible_content,
                    );
                    if let Some(section_break) = section_break_kind {
                        current_page = page_after_section_break(current_page, section_break);
                    }
                    current_section += 1;
                    section_start_page = current_page;
                    section_has_visible_content = false;
                    section_break_pending = false;
                    section_break_kind = Some(PageRefSectionBreak::Next);
                    section_type_seen = false;
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    push_section_page_count(
        &mut section_page_counts,
        current_page,
        section_start_page,
        section_has_visible_content,
    );
    SectionContext {
        field_sections,
        section_page_counts,
    }
}

fn push_section_page_count(
    section_page_counts: &mut Vec<Option<usize>>,
    current_page: usize,
    section_start_page: usize,
    section_has_visible_content: bool,
) {
    section_page_counts.push(
        (!section_has_visible_content)
            .then_some(current_page.saturating_sub(section_start_page) + 1),
    );
}

fn record_section_field(
    instruction: Option<&str>,
    current_section: usize,
    field_sections: &mut Vec<usize>,
) -> bool {
    let is_section_field = instruction
        .map(normalize_instruction)
        .as_deref()
        .and_then(section_instruction)
        .is_some();
    if is_section_field {
        field_sections.push(current_section);
    }
    is_section_field
}

#[derive(Debug)]
struct SectionComputedFieldState<'a> {
    document_bookmarks: &'a HashMap<String, String>,
    properties: FieldDocumentProperties<'a>,
    legacy_forms: LegacyFormContext,
    field_bookmarks: HashMap<String, String>,
    sequence_counters: HashMap<String, i64>,
    autonum_counter: i64,
    listnum_counter: i64,
    form_field_index: usize,
}

impl<'a> SectionComputedFieldState<'a> {
    fn new(
        document_bookmarks: &'a HashMap<String, String>,
        properties: FieldDocumentProperties<'a>,
        legacy_forms: LegacyFormContext,
    ) -> Self {
        Self {
            document_bookmarks,
            properties,
            legacy_forms,
            field_bookmarks: HashMap::new(),
            sequence_counters: HashMap::new(),
            autonum_counter: 0,
            listnum_counter: 0,
            form_field_index: 0,
        }
    }
}

fn computed_section_scan_field_result(
    instruction: Option<&str>,
    state: &mut SectionComputedFieldState<'_>,
) -> Option<String> {
    let instruction = normalize_instruction(instruction?);
    if is_section_scan_instruction(&instruction) {
        return None;
    }
    let kind = FieldKind::from_instruction(&instruction);
    match &kind {
        FieldKind::Ref => {
            return computed_section_scan_ref_result(&instruction, state);
        }
        FieldKind::Dynamic(kind) if kind == "SET" => {
            return computed_set_result(&instruction, &mut state.field_bookmarks);
        }
        FieldKind::Dynamic(kind) if kind == "ASK" => {
            return computed_ask_result(&instruction, &mut state.field_bookmarks);
        }
        FieldKind::FormField(_) => {
            let result = computed_legacy_form_result(
                &instruction,
                "",
                &state.legacy_forms,
                state.form_field_index,
            );
            state.form_field_index += 1;
            return result;
        }
        _ => {}
    }
    if matches!(kind, FieldKind::Unknown(_)) {
        if let Some(text) = computed_section_scan_direct_bookmark_ref_result(&instruction, state) {
            return Some(text);
        }
    }
    computed_numbering_result(&instruction, &mut state.autonum_counter)
        .or_else(|| computed_listnum_result(&instruction, &mut state.listnum_counter))
        .or_else(|| computed_sequence_result(&instruction, &mut state.sequence_counters))
        .or_else(|| computed_dynamic_result_with_bookmarks(&instruction, &state.field_bookmarks))
        .or_else(|| {
            computed_formula_result_with_bookmark_context(
                &instruction,
                state.document_bookmarks,
                &state.field_bookmarks,
            )
        })
        .or_else(|| {
            computed_if_compare_result_with_bookmark_context(
                &instruction,
                state.document_bookmarks,
                &state.field_bookmarks,
            )
        })
        .or_else(|| {
            computed_merge_control_result_with_bookmark_context(
                &instruction,
                state.document_bookmarks,
                &state.field_bookmarks,
            )
        })
        .or_else(|| computed_display_result(&instruction))
        .or_else(|| computed_action_result(&instruction))
        .or_else(|| computed_revision_number_result(&instruction, state.properties.core))
        .or_else(|| {
            computed_document_info_result(
                &instruction,
                state.properties.core,
                state.properties.custom,
                state.properties.variables,
                state.properties.extended,
                state.properties.file_size_bytes,
            )
        })
        .or_else(|| computed_reference_index_result(&instruction))
        .or_else(|| computed_toc_entry_result(&instruction))
}

fn computed_section_scan_ref_result(
    instruction: &str,
    state: &SectionComputedFieldState<'_>,
) -> Option<String> {
    let ref_positions = RefPositionContext::default();
    let ref_numbers = RefNumberContext::empty();
    let note_refs = NoteRefContext::empty();
    let ctx = RefResultContext {
        bookmarks: state.document_bookmarks,
        ref_positions: &ref_positions,
        ref_numbers: &ref_numbers,
        note_refs: &note_refs,
        field_bookmarks: &state.field_bookmarks,
    };
    computed_ref_result(instruction, &ctx, None, None)
}

fn computed_section_scan_direct_bookmark_ref_result(
    instruction: &str,
    state: &SectionComputedFieldState<'_>,
) -> Option<String> {
    let ref_positions = RefPositionContext::default();
    let ref_numbers = RefNumberContext::empty();
    let note_refs = NoteRefContext::empty();
    let ctx = RefResultContext {
        bookmarks: state.document_bookmarks,
        ref_positions: &ref_positions,
        ref_numbers: &ref_numbers,
        note_refs: &note_refs,
        field_bookmarks: &state.field_bookmarks,
    };
    computed_direct_bookmark_ref_result(instruction, &ctx, None, None)
}

fn section_field_result_content_is_hidden(
    current: &Option<SectionScanField>,
    simple_section_field_result_depth: Option<usize>,
) -> bool {
    simple_section_field_result_depth.is_some()
        || current.as_ref().is_some_and(|field| {
            field.phase == FieldPhase::Result && is_section_scan_instruction(&field.instruction)
        })
}

fn mark_visible_section_content(
    section_has_visible_content: &mut bool,
    current: &Option<SectionScanField>,
    simple_section_field_result_depth: Option<usize>,
) {
    if !section_field_result_content_is_hidden(current, simple_section_field_result_depth) {
        *section_has_visible_content = true;
    }
}

fn suppresses_section_complex_result_scan(current: &Option<SectionScanField>) -> bool {
    current
        .as_ref()
        .is_some_and(|field| field.phase == FieldPhase::Result && field.suppress_result)
}

fn is_section_scan_instruction(instruction: &str) -> bool {
    let instruction = normalize_instruction(instruction);
    section_instruction(&instruction).is_some()
}

fn apply_section_scan_fld_char(
    e: &BytesStart<'_>,
    current_section: usize,
    current: &mut Option<SectionScanField>,
    field_sections: &mut Vec<usize>,
    section_has_visible_content: &mut bool,
    computed_fields: &mut SectionComputedFieldState<'_>,
) {
    match field_char_type(e).as_deref() {
        Some("begin") => {
            if let Some(field) = current.as_mut() {
                if field.phase == FieldPhase::Result && field.suppress_result {
                    field.nested_suppressed_fields =
                        field.nested_suppressed_fields.saturating_add(1);
                    return;
                }
            }
            *current = Some(SectionScanField {
                instruction: String::new(),
                phase: FieldPhase::Instruction,
                suppress_result: false,
                nested_suppressed_fields: 0,
            });
        }
        Some("separate") => {
            if let Some(field) = current.as_mut() {
                if field.suppress_result {
                    return;
                }
                if let Some(text) =
                    computed_section_scan_field_result(Some(&field.instruction), computed_fields)
                {
                    if !text.is_empty() {
                        *section_has_visible_content = true;
                    }
                    field.suppress_result = true;
                }
                field.phase = FieldPhase::Result;
            }
        }
        Some("end") => {
            if let Some(field) = current.as_mut() {
                if field.suppress_result && field.nested_suppressed_fields > 0 {
                    field.nested_suppressed_fields -= 1;
                    return;
                }
            }
            if let Some(field) = current.take() {
                record_section_field(Some(&field.instruction), current_section, field_sections);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SectionInstruction {
    result: SectionResult,
    number_format: Option<PageNumberFormat>,
    text_format: Option<FieldTextFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SectionResult {
    Number,
    Pages,
}

pub(crate) fn is_section_field_instruction(instruction: &str) -> bool {
    section_instruction(instruction).is_some()
}

pub(crate) fn computed_section_result(
    instruction: &str,
    position: Option<SectionFieldPosition>,
) -> Option<String> {
    let spec = section_instruction(instruction)?;
    let position = position?;
    let value = match spec.result {
        SectionResult::Number => position.section,
        SectionResult::Pages => position.section_pages?,
    };
    Some(apply_field_text_format(
        format_page_number(value, spec.number_format)?,
        spec.text_format,
    ))
}

fn section_instruction(instruction: &str) -> Option<SectionInstruction> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    let result = if kind.eq_ignore_ascii_case("SECTION") {
        SectionResult::Number
    } else if kind.eq_ignore_ascii_case("SECTIONPAGES") {
        SectionResult::Pages
    } else {
        return None;
    };
    let format = page_field_format_syntax_tail(&mut parts)?;
    Some(SectionInstruction {
        result,
        number_format: format
            .number_format
            .map(page_number_format_from_field_format),
        text_format: format.text_format,
    })
}
