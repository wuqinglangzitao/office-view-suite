use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct LegacyFormContext {
    results: Vec<Option<String>>,
    preserve_cached: bool,
}

impl LegacyFormContext {
    fn field_result(&self, index: usize) -> Option<String> {
        self.results.get(index).cloned().flatten()
    }

    fn preserves_cached(&self) -> bool {
        self.preserve_cached
    }
}

#[derive(Debug, Clone, Default)]
struct LegacyFormData {
    checkbox: Option<bool>,
    dropdown_entries: Vec<String>,
    dropdown_default: Option<usize>,
    dropdown_result: Option<usize>,
    text_default: Option<String>,
}

#[derive(Debug, Clone)]
struct LegacyFormScanField {
    instruction: String,
    form_data: Option<LegacyFormData>,
    phase: FieldPhase,
}

pub(crate) fn legacy_form_context(xml: &str, preserve_cached: bool) -> LegacyFormContext {
    let mut r = Reader::from_str(xml);
    let mut results = Vec::new();
    let mut current: Option<LegacyFormScanField> = None;
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
                    b"fldSimple" => {
                        let instruction = attr_local(&e, b"instr").unwrap_or_default();
                        let form_data = read_legacy_form_data_until(&mut r);
                        record_simple_legacy_form_result(&instruction, form_data, &mut results);
                        consumed_element = true;
                    }
                    b"fldChar" => {
                        let kind = field_char_type(&e);
                        let form_data = read_legacy_form_data_until(&mut r);
                        apply_legacy_form_scan_fld_char(
                            kind.as_deref(),
                            form_data,
                            &mut current,
                            &mut results,
                        );
                        consumed_element = true;
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
                        record_simple_legacy_form_result(
                            attr_local(&e, b"instr").as_deref().unwrap_or_default(),
                            None,
                            &mut results,
                        );
                    }
                    b"fldChar" => apply_legacy_form_scan_fld_char(
                        field_char_type(&e).as_deref(),
                        None,
                        &mut current,
                        &mut results,
                    ),
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
    LegacyFormContext {
        results,
        preserve_cached,
    }
}

fn record_simple_legacy_form_result(
    instruction: &str,
    form_data: Option<LegacyFormData>,
    results: &mut Vec<Option<String>>,
) {
    let instruction = normalize_instruction(instruction);
    if matches!(field_kind(&instruction), FieldKind::FormField(_)) {
        results.push(form_data.and_then(|data| legacy_form_field_result(&instruction, &data)));
    }
}

fn apply_legacy_form_scan_fld_char(
    kind: Option<&str>,
    form_data: Option<LegacyFormData>,
    current: &mut Option<LegacyFormScanField>,
    results: &mut Vec<Option<String>>,
) {
    match kind {
        Some("begin") => {
            *current = Some(LegacyFormScanField {
                instruction: String::new(),
                form_data,
                phase: FieldPhase::Instruction,
            });
        }
        Some("separate") => {
            if let Some(field) = current.as_mut() {
                field.phase = FieldPhase::Result;
            }
        }
        Some("end") => {
            if let Some(field) = current.take() {
                let instruction = normalize_instruction(&field.instruction);
                if matches!(field_kind(&instruction), FieldKind::FormField(_)) {
                    results.push(
                        field
                            .form_data
                            .and_then(|data| legacy_form_field_result(&instruction, &data)),
                    );
                }
            }
        }
        _ => {}
    }
}

fn read_legacy_form_data_until(r: &mut Xml<'_>) -> Option<LegacyFormData> {
    let mut depth = 1usize;
    let mut xml_depth = 1usize;
    let mut alternate_content_stack = Vec::new();
    let mut checkbox_depth = 0usize;
    let mut dropdown_depth = 0usize;
    let mut text_input_depth = 0usize;
    let mut data = LegacyFormData::default();
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
                let is_checkbox = name == b"checkBox";
                let is_dropdown = name == b"ddList";
                let is_text_input = name == b"textInput";
                read_legacy_form_data_element(
                    &e,
                    checkbox_depth > 0,
                    dropdown_depth > 0,
                    text_input_depth > 0,
                    &mut data,
                );
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.push(AlternateContentBranchState {
                            branch_depth: xml_depth + 1,
                            took_branch: false,
                        });
                    }
                    _ if is_checkbox => checkbox_depth += 1,
                    _ if is_dropdown => dropdown_depth += 1,
                    _ if is_text_input => text_input_depth += 1,
                    _ => {}
                }
                depth += 1;
                xml_depth = xml_depth.saturating_add(1);
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    continue;
                }
                read_legacy_form_data_element(
                    &e,
                    checkbox_depth > 0,
                    dropdown_depth > 0,
                    text_input_depth > 0,
                    &mut data,
                );
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                if name == b"checkBox" {
                    checkbox_depth = checkbox_depth.saturating_sub(1);
                } else if name == b"ddList" {
                    dropdown_depth = dropdown_depth.saturating_sub(1);
                } else if name == b"textInput" {
                    text_input_depth = text_input_depth.saturating_sub(1);
                }
                depth = depth.saturating_sub(1);
                xml_depth = xml_depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    (data.checkbox.is_some()
        || !data.dropdown_entries.is_empty()
        || data.dropdown_default.is_some()
        || data.dropdown_result.is_some()
        || data.text_default.is_some())
    .then_some(data)
}

fn read_legacy_form_data_element(
    e: &BytesStart<'_>,
    in_checkbox: bool,
    in_dropdown: bool,
    in_text_input: bool,
    data: &mut LegacyFormData,
) {
    match local(e.name().as_ref()) {
        b"checked" if in_checkbox => data.checkbox = Some(toggle_on(attr_local(e, b"val"))),
        b"default" if in_checkbox && data.checkbox.is_none() => {
            data.checkbox = Some(toggle_on(attr_local(e, b"val")));
        }
        b"default" if in_dropdown => data.dropdown_default = attr_usize(e, b"val"),
        b"default" if in_text_input => {
            data.text_default = attr_local(e, b"val");
        }
        b"result" if in_dropdown => data.dropdown_result = attr_usize(e, b"val"),
        b"listEntry" if in_dropdown => {
            if let Some(value) = attr_local(e, b"val") {
                data.dropdown_entries.push(value);
            }
        }
        _ => {}
    }
}

fn legacy_form_field_result(instruction: &str, data: &LegacyFormData) -> Option<String> {
    let spec = legacy_form_field_syntax(instruction)?;
    let text = match spec.kind.as_str() {
        "FORMCHECKBOX" => Some(
            if data.checkbox? {
                "\u{2612}"
            } else {
                "\u{2610}"
            }
            .to_string(),
        ),
        "FORMDROPDOWN" => {
            let result = data
                .dropdown_result
                .and_then(|index| data.dropdown_entries.get(index));
            let default = data
                .dropdown_default
                .and_then(|index| data.dropdown_entries.get(index));
            result.or(default).cloned()
        }
        "FORMTEXT" => data.text_default.clone(),
        _ => None,
    }?;
    Some(apply_field_text_format(text, spec.text_format))
}

pub(crate) fn computed_legacy_form_result(
    instruction: &str,
    current_result: &str,
    legacy_forms: &LegacyFormContext,
    field_index: usize,
) -> Option<String> {
    if legacy_forms.preserves_cached() {
        return None;
    }
    let spec = legacy_form_field_syntax(instruction)?;
    match spec.kind.as_str() {
        "FORMTEXT" if !current_result.is_empty() => Some(apply_field_text_format(
            current_result.to_string(),
            spec.text_format,
        )),
        _ => legacy_forms.field_result(field_index),
    }
}
