use super::*;

pub(crate) fn ref_targets(xml: &str) -> HashMap<String, String> {
    let core_properties = CoreProperties::default();
    let empty_properties = HashMap::new();
    ref_targets_with_properties(
        xml,
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

pub(crate) fn ref_targets_with_properties(
    xml: &str,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
) -> HashMap<String, String> {
    ref_targets_with_optional_note_context(xml, properties, preserve_legacy_form_cache, None)
}

pub(crate) fn ref_targets_with_note_context(
    xml: &str,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
    note_refs: &NoteRefContext,
) -> HashMap<String, String> {
    ref_targets_with_optional_note_context(
        xml,
        properties,
        preserve_legacy_form_cache,
        Some(note_refs),
    )
}

fn ref_targets_with_optional_note_context(
    xml: &str,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
    note_refs: Option<&NoteRefContext>,
) -> HashMap<String, String> {
    let legacy_forms = legacy_form_context(xml, preserve_legacy_form_cache);
    let mut r = Reader::from_str(xml);
    let mut active: Vec<(String, String)> = Vec::new();
    let mut out: HashMap<String, String> = HashMap::new();
    let mut xml_depth = 0usize;
    let mut alternate_content_stack = Vec::new();
    let mut current = Vec::new();
    let mut autonum_counter = 0i64;
    let mut listnum_counter = 0i64;
    let mut sequence_counters = HashMap::new();
    let mut field_bookmarks = HashMap::new();
    let mut form_field_index = 0usize;
    let mut ref_field_index = 0usize;
    let mut note_ref_field_index = 0usize;
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
                    b"fldSimple" if !active.is_empty() => {
                        append_ref_simple_field_result(
                            &active,
                            &mut current,
                            &mut out,
                            &mut RefTargetCtx {
                                legacy_forms: &legacy_forms,
                                properties,
                                note_refs,
                                autonum_counter: &mut autonum_counter,
                                listnum_counter: &mut listnum_counter,
                                sequence_counters: &mut sequence_counters,
                                field_bookmarks: &mut field_bookmarks,
                                form_field_index: &mut form_field_index,
                                ref_field_index: &mut ref_field_index,
                                note_ref_field_index: &mut note_ref_field_index,
                            },
                            &mut r,
                            &e,
                        );
                        continue;
                    }
                    b"fldSimple" if ref_target_complex_in_result(&current) => {
                        append_ref_simple_field_result(
                            &active,
                            &mut current,
                            &mut out,
                            &mut RefTargetCtx {
                                legacy_forms: &legacy_forms,
                                properties,
                                note_refs,
                                autonum_counter: &mut autonum_counter,
                                listnum_counter: &mut listnum_counter,
                                sequence_counters: &mut sequence_counters,
                                field_bookmarks: &mut field_bookmarks,
                                form_field_index: &mut form_field_index,
                                ref_field_index: &mut ref_field_index,
                                note_ref_field_index: &mut note_ref_field_index,
                            },
                            &mut r,
                            &e,
                        );
                        continue;
                    }
                    b"fldSimple"
                        if is_ref_position_field_instruction(
                            &attr_local(&e, b"instr").unwrap_or_default(),
                        ) =>
                    {
                        let _ = ref_target_note_ref_field_position(
                            attr_local(&e, b"instr").as_deref().unwrap_or_default(),
                            note_refs,
                            &mut ref_field_index,
                        );
                    }
                    b"fldSimple"
                        if is_note_ref_source_position_field_instruction(
                            attr_local(&e, b"instr").as_deref(),
                        ) =>
                    {
                        let _ = ref_target_direct_note_ref_field_position(
                            attr_local(&e, b"instr").as_deref().unwrap_or_default(),
                            note_refs,
                            &mut note_ref_field_index,
                        );
                    }
                    b"fldSimple"
                        if is_ref_target_source_order_field_instruction(
                            attr_local(&e, b"instr").as_deref(),
                        ) =>
                    {
                        let field = read_simple_field(&mut r, &e);
                        let _ = computed_ref_target_field_result(
                            &field.instruction,
                            &out,
                            &[],
                            &mut RefTargetCtx {
                                legacy_forms: &legacy_forms,
                                properties,
                                note_refs,
                                autonum_counter: &mut autonum_counter,
                                listnum_counter: &mut listnum_counter,
                                sequence_counters: &mut sequence_counters,
                                field_bookmarks: &mut field_bookmarks,
                                form_field_index: &mut form_field_index,
                                ref_field_index: &mut ref_field_index,
                                note_ref_field_index: &mut note_ref_field_index,
                            },
                            None,
                            &field.result,
                        )
                        .unwrap_or(field.result);
                        continue;
                    }
                    b"fldSimple"
                        if is_legacy_form_field_instruction(
                            attr_local(&e, b"instr").as_deref(),
                        ) =>
                    {
                        let field = read_simple_field(&mut r, &e);
                        let _ = computed_ref_target_field_result(
                            &field.instruction,
                            &out,
                            &[],
                            &mut RefTargetCtx {
                                legacy_forms: &legacy_forms,
                                properties,
                                note_refs,
                                autonum_counter: &mut autonum_counter,
                                listnum_counter: &mut listnum_counter,
                                sequence_counters: &mut sequence_counters,
                                field_bookmarks: &mut field_bookmarks,
                                form_field_index: &mut form_field_index,
                                ref_field_index: &mut ref_field_index,
                                note_ref_field_index: &mut note_ref_field_index,
                            },
                            None,
                            &field.result,
                        );
                        continue;
                    }
                    b"fldChar" => {
                        apply_ref_target_fld_char(
                            &e,
                            &active,
                            &mut current,
                            &mut out,
                            &mut RefTargetCtx {
                                legacy_forms: &legacy_forms,
                                properties,
                                note_refs,
                                autonum_counter: &mut autonum_counter,
                                listnum_counter: &mut listnum_counter,
                                sequence_counters: &mut sequence_counters,
                                field_bookmarks: &mut field_bookmarks,
                                form_field_index: &mut form_field_index,
                                ref_field_index: &mut ref_field_index,
                                note_ref_field_index: &mut note_ref_field_index,
                            },
                        );
                    }
                    b"p" => append_ref_target_paragraph_breaks(&active, &mut current, &mut out),
                    b"bookmarkStart" => {
                        if let Some((id, name)) = bookmark_start(&e) {
                            out.entry(name.clone()).or_default();
                            active.push((id, name));
                        }
                    }
                    b"bookmarkEnd" => {
                        if let Some(id) = bookmark_end_id(&e) {
                            active.retain(|(active_id, _)| active_id != &id);
                        }
                    }
                    b"t" => {
                        let text = read_text(&mut r);
                        if !text.is_empty() {
                            append_ref_target_text(&active, &mut current, &mut out, &text);
                        }
                        continue;
                    }
                    b"instrText" => {
                        let text = read_text(&mut r);
                        if let Some(field) = current.last_mut() {
                            if field.phase == FieldPhase::Instruction {
                                field.instruction.push_str(&text);
                            }
                        }
                        continue;
                    }
                    b"tab" => append_ref_target_text(&active, &mut current, &mut out, "\t"),
                    b"br" => {
                        if is_page_break_type(&e) {
                            append_ref_target_text(&active, &mut current, &mut out, "\u{000C}");
                        } else {
                            append_ref_target_text(&active, &mut current, &mut out, "\n");
                        }
                    }
                    b"cr" => append_ref_target_text(&active, &mut current, &mut out, "\n"),
                    b"noBreakHyphen" => {
                        append_ref_target_text(&active, &mut current, &mut out, "-")
                    }
                    b"softHyphen" => {
                        append_ref_target_text(&active, &mut current, &mut out, "\u{00ad}")
                    }
                    b"sym" => append_ref_target_symbol(&active, &mut current, &mut out, &e),
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
                match name {
                    b"bookmarkStart" => {
                        if let Some((id, name)) = bookmark_start(&e) {
                            out.entry(name.clone()).or_default();
                            active.push((id, name));
                        }
                    }
                    b"bookmarkEnd" => {
                        if let Some(id) = bookmark_end_id(&e) {
                            active.retain(|(active_id, _)| active_id != &id);
                        }
                    }
                    b"fldSimple"
                        if !active.is_empty() || ref_target_complex_in_result(&current) =>
                    {
                        append_ref_empty_simple_field_result(
                            attr_local(&e, b"instr").as_deref(),
                            &active,
                            &mut current,
                            &mut out,
                            &mut RefTargetCtx {
                                legacy_forms: &legacy_forms,
                                properties,
                                note_refs,
                                autonum_counter: &mut autonum_counter,
                                listnum_counter: &mut listnum_counter,
                                sequence_counters: &mut sequence_counters,
                                field_bookmarks: &mut field_bookmarks,
                                form_field_index: &mut form_field_index,
                                ref_field_index: &mut ref_field_index,
                                note_ref_field_index: &mut note_ref_field_index,
                            },
                        );
                    }
                    b"fldSimple"
                        if is_ref_position_field_instruction(
                            &attr_local(&e, b"instr").unwrap_or_default(),
                        ) =>
                    {
                        let _ = ref_target_note_ref_field_position(
                            attr_local(&e, b"instr").as_deref().unwrap_or_default(),
                            note_refs,
                            &mut ref_field_index,
                        );
                    }
                    b"fldSimple"
                        if is_note_ref_source_position_field_instruction(
                            attr_local(&e, b"instr").as_deref(),
                        ) =>
                    {
                        let _ = ref_target_direct_note_ref_field_position(
                            attr_local(&e, b"instr").as_deref().unwrap_or_default(),
                            note_refs,
                            &mut note_ref_field_index,
                        );
                    }
                    b"fldSimple"
                        if is_ref_target_source_order_field_instruction(
                            attr_local(&e, b"instr").as_deref(),
                        ) =>
                    {
                        let instruction = attr_local(&e, b"instr").unwrap_or_default();
                        let _ = computed_ref_target_field_result(
                            &instruction,
                            &out,
                            &[],
                            &mut RefTargetCtx {
                                legacy_forms: &legacy_forms,
                                properties,
                                note_refs,
                                autonum_counter: &mut autonum_counter,
                                listnum_counter: &mut listnum_counter,
                                sequence_counters: &mut sequence_counters,
                                field_bookmarks: &mut field_bookmarks,
                                form_field_index: &mut form_field_index,
                                ref_field_index: &mut ref_field_index,
                                note_ref_field_index: &mut note_ref_field_index,
                            },
                            None,
                            "",
                        );
                    }
                    b"fldSimple"
                        if is_legacy_form_field_instruction(
                            attr_local(&e, b"instr").as_deref(),
                        ) =>
                    {
                        let _ = computed_ref_target_field_result(
                            attr_local(&e, b"instr").as_deref().unwrap_or_default(),
                            &out,
                            &[],
                            &mut RefTargetCtx {
                                legacy_forms: &legacy_forms,
                                properties,
                                note_refs,
                                autonum_counter: &mut autonum_counter,
                                listnum_counter: &mut listnum_counter,
                                sequence_counters: &mut sequence_counters,
                                field_bookmarks: &mut field_bookmarks,
                                form_field_index: &mut form_field_index,
                                ref_field_index: &mut ref_field_index,
                                note_ref_field_index: &mut note_ref_field_index,
                            },
                            None,
                            "",
                        );
                    }
                    b"fldChar" => {
                        apply_ref_target_fld_char(
                            &e,
                            &active,
                            &mut current,
                            &mut out,
                            &mut RefTargetCtx {
                                legacy_forms: &legacy_forms,
                                properties,
                                note_refs,
                                autonum_counter: &mut autonum_counter,
                                listnum_counter: &mut listnum_counter,
                                sequence_counters: &mut sequence_counters,
                                field_bookmarks: &mut field_bookmarks,
                                form_field_index: &mut form_field_index,
                                ref_field_index: &mut ref_field_index,
                                note_ref_field_index: &mut note_ref_field_index,
                            },
                        );
                    }
                    b"tab" => append_ref_target_text(&active, &mut current, &mut out, "\t"),
                    b"br" => {
                        if is_page_break_type(&e) {
                            append_ref_target_text(&active, &mut current, &mut out, "\u{000C}");
                        } else {
                            append_ref_target_text(&active, &mut current, &mut out, "\n");
                        }
                    }
                    b"cr" => append_ref_target_text(&active, &mut current, &mut out, "\n"),
                    b"noBreakHyphen" => {
                        append_ref_target_text(&active, &mut current, &mut out, "-")
                    }
                    b"softHyphen" => {
                        append_ref_target_text(&active, &mut current, &mut out, "\u{00ad}")
                    }
                    b"sym" => append_ref_target_symbol(&active, &mut current, &mut out, &e),
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
    out
}

#[derive(Debug, Clone)]
struct RefTargetComplexField {
    active: Vec<(String, String)>,
    instruction: String,
    result: String,
    phase: FieldPhase,
    note_ref_field_position: Option<NoteRefFieldPosition>,
}

/// Threaded REF-target evaluation state: read-only document context plus the
/// mutable sequence/index accumulators that travel together through the scan.
struct RefTargetCtx<'a> {
    legacy_forms: &'a LegacyFormContext,
    properties: FieldDocumentProperties<'a>,
    note_refs: Option<&'a NoteRefContext>,
    autonum_counter: &'a mut i64,
    listnum_counter: &'a mut i64,
    sequence_counters: &'a mut HashMap<String, i64>,
    field_bookmarks: &'a mut HashMap<String, String>,
    form_field_index: &'a mut usize,
    ref_field_index: &'a mut usize,
    note_ref_field_index: &'a mut usize,
}

fn apply_ref_target_fld_char(
    e: &BytesStart<'_>,
    active: &[(String, String)],
    current: &mut Vec<RefTargetComplexField>,
    out: &mut HashMap<String, String>,
    ctx: &mut RefTargetCtx<'_>,
) {
    match field_char_type(e).as_deref() {
        Some("begin") => {
            current.push(RefTargetComplexField {
                active: active.to_vec(),
                instruction: String::new(),
                result: String::new(),
                phase: FieldPhase::Instruction,
                note_ref_field_position: None,
            });
        }
        Some("separate") => {
            let mut consumed_ref_position = false;
            if current.len() == 1 {
                let field = &mut current[0];
                field.note_ref_field_position = ref_target_note_ref_field_position(
                    &field.instruction,
                    ctx.note_refs,
                    ctx.ref_field_index,
                );
                consumed_ref_position = true;
                if field.active.is_empty()
                    && !is_ref_target_source_order_field_instruction(Some(&field.instruction))
                    && !is_legacy_form_field_instruction(Some(&field.instruction))
                {
                    current.pop();
                    return;
                }
            }
            if let Some(field) = current.last_mut() {
                if !consumed_ref_position {
                    field.note_ref_field_position = ref_target_note_ref_field_position(
                        &field.instruction,
                        ctx.note_refs,
                        ctx.ref_field_index,
                    );
                }
                field.phase = FieldPhase::Result;
            }
        }
        Some("end") => {
            let Some(field) = current.pop() else {
                return;
            };
            let text = computed_ref_target_field_result(
                &field.instruction,
                out,
                &field.active,
                ctx,
                field.note_ref_field_position,
                &field.result,
            )
            .unwrap_or(field.result);
            if let Some(parent) = current.last_mut() {
                if parent.phase == FieldPhase::Result {
                    parent.result.push_str(&text);
                }
            } else {
                append_ref_text(&field.active, out, &text);
            }
        }
        _ => {}
    }
}

fn append_ref_target_paragraph_breaks(
    active: &[(String, String)],
    current: &mut [RefTargetComplexField],
    out: &mut HashMap<String, String>,
) {
    if let Some(field) = current.last_mut() {
        if field.phase == FieldPhase::Result && !field.result.is_empty() {
            field.result.push('\n');
        }
        return;
    }
    append_ref_paragraph_breaks(active, out);
}

fn append_ref_target_text(
    active: &[(String, String)],
    current: &mut [RefTargetComplexField],
    out: &mut HashMap<String, String>,
    text: &str,
) {
    if let Some(field) = current.last_mut() {
        if field.phase == FieldPhase::Result {
            field.result.push_str(text);
        }
        return;
    }
    append_ref_text(active, out, text);
}

fn append_ref_target_symbol(
    active: &[(String, String)],
    current: &mut [RefTargetComplexField],
    out: &mut HashMap<String, String>,
    e: &BytesStart<'_>,
) {
    let font = attr_local_trimmed(e, b"font");
    let Some(value) = attr_local_trimmed(e, b"char") else {
        return;
    };
    let Some(ch) = super::display::computed_run_symbol_char(font.as_deref(), &value) else {
        return;
    };
    append_ref_target_text(active, current, out, &ch.to_string());
}

fn append_ref_simple_field_result(
    active: &[(String, String)],
    current: &mut [RefTargetComplexField],
    out: &mut HashMap<String, String>,
    ctx: &mut RefTargetCtx<'_>,
    r: &mut Xml<'_>,
    e: &BytesStart<'_>,
) {
    let excluded = ref_target_excluded_bookmarks(active, current);
    let (instruction, result) = read_simple_field_result(r, e, |instruction, result| {
        let note_ref_field_position =
            ref_target_note_ref_field_position(instruction, ctx.note_refs, ctx.ref_field_index);
        computed_ref_target_field_result(
            instruction,
            out,
            excluded,
            ctx,
            note_ref_field_position,
            result,
        )
    });
    let note_ref_field_position =
        ref_target_note_ref_field_position(&instruction, ctx.note_refs, ctx.ref_field_index);
    let text = computed_ref_target_field_result(
        &instruction,
        out,
        excluded,
        ctx,
        note_ref_field_position,
        &result,
    )
    .unwrap_or(result);
    append_ref_target_text(active, current, out, &text);
}

fn append_ref_empty_simple_field_result(
    instruction: Option<&str>,
    active: &[(String, String)],
    current: &mut [RefTargetComplexField],
    out: &mut HashMap<String, String>,
    ctx: &mut RefTargetCtx<'_>,
) {
    let instruction = instruction.unwrap_or_default();
    let excluded = ref_target_excluded_bookmarks(active, current);
    let note_ref_field_position =
        ref_target_note_ref_field_position(instruction, ctx.note_refs, ctx.ref_field_index);
    let text = computed_ref_target_field_result(
        instruction,
        out,
        excluded,
        ctx,
        note_ref_field_position,
        "",
    )
    .unwrap_or_default();
    append_ref_target_text(active, current, out, &text);
}

fn computed_ref_target_field_result(
    instruction: &str,
    bookmarks: &HashMap<String, String>,
    excluded: &[(String, String)],
    ctx: &mut RefTargetCtx<'_>,
    note_ref_field_position: Option<NoteRefFieldPosition>,
    current_result: &str,
) -> Option<String> {
    let legacy_forms = ctx.legacy_forms;
    let properties = ctx.properties;
    let note_refs = ctx.note_refs;
    let autonum_counter = &mut *ctx.autonum_counter;
    let listnum_counter = &mut *ctx.listnum_counter;
    let sequence_counters = &mut *ctx.sequence_counters;
    let field_bookmarks = &mut *ctx.field_bookmarks;
    let form_field_index = &mut *ctx.form_field_index;
    let note_ref_field_index = &mut *ctx.note_ref_field_index;
    let available_bookmarks = ref_target_available_bookmarks(bookmarks, excluded);
    let instruction = normalize_instruction(instruction);
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
        _ => {}
    }
    let bookmarks = ref_target_merged_bookmarks(&available_bookmarks, field_bookmarks);
    let direct_note_ref_field_position =
        ref_target_direct_note_ref_field_position(&instruction, note_refs, note_ref_field_index);
    computed_ref_target_ref_result(&instruction, &bookmarks)
        .or_else(|| {
            computed_ref_target_ref_note_reference_result(
                &instruction,
                note_refs,
                note_ref_field_position,
            )
        })
        .or_else(|| {
            note_refs.and_then(|note_refs| {
                computed_note_ref_result(&instruction, note_refs, direct_note_ref_field_position)
            })
        })
        .or_else(|| computed_numbering_result(&instruction, autonum_counter))
        .or_else(|| computed_listnum_result(&instruction, listnum_counter))
        .or_else(|| computed_sequence_result(&instruction, sequence_counters))
        .or_else(|| computed_toc_entry_result(&instruction))
        .or_else(|| computed_dynamic_result_with_bookmarks(&instruction, &bookmarks))
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
        .or_else(|| computed_display_result(&instruction))
        .or_else(|| computed_action_result(&instruction))
        .or_else(|| computed_reference_index_result(&instruction))
}

fn computed_ref_target_ref_result(
    instruction: &str,
    bookmarks: &HashMap<String, String>,
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
    let text = bookmarks.get(&spec.target)?;
    let text = computed_ref_bookmark_text_result(
        text,
        spec.number_format,
        spec.number_picture.as_deref(),
    )?;
    Some(apply_field_text_format(text, spec.text_format))
}

fn computed_ref_target_ref_note_reference_result(
    instruction: &str,
    note_refs: Option<&NoteRefContext>,
    note_ref_field_position: Option<NoteRefFieldPosition>,
) -> Option<String> {
    let note_refs = note_refs?;
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

fn ref_target_note_ref_field_position(
    instruction: &str,
    note_refs: Option<&NoteRefContext>,
    ref_field_index: &mut usize,
) -> Option<NoteRefFieldPosition> {
    let instruction = normalize_instruction(instruction);
    if !is_ref_position_field_instruction(&instruction) {
        return None;
    }
    let position = note_refs.and_then(|note_refs| note_refs.ref_field_position(*ref_field_index));
    *ref_field_index += 1;
    position
}

fn ref_target_direct_note_ref_field_position(
    instruction: &str,
    note_refs: Option<&NoteRefContext>,
    note_ref_field_index: &mut usize,
) -> Option<NoteRefFieldPosition> {
    note_refs.and_then(|note_refs| {
        note_ref_source_field_position(instruction, note_refs, note_ref_field_index)
    })
}

fn is_note_ref_source_position_field_instruction(instruction: Option<&str>) -> bool {
    instruction
        .map(normalize_instruction)
        .as_deref()
        .is_some_and(|instruction| field_kind(instruction) == FieldKind::NoteRef)
}

fn is_ref_target_source_order_field_instruction(instruction: Option<&str>) -> bool {
    instruction
        .map(normalize_instruction)
        .as_deref()
        .is_some_and(
            |instruction| match FieldKind::from_instruction(instruction) {
                FieldKind::Dynamic(kind) => matches!(kind.as_str(), "ASK" | "SET"),
                FieldKind::Numbering(kind) => matches!(
                    kind.as_str(),
                    "AUTONUM" | "AUTONUMLGL" | "AUTONUMOUT" | "BIDIOUTLINE" | "LISTNUM"
                ),
                FieldKind::Sequence => true,
                _ => false,
            },
        )
}

fn is_legacy_form_field_instruction(instruction: Option<&str>) -> bool {
    instruction
        .map(normalize_instruction)
        .as_deref()
        .is_some_and(|instruction| {
            matches!(
                FieldKind::from_instruction(instruction),
                FieldKind::FormField(_)
            )
        })
}

fn ref_target_merged_bookmarks(
    document_bookmarks: &HashMap<String, String>,
    field_bookmarks: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut bookmarks = document_bookmarks.clone();
    bookmarks.extend(
        field_bookmarks
            .iter()
            .map(|(name, value)| (name.clone(), value.clone())),
    );
    bookmarks
}

fn ref_target_excluded_bookmarks<'a>(
    active: &'a [(String, String)],
    current: &'a [RefTargetComplexField],
) -> &'a [(String, String)] {
    current
        .last()
        .map(|field| field.active.as_slice())
        .unwrap_or(active)
}

fn ref_target_available_bookmarks(
    bookmarks: &HashMap<String, String>,
    excluded: &[(String, String)],
) -> HashMap<String, String> {
    if excluded.is_empty() {
        return bookmarks.clone();
    }
    bookmarks
        .iter()
        .filter(|(name, _)| !excluded.iter().any(|(_, active_name)| active_name == *name))
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

fn ref_target_complex_in_result(current: &[RefTargetComplexField]) -> bool {
    current
        .last()
        .is_some_and(|field| field.phase == FieldPhase::Result)
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RefPositionContext {
    target_positions: HashMap<String, RefTargetPosition>,
    field_positions: Vec<RefFieldPosition>,
}

impl RefPositionContext {
    pub(crate) fn target_position(&self, name: &str) -> Option<RefTargetPosition> {
        self.target_positions.get(name).copied()
    }

    pub(crate) fn field_position(&self, index: usize) -> Option<RefFieldPosition> {
        self.field_positions.get(index).cloned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RefTargetPosition {
    start: usize,
    end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RefFieldPosition {
    order: usize,
    number_context: Option<String>,
}

#[derive(Debug, Clone)]
struct RefScanField {
    instruction: String,
    phase: FieldPhase,
}

#[derive(Debug, Clone, Default)]
struct RefPositionParagraph {
    depth: usize,
    properties_depth: usize,
    num_id: Option<String>,
    ilvl: u8,
    field_position_indices: Vec<usize>,
}

impl RefPositionParagraph {
    fn active(&self) -> bool {
        self.depth > 0
    }

    fn reset(&mut self) {
        *self = Self {
            depth: 1,
            ..Self::default()
        };
    }
}

pub(crate) fn ref_position_context(xml: &str, numbering: &Numbering) -> RefPositionContext {
    let mut r = Reader::from_str(xml);
    let mut target_positions = HashMap::new();
    let mut field_positions = Vec::new();
    let mut active_bookmarks: Vec<(String, String, usize)> = Vec::new();
    let mut paragraph = RefPositionParagraph::default();
    let mut counters: HashMap<String, [u32; 9]> = HashMap::new();
    let mut source_order = 0usize;
    let mut current: Option<RefScanField> = None;
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
                        }
                    }
                    b"pPr" if paragraph.active() => paragraph.properties_depth += 1,
                    b"ilvl" if paragraph.properties_depth > 0 => {
                        if let Some(value) = attr_u8(&e, b"val") {
                            paragraph.ilvl = value;
                        }
                    }
                    b"numId" if paragraph.properties_depth > 0 => {
                        paragraph.num_id = attr_local_trimmed(&e, b"val");
                    }
                    b"fldSimple" => record_ref_field_position(
                        attr_local(&e, b"instr").as_deref(),
                        &mut source_order,
                        &mut field_positions,
                        &mut paragraph,
                    ),
                    b"fldChar" => apply_ref_scan_fld_char(
                        &e,
                        &mut source_order,
                        &mut current,
                        &mut field_positions,
                        &mut paragraph,
                    ),
                    b"instrText" => {
                        let text = read_text(&mut r);
                        consumed_element = true;
                        if let Some(field) = current.as_mut() {
                            if field.phase == FieldPhase::Instruction {
                                field.instruction.push_str(&text);
                            }
                        }
                    }
                    b"bookmarkStart" => {
                        if let Some((id, name)) = bookmark_start(&e) {
                            active_bookmarks.push((id, name, source_order));
                            source_order += 1;
                        }
                    }
                    b"bookmarkEnd" => {
                        close_ref_position_bookmark(
                            bookmark_end_id(&e).as_deref(),
                            source_order,
                            &mut active_bookmarks,
                            &mut target_positions,
                        );
                        source_order += 1;
                    }
                    b"t" => {
                        if !read_text(&mut r).is_empty() {
                            source_order += 1;
                        }
                        consumed_element = true;
                    }
                    b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" | b"drawing"
                    | b"pict" | b"object" => {
                        source_order += 1;
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
                        finish_ref_position_paragraph(
                            &mut paragraph,
                            numbering,
                            &mut counters,
                            &mut field_positions,
                        );
                    }
                    b"pPr" if paragraph.active() => {}
                    b"ilvl" if paragraph.properties_depth > 0 => {
                        if let Some(value) = attr_u8(&e, b"val") {
                            paragraph.ilvl = value;
                        }
                    }
                    b"numId" if paragraph.properties_depth > 0 => {
                        paragraph.num_id = attr_local_trimmed(&e, b"val");
                    }
                    b"fldSimple" => record_ref_field_position(
                        attr_local(&e, b"instr").as_deref(),
                        &mut source_order,
                        &mut field_positions,
                        &mut paragraph,
                    ),
                    b"fldChar" => apply_ref_scan_fld_char(
                        &e,
                        &mut source_order,
                        &mut current,
                        &mut field_positions,
                        &mut paragraph,
                    ),
                    b"bookmarkStart" => {
                        if let Some((id, name)) = bookmark_start(&e) {
                            active_bookmarks.push((id, name, source_order));
                            source_order += 1;
                        }
                    }
                    b"bookmarkEnd" => {
                        close_ref_position_bookmark(
                            bookmark_end_id(&e).as_deref(),
                            source_order,
                            &mut active_bookmarks,
                            &mut target_positions,
                        );
                        source_order += 1;
                    }
                    b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" | b"drawing"
                    | b"pict" | b"object" => {
                        source_order += 1;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if name == b"AlternateContent" {
                    alternate_content_stack.pop();
                } else {
                    match name {
                        b"p" if paragraph.active() => {
                            if paragraph.depth == 1 {
                                finish_ref_position_paragraph(
                                    &mut paragraph,
                                    numbering,
                                    &mut counters,
                                    &mut field_positions,
                                );
                            } else {
                                paragraph.depth -= 1;
                            }
                        }
                        b"pPr" if paragraph.properties_depth > 0 => {
                            paragraph.properties_depth -= 1;
                        }
                        _ => {}
                    }
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    RefPositionContext {
        target_positions,
        field_positions,
    }
}

fn close_ref_position_bookmark(
    id: Option<&str>,
    end: usize,
    active_bookmarks: &mut Vec<(String, String, usize)>,
    target_positions: &mut HashMap<String, RefTargetPosition>,
) {
    let Some(id) = id else {
        return;
    };
    if let Some(index) = active_bookmarks
        .iter()
        .position(|(active_id, _, _)| active_id == id)
    {
        let (_, name, start) = active_bookmarks.remove(index);
        target_positions
            .entry(name)
            .or_insert(RefTargetPosition { start, end });
    }
}

fn record_ref_field_position(
    instruction: Option<&str>,
    source_order: &mut usize,
    field_positions: &mut Vec<RefFieldPosition>,
    paragraph: &mut RefPositionParagraph,
) {
    if instruction
        .map(normalize_instruction)
        .as_deref()
        .is_some_and(is_ref_position_field_instruction)
    {
        let index = field_positions.len();
        field_positions.push(RefFieldPosition {
            order: *source_order,
            number_context: None,
        });
        if paragraph.active() {
            paragraph.field_position_indices.push(index);
        }
        *source_order += 1;
    }
}

fn apply_ref_scan_fld_char(
    e: &BytesStart<'_>,
    source_order: &mut usize,
    current: &mut Option<RefScanField>,
    field_positions: &mut Vec<RefFieldPosition>,
    paragraph: &mut RefPositionParagraph,
) {
    match field_char_type(e).as_deref() {
        Some("begin") => {
            *current = Some(RefScanField {
                instruction: String::new(),
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
                record_ref_field_position(
                    Some(&field.instruction),
                    source_order,
                    field_positions,
                    paragraph,
                );
            }
        }
        _ => {}
    }
}

fn finish_ref_position_paragraph(
    paragraph: &mut RefPositionParagraph,
    numbering: &Numbering,
    counters: &mut HashMap<String, [u32; 9]>,
    field_positions: &mut [RefFieldPosition],
) {
    if let Some(num_id) = paragraph.num_id.as_deref().filter(|num_id| *num_id != "0") {
        let counter = counters.entry(num_id.to_string()).or_insert([0; 9]);
        let number_context = numbering
            .label(num_id, paragraph.ilvl, counter)
            .and_then(|_| numbering.full_context_label(num_id, paragraph.ilvl, counter))
            .and_then(|label| ref_paragraph_number(&label));
        if let Some(number_context) = number_context {
            for index in &paragraph.field_position_indices {
                if let Some(field) = field_positions.get_mut(*index) {
                    field.number_context = Some(number_context.clone());
                }
            }
        }
    }
    *paragraph = RefPositionParagraph::default();
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RefNumberContext {
    target_numbers: HashMap<String, RefTargetNumber>,
}

impl RefNumberContext {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    fn target_number(&self, name: &str, suppress_non_numeric: bool) -> Option<&str> {
        let number = self.target_numbers.get(name)?;
        if suppress_non_numeric {
            number.numeric.as_deref()
        } else {
            Some(number.text.as_str())
        }
    }

    fn target_full_context_number(&self, name: &str) -> Option<&str> {
        self.target_numbers.get(name)?.full_context.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RefTargetNumber {
    text: String,
    numeric: Option<String>,
    full_context: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct RefNumberParagraph {
    depth: usize,
    properties_depth: usize,
    num_id: Option<String>,
    ilvl: u8,
    bookmarks: Vec<String>,
}

impl RefNumberParagraph {
    fn active(&self) -> bool {
        self.depth > 0
    }

    fn reset(&mut self) {
        *self = Self {
            depth: 1,
            ..Self::default()
        };
    }
}

pub(crate) fn ref_number_context(xml: &str, numbering: &Numbering) -> RefNumberContext {
    let mut r = Reader::from_str(xml);
    let mut paragraph = RefNumberParagraph::default();
    let mut counters: HashMap<String, [u32; 9]> = HashMap::new();
    let mut target_numbers = HashMap::new();
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
                        }
                    }
                    b"pPr" if paragraph.active() => paragraph.properties_depth += 1,
                    b"ilvl" if paragraph.properties_depth > 0 => {
                        if let Some(value) = attr_u8(&e, b"val") {
                            paragraph.ilvl = value;
                        }
                    }
                    b"numId" if paragraph.properties_depth > 0 => {
                        paragraph.num_id = attr_local_trimmed(&e, b"val");
                    }
                    b"bookmarkStart" if paragraph.active() => {
                        if let Some(name) = bookmark_name(&e) {
                            push_unique(&mut paragraph.bookmarks, name);
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
                match name {
                    b"p" => {
                        paragraph.reset();
                        finish_ref_number_paragraph(
                            &mut paragraph,
                            numbering,
                            &mut counters,
                            &mut target_numbers,
                        );
                    }
                    b"pPr" if paragraph.active() => {}
                    b"ilvl" if paragraph.properties_depth > 0 => {
                        if let Some(value) = attr_u8(&e, b"val") {
                            paragraph.ilvl = value;
                        }
                    }
                    b"numId" if paragraph.properties_depth > 0 => {
                        paragraph.num_id = attr_local_trimmed(&e, b"val");
                    }
                    b"bookmarkStart" if paragraph.active() => {
                        if let Some(name) = bookmark_name(&e) {
                            push_unique(&mut paragraph.bookmarks, name);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.pop();
                    }
                    b"p" if paragraph.active() => {
                        if paragraph.depth == 1 {
                            finish_ref_number_paragraph(
                                &mut paragraph,
                                numbering,
                                &mut counters,
                                &mut target_numbers,
                            );
                        } else {
                            paragraph.depth -= 1;
                        }
                    }
                    b"pPr" if paragraph.properties_depth > 0 => {
                        paragraph.properties_depth -= 1;
                    }
                    _ => {}
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    RefNumberContext { target_numbers }
}

fn finish_ref_number_paragraph(
    paragraph: &mut RefNumberParagraph,
    numbering: &Numbering,
    counters: &mut HashMap<String, [u32; 9]>,
    target_numbers: &mut HashMap<String, RefTargetNumber>,
) {
    if let Some(num_id) = paragraph.num_id.as_deref().filter(|num_id| *num_id != "0") {
        let counter = counters.entry(num_id.to_string()).or_insert([0; 9]);
        if let Some(label) = numbering.label(num_id, paragraph.ilvl, counter) {
            if let Some(number) = ref_paragraph_number(&label) {
                let full_context = numbering
                    .full_context_label(num_id, paragraph.ilvl, counter)
                    .and_then(|label| ref_paragraph_number(&label));
                let target_number = RefTargetNumber {
                    full_context,
                    numeric: ref_numeric_paragraph_number(&number),
                    text: number,
                };
                for bookmark in &paragraph.bookmarks {
                    target_numbers
                        .entry(bookmark.clone())
                        .or_insert_with(|| target_number.clone());
                }
            }
        }
    }
    *paragraph = RefNumberParagraph::default();
}

pub(super) fn ref_paragraph_number(label: &str) -> Option<String> {
    let without_periods = label.trim().trim_end_matches('.').trim_end();
    (!without_periods.is_empty()).then(|| without_periods.to_string())
}

pub(super) fn ref_numeric_paragraph_number(label: &str) -> Option<String> {
    let retained: String = label
        .chars()
        .filter(|ch| ch.is_ascii_digit() || matches!(ch, '.' | ',' | ':' | '-' | '/'))
        .collect();
    let numeric = retained.trim_matches(|ch: char| !ch.is_ascii_digit());
    (!numeric.is_empty()).then(|| numeric.to_string())
}

fn preserved_note_local_ref_instruction(instruction: &str) -> Option<RefInstruction> {
    let spec = ref_instruction(instruction)?;
    (!spec.note_reference
        && !spec.sequence_separator
        && !spec.relative
        && !spec.paragraph_number
        && !spec.full_context_number
        && !spec.relative_context_number
        && !spec.suppress_non_numeric)
        .then_some(spec)
}

pub(crate) fn preserved_note_local_ref_target(instruction: &str) -> Option<String> {
    Some(preserved_note_local_ref_instruction(instruction)?.target)
}

pub(crate) fn computed_preserved_note_local_ref_result(
    instruction: &str,
    bookmarks: &HashMap<String, String>,
) -> Option<String> {
    let spec = preserved_note_local_ref_instruction(instruction)?;
    let text = bookmarks.get(&spec.target)?;
    let result = computed_ref_bookmark_text_result(
        text,
        spec.number_format,
        spec.number_picture.as_deref(),
    )?;
    Some(apply_field_text_format(result, spec.text_format))
}

pub(crate) struct RefResultContext<'a> {
    pub(crate) bookmarks: &'a HashMap<String, String>,
    pub(crate) ref_positions: &'a RefPositionContext,
    pub(crate) ref_numbers: &'a RefNumberContext,
    pub(crate) note_refs: &'a NoteRefContext,
    pub(crate) field_bookmarks: &'a HashMap<String, String>,
}

pub(crate) fn computed_direct_bookmark_ref_result(
    instruction: &str,
    ctx: &RefResultContext<'_>,
    field_position: Option<RefFieldPosition>,
    note_ref_field_position: Option<NoteRefFieldPosition>,
) -> Option<String> {
    let spec = unknown_direct_bookmark_ref_instruction(instruction)?;
    let text =
        computed_ref_instruction_result(&spec, ctx, field_position, note_ref_field_position)?;
    Some(apply_field_text_format(text, spec.text_format))
}

pub(crate) fn computed_ref_result(
    instruction: &str,
    ctx: &RefResultContext<'_>,
    field_position: Option<RefFieldPosition>,
    note_ref_field_position: Option<NoteRefFieldPosition>,
) -> Option<String> {
    let spec = ref_instruction(instruction)?;
    let text =
        computed_ref_instruction_result(&spec, ctx, field_position, note_ref_field_position)?;
    Some(apply_field_text_format(text, spec.text_format))
}

pub(super) fn computed_ref_instruction_result(
    spec: &RefInstruction,
    ctx: &RefResultContext<'_>,
    field_position: Option<RefFieldPosition>,
    note_ref_field_position: Option<NoteRefFieldPosition>,
) -> Option<String> {
    if spec.sequence_separator {
        let _separator = spec.sequence_separator_value.as_deref()?;
        if spec.note_reference
            || spec.relative
            || spec.paragraph_number
            || spec.full_context_number
            || spec.relative_context_number
        {
            return None;
        }
    }
    if spec.note_reference {
        let number = ctx
            .note_refs
            .ref_note_number(&spec.target, note_ref_field_position)?;
        return format_page_number(number, spec.number_format);
    }
    if spec.relative_context_number {
        let number =
            computed_relative_context_ref_number(spec, ctx.ref_numbers, field_position.clone())?;
        return if spec.relative {
            let relative = computed_relative_ref_result(spec, ctx.ref_positions, field_position)?;
            Some(format!("{number} {relative}"))
        } else {
            Some(number)
        };
    }
    if spec.full_context_number {
        let number = ctx.ref_numbers.target_full_context_number(&spec.target)?;
        return if spec.relative {
            let relative = computed_relative_ref_result(spec, ctx.ref_positions, field_position)?;
            Some(format!("{number} {relative}"))
        } else {
            Some(number.to_string())
        };
    }
    if spec.paragraph_number {
        let number = ctx
            .ref_numbers
            .target_number(&spec.target, spec.suppress_non_numeric)?;
        return if spec.relative {
            let relative = computed_relative_ref_result(spec, ctx.ref_positions, field_position)?;
            Some(format!("{number} {relative}"))
        } else {
            Some(number.to_string())
        };
    }
    if spec.relative {
        computed_relative_ref_result(spec, ctx.ref_positions, field_position)
    } else if let Some(text) = ctx.field_bookmarks.get(&spec.target) {
        computed_ref_bookmark_text_result(text, spec.number_format, spec.number_picture.as_deref())
    } else {
        ctx.bookmarks.get(&spec.target).and_then(|text| {
            computed_ref_bookmark_text_result(
                text,
                spec.number_format,
                spec.number_picture.as_deref(),
            )
        })
    }
}

pub(super) fn computed_ref_bookmark_text_result(
    text: &str,
    number_format: Option<PageNumberFormat>,
    number_picture: Option<&str>,
) -> Option<String> {
    if let Some(picture) = number_picture {
        let value = text.trim().parse::<f64>().ok()?;
        return super::formula::format_formula_number(value, picture);
    }
    if let Some(format) = number_format {
        let number = text.trim().parse::<usize>().ok()?;
        return format_page_number(number, Some(format));
    }
    Some(text.to_string())
}

pub(super) fn ref_instruction_target_known(
    spec: &RefInstruction,
    ctx: &RefResultContext<'_>,
) -> bool {
    ctx.bookmarks.contains_key(&spec.target)
        || ctx.field_bookmarks.contains_key(&spec.target)
        || ctx.ref_positions.target_position(&spec.target).is_some()
        || ctx.ref_numbers.target_numbers.contains_key(&spec.target)
        || ctx.note_refs.target(&spec.target).is_some()
}

fn computed_relative_context_ref_number(
    spec: &RefInstruction,
    ref_numbers: &RefNumberContext,
    field_position: Option<RefFieldPosition>,
) -> Option<String> {
    let target = ref_numbers.target_full_context_number(&spec.target)?;
    let field = field_position?.number_context?;
    Some(relative_context_ref_number(target, &field))
}

pub(super) fn relative_context_ref_number(target: &str, field: &str) -> String {
    let target_parts = target.split('.').collect::<Vec<_>>();
    let field_parts = field.split('.').collect::<Vec<_>>();
    let common = target_parts
        .iter()
        .zip(field_parts.iter())
        .take_while(|(target, field)| target == field)
        .count();
    let relative = target_parts
        .get(common..)
        .filter(|parts| !parts.is_empty())
        .unwrap_or(&target_parts);
    relative.join(".")
}

pub(crate) fn is_ref_position_field_instruction(instruction: &str) -> bool {
    ref_or_unknown_direct_bookmark_instruction(instruction).is_some()
}

pub(crate) fn is_direct_bookmark_ref_field_instruction(instruction: &str) -> bool {
    unknown_direct_bookmark_ref_instruction(instruction).is_some()
}

fn computed_relative_ref_result(
    spec: &RefInstruction,
    ref_positions: &RefPositionContext,
    field_position: Option<RefFieldPosition>,
) -> Option<String> {
    let target = ref_positions.target_position(&spec.target)?;
    let field = field_position?;
    if field.order < target.start {
        return Some("below".to_string());
    }
    (field.order > target.end).then(|| "above".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RefInstruction {
    pub(super) target: String,
    pub(super) number_format: Option<PageNumberFormat>,
    pub(super) number_picture: Option<String>,
    pub(super) text_format: Option<FieldTextFormat>,
    pub(super) note_reference: bool,
    pub(super) sequence_separator: bool,
    pub(super) sequence_separator_value: Option<String>,
    pub(super) relative: bool,
    pub(super) paragraph_number: bool,
    pub(super) full_context_number: bool,
    pub(super) relative_context_number: bool,
    pub(super) suppress_non_numeric: bool,
}

pub(super) fn ref_instruction(instruction: &str) -> Option<RefInstruction> {
    ref_instruction_from_syntax(ref_field_syntax(instruction)?)
}

pub(super) fn direct_bookmark_ref_instruction(instruction: &str) -> Option<RefInstruction> {
    ref_instruction_from_syntax(direct_ref_field_syntax(instruction)?)
}

fn unknown_direct_bookmark_ref_instruction(instruction: &str) -> Option<RefInstruction> {
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Unknown(_)
    )
    .then(|| direct_bookmark_ref_instruction(instruction))
    .flatten()
}

pub(super) fn ref_or_unknown_direct_bookmark_instruction(
    instruction: &str,
) -> Option<RefInstruction> {
    ref_instruction(instruction).or_else(|| unknown_direct_bookmark_ref_instruction(instruction))
}

fn ref_instruction_from_syntax(
    syntax: crate::annotation::RefFieldSyntax,
) -> Option<RefInstruction> {
    Some(RefInstruction {
        target: syntax.target,
        number_format: syntax
            .number_format
            .map(page_number_format_from_field_format),
        number_picture: syntax.number_picture,
        text_format: syntax.text_format,
        note_reference: syntax.note_reference,
        sequence_separator: syntax.sequence_separator,
        sequence_separator_value: syntax.sequence_separator_value,
        relative: syntax.relative,
        paragraph_number: syntax.paragraph_number,
        full_context_number: syntax.full_context_number,
        relative_context_number: syntax.relative_context_number,
        suppress_non_numeric: syntax.suppress_non_numeric,
    })
}

pub(super) fn ref_note_field_target(instruction: &str) -> Option<String> {
    let spec = ref_or_unknown_direct_bookmark_instruction(instruction)?;
    (spec.note_reference && !spec.sequence_separator).then_some(spec.target)
}
