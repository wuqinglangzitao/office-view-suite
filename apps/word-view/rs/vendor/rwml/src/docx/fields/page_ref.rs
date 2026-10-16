use super::*;

use crate::model::{PageNumberFormat as ModelPageNumberFormat, SectionBreakKind};
use crate::numfmt;

#[derive(Debug, Clone, Default)]
pub(crate) struct PageRefContext {
    targets: HashMap<String, PageRefTarget>,
    target_positions: HashMap<String, PageRefPosition>,
    rendered_target_positions: HashMap<String, PageRefPosition>,
    target_forced_break_after_orders: HashMap<String, usize>,
    unsupported_section_format_targets: HashSet<String>,
    field_positions: Vec<Option<PageRefPosition>>,
    field_orders: Vec<usize>,
    page_field_positions: Vec<Option<PageRefPosition>>,
}

impl PageRefContext {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    fn target(&self, name: &str) -> Option<PageRefTarget> {
        self.targets.get(name).copied()
    }

    pub(crate) fn target_uses_unsupported_display_format(&self, name: &str) -> bool {
        if self.unsupported_section_format_targets.contains(name) {
            return true;
        }
        self.target(name)
            .is_some_and(|target| target.display_format == PageRefDisplayFormat::Unsupported)
    }

    pub(crate) fn target_position(&self, name: &str) -> Option<PageRefPosition> {
        self.target_positions.get(name).copied()
    }

    fn target_forced_break_after_order(&self, name: &str) -> Option<usize> {
        self.target_forced_break_after_orders.get(name).copied()
    }

    fn rendered_target_position(&self, name: &str) -> Option<PageRefPosition> {
        self.rendered_target_positions.get(name).copied()
    }

    pub(crate) fn field_position(&self, index: usize) -> Option<PageRefPosition> {
        self.field_positions.get(index).copied().flatten()
    }

    pub(crate) fn field_order(&self, index: usize) -> Option<usize> {
        self.field_orders.get(index).copied()
    }

    pub(crate) fn page_field_position(&self, index: usize) -> Option<PageRefPosition> {
        self.page_field_positions.get(index).copied().flatten()
    }

    pub(crate) fn page_field_uses_unsupported_display_format(&self, index: usize) -> bool {
        self.page_field_position(index)
            .is_some_and(|position| position.display_format == PageRefDisplayFormat::Unsupported)
    }

    pub(crate) fn page_field_unsupported_display_formats(&self) -> Vec<bool> {
        self.page_field_positions
            .iter()
            .enumerate()
            .map(|(index, _)| self.page_field_uses_unsupported_display_format(index))
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PageRefTarget {
    display_page: usize,
    display_format: PageRefDisplayFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageRefPosition {
    physical_page: usize,
    display_page: usize,
    display_format: PageRefDisplayFormat,
    order: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageRefDisplayFormat {
    Known(Option<PageNumberFormat>),
    Unsupported,
}

impl Default for PageRefDisplayFormat {
    fn default() -> Self {
        Self::Known(None)
    }
}

#[derive(Debug, Clone)]
struct PageRefScanField {
    instruction: String,
    page_position: Option<PageRefPosition>,
    phase: FieldPhase,
    suppress_result: bool,
    nested_suppressed_fields: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageRefSectionBreak {
    Next,
    Even,
    Odd,
}

impl From<SectionBreakKind> for PageRefSectionBreak {
    fn from(kind: SectionBreakKind) -> Self {
        match kind {
            SectionBreakKind::NextPage => PageRefSectionBreak::Next,
            SectionBreakKind::EvenPage => PageRefSectionBreak::Even,
            SectionBreakKind::OddPage => PageRefSectionBreak::Odd,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PageRefPageState {
    leading_page_number: usize,
    leading_display_page_number: usize,
    leading_display_format: PageRefDisplayFormat,
    rendered_page_number: usize,
    rendered_display_page_number: usize,
    rendered_display_format: PageRefDisplayFormat,
    rendered_context_trusted: bool,
    display_only_restart_target: Option<PageRefTarget>,
    display_only_restart_page_ref_target: Option<PageRefTarget>,
}

impl Default for PageRefPageState {
    fn default() -> Self {
        Self {
            leading_page_number: 1,
            leading_display_page_number: 1,
            leading_display_format: PageRefDisplayFormat::default(),
            rendered_page_number: 1,
            rendered_display_page_number: 1,
            rendered_display_format: PageRefDisplayFormat::default(),
            rendered_context_trusted: true,
            display_only_restart_target: None,
            display_only_restart_page_ref_target: None,
        }
    }
}

impl PageRefPageState {
    fn with_initial_page_numbering(
        page_number_start: Option<usize>,
        page_number_format: Option<PageRefDisplayFormat>,
    ) -> Self {
        let mut state = Self::default();
        if let Some(start) = page_number_start {
            state.leading_display_page_number = start;
            state.rendered_display_page_number = start;
        }
        if let Some(format) = page_number_format {
            state.leading_display_format = format;
            state.rendered_display_format = format;
        }
        state
    }

    fn advance_section_break(
        &mut self,
        section_break: PageRefSectionBreak,
        saw_visible_content: bool,
        saw_rendered_page_break: bool,
        page_number_start: Option<usize>,
        page_number_format: Option<PageRefDisplayFormat>,
        source_order: &mut usize,
    ) {
        let previous_leading_page = self.leading_page_number;
        self.leading_page_number =
            page_after_section_break(self.leading_page_number, section_break);
        self.leading_display_page_number = page_after_section_break_display_page(
            self.leading_display_page_number,
            self.leading_page_number - previous_leading_page,
            page_number_start,
        );
        if let Some(format) = page_number_format {
            self.leading_display_format = format;
        }
        if !saw_visible_content || saw_rendered_page_break {
            let previous_rendered_page = self.rendered_page_number;
            self.rendered_page_number =
                page_after_section_break(self.rendered_page_number, section_break);
            self.rendered_display_page_number = page_after_section_break_display_page(
                self.rendered_display_page_number,
                self.rendered_page_number - previous_rendered_page,
                page_number_start,
            );
            if let Some(format) = page_number_format {
                self.rendered_display_format = format;
            }
            self.rendered_context_trusted = true;
            self.display_only_restart_target = None;
            self.display_only_restart_page_ref_target = None;
        } else {
            if let Some(format) = page_number_format {
                self.rendered_display_format = format;
            }
            self.rendered_context_trusted = false;
            let display_only_target = page_number_start.map(|_| PageRefTarget {
                display_page: self.leading_display_page_number,
                display_format: self.leading_display_format,
            });
            self.display_only_restart_target = display_only_target;
            self.display_only_restart_page_ref_target = display_only_target;
        }
        *source_order += 1;
    }

    fn apply_continuous_section_break(
        &mut self,
        saw_visible_content: bool,
        saw_rendered_page_break: bool,
        page_number_start: Option<usize>,
        page_number_format: Option<PageRefDisplayFormat>,
        source_order: &mut usize,
    ) {
        // A continuous (non-paginating) section break keeps the physical page but
        // can restart the display page number and/or change the display format. It
        // never advances a page counter or changes rendered_context_trusted.
        if let Some(start) = page_number_start {
            self.leading_display_page_number = start;
        }
        if let Some(format) = page_number_format {
            self.leading_display_format = format;
        }
        if !saw_visible_content || saw_rendered_page_break {
            if let Some(start) = page_number_start {
                self.rendered_display_page_number = start;
            }
            if let Some(format) = page_number_format {
                self.rendered_display_format = format;
            }
            self.display_only_restart_target = None;
            self.display_only_restart_page_ref_target = None;
        } else {
            if let Some(format) = page_number_format {
                self.rendered_display_format = format;
            }
            let display_only_target = page_number_start.map(|_| PageRefTarget {
                display_page: self.leading_display_page_number,
                display_format: self.leading_display_format,
            });
            self.display_only_restart_target = display_only_target;
            self.display_only_restart_page_ref_target = display_only_target;
        }
        *source_order += 1;
    }

    fn advance_explicit_break(
        &mut self,
        saw_visible_content: bool,
        saw_rendered_page_break: bool,
        source_order: &mut usize,
    ) {
        self.leading_page_number += 1;
        self.leading_display_page_number += 1;
        if !saw_visible_content || saw_rendered_page_break {
            self.rendered_page_number += 1;
            self.rendered_display_page_number += 1;
            self.rendered_context_trusted = true;
        } else {
            self.rendered_context_trusted = false;
        }
        self.display_only_restart_target = None;
        self.display_only_restart_page_ref_target = None;
        *source_order += 1;
    }

    fn advance_page_break_before(&mut self, source_order: &mut usize) {
        self.leading_page_number += 1;
        self.leading_display_page_number += 1;
        self.rendered_page_number += 1;
        self.rendered_display_page_number += 1;
        self.rendered_context_trusted = true;
        self.display_only_restart_target = None;
        self.display_only_restart_page_ref_target = None;
        *source_order += 1;
    }

    fn advance_last_rendered_page_break(&mut self, source_order: &mut usize) {
        self.rendered_page_number += 1;
        self.rendered_display_page_number += 1;
        self.rendered_context_trusted = true;
        self.display_only_restart_target = None;
        self.display_only_restart_page_ref_target = None;
        *source_order += 1;
    }

    fn note_visible_content(&mut self) {
        self.display_only_restart_target = None;
    }
}

pub(crate) fn page_ref_context(
    xml: &str,
    document_bookmarks: &HashMap<String, String>,
) -> PageRefContext {
    let core_properties = CoreProperties::default();
    let empty_properties = HashMap::new();
    page_ref_context_with_properties(
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

pub(crate) fn page_ref_context_with_properties(
    xml: &str,
    document_bookmarks: &HashMap<String, String>,
    properties: FieldDocumentProperties<'_>,
    preserve_legacy_form_cache: bool,
) -> PageRefContext {
    let mut r = Reader::from_str(xml);
    let mut targets = HashMap::new();
    let (initial_page_number_start, initial_page_number_format) =
        single_section_initial_page_numbering(xml);
    let mut pages = PageRefPageState::with_initial_page_numbering(
        initial_page_number_start,
        initial_page_number_format,
    );
    let mut source_order = 0usize;
    let mut saw_visible_content = false;
    let mut saw_rendered_page_break = false;
    let mut rendered_targets = HashMap::new();
    let mut target_positions = HashMap::new();
    let mut target_forced_break_after_orders = HashMap::new();
    let mut unsupported_section_format_targets = HashSet::new();
    let mut field_positions = Vec::new();
    let mut field_orders = Vec::new();
    let mut page_field_positions = Vec::new();
    let mut current: Option<PageRefScanField> = None;
    let mut computed_fields = PageRefComputedFieldState::new(
        document_bookmarks,
        properties,
        legacy_form_context(xml, preserve_legacy_form_cache),
    );
    let mut paragraph_depth = 0usize;
    let mut paragraph_properties_depth = 0usize;
    let mut section_properties_depth = 0usize;
    let mut section_type_seen = false;
    let mut section_continuous_seen = false;
    let mut section_is_paragraph_break = false;
    let mut section_break_pending = None;
    let mut paragraph_section_break_pending = None;
    let mut paragraph_continuous_restart_pending: Option<(
        Option<usize>,
        Option<PageRefDisplayFormat>,
    )> = None;
    let mut paragraph_section_break_targets = Vec::new();
    let mut paragraph_forced_break_after_targets = Vec::new();
    let mut section_page_number_start = None;
    let mut section_page_number_format = None;
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
                if suppresses_page_ref_complex_result_scan(&current) {
                    match name {
                        b"fldChar" => apply_page_ref_scan_fld_char(
                            &e,
                            current_page_ref_position(&pages, source_order),
                            current_page_field_position(&pages, source_order),
                            &mut current,
                            &mut computed_fields,
                            &mut PageRefFieldPositions {
                                field_positions: &mut field_positions,
                                field_orders: &mut field_orders,
                                page_field_positions: &mut page_field_positions,
                            },
                            &mut PageRefScanProgress {
                                source_order: &mut source_order,
                                saw_visible_content: &mut saw_visible_content,
                                pages: &mut pages,
                            },
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
                    b"p" => {
                        if paragraph_depth == 0 {
                            paragraph_section_break_targets.clear();
                            paragraph_forced_break_after_targets.clear();
                        }
                        paragraph_depth += 1;
                    }
                    b"pPr" => paragraph_properties_depth += 1,
                    b"pageBreakBefore"
                        if paragraph_properties_depth > 0 && page_ref_on_off_enabled(&e) =>
                    {
                        pages.advance_page_break_before(&mut source_order);
                    }
                    b"sectPr" => {
                        section_properties_depth += 1;
                        if section_properties_depth == 1 {
                            section_type_seen = false;
                            section_continuous_seen = false;
                            section_is_paragraph_break = paragraph_properties_depth > 0;
                            section_break_pending = None;
                            section_page_number_start = None;
                            section_page_number_format = None;
                        }
                    }
                    b"type" if section_properties_depth > 0 && !section_type_seen => {
                        section_type_seen = true;
                        section_break_pending = page_ref_section_break(&e);
                        section_continuous_seen = page_ref_section_is_continuous(&e);
                    }
                    b"pgNumType" if section_properties_depth > 0 => {
                        if section_page_number_start.is_none() {
                            section_page_number_start = page_ref_section_page_number_start(&e);
                        }
                        if section_page_number_format.is_none() {
                            section_page_number_format = page_ref_section_page_number_format(&e);
                        }
                    }
                    b"fldSimple" => {
                        let instruction = attr_local(&e, b"instr");
                        if !record_page_ref_field_position(
                            instruction.as_deref(),
                            current_page_ref_position(&pages, source_order),
                            current_page_field_position(&pages, source_order),
                            &mut source_order,
                            &mut field_positions,
                            &mut field_orders,
                            &mut page_field_positions,
                        ) {
                            if let Some(text) = computed_page_ref_scan_field_result(
                                instruction.as_deref(),
                                &mut computed_fields,
                            ) {
                                note_page_ref_computed_scan_text(
                                    &text,
                                    &mut saw_visible_content,
                                    &mut pages,
                                    &mut source_order,
                                );
                                skip_subtree(&mut r);
                                continue;
                            }
                        }
                    }
                    b"fldChar" => apply_page_ref_scan_fld_char(
                        &e,
                        current_page_ref_position(&pages, source_order),
                        current_page_field_position(&pages, source_order),
                        &mut current,
                        &mut computed_fields,
                        &mut PageRefFieldPositions {
                            field_positions: &mut field_positions,
                            field_orders: &mut field_orders,
                            page_field_positions: &mut page_field_positions,
                        },
                        &mut PageRefScanProgress {
                            source_order: &mut source_order,
                            saw_visible_content: &mut saw_visible_content,
                            pages: &mut pages,
                        },
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
                    b"bookmarkStart" => record_page_ref_bookmark_start(
                        &e,
                        &pages,
                        saw_visible_content,
                        paragraph_section_break_pending.is_some(),
                        &mut source_order,
                        &mut PageRefTargetSink {
                            targets: &mut targets,
                            rendered_targets: &mut rendered_targets,
                            target_positions: &mut target_positions,
                            unsupported_section_format_targets:
                                &mut unsupported_section_format_targets,
                            paragraph_section_break_targets: &mut paragraph_section_break_targets,
                            paragraph_forced_break_after_targets:
                                &mut paragraph_forced_break_after_targets,
                        },
                    ),
                    b"t" => {
                        let visible_text = !read_text(&mut r).is_empty();
                        consumed_element = true;
                        saw_visible_content |= visible_text;
                        if visible_text {
                            pages.note_visible_content();
                            source_order += 1;
                        }
                    }
                    b"br" if is_page_break_type(&e) => {
                        record_page_ref_forced_break_after_targets(
                            source_order,
                            &paragraph_forced_break_after_targets,
                            &mut target_forced_break_after_orders,
                        );
                        pages.advance_explicit_break(
                            saw_visible_content,
                            saw_rendered_page_break,
                            &mut source_order,
                        );
                    }
                    b"lastRenderedPageBreak" => {
                        saw_rendered_page_break = true;
                        pages.advance_last_rendered_page_break(&mut source_order);
                    }
                    b"sym" if is_supported_run_symbol(&e) => {
                        saw_visible_content = true;
                        pages.note_visible_content();
                        source_order += 1;
                    }
                    _ if is_visible_reference_mark(name) => {
                        saw_visible_content = true;
                        pages.note_visible_content();
                        source_order += 1;
                    }
                    b"tab" | b"cr" | b"noBreakHyphen" | b"softHyphen" | b"drawing" | b"pict"
                    | b"object" => {
                        saw_visible_content = true;
                        pages.note_visible_content();
                        source_order += 1;
                    }
                    b"br" => {
                        saw_visible_content = true;
                        pages.note_visible_content();
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
                if suppresses_page_ref_complex_result_scan(&current) {
                    if name == b"fldChar" {
                        apply_page_ref_scan_fld_char(
                            &e,
                            current_page_ref_position(&pages, source_order),
                            current_page_field_position(&pages, source_order),
                            &mut current,
                            &mut computed_fields,
                            &mut PageRefFieldPositions {
                                field_positions: &mut field_positions,
                                field_orders: &mut field_orders,
                                page_field_positions: &mut page_field_positions,
                            },
                            &mut PageRefScanProgress {
                                source_order: &mut source_order,
                                saw_visible_content: &mut saw_visible_content,
                                pages: &mut pages,
                            },
                        );
                    }
                    continue;
                }
                match name {
                    b"pageBreakBefore"
                        if paragraph_properties_depth > 0 && page_ref_on_off_enabled(&e) =>
                    {
                        pages.advance_page_break_before(&mut source_order);
                    }
                    b"sectPr" if paragraph_properties_depth > 0 => {
                        paragraph_section_break_pending =
                            Some((PageRefSectionBreak::Next, None, None));
                    }
                    b"type" if section_properties_depth > 0 && !section_type_seen => {
                        section_type_seen = true;
                        section_break_pending = page_ref_section_break(&e);
                        section_continuous_seen = page_ref_section_is_continuous(&e);
                    }
                    b"pgNumType" if section_properties_depth > 0 => {
                        if section_page_number_start.is_none() {
                            section_page_number_start = page_ref_section_page_number_start(&e);
                        }
                        if section_page_number_format.is_none() {
                            section_page_number_format = page_ref_section_page_number_format(&e);
                        }
                    }
                    b"fldSimple" => {
                        let instruction = attr_local(&e, b"instr");
                        if !record_page_ref_field_position(
                            instruction.as_deref(),
                            current_page_ref_position(&pages, source_order),
                            current_page_field_position(&pages, source_order),
                            &mut source_order,
                            &mut field_positions,
                            &mut field_orders,
                            &mut page_field_positions,
                        ) {
                            if let Some(text) = computed_page_ref_scan_field_result(
                                instruction.as_deref(),
                                &mut computed_fields,
                            ) {
                                note_page_ref_computed_scan_text(
                                    &text,
                                    &mut saw_visible_content,
                                    &mut pages,
                                    &mut source_order,
                                );
                            }
                        }
                    }
                    b"fldChar" => apply_page_ref_scan_fld_char(
                        &e,
                        current_page_ref_position(&pages, source_order),
                        current_page_field_position(&pages, source_order),
                        &mut current,
                        &mut computed_fields,
                        &mut PageRefFieldPositions {
                            field_positions: &mut field_positions,
                            field_orders: &mut field_orders,
                            page_field_positions: &mut page_field_positions,
                        },
                        &mut PageRefScanProgress {
                            source_order: &mut source_order,
                            saw_visible_content: &mut saw_visible_content,
                            pages: &mut pages,
                        },
                    ),
                    b"bookmarkStart" => record_page_ref_bookmark_start(
                        &e,
                        &pages,
                        saw_visible_content,
                        paragraph_section_break_pending.is_some(),
                        &mut source_order,
                        &mut PageRefTargetSink {
                            targets: &mut targets,
                            rendered_targets: &mut rendered_targets,
                            target_positions: &mut target_positions,
                            unsupported_section_format_targets:
                                &mut unsupported_section_format_targets,
                            paragraph_section_break_targets: &mut paragraph_section_break_targets,
                            paragraph_forced_break_after_targets:
                                &mut paragraph_forced_break_after_targets,
                        },
                    ),
                    b"br" if is_page_break_type(&e) => {
                        record_page_ref_forced_break_after_targets(
                            source_order,
                            &paragraph_forced_break_after_targets,
                            &mut target_forced_break_after_orders,
                        );
                        pages.advance_explicit_break(
                            saw_visible_content,
                            saw_rendered_page_break,
                            &mut source_order,
                        );
                    }
                    b"lastRenderedPageBreak" => {
                        saw_rendered_page_break = true;
                        pages.advance_last_rendered_page_break(&mut source_order);
                    }
                    b"sym" if is_supported_run_symbol(&e) => {
                        saw_visible_content = true;
                        pages.note_visible_content();
                        source_order += 1;
                    }
                    _ if is_visible_reference_mark(name) => {
                        saw_visible_content = true;
                        pages.note_visible_content();
                        source_order += 1;
                    }
                    b"tab" | b"br" | b"cr" | b"noBreakHyphen" | b"softHyphen" | b"drawing"
                    | b"pict" | b"object" => {
                        saw_visible_content = true;
                        pages.note_visible_content();
                        source_order += 1;
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) => {
                let qname = e.name();
                let name = local(qname.as_ref());
                if suppresses_page_ref_complex_result_scan(&current) {
                    xml_depth = xml_depth.saturating_sub(1);
                    continue;
                }
                match name {
                    b"AlternateContent" => {
                        alternate_content_stack.pop();
                    }
                    b"sectPr" => {
                        if section_properties_depth == 1 && section_is_paragraph_break {
                            let section_break = if section_type_seen {
                                section_break_pending
                            } else {
                                Some(PageRefSectionBreak::Next)
                            };
                            if let Some(section_break) = section_break {
                                paragraph_section_break_pending = Some((
                                    section_break,
                                    section_page_number_start,
                                    section_page_number_format,
                                ));
                            } else if section_continuous_seen
                                && (section_page_number_start.is_some()
                                    || section_page_number_format.is_some())
                            {
                                paragraph_continuous_restart_pending =
                                    Some((section_page_number_start, section_page_number_format));
                            }
                        }
                        section_properties_depth = section_properties_depth.saturating_sub(1);
                        if section_properties_depth == 0 {
                            section_type_seen = false;
                            section_continuous_seen = false;
                            section_is_paragraph_break = false;
                            section_break_pending = None;
                            section_page_number_start = None;
                            section_page_number_format = None;
                        }
                    }
                    b"p" => {
                        if paragraph_depth == 1 {
                            if let Some((section_break, page_number_start, page_number_format)) =
                                paragraph_section_break_pending.take()
                            {
                                let break_order = source_order;
                                pages.advance_section_break(
                                    section_break,
                                    saw_visible_content,
                                    saw_rendered_page_break,
                                    page_number_start,
                                    page_number_format,
                                    &mut source_order,
                                );
                                for name in paragraph_section_break_targets.drain(..) {
                                    target_forced_break_after_orders
                                        .entry(name)
                                        .or_insert(break_order);
                                }
                            } else if let Some((page_number_start, page_number_format)) =
                                paragraph_continuous_restart_pending.take()
                            {
                                pages.apply_continuous_section_break(
                                    saw_visible_content,
                                    saw_rendered_page_break,
                                    page_number_start,
                                    page_number_format,
                                    &mut source_order,
                                );
                            }
                        }
                        paragraph_depth = paragraph_depth.saturating_sub(1);
                        if paragraph_depth == 0 {
                            paragraph_section_break_targets.clear();
                            paragraph_forced_break_after_targets.clear();
                        }
                    }
                    b"pPr" => {
                        paragraph_properties_depth = paragraph_properties_depth.saturating_sub(1);
                    }
                    _ => {}
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let rendered_target_positions = rendered_targets.clone();
    if saw_rendered_page_break {
        for (name, position) in rendered_targets {
            match targets.entry(name.clone()) {
                std::collections::hash_map::Entry::Vacant(entry) => {
                    entry.insert(PageRefTarget {
                        display_page: position.display_page,
                        display_format: position.display_format,
                    });
                    target_positions_insert(&mut target_positions, name, position);
                }
                std::collections::hash_map::Entry::Occupied(entry) => {
                    if entry.get().display_page == position.display_page
                        && entry.get().display_format == position.display_format
                    {
                        target_positions_insert(&mut target_positions, name, position);
                    }
                }
            }
        }
    }
    PageRefContext {
        targets,
        target_positions,
        rendered_target_positions,
        target_forced_break_after_orders,
        unsupported_section_format_targets,
        field_positions,
        field_orders,
        page_field_positions,
    }
}

fn single_section_initial_page_numbering(
    xml: &str,
) -> (Option<usize>, Option<PageRefDisplayFormat>) {
    let mut r = Reader::from_str(xml);
    let mut paragraph_properties_depth = 0usize;
    let mut body_section_properties_depth = 0usize;
    let mut has_paragraph_section_properties = false;
    let mut page_number_start = None;
    let mut page_number_format = None;
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
                    b"pPr" => paragraph_properties_depth += 1,
                    b"sectPr" if paragraph_properties_depth > 0 => {
                        has_paragraph_section_properties = true;
                        skip_subtree(&mut r);
                        continue;
                    }
                    b"sectPr" => body_section_properties_depth += 1,
                    b"pgNumType" if body_section_properties_depth > 0 => {
                        if page_number_start.is_none() {
                            page_number_start = page_ref_section_page_number_start(&e);
                        }
                        if page_number_format.is_none() {
                            page_number_format = page_ref_section_page_number_format(&e);
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
                    b"sectPr" if paragraph_properties_depth > 0 => {
                        has_paragraph_section_properties = true;
                    }
                    b"pgNumType" if body_section_properties_depth > 0 => {
                        if page_number_start.is_none() {
                            page_number_start = page_ref_section_page_number_start(&e);
                        }
                        if page_number_format.is_none() {
                            page_number_format = page_ref_section_page_number_format(&e);
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
                    b"sectPr" => {
                        body_section_properties_depth =
                            body_section_properties_depth.saturating_sub(1);
                    }
                    b"pPr" => {
                        paragraph_properties_depth = paragraph_properties_depth.saturating_sub(1);
                    }
                    _ => {}
                }
                xml_depth = xml_depth.saturating_sub(1);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    if has_paragraph_section_properties {
        (None, None)
    } else {
        (page_number_start, page_number_format)
    }
}

pub(super) fn page_ref_on_off_enabled(e: &BytesStart<'_>) -> bool {
    toggle_on(attr_local(e, b"val"))
}

pub(super) fn page_ref_section_break(e: &BytesStart<'_>) -> Option<PageRefSectionBreak> {
    match attr_local_trimmed_preserve_empty(e, b"val").as_deref() {
        None | Some("") | Some("nextPage") => Some(PageRefSectionBreak::Next),
        Some(value) => SectionBreakKind::from_wml_value(value).map(PageRefSectionBreak::from),
    }
}

// A `continuous`/`nextColumn` section break does not paginate, so any pgNumType
// restart/format on it is applied to the current physical page without a break.
fn page_ref_section_is_continuous(e: &BytesStart<'_>) -> bool {
    matches!(
        attr_local_trimmed_preserve_empty(e, b"val").as_deref(),
        Some("continuous") | Some("nextColumn")
    )
}

fn page_ref_section_page_number_start(e: &BytesStart<'_>) -> Option<usize> {
    attr_usize(e, b"start").filter(|start| *start > 0)
}

fn page_ref_section_page_number_format(e: &BytesStart<'_>) -> Option<PageRefDisplayFormat> {
    let value = attr_local_trimmed_preserve_empty(e, b"fmt")?;
    if value.is_empty() {
        return Some(PageRefDisplayFormat::default());
    }
    Some(
        ModelPageNumberFormat::from_wml_value(&value)
            .map(|format| PageRefDisplayFormat::Known(Some(PageNumberFormat::from(format))))
            .unwrap_or(PageRefDisplayFormat::Unsupported),
    )
}

pub(super) fn page_after_section_break(page: usize, section_break: PageRefSectionBreak) -> usize {
    let next = page + 1;
    match section_break {
        PageRefSectionBreak::Next => next,
        PageRefSectionBreak::Even if next % 2 == 1 => next + 1,
        PageRefSectionBreak::Odd if next % 2 == 0 => next + 1,
        PageRefSectionBreak::Even | PageRefSectionBreak::Odd => next,
    }
}

fn page_after_section_break_display_page(
    display_page: usize,
    physical_delta: usize,
    page_number_start: Option<usize>,
) -> usize {
    page_number_start.unwrap_or(display_page + physical_delta)
}

fn target_positions_insert(
    target_positions: &mut HashMap<String, PageRefPosition>,
    name: String,
    position: PageRefPosition,
) {
    target_positions.entry(name).or_insert(position);
}

/// Page-ref target accumulators that travel together through the bookmark scan.
struct PageRefTargetSink<'a> {
    targets: &'a mut HashMap<String, PageRefTarget>,
    rendered_targets: &'a mut HashMap<String, PageRefPosition>,
    target_positions: &'a mut HashMap<String, PageRefPosition>,
    unsupported_section_format_targets: &'a mut HashSet<String>,
    paragraph_section_break_targets: &'a mut Vec<String>,
    paragraph_forced_break_after_targets: &'a mut Vec<String>,
}

/// Field-position accumulators threaded through the complex-field scan.
struct PageRefFieldPositions<'a> {
    field_positions: &'a mut Vec<Option<PageRefPosition>>,
    field_orders: &'a mut Vec<usize>,
    page_field_positions: &'a mut Vec<Option<PageRefPosition>>,
}

/// Mutable scan-progress state shared across the page-ref complex-field scan.
struct PageRefScanProgress<'a> {
    source_order: &'a mut usize,
    saw_visible_content: &'a mut bool,
    pages: &'a mut PageRefPageState,
}

fn record_page_ref_bookmark_start(
    e: &BytesStart<'_>,
    pages: &PageRefPageState,
    saw_visible_content: bool,
    paragraph_section_break_pending: bool,
    source_order: &mut usize,
    sink: &mut PageRefTargetSink<'_>,
) {
    let targets = &mut *sink.targets;
    let rendered_targets = &mut *sink.rendered_targets;
    let target_positions = &mut *sink.target_positions;
    let unsupported_section_format_targets = &mut *sink.unsupported_section_format_targets;
    let paragraph_section_break_targets = &mut *sink.paragraph_section_break_targets;
    let paragraph_forced_break_after_targets = &mut *sink.paragraph_forced_break_after_targets;
    let Some(name) = bookmark_name(e) else {
        return;
    };
    let mut has_recoverable_page_position = false;
    if pages.leading_display_format == PageRefDisplayFormat::Unsupported {
        unsupported_section_format_targets.insert(name.clone());
    }
    if pages.leading_page_number > 1 && !saw_visible_content {
        targets.entry(name.clone()).or_insert(PageRefTarget {
            display_page: pages.leading_display_page_number,
            display_format: pages.leading_display_format,
        });
        target_positions_insert(
            target_positions,
            name.clone(),
            PageRefPosition {
                physical_page: pages.leading_page_number,
                display_page: pages.leading_display_page_number,
                display_format: pages.leading_display_format,
                order: *source_order,
            },
        );
        has_recoverable_page_position = true;
    } else if paragraph_section_break_pending && pages.rendered_context_trusted {
        targets.entry(name.clone()).or_insert(PageRefTarget {
            display_page: pages.rendered_display_page_number,
            display_format: pages.rendered_display_format,
        });
        target_positions_insert(
            target_positions,
            name.clone(),
            PageRefPosition {
                physical_page: pages.rendered_page_number,
                display_page: pages.rendered_display_page_number,
                display_format: pages.rendered_display_format,
                order: *source_order,
            },
        );
        paragraph_section_break_targets.push(name.clone());
        has_recoverable_page_position = true;
    }
    if pages.rendered_context_trusted {
        rendered_targets
            .entry(name.clone())
            .or_insert(PageRefPosition {
                physical_page: pages.rendered_page_number,
                display_page: pages.rendered_display_page_number,
                display_format: pages.rendered_display_format,
                order: *source_order,
            });
        has_recoverable_page_position = true;
    } else if let Some(target) = pages.display_only_restart_target {
        targets.entry(name.clone()).or_insert(target);
        if let Some(position) = display_only_restart_page_ref_position(pages, *source_order) {
            target_positions_insert(target_positions, name.clone(), position);
            has_recoverable_page_position = true;
        }
    }
    if has_recoverable_page_position {
        push_unique(paragraph_forced_break_after_targets, name);
    }
    *source_order += 1;
}

fn record_page_ref_forced_break_after_targets(
    break_order: usize,
    paragraph_forced_break_after_targets: &[String],
    target_forced_break_after_orders: &mut HashMap<String, usize>,
) {
    for name in paragraph_forced_break_after_targets {
        target_forced_break_after_orders
            .entry(name.clone())
            .or_insert(break_order);
    }
}

fn current_page_ref_position(
    pages: &PageRefPageState,
    source_order: usize,
) -> Option<PageRefPosition> {
    pages
        .rendered_context_trusted
        .then_some(PageRefPosition {
            physical_page: pages.rendered_page_number,
            display_page: pages.rendered_display_page_number,
            display_format: pages.rendered_display_format,
            order: source_order,
        })
        .or_else(|| display_only_restart_page_ref_position(pages, source_order))
}

fn display_only_restart_page_ref_position(
    pages: &PageRefPageState,
    source_order: usize,
) -> Option<PageRefPosition> {
    pages
        .display_only_restart_page_ref_target
        .map(|target| PageRefPosition {
            physical_page: pages.leading_page_number,
            display_page: target.display_page,
            display_format: target.display_format,
            order: source_order,
        })
}

fn current_page_field_position(
    pages: &PageRefPageState,
    source_order: usize,
) -> Option<PageRefPosition> {
    current_page_ref_position(pages, source_order).or_else(|| {
        pages
            .display_only_restart_target
            .map(|target| PageRefPosition {
                physical_page: pages.leading_page_number,
                display_page: target.display_page,
                display_format: target.display_format,
                order: source_order,
            })
    })
}

fn record_page_ref_field_position(
    instruction: Option<&str>,
    page_ref_position: Option<PageRefPosition>,
    page_position: Option<PageRefPosition>,
    source_order: &mut usize,
    field_positions: &mut Vec<Option<PageRefPosition>>,
    field_orders: &mut Vec<usize>,
    page_field_positions: &mut Vec<Option<PageRefPosition>>,
) -> bool {
    match instruction
        .map(normalize_instruction)
        .as_deref()
        .map(field_kind)
    {
        Some(FieldKind::PageRef) => {
            field_positions.push(page_ref_position);
            field_orders.push(*source_order);
            *source_order += 1;
            true
        }
        Some(FieldKind::Page) => {
            page_field_positions.push(page_position);
            *source_order += 1;
            true
        }
        _ => false,
    }
}

#[derive(Debug)]
struct PageRefComputedFieldState<'a> {
    document_bookmarks: &'a HashMap<String, String>,
    properties: FieldDocumentProperties<'a>,
    legacy_forms: LegacyFormContext,
    field_bookmarks: HashMap<String, String>,
    sequence_counters: HashMap<String, i64>,
    autonum_counter: i64,
    listnum_counter: i64,
    form_field_index: usize,
}

impl<'a> PageRefComputedFieldState<'a> {
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

fn computed_page_ref_scan_field_result(
    instruction: Option<&str>,
    state: &mut PageRefComputedFieldState<'_>,
) -> Option<String> {
    let instruction = normalize_instruction(instruction?);
    let kind = field_kind(&instruction);
    match &kind {
        FieldKind::Page | FieldKind::PageRef => return None,
        FieldKind::Ref => {
            return computed_page_ref_scan_ref_result(&instruction, state);
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
        if let Some(text) = computed_page_ref_scan_direct_bookmark_ref_result(&instruction, state) {
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

fn computed_page_ref_scan_ref_result(
    instruction: &str,
    state: &PageRefComputedFieldState<'_>,
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

fn computed_page_ref_scan_direct_bookmark_ref_result(
    instruction: &str,
    state: &PageRefComputedFieldState<'_>,
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

fn note_page_ref_computed_scan_text(
    text: &str,
    saw_visible_content: &mut bool,
    pages: &mut PageRefPageState,
    source_order: &mut usize,
) {
    if !text.is_empty() {
        *saw_visible_content = true;
        pages.note_visible_content();
        *source_order += 1;
    }
}

fn suppresses_page_ref_complex_result_scan(current: &Option<PageRefScanField>) -> bool {
    current
        .as_ref()
        .is_some_and(|field| field.phase == FieldPhase::Result && field.suppress_result)
}

fn apply_page_ref_scan_fld_char(
    e: &BytesStart<'_>,
    page_ref_position: Option<PageRefPosition>,
    page_position: Option<PageRefPosition>,
    current: &mut Option<PageRefScanField>,
    computed_fields: &mut PageRefComputedFieldState<'_>,
    positions: &mut PageRefFieldPositions<'_>,
    progress: &mut PageRefScanProgress<'_>,
) {
    let source_order = &mut *progress.source_order;
    let saw_visible_content = &mut *progress.saw_visible_content;
    let pages = &mut *progress.pages;
    let field_positions = &mut *positions.field_positions;
    let field_orders = &mut *positions.field_orders;
    let page_field_positions = &mut *positions.page_field_positions;
    match field_char_type(e).as_deref() {
        Some("begin") => {
            if let Some(field) = current.as_mut() {
                if field.phase == FieldPhase::Result && field.suppress_result {
                    field.nested_suppressed_fields =
                        field.nested_suppressed_fields.saturating_add(1);
                    return;
                }
            }
            *current = Some(PageRefScanField {
                instruction: String::new(),
                page_position,
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
                    computed_page_ref_scan_field_result(Some(&field.instruction), computed_fields)
                {
                    note_page_ref_computed_scan_text(
                        &text,
                        saw_visible_content,
                        pages,
                        source_order,
                    );
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
                let _ = record_page_ref_field_position(
                    Some(&field.instruction),
                    page_ref_position,
                    field.page_position,
                    source_order,
                    field_positions,
                    field_orders,
                    page_field_positions,
                );
            }
        }
        _ => {}
    }
}

pub(crate) fn computed_page_result(
    instruction: &str,
    field_position: Option<PageRefPosition>,
) -> Option<String> {
    let spec = page_instruction(instruction)?;
    let field_position = field_position?;
    let text = format_page_ref_number(
        field_position.display_page,
        spec.number_format,
        field_position.display_format,
    )?;
    Some(apply_field_text_format(text, spec.text_format))
}

pub(crate) fn supports_page_field_syntax(instruction: &str) -> bool {
    page_instruction(instruction).is_some()
}

pub(crate) fn computed_page_ref_result(
    instruction: &str,
    page_refs: &PageRefContext,
    field_position: Option<PageRefPosition>,
    field_order: Option<usize>,
) -> Option<String> {
    let spec = page_ref_instruction(instruction)?;
    let target = page_refs.target(&spec.target)?;
    let text = if spec.relative {
        computed_relative_page_ref_result(&spec, page_refs, field_position, field_order)?
    } else {
        format_page_ref_number(
            target.display_page,
            spec.number_format,
            target.display_format,
        )?
    };
    Some(apply_field_text_format(text, spec.text_format))
}

fn computed_relative_page_ref_result(
    spec: &PageRefInstruction,
    page_refs: &PageRefContext,
    field_position: Option<PageRefPosition>,
    field_order: Option<usize>,
) -> Option<String> {
    let target = page_refs.target_position(&spec.target)?;
    if let Some(field) = field_position {
        if target.physical_page == field.physical_page {
            return Some(if target.order <= field.order {
                "above".to_string()
            } else {
                "below".to_string()
            });
        }
    } else {
        let field_order = field_order?;
        // The field is in an untrusted (auto-paginated) context. It resolves to a
        // definite page only when the target is provably on a different page: either
        // a forced break was recorded after the target, or the target itself sits on
        // a trusted rendered page later in source order (every trusted-rendered
        // position is preceded by a page advance, so a later one is a later page).
        let different_page = match page_refs.target_forced_break_after_order(&spec.target) {
            Some(break_order) => field_order > break_order,
            None => page_refs
                .rendered_target_position(&spec.target)
                .is_some_and(|position| position.order > field_order),
        };
        if !different_page {
            return None;
        }
    }
    Some(format!(
        "on page {}",
        format_relative_page_ref_number(
            target.display_page,
            spec.number_format,
            target.display_format,
        )?
    ))
}

fn format_relative_page_ref_number(
    page: usize,
    field_format: Option<PageNumberFormat>,
    page_format: PageRefDisplayFormat,
) -> Option<String> {
    let format = match (field_format, page_format) {
        (None, PageRefDisplayFormat::Unsupported) => Some(PageNumberFormat::Arabic),
        (format, _) => format,
    };
    format_page_ref_number(page, format, page_format)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PageInstruction {
    number_format: Option<PageNumberFormat>,
    text_format: Option<FieldTextFormat>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PageRefInstruction {
    target: String,
    number_format: Option<PageNumberFormat>,
    text_format: Option<FieldTextFormat>,
    relative: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PageNumberFormat {
    Arabic,
    ArabicDash,
    DecimalZero,
    DecimalFullWidth,
    DecimalHalfWidth,
    DecimalFullWidth2,
    DecimalEnclosedCircle,
    DecimalEnclosedFullstop,
    DecimalEnclosedParen,
    Ganada,
    Chosung,
    KoreanDigital,
    KoreanCounting,
    KoreanLegal,
    KoreanDigital2,
    AlphabeticLower,
    AlphabeticUpper,
    RomanLower,
    RomanUpper,
    Ordinal,
    CardText,
    OrdText,
    Hex,
    DollarText,
}

impl From<ModelPageNumberFormat> for PageNumberFormat {
    fn from(format: ModelPageNumberFormat) -> Self {
        match format {
            ModelPageNumberFormat::Decimal => PageNumberFormat::Arabic,
            ModelPageNumberFormat::DecimalZero => PageNumberFormat::DecimalZero,
            ModelPageNumberFormat::NumberInDash => PageNumberFormat::ArabicDash,
            ModelPageNumberFormat::DecimalFullWidth => PageNumberFormat::DecimalFullWidth,
            ModelPageNumberFormat::DecimalHalfWidth => PageNumberFormat::DecimalHalfWidth,
            ModelPageNumberFormat::DecimalFullWidth2 => PageNumberFormat::DecimalFullWidth2,
            ModelPageNumberFormat::DecimalEnclosedCircle => PageNumberFormat::DecimalEnclosedCircle,
            ModelPageNumberFormat::DecimalEnclosedFullstop => {
                PageNumberFormat::DecimalEnclosedFullstop
            }
            ModelPageNumberFormat::DecimalEnclosedParen => PageNumberFormat::DecimalEnclosedParen,
            ModelPageNumberFormat::Ganada => PageNumberFormat::Ganada,
            ModelPageNumberFormat::Chosung => PageNumberFormat::Chosung,
            ModelPageNumberFormat::KoreanDigital => PageNumberFormat::KoreanDigital,
            ModelPageNumberFormat::KoreanCounting => PageNumberFormat::KoreanCounting,
            ModelPageNumberFormat::KoreanLegal => PageNumberFormat::KoreanLegal,
            ModelPageNumberFormat::KoreanDigital2 => PageNumberFormat::KoreanDigital2,
            ModelPageNumberFormat::LowerLetter => PageNumberFormat::AlphabeticLower,
            ModelPageNumberFormat::UpperLetter => PageNumberFormat::AlphabeticUpper,
            ModelPageNumberFormat::LowerRoman => PageNumberFormat::RomanLower,
            ModelPageNumberFormat::UpperRoman => PageNumberFormat::RomanUpper,
            ModelPageNumberFormat::Ordinal => PageNumberFormat::Ordinal,
            ModelPageNumberFormat::CardinalText => PageNumberFormat::CardText,
            ModelPageNumberFormat::OrdinalText => PageNumberFormat::OrdText,
        }
    }
}

fn page_instruction(instruction: &str) -> Option<PageInstruction> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("PAGE") {
        return None;
    }
    let format = page_field_format_syntax_tail(&mut parts)?;
    Some(PageInstruction {
        number_format: format
            .number_format
            .map(page_number_format_from_field_format),
        text_format: format.text_format,
    })
}

pub(super) fn page_ref_instruction(instruction: &str) -> Option<PageRefInstruction> {
    let syntax = page_ref_field_syntax(instruction)?;
    Some(PageRefInstruction {
        target: syntax.target,
        number_format: syntax
            .number_format
            .map(page_number_format_from_field_format),
        text_format: syntax.text_format,
        relative: syntax.relative,
    })
}

fn accept_page_field_format_switch(
    part: &str,
    number_format: &mut Option<PageNumberFormat>,
    text_format: &mut Option<FieldTextFormat>,
) -> bool {
    accept_page_number_format_switch(part, number_format)
        || accept_field_format_switch(part, text_format)
}

pub(super) fn accept_page_field_format_switch_for_tail<'a>(
    part: &'a str,
    parts: &mut impl Iterator<Item = &'a str>,
    number_format: &mut Option<PageNumberFormat>,
    text_format: &mut Option<FieldTextFormat>,
) -> Option<bool> {
    accept_general_format_switch(part, parts, |format| {
        accept_page_field_format_switch(format, number_format, text_format)
    })
}

fn accept_page_number_format_switch(
    part: &str,
    number_format: &mut Option<PageNumberFormat>,
) -> bool {
    let mut format = number_format.map(|_| FieldNumberFormat::Arabic);
    let accepted = accept_field_number_format_switch(part, &mut format);
    if accepted {
        *number_format = format.map(page_number_format_from_field_format);
    }
    accepted
}

pub(super) fn page_number_format_from_field_format(format: FieldNumberFormat) -> PageNumberFormat {
    match format {
        FieldNumberFormat::Arabic => PageNumberFormat::Arabic,
        FieldNumberFormat::ArabicDash => PageNumberFormat::ArabicDash,
        FieldNumberFormat::AlphabeticLower => PageNumberFormat::AlphabeticLower,
        FieldNumberFormat::AlphabeticUpper => PageNumberFormat::AlphabeticUpper,
        FieldNumberFormat::RomanLower => PageNumberFormat::RomanLower,
        FieldNumberFormat::RomanUpper => PageNumberFormat::RomanUpper,
        FieldNumberFormat::Ordinal => PageNumberFormat::Ordinal,
        FieldNumberFormat::CardText => PageNumberFormat::CardText,
        FieldNumberFormat::OrdText => PageNumberFormat::OrdText,
        FieldNumberFormat::Hex => PageNumberFormat::Hex,
        FieldNumberFormat::DollarText => PageNumberFormat::DollarText,
    }
}

pub(super) fn format_page_number(page: usize, format: Option<PageNumberFormat>) -> Option<String> {
    match format.unwrap_or(PageNumberFormat::Arabic) {
        PageNumberFormat::Arabic => Some(page.to_string()),
        PageNumberFormat::ArabicDash => Some(format!("- {page} -")),
        PageNumberFormat::DecimalZero => Some(format!("{page:02}")),
        PageNumberFormat::DecimalFullWidth => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x0E)),
        PageNumberFormat::DecimalHalfWidth => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x0F)),
        PageNumberFormat::DecimalFullWidth2 => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x13)),
        PageNumberFormat::DecimalEnclosedCircle => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x12)),
        PageNumberFormat::DecimalEnclosedFullstop => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x1A)),
        PageNumberFormat::DecimalEnclosedParen => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x1B)),
        PageNumberFormat::Ganada => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x18)),
        PageNumberFormat::Chosung => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x19)),
        PageNumberFormat::KoreanDigital => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x29)),
        PageNumberFormat::KoreanCounting => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x2A)),
        PageNumberFormat::KoreanLegal => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x2B)),
        PageNumberFormat::KoreanDigital2 => u32::try_from(page)
            .ok()
            .map(|page| numfmt::format(page, 0x2C)),
        PageNumberFormat::AlphabeticLower => alphabetic_page_number(page, false),
        PageNumberFormat::AlphabeticUpper => alphabetic_page_number(page, true),
        PageNumberFormat::RomanLower => roman_page_number(page).map(|value| value.to_lowercase()),
        PageNumberFormat::RomanUpper => roman_page_number(page),
        PageNumberFormat::Ordinal => Some(ordinal_page_number(page)),
        PageNumberFormat::CardText => cardinal_page_number_text(page),
        PageNumberFormat::OrdText => ordinal_page_number_text(page),
        PageNumberFormat::Hex => Some(format!("{page:X}")),
        PageNumberFormat::DollarText => dollar_page_number_text(page),
    }
}

fn format_page_ref_number(
    page: usize,
    field_format: Option<PageNumberFormat>,
    page_format: PageRefDisplayFormat,
) -> Option<String> {
    let format = match field_format {
        Some(format) => Some(format),
        None => match page_format {
            PageRefDisplayFormat::Known(format) => format,
            PageRefDisplayFormat::Unsupported => return None,
        },
    };
    format_page_number(page, format)
}

fn alphabetic_page_number(mut page: usize, uppercase: bool) -> Option<String> {
    if page == 0 {
        return None;
    }
    let base = if uppercase { b'A' } else { b'a' };
    let mut chars = Vec::new();
    while page > 0 {
        page -= 1;
        chars.push((base + (page % 26) as u8) as char);
        page /= 26;
    }
    chars.reverse();
    Some(chars.into_iter().collect())
}

fn roman_page_number(mut page: usize) -> Option<String> {
    if page == 0 || page > 3999 {
        return None;
    }
    let mut out = String::new();
    for (value, numeral) in [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ] {
        while page >= value {
            out.push_str(numeral);
            page -= value;
        }
    }
    Some(out)
}

fn ordinal_page_number(page: usize) -> String {
    let suffix = if (11..=13).contains(&(page % 100)) {
        "th"
    } else {
        match page % 10 {
            1 => "st",
            2 => "nd",
            3 => "rd",
            _ => "th",
        }
    };
    format!("{page}{suffix}")
}

pub(super) fn cardinal_page_number_text(page: usize) -> Option<String> {
    cardinal_number_text(page)
}

pub(super) fn ordinal_page_number_text(page: usize) -> Option<String> {
    ordinal_number_text(page)
}

fn dollar_page_number_text(page: usize) -> Option<String> {
    Some(format!("{} and 00/100", cardinal_number_text(page)?))
}

fn cardinal_number_text(number: usize) -> Option<String> {
    if number == 0 {
        return Some("zero".to_string());
    }
    cardinal_positive_number_text(number as u64)
}

fn cardinal_positive_number_text(number: u64) -> Option<String> {
    const SCALES: &[(u64, &str)] = &[
        (1_000_000_000_000, "trillion"),
        (1_000_000_000, "billion"),
        (1_000_000, "million"),
        (1_000, "thousand"),
    ];
    if number < 20 {
        return Some(SMALL_NUMBER_WORDS[number as usize].to_string());
    }
    if number < 100 {
        let tens = number / 10;
        let rest = number % 10;
        let tens_word = TENS_NUMBER_WORDS[tens as usize];
        return Some(if rest == 0 {
            tens_word.to_string()
        } else {
            format!("{tens_word}-{}", SMALL_NUMBER_WORDS[rest as usize])
        });
    }
    if number < 1_000 {
        let hundreds = number / 100;
        let rest = number % 100;
        let prefix = format!("{} hundred", SMALL_NUMBER_WORDS[hundreds as usize]);
        return Some(if rest == 0 {
            prefix
        } else {
            format!("{prefix} {}", cardinal_positive_number_text(rest)?)
        });
    }
    for (value, name) in SCALES {
        if number >= *value {
            let major = number / *value;
            let rest = number % *value;
            let prefix = format!("{} {name}", cardinal_positive_number_text(major)?);
            return Some(if rest == 0 {
                prefix
            } else {
                format!("{prefix} {}", cardinal_positive_number_text(rest)?)
            });
        }
    }
    None
}

fn ordinal_number_text(number: usize) -> Option<String> {
    ordinal_positive_number_text(number as u64)
}

fn ordinal_positive_number_text(number: u64) -> Option<String> {
    if number < 20 {
        return Some(SMALL_ORDINAL_WORDS[number as usize].to_string());
    }
    if number < 100 {
        let tens = number / 10;
        let rest = number % 10;
        let tens_word = TENS_NUMBER_WORDS[tens as usize];
        return Some(if rest == 0 {
            TENS_ORDINAL_WORDS[tens as usize].to_string()
        } else {
            format!("{tens_word}-{}", ordinal_positive_number_text(rest)?)
        });
    }
    if number < 1_000 {
        let hundreds = number / 100;
        let rest = number % 100;
        let prefix = format!("{} hundred", SMALL_NUMBER_WORDS[hundreds as usize]);
        return Some(if rest == 0 {
            format!("{prefix}th")
        } else {
            format!("{prefix} {}", ordinal_positive_number_text(rest)?)
        });
    }
    for (value, name) in [
        (1_000_000_000_000u64, "trillion"),
        (1_000_000_000, "billion"),
        (1_000_000, "million"),
        (1_000, "thousand"),
    ] {
        if number >= value {
            let major = number / value;
            let rest = number % value;
            let prefix = cardinal_positive_number_text(major)?;
            return Some(if rest == 0 {
                format!("{prefix} {name}th")
            } else {
                format!("{prefix} {name} {}", ordinal_positive_number_text(rest)?)
            });
        }
    }
    None
}

const SMALL_NUMBER_WORDS: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];

const SMALL_ORDINAL_WORDS: [&str; 20] = [
    "zeroth",
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
];

const TENS_NUMBER_WORDS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

const TENS_ORDINAL_WORDS: [&str; 10] = [
    "",
    "",
    "twentieth",
    "thirtieth",
    "fortieth",
    "fiftieth",
    "sixtieth",
    "seventieth",
    "eightieth",
    "ninetieth",
];
