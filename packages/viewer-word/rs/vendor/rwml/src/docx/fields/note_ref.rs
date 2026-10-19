use super::*;

#[derive(Debug, Clone, Default)]
pub(crate) struct NoteRefContext {
    pub(crate) targets: HashMap<String, NoteRefTarget>,
    custom_marks: HashMap<usize, String>,
    field_positions: Vec<NoteRefFieldPosition>,
    ref_field_positions: Vec<NoteRefFieldPosition>,
    markers: Vec<NoteRefMarker>,
    generated_ref_note_fields: Vec<NoteRefGeneratedField>,
}

impl NoteRefContext {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(super) fn target(&self, name: &str) -> Option<NoteRefTarget> {
        self.targets.get(name).copied()
    }

    pub(crate) fn field_position(&self, index: usize) -> Option<NoteRefFieldPosition> {
        self.field_positions.get(index).copied()
    }

    pub(crate) fn ref_field_position(&self, index: usize) -> Option<NoteRefFieldPosition> {
        self.ref_field_positions.get(index).copied()
    }

    pub(crate) fn target_is_note_marker(&self, name: &str) -> bool {
        self.target(name).is_some_and(NoteRefTarget::is_note_marker)
    }

    pub(super) fn target_reference_number(&self, name: &str) -> Option<usize> {
        let target = self.target(name)?;
        target.custom_mark_order.is_none().then_some(target.number)
    }

    pub(super) fn ref_note_number(
        &self,
        name: &str,
        field_position: Option<NoteRefFieldPosition>,
    ) -> Option<usize> {
        let target = self.target(name)?;
        if target.custom_mark_order.is_some() {
            return None;
        }
        let field = field_position?;
        let actual_before = self
            .markers
            .iter()
            .filter(|marker| marker.kind == target.kind && marker.order < field.order)
            .count();
        let generated_before = self
            .generated_ref_note_fields
            .iter()
            .filter(|generated| generated.order < field.order)
            .filter_map(|generated| self.target(&generated.target))
            .filter(|generated_target| generated_target.kind == target.kind)
            .count();
        Some(actual_before + generated_before + 1)
    }

    fn custom_mark(&self, target: NoteRefTarget) -> Option<&str> {
        self.custom_marks
            .get(&target.custom_mark_order?)
            .map(String::as_str)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NoteRefTarget {
    kind: NoteRefKind,
    number: usize,
    start: usize,
    end: usize,
    // A note reference carrying `w:customMarkFollows` uses the immediately
    // following literal mark instead of an auto-numbered ordinal.
    custom_mark_order: Option<usize>,
    // Document-level `numFmt` for this note kind, used when the NOTEREF field
    // itself carries no explicit `\*` number-format switch.
    format: Option<PageNumberFormat>,
}

impl NoteRefTarget {
    fn is_note_marker(self) -> bool {
        matches!(self.kind, NoteRefKind::Footnote | NoteRefKind::Endnote)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoteRefKind {
    Footnote,
    Endnote,
    Comment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NoteRefMarker {
    kind: NoteRefKind,
    order: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NoteRefGeneratedField {
    target: String,
    order: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NoteRefFieldPosition {
    order: usize,
}

/// Document-level footnote/endnote numbering read from `word/settings.xml`
/// (`<w:footnotePr>`/`<w:endnotePr>`): the `numStart` offset applied to the note
/// ordinal and the `numFmt` format applied to the note number.
///
/// Ceiling: `numRestart="eachPage"` is layout-dependent and is not modeled here;
/// per-part local numbering (headers/footers/notes restarting at 1) also stays
/// out of scope, so these settings apply only to the main document body.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct NoteNumbering {
    footnote_start: Option<usize>,
    footnote_format: Option<PageNumberFormat>,
    endnote_start: Option<usize>,
    endnote_format: Option<PageNumberFormat>,
}

impl NoteNumbering {
    fn footnote_seed(&self) -> usize {
        self.footnote_start
            .map_or(0, |start| start.saturating_sub(1))
    }

    fn endnote_seed(&self) -> usize {
        self.endnote_start
            .map_or(0, |start| start.saturating_sub(1))
    }

    fn format_for(&self, kind: NoteRefKind) -> Option<PageNumberFormat> {
        match kind {
            NoteRefKind::Footnote => self.footnote_format,
            NoteRefKind::Endnote => self.endnote_format,
            NoteRefKind::Comment => None,
        }
    }
}

/// Parse document-level footnote/endnote numbering from `word/settings.xml`.
pub(crate) fn note_numbering_from_settings(settings_xml: &str) -> NoteNumbering {
    let mut numbering = NoteNumbering::default();
    let mut r = Reader::from_str(settings_xml);
    // Which `*Pr` block we are inside; note settings only appear directly under
    // `<w:footnotePr>`/`<w:endnotePr>` at the settings root.
    let mut current_kind: Option<NoteRefKind> = None;
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => match local(e.name().as_ref()) {
                b"footnotePr" => current_kind = Some(NoteRefKind::Footnote),
                b"endnotePr" => current_kind = Some(NoteRefKind::Endnote),
                b"numStart" => apply_note_num_start(&e, current_kind, &mut numbering),
                b"numFmt" => apply_note_num_fmt(&e, current_kind, &mut numbering),
                _ => {}
            },
            Ok(Event::Empty(e)) => match local(e.name().as_ref()) {
                b"numStart" => apply_note_num_start(&e, current_kind, &mut numbering),
                b"numFmt" => apply_note_num_fmt(&e, current_kind, &mut numbering),
                _ => {}
            },
            Ok(Event::End(e)) => {
                if matches!(local(e.name().as_ref()), b"footnotePr" | b"endnotePr") {
                    current_kind = None;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    numbering
}

fn apply_note_num_start(
    e: &BytesStart<'_>,
    kind: Option<NoteRefKind>,
    numbering: &mut NoteNumbering,
) {
    let Some(start) = attr_usize(e, b"val").filter(|start| *start > 0) else {
        return;
    };
    match kind {
        Some(NoteRefKind::Footnote) => numbering.footnote_start = Some(start),
        Some(NoteRefKind::Endnote) => numbering.endnote_start = Some(start),
        _ => {}
    }
}

fn apply_note_num_fmt(
    e: &BytesStart<'_>,
    kind: Option<NoteRefKind>,
    numbering: &mut NoteNumbering,
) {
    let Some(value) = attr_local_trimmed(e, b"val") else {
        return;
    };
    let Some(format) =
        crate::model::PageNumberFormat::from_wml_value(&value).map(PageNumberFormat::from)
    else {
        return;
    };
    match kind {
        Some(NoteRefKind::Footnote) => numbering.footnote_format = Some(format),
        Some(NoteRefKind::Endnote) => numbering.endnote_format = Some(format),
        _ => {}
    }
}

#[derive(Debug, Clone)]
struct NoteRefScanField {
    instruction: String,
    phase: FieldPhase,
    suppress_result: bool,
    position_recorded: bool,
    nested_suppressed_fields: usize,
}

#[derive(Debug, Clone)]
struct NoteRefActiveBookmark {
    id: String,
    name: String,
    start: usize,
}

#[derive(Debug, Clone)]
struct NoteRefActiveCommentRange {
    id: String,
    bookmark_names: Vec<String>,
    start: usize,
}

#[derive(Debug, Clone)]
struct NoteRefCommentRangeTarget {
    name: String,
    start: usize,
    end: usize,
}

pub(crate) fn note_ref_context(
    xml: &str,
    document_bookmarks: &HashMap<String, String>,
) -> NoteRefContext {
    let core_properties = CoreProperties::default();
    let empty_properties = HashMap::new();
    note_ref_context_with_properties(
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

pub(crate) fn note_ref_context_with_properties(
    xml: &str,
    document_bookmarks: &HashMap<String, String>,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
) -> NoteRefContext {
    note_ref_context_with_numbering(
        xml,
        document_bookmarks,
        properties,
        preserve_legacy_form_cache,
        NoteNumbering::default(),
    )
}

pub(crate) fn note_ref_context_with_numbering(
    xml: &str,
    document_bookmarks: &HashMap<String, String>,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
    numbering: NoteNumbering,
) -> NoteRefContext {
    let mut r = Reader::from_str(xml);
    let mut targets = HashMap::new();
    let mut custom_marks = HashMap::new();
    let mut field_positions = Vec::new();
    let mut ref_field_positions = Vec::new();
    let mut markers = Vec::new();
    let mut generated_ref_note_fields = Vec::new();
    let mut active_bookmarks: Vec<NoteRefActiveBookmark> = Vec::new();
    let mut active_comment_ranges = Vec::new();
    let mut comment_range_targets: HashMap<String, Vec<NoteRefCommentRangeTarget>> = HashMap::new();
    let mut source_order = 0usize;
    let mut pending_custom_mark = None;
    // Seed the auto-counters so the first note lands on the document-level
    // `numStart` (default 1). numRestart="eachPage" is layout-dependent and not
    // modeled here.
    let mut footnote_number = numbering.footnote_seed();
    let mut endnote_number = numbering.endnote_seed();
    let mut comment_number = 0usize;
    let mut current: Option<NoteRefScanField> = None;
    let mut computed_fields = NoteRefComputedFieldState::new(
        document_bookmarks,
        properties,
        legacy_form_context(xml, preserve_legacy_form_cache),
    );
    let mut xml_depth = 0usize;
    let mut alternate_content_stack = Vec::new();
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    pending_custom_mark = None;
                    skip_subtree(&mut r);
                    continue;
                }
                if suppresses_note_ref_complex_result_scan(&current) {
                    match name {
                        b"fldChar" => apply_note_ref_scan_fld_char(
                            &e,
                            &mut source_order,
                            &mut current,
                            &mut field_positions,
                            &mut ref_field_positions,
                            &mut generated_ref_note_fields,
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
                match name {
                    b"del" | b"moveFrom" => {
                        pending_custom_mark = None;
                        skip_subtree(&mut r);
                        continue;
                    }
                    b"AlternateContent" => {
                        alternate_content_stack.push(AlternateContentBranchState {
                            branch_depth: xml_depth + 1,
                            took_branch: false,
                        });
                    }
                    b"fldSimple" => {
                        pending_custom_mark = None;
                        if let Some(text) = computed_note_ref_scan_field_result(
                            attr_local(&e, b"instr").as_deref(),
                            &mut computed_fields,
                        ) {
                            let recorded = record_note_ref_scan_field_position(
                                attr_local(&e, b"instr").as_deref(),
                                &mut source_order,
                                &mut field_positions,
                                &mut ref_field_positions,
                                &mut generated_ref_note_fields,
                            );
                            if !recorded && !text.is_empty() {
                                source_order += 1;
                            }
                            skip_subtree(&mut r);
                            continue;
                        }
                        record_note_ref_scan_field_position(
                            attr_local(&e, b"instr").as_deref(),
                            &mut source_order,
                            &mut field_positions,
                            &mut ref_field_positions,
                            &mut generated_ref_note_fields,
                        );
                    }
                    b"fldChar" => {
                        pending_custom_mark = None;
                        apply_note_ref_scan_fld_char(
                            &e,
                            &mut source_order,
                            &mut current,
                            &mut field_positions,
                            &mut ref_field_positions,
                            &mut generated_ref_note_fields,
                            &mut computed_fields,
                        );
                    }
                    b"instrText" => {
                        let text = read_text(&mut r);
                        if let Some(field) = current.as_mut() {
                            if field.phase == FieldPhase::Instruction {
                                field.instruction.push_str(&text);
                            }
                        }
                        continue;
                    }
                    b"bookmarkStart" => {
                        if let Some((id, name)) = bookmark_start(&e) {
                            active_bookmarks.push(NoteRefActiveBookmark {
                                id,
                                name,
                                start: source_order,
                            });
                            source_order += 1;
                        }
                    }
                    b"bookmarkEnd" => {
                        let id = bookmark_end_id(&e);
                        record_note_ref_contained_comment_range_target(
                            id.as_deref(),
                            source_order,
                            &active_bookmarks,
                            &active_comment_ranges,
                            &mut comment_range_targets,
                        );
                        close_note_ref_bookmark(
                            id.as_deref(),
                            source_order,
                            &mut active_bookmarks,
                            &mut targets,
                        );
                        source_order += 1;
                    }
                    b"commentRangeStart" => {
                        record_note_ref_comment_range_start(
                            &e,
                            &active_bookmarks,
                            source_order,
                            &mut active_comment_ranges,
                        );
                    }
                    b"commentRangeEnd" => {
                        close_note_ref_comment_range(
                            &e,
                            source_order,
                            &active_bookmarks,
                            &mut active_comment_ranges,
                            &mut comment_range_targets,
                        );
                    }
                    b"footnoteReference" => {
                        let custom_mark = note_reference_uses_custom_mark(&e);
                        pending_custom_mark = custom_mark.then_some(source_order);
                        if !custom_mark {
                            footnote_number += 1;
                            markers.push(NoteRefMarker {
                                kind: NoteRefKind::Footnote,
                                order: source_order,
                            });
                        }
                        record_note_ref_target(
                            &active_bookmarks,
                            NoteRefKind::Footnote,
                            footnote_number,
                            source_order,
                            custom_mark,
                            numbering.format_for(NoteRefKind::Footnote),
                            &mut targets,
                        );
                        source_order += 1;
                        skip_subtree(&mut r);
                        continue;
                    }
                    b"endnoteReference" => {
                        let custom_mark = note_reference_uses_custom_mark(&e);
                        pending_custom_mark = custom_mark.then_some(source_order);
                        if !custom_mark {
                            endnote_number += 1;
                            markers.push(NoteRefMarker {
                                kind: NoteRefKind::Endnote,
                                order: source_order,
                            });
                        }
                        record_note_ref_target(
                            &active_bookmarks,
                            NoteRefKind::Endnote,
                            endnote_number,
                            source_order,
                            custom_mark,
                            numbering.format_for(NoteRefKind::Endnote),
                            &mut targets,
                        );
                        source_order += 1;
                        skip_subtree(&mut r);
                        continue;
                    }
                    b"commentReference" => {
                        comment_number += 1;
                        markers.push(NoteRefMarker {
                            kind: NoteRefKind::Comment,
                            order: source_order,
                        });
                        record_note_ref_target(
                            &active_bookmarks,
                            NoteRefKind::Comment,
                            comment_number,
                            source_order,
                            false,
                            None,
                            &mut targets,
                        );
                        record_note_ref_comment_range_targets(
                            &e,
                            comment_number,
                            &comment_range_targets,
                            &mut targets,
                        );
                        source_order += 1;
                        skip_subtree(&mut r);
                        continue;
                    }
                    b"t" => {
                        let text = read_text(&mut r);
                        if !text.is_empty() {
                            capture_following_custom_mark(
                                &mut pending_custom_mark,
                                source_order,
                                &text,
                                &mut custom_marks,
                            );
                            source_order += 1;
                        }
                        continue;
                    }
                    b"p" => pending_custom_mark = None,
                    b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" | b"drawing"
                    | b"pict" | b"object" => {
                        source_order += 1;
                    }
                    _ => {}
                }
                xml_depth = xml_depth.saturating_add(1);
            }
            Ok(Event::Empty(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if should_skip_alternate_branch(&mut alternate_content_stack, xml_depth, name) {
                    pending_custom_mark = None;
                    continue;
                }
                if suppresses_note_ref_complex_result_scan(&current) {
                    if name == b"fldChar" {
                        apply_note_ref_scan_fld_char(
                            &e,
                            &mut source_order,
                            &mut current,
                            &mut field_positions,
                            &mut ref_field_positions,
                            &mut generated_ref_note_fields,
                            &mut computed_fields,
                        );
                    }
                    continue;
                }
                match name {
                    b"fldSimple" => {
                        pending_custom_mark = None;
                        if let Some(text) = computed_note_ref_scan_field_result(
                            attr_local(&e, b"instr").as_deref(),
                            &mut computed_fields,
                        ) {
                            let recorded = record_note_ref_scan_field_position(
                                attr_local(&e, b"instr").as_deref(),
                                &mut source_order,
                                &mut field_positions,
                                &mut ref_field_positions,
                                &mut generated_ref_note_fields,
                            );
                            if !recorded && !text.is_empty() {
                                source_order += 1;
                            }
                        } else {
                            record_note_ref_scan_field_position(
                                attr_local(&e, b"instr").as_deref(),
                                &mut source_order,
                                &mut field_positions,
                                &mut ref_field_positions,
                                &mut generated_ref_note_fields,
                            );
                        }
                    }
                    b"fldChar" => {
                        pending_custom_mark = None;
                        apply_note_ref_scan_fld_char(
                            &e,
                            &mut source_order,
                            &mut current,
                            &mut field_positions,
                            &mut ref_field_positions,
                            &mut generated_ref_note_fields,
                            &mut computed_fields,
                        );
                    }
                    b"bookmarkStart" => {
                        if let Some((id, name)) = bookmark_start(&e) {
                            active_bookmarks.push(NoteRefActiveBookmark {
                                id,
                                name,
                                start: source_order,
                            });
                            source_order += 1;
                        }
                    }
                    b"bookmarkEnd" => {
                        let id = bookmark_end_id(&e);
                        record_note_ref_contained_comment_range_target(
                            id.as_deref(),
                            source_order,
                            &active_bookmarks,
                            &active_comment_ranges,
                            &mut comment_range_targets,
                        );
                        close_note_ref_bookmark(
                            id.as_deref(),
                            source_order,
                            &mut active_bookmarks,
                            &mut targets,
                        );
                        source_order += 1;
                    }
                    b"commentRangeStart" => {
                        record_note_ref_comment_range_start(
                            &e,
                            &active_bookmarks,
                            source_order,
                            &mut active_comment_ranges,
                        );
                    }
                    b"commentRangeEnd" => {
                        close_note_ref_comment_range(
                            &e,
                            source_order,
                            &active_bookmarks,
                            &mut active_comment_ranges,
                            &mut comment_range_targets,
                        );
                    }
                    b"footnoteReference" => {
                        let custom_mark = note_reference_uses_custom_mark(&e);
                        pending_custom_mark = custom_mark.then_some(source_order);
                        if !custom_mark {
                            footnote_number += 1;
                            markers.push(NoteRefMarker {
                                kind: NoteRefKind::Footnote,
                                order: source_order,
                            });
                        }
                        record_note_ref_target(
                            &active_bookmarks,
                            NoteRefKind::Footnote,
                            footnote_number,
                            source_order,
                            custom_mark,
                            numbering.format_for(NoteRefKind::Footnote),
                            &mut targets,
                        );
                        source_order += 1;
                    }
                    b"endnoteReference" => {
                        let custom_mark = note_reference_uses_custom_mark(&e);
                        pending_custom_mark = custom_mark.then_some(source_order);
                        if !custom_mark {
                            endnote_number += 1;
                            markers.push(NoteRefMarker {
                                kind: NoteRefKind::Endnote,
                                order: source_order,
                            });
                        }
                        record_note_ref_target(
                            &active_bookmarks,
                            NoteRefKind::Endnote,
                            endnote_number,
                            source_order,
                            custom_mark,
                            numbering.format_for(NoteRefKind::Endnote),
                            &mut targets,
                        );
                        source_order += 1;
                    }
                    b"commentReference" => {
                        comment_number += 1;
                        markers.push(NoteRefMarker {
                            kind: NoteRefKind::Comment,
                            order: source_order,
                        });
                        record_note_ref_target(
                            &active_bookmarks,
                            NoteRefKind::Comment,
                            comment_number,
                            source_order,
                            false,
                            None,
                            &mut targets,
                        );
                        record_note_ref_comment_range_targets(
                            &e,
                            comment_number,
                            &comment_range_targets,
                            &mut targets,
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
                if suppresses_note_ref_complex_result_scan(&current) {
                    xml_depth = xml_depth.saturating_sub(1);
                    continue;
                }
                if local(e.name().as_ref()) == b"p" {
                    pending_custom_mark = None;
                }
                if local(e.name().as_ref()) == b"AlternateContent" {
                    alternate_content_stack.pop();
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    NoteRefContext {
        targets,
        custom_marks,
        field_positions,
        ref_field_positions,
        markers,
        generated_ref_note_fields,
    }
}

#[derive(Debug)]
struct NoteRefComputedFieldState<'a> {
    document_bookmarks: &'a HashMap<String, String>,
    properties: FieldDocumentProperties<'a>,
    legacy_forms: LegacyFormContext,
    field_bookmarks: HashMap<String, String>,
    sequence_counters: HashMap<String, i64>,
    autonum_counter: i64,
    listnum_counter: i64,
    form_field_index: usize,
}

impl<'a> NoteRefComputedFieldState<'a> {
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

fn computed_note_ref_scan_field_result(
    instruction: Option<&str>,
    state: &mut NoteRefComputedFieldState<'_>,
) -> Option<String> {
    let instruction = normalize_instruction(instruction?);
    let kind = FieldKind::from_instruction(&instruction);
    match &kind {
        FieldKind::Ref => {
            return computed_note_ref_scan_ref_result(&instruction, state);
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
        if let Some(text) = computed_note_ref_scan_direct_bookmark_ref_result(&instruction, state) {
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

fn computed_note_ref_scan_ref_result(
    instruction: &str,
    state: &NoteRefComputedFieldState<'_>,
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

fn computed_note_ref_scan_direct_bookmark_ref_result(
    instruction: &str,
    state: &NoteRefComputedFieldState<'_>,
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

fn suppresses_note_ref_complex_result_scan(current: &Option<NoteRefScanField>) -> bool {
    current
        .as_ref()
        .is_some_and(|field| field.phase == FieldPhase::Result && field.suppress_result)
}

pub(crate) fn note_ref_target_names(xml: &str) -> HashSet<String> {
    let document_bookmarks = ref_targets(xml);
    note_ref_context(xml, &document_bookmarks)
        .targets
        .into_iter()
        .filter_map(|(name, target)| target.is_note_marker().then_some(name))
        .collect()
}

/// A `w:footnoteReference`/`w:endnoteReference` with `w:customMarkFollows` set
/// uses a custom mark glyph and is not part of the auto-numbered sequence.
fn note_reference_uses_custom_mark(e: &BytesStart<'_>) -> bool {
    attr_local(e, b"customMarkFollows").is_some_and(|value| toggle_on(Some(value)))
}

fn capture_following_custom_mark(
    pending_order: &mut Option<usize>,
    source_order: usize,
    text: &str,
    custom_marks: &mut HashMap<usize, String>,
) {
    let Some(marker_order) = pending_order.take() else {
        return;
    };
    if marker_order.checked_add(1) == Some(source_order) {
        custom_marks
            .entry(marker_order)
            .or_insert_with(|| text.to_string());
    }
}

fn record_note_ref_target(
    active_bookmarks: &[NoteRefActiveBookmark],
    kind: NoteRefKind,
    number: usize,
    order: usize,
    custom_mark: bool,
    format: Option<PageNumberFormat>,
    targets: &mut HashMap<String, NoteRefTarget>,
) {
    for bookmark in active_bookmarks {
        targets
            .entry(bookmark.name.clone())
            .or_insert(NoteRefTarget {
                kind,
                number,
                start: bookmark.start,
                end: order,
                custom_mark_order: custom_mark.then_some(order),
                format,
            });
    }
}

fn record_note_ref_comment_range_start(
    e: &BytesStart<'_>,
    active_bookmarks: &[NoteRefActiveBookmark],
    source_order: usize,
    active_comment_ranges: &mut Vec<NoteRefActiveCommentRange>,
) {
    let Some(id) = attr_local_trimmed(e, b"id") else {
        return;
    };
    let bookmark_names = active_bookmarks
        .iter()
        .map(|bookmark| bookmark.name.clone())
        .collect::<Vec<_>>();
    active_comment_ranges.push(NoteRefActiveCommentRange {
        id,
        bookmark_names,
        start: source_order,
    });
}

fn close_note_ref_comment_range(
    e: &BytesStart<'_>,
    end: usize,
    active_bookmarks: &[NoteRefActiveBookmark],
    active_comment_ranges: &mut Vec<NoteRefActiveCommentRange>,
    comment_range_targets: &mut HashMap<String, Vec<NoteRefCommentRangeTarget>>,
) {
    let Some(id) = attr_local_trimmed(e, b"id") else {
        return;
    };
    let Some(index) = active_comment_ranges
        .iter()
        .rposition(|range| range.id == id)
    else {
        return;
    };
    let range = active_comment_ranges.remove(index);
    let active_names = active_bookmarks
        .iter()
        .map(|bookmark| bookmark.name.as_str())
        .collect::<Vec<_>>();
    let range_targets = comment_range_targets.entry(id).or_default();
    for name in range.bookmark_names {
        if active_names.iter().any(|active| *active == name) {
            range_targets.push(NoteRefCommentRangeTarget {
                name,
                start: range.start,
                end,
            });
        }
    }
}

fn record_note_ref_comment_range_targets(
    e: &BytesStart<'_>,
    number: usize,
    comment_range_targets: &HashMap<String, Vec<NoteRefCommentRangeTarget>>,
    targets: &mut HashMap<String, NoteRefTarget>,
) {
    let Some(id) = attr_local_trimmed(e, b"id") else {
        return;
    };
    let Some(range_targets) = comment_range_targets.get(&id) else {
        return;
    };
    for range in range_targets {
        targets.entry(range.name.clone()).or_insert(NoteRefTarget {
            kind: NoteRefKind::Comment,
            number,
            start: range.start,
            end: range.end,
            custom_mark_order: None,
            format: None,
        });
    }
}

fn record_note_ref_contained_comment_range_target(
    id: Option<&str>,
    end: usize,
    active_bookmarks: &[NoteRefActiveBookmark],
    active_comment_ranges: &[NoteRefActiveCommentRange],
    comment_range_targets: &mut HashMap<String, Vec<NoteRefCommentRangeTarget>>,
) {
    let Some(id) = id else {
        return;
    };
    let Some(bookmark) = active_bookmarks.iter().find(|bookmark| bookmark.id == id) else {
        return;
    };
    for range in active_comment_ranges
        .iter()
        .filter(|range| bookmark.start >= range.start)
    {
        comment_range_targets
            .entry(range.id.clone())
            .or_default()
            .push(NoteRefCommentRangeTarget {
                name: bookmark.name.clone(),
                start: bookmark.start,
                end,
            });
    }
}

fn close_note_ref_bookmark(
    id: Option<&str>,
    end: usize,
    active_bookmarks: &mut Vec<NoteRefActiveBookmark>,
    targets: &mut HashMap<String, NoteRefTarget>,
) {
    let Some(id) = id else {
        return;
    };
    if let Some(index) = active_bookmarks
        .iter()
        .position(|bookmark| bookmark.id == id)
    {
        let bookmark = active_bookmarks.remove(index);
        if let Some(target) = targets.get_mut(&bookmark.name) {
            if target.start == bookmark.start {
                target.end = end;
            }
        }
    }
}

fn record_note_ref_scan_field_position(
    instruction: Option<&str>,
    source_order: &mut usize,
    field_positions: &mut Vec<NoteRefFieldPosition>,
    ref_field_positions: &mut Vec<NoteRefFieldPosition>,
    generated_ref_note_fields: &mut Vec<NoteRefGeneratedField>,
) -> bool {
    let Some(instruction) = instruction.map(normalize_instruction) else {
        return false;
    };
    let mut recorded = false;
    if field_kind(&instruction) == FieldKind::NoteRef {
        field_positions.push(NoteRefFieldPosition {
            order: *source_order,
        });
        recorded = true;
    }
    if is_ref_position_field_instruction(&instruction) {
        ref_field_positions.push(NoteRefFieldPosition {
            order: *source_order,
        });
        if let Some(target) = ref_note_field_target(&instruction) {
            generated_ref_note_fields.push(NoteRefGeneratedField {
                target,
                order: *source_order,
            });
        }
        recorded = true;
    }
    if recorded {
        *source_order += 1;
    }
    recorded
}

fn apply_note_ref_scan_fld_char(
    e: &BytesStart<'_>,
    source_order: &mut usize,
    current: &mut Option<NoteRefScanField>,
    field_positions: &mut Vec<NoteRefFieldPosition>,
    ref_field_positions: &mut Vec<NoteRefFieldPosition>,
    generated_ref_note_fields: &mut Vec<NoteRefGeneratedField>,
    computed_fields: &mut NoteRefComputedFieldState<'_>,
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
            *current = Some(NoteRefScanField {
                instruction: String::new(),
                phase: FieldPhase::Instruction,
                suppress_result: false,
                position_recorded: false,
                nested_suppressed_fields: 0,
            });
        }
        Some("separate") => {
            if let Some(field) = current.as_mut() {
                if field.suppress_result {
                    return;
                }
                if let Some(text) =
                    computed_note_ref_scan_field_result(Some(&field.instruction), computed_fields)
                {
                    let recorded = record_note_ref_scan_field_position(
                        Some(&field.instruction),
                        source_order,
                        field_positions,
                        ref_field_positions,
                        generated_ref_note_fields,
                    );
                    if !recorded && !text.is_empty() {
                        *source_order += 1;
                    }
                    field.position_recorded = recorded;
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
                if !field.position_recorded {
                    record_note_ref_scan_field_position(
                        Some(&field.instruction),
                        source_order,
                        field_positions,
                        ref_field_positions,
                        generated_ref_note_fields,
                    );
                }
            }
        }
        _ => {}
    }
}

pub(crate) fn computed_note_ref_result(
    instruction: &str,
    note_refs: &NoteRefContext,
    field_position: Option<NoteRefFieldPosition>,
) -> Option<String> {
    let spec = note_ref_instruction(instruction)?;
    let target = note_refs.target(&spec.target)?;
    if !target.is_note_marker() {
        return None;
    }
    let text = if spec.relative {
        computed_relative_note_ref_result(target, field_position)?
    } else if target.custom_mark_order.is_some() {
        note_refs.custom_mark(target)?.to_string()
    } else {
        // An explicit `\*` number-format switch on the field wins; otherwise fall
        // back to the note kind's document-level `numFmt`.
        let number_format = spec.number_format.or(target.format);
        format_page_number(target.number, number_format)?
    };
    Some(apply_field_text_format(text, spec.text_format))
}

pub(crate) fn note_ref_source_field_position(
    instruction: &str,
    note_refs: &NoteRefContext,
    field_index: &mut usize,
) -> Option<NoteRefFieldPosition> {
    let instruction = normalize_instruction(instruction);
    if field_kind(&instruction) != FieldKind::NoteRef {
        return None;
    }
    let position = note_refs.field_position(*field_index);
    *field_index += 1;
    position
}

fn computed_relative_note_ref_result(
    target: NoteRefTarget,
    field_position: Option<NoteRefFieldPosition>,
) -> Option<String> {
    let field = field_position?;
    if field.order < target.start {
        return Some("below".to_string());
    }
    (field.order > target.end).then(|| "above".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NoteRefInstruction {
    target: String,
    number_format: Option<PageNumberFormat>,
    text_format: Option<FieldTextFormat>,
    relative: bool,
}

pub(super) fn note_ref_instruction(instruction: &str) -> Option<NoteRefInstruction> {
    let syntax = note_ref_field_syntax(instruction)?;
    Some(NoteRefInstruction {
        target: syntax.target,
        number_format: syntax
            .number_format
            .map(page_number_format_from_field_format),
        text_format: syntax.text_format,
        relative: syntax.relative,
    })
}
