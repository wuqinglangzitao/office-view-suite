use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DocumentInfoInstruction {
    property: DocumentInfoProperty,
    text_format: Option<FieldTextFormat>,
    number_format: Option<FieldNumberFormat>,
    date_format: Option<String>,
    file_size_unit: FileSizeUnit,
    user_override: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum DocumentInfoProperty {
    Title,
    Subject,
    Creator,
    Description,
    Keywords,
    Category,
    ContentStatus,
    LastModifiedBy,
    CreatedDate,
    SavedDate,
    PrintDate,
    Version,
    Revision,
    Custom(String),
    Variable(String),
    Extended(String),
    FileSize,
    UserName,
    UserInitials,
    UserAddress,
    DisplayOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSizeUnit {
    Bytes,
    Kilobytes,
    Megabytes,
}

pub(crate) fn computed_document_info_result(
    instruction: &str,
    core_properties: &CoreProperties,
    custom_properties: &HashMap<String, String>,
    document_variables: &HashMap<String, String>,
    extended_properties: &HashMap<String, String>,
    file_size_bytes: Option<usize>,
) -> Option<String> {
    let spec = document_info_instruction(instruction)?;
    let text = match spec.property {
        DocumentInfoProperty::Title => core_properties.title.clone()?,
        DocumentInfoProperty::Subject => core_properties.subject.clone()?,
        DocumentInfoProperty::Creator => core_properties.creator.clone()?,
        DocumentInfoProperty::Description => core_properties.description.clone()?,
        DocumentInfoProperty::Keywords => core_properties.keywords.clone()?,
        DocumentInfoProperty::Category => core_properties.category.clone()?,
        DocumentInfoProperty::ContentStatus => core_properties.content_status.clone()?,
        DocumentInfoProperty::LastModifiedBy => core_properties.last_modified_by.clone()?,
        DocumentInfoProperty::CreatedDate => core_properties.created.clone()?,
        DocumentInfoProperty::SavedDate => core_properties.modified.clone()?,
        DocumentInfoProperty::PrintDate => core_properties.last_printed.clone()?,
        DocumentInfoProperty::Version => core_properties.version.clone()?,
        DocumentInfoProperty::Revision => core_properties.revision.clone()?,
        DocumentInfoProperty::Custom(key) => custom_properties.get(&key).cloned()?,
        DocumentInfoProperty::Variable(key) => document_variables.get(&key).cloned()?,
        DocumentInfoProperty::Extended(key) => extended_properties.get(&key).cloned()?,
        DocumentInfoProperty::FileSize => {
            format_file_size_result(file_size_bytes?, spec.file_size_unit)
        }
        DocumentInfoProperty::UserName
        | DocumentInfoProperty::UserInitials
        | DocumentInfoProperty::UserAddress => spec.user_override.clone()?,
        DocumentInfoProperty::DisplayOnly => return None,
    };
    let text = match spec.date_format {
        Some(format) => format_core_timestamp(&text, &format)?,
        None => text,
    };
    let text = match spec.number_format {
        Some(format) => {
            let value = text.trim().parse::<usize>().ok()?;
            format_page_number(value, Some(page_number_format_from_field_format(format)))?
        }
        None => text,
    };
    Some(apply_field_text_format(text, spec.text_format))
}

fn is_numeric_document_info_property(property: &DocumentInfoProperty) -> bool {
    match property {
        DocumentInfoProperty::FileSize => true,
        DocumentInfoProperty::Extended(key) => matches!(
            key.as_str(),
            "PAGES"
                | "WORDS"
                | "CHARACTERS"
                | "CHARACTERSWITHSPACES"
                | "LINES"
                | "PARAGRAPHS"
                | "TOTALTIME"
        ),
        _ => false,
    }
}

pub(crate) fn supports_document_info_field_syntax(instruction: &str) -> bool {
    document_info_instruction(instruction).is_some()
}

pub(crate) fn supports_preserved_document_info_field_syntax(instruction: &str) -> bool {
    document_info_instruction(instruction)
        .is_some_and(|spec| is_preserved_document_info_property(&spec.property))
}

pub(crate) fn computed_preserved_document_info_result(
    instruction: &str,
    core_properties: &CoreProperties,
    custom_properties: &HashMap<String, String>,
) -> Option<String> {
    let spec = document_info_instruction(instruction)?;
    match &spec.property {
        DocumentInfoProperty::Custom(key) => {
            custom_properties.get(key)?;
        }
        property if is_core_document_info_property(property) => {
            let value = core_document_info_value(property, core_properties)?;
            if value.is_empty() || value.trim() != value {
                return None;
            }
        }
        _ => return None,
    }
    computed_document_info_result(
        instruction,
        core_properties,
        custom_properties,
        &HashMap::new(),
        &HashMap::new(),
        None,
    )
}

fn is_preserved_document_info_property(property: &DocumentInfoProperty) -> bool {
    is_core_document_info_property(property) || matches!(property, DocumentInfoProperty::Custom(_))
}

fn is_core_document_info_property(property: &DocumentInfoProperty) -> bool {
    matches!(
        property,
        DocumentInfoProperty::Title
            | DocumentInfoProperty::Subject
            | DocumentInfoProperty::Creator
            | DocumentInfoProperty::Description
            | DocumentInfoProperty::Keywords
            | DocumentInfoProperty::Category
            | DocumentInfoProperty::ContentStatus
            | DocumentInfoProperty::LastModifiedBy
            | DocumentInfoProperty::CreatedDate
            | DocumentInfoProperty::SavedDate
            | DocumentInfoProperty::PrintDate
            | DocumentInfoProperty::Version
            | DocumentInfoProperty::Revision
    )
}

fn core_document_info_value<'a>(
    property: &DocumentInfoProperty,
    core_properties: &'a CoreProperties,
) -> Option<&'a str> {
    match property {
        DocumentInfoProperty::Title => core_properties.title.as_deref(),
        DocumentInfoProperty::Subject => core_properties.subject.as_deref(),
        DocumentInfoProperty::Creator => core_properties.creator.as_deref(),
        DocumentInfoProperty::Description => core_properties.description.as_deref(),
        DocumentInfoProperty::Keywords => core_properties.keywords.as_deref(),
        DocumentInfoProperty::Category => core_properties.category.as_deref(),
        DocumentInfoProperty::ContentStatus => core_properties.content_status.as_deref(),
        DocumentInfoProperty::LastModifiedBy => core_properties.last_modified_by.as_deref(),
        DocumentInfoProperty::CreatedDate => core_properties.created.as_deref(),
        DocumentInfoProperty::SavedDate => core_properties.modified.as_deref(),
        DocumentInfoProperty::PrintDate => core_properties.last_printed.as_deref(),
        DocumentInfoProperty::Version => core_properties.version.as_deref(),
        DocumentInfoProperty::Revision => core_properties.revision.as_deref(),
        _ => None,
    }
}

// Deterministic-given-inputs evaluation for volatile document-info fields: the
// caller-supplied FieldContext is the input, so identical document + context
// always yields identical results. Explicit literal overrides win over context.
pub(crate) fn computed_context_document_info_result(
    kind: &str,
    instruction: &str,
    context: &crate::annotation::FieldContext,
) -> Option<String> {
    let spec = document_info_instruction(instruction)?;
    let text = match spec.property {
        DocumentInfoProperty::DisplayOnly
            if kind.eq_ignore_ascii_case("DATE") || kind.eq_ignore_ascii_case("TIME") =>
        {
            // A pictureless DATE/TIME needs a locale-default picture rwml
            // does not guess; require the explicit `\@` picture.
            spec.date_format.as_deref()?;
            context.now.clone()?
        }
        DocumentInfoProperty::UserName if spec.user_override.is_none() => {
            context.user_name.clone()?
        }
        DocumentInfoProperty::UserInitials if spec.user_override.is_none() => {
            context.user_initials.clone()?
        }
        DocumentInfoProperty::UserAddress if spec.user_override.is_none() => {
            context.user_address.clone()?
        }
        _ => return None,
    };
    let text = match spec.date_format {
        Some(format) => format_core_timestamp(&text, &format)?,
        None => text,
    };
    Some(apply_field_text_format(text, spec.text_format))
}

pub(super) fn document_info_instruction(instruction: &str) -> Option<DocumentInfoInstruction> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str).peekable();
    let kind = parts.next()?;
    let mut text_format = None;
    let mut number_format = None;
    let mut date_format = None;
    let mut file_size_unit = FileSizeUnit::Bytes;
    let mut file_size_unit_seen = false;
    let mut user_override = None;
    let property = if kind.eq_ignore_ascii_case("DOCPROPERTY") {
        let name = field_name_operand(parts.next()?, &mut parts)?;
        doc_property_instruction_property(&name)?
    } else if kind.eq_ignore_ascii_case("DOCVARIABLE") {
        let name = field_name_operand(parts.next()?, &mut parts)?;
        (!name.is_empty()).then(|| DocumentInfoProperty::Variable(document_property_key(&name)))?
    } else if kind.eq_ignore_ascii_case("INFO") {
        document_info_property(field_name_token(parts.next()?)?)
            .unwrap_or(DocumentInfoProperty::DisplayOnly)
    } else if let Some(property) = user_info_property(kind) {
        property
    } else if kind.eq_ignore_ascii_case("DATE") || kind.eq_ignore_ascii_case("TIME") {
        DocumentInfoProperty::DisplayOnly
    } else {
        document_info_property(kind)?
    };
    let numeric = is_numeric_document_info_property(&property);
    while let Some(part) = parts.next() {
        // Numeric document-info properties accept `\*` number-format switches like
        // their PAGE/SECTION/SEQ siblings; string properties keep rejecting them.
        let switch = if numeric {
            accept_field_format_or_number_switch_for_tail(
                part,
                &mut parts,
                &mut text_format,
                &mut number_format,
            )
        } else {
            accept_field_format_switch_for_tail(part, &mut parts, &mut text_format)
        };
        match switch {
            Some(true) => continue,
            None => return None,
            Some(false) => {}
        }
        if part.eq_ignore_ascii_case("\\@") {
            if date_format.is_some() {
                return None;
            }
            date_format = Some(document_info_date_format_literal(
                parts.next()?,
                &mut parts,
            )?);
            continue;
        }
        if let Some(format) = strip_ascii_switch_prefix(part, "\\@") {
            if date_format.is_some() {
                return None;
            }
            date_format = Some(document_info_date_format_literal(format, &mut parts)?);
            continue;
        }
        if let Some(unit) = file_size_unit_switch(part) {
            if !matches!(&property, DocumentInfoProperty::FileSize) || file_size_unit_seen {
                return None;
            }
            file_size_unit = unit;
            file_size_unit_seen = true;
            continue;
        }
        if is_user_info_property(&property) && !part.starts_with('\\') {
            let value = document_info_literal_operand(part, &mut parts)?;
            if user_override.replace(value).is_some() {
                return None;
            }
            continue;
        }
        return None;
    }
    Some(DocumentInfoInstruction {
        property,
        text_format,
        number_format,
        date_format,
        file_size_unit,
        user_override,
    })
}

fn file_size_unit_switch(part: &str) -> Option<FileSizeUnit> {
    if part.eq_ignore_ascii_case("\\k") {
        return Some(FileSizeUnit::Kilobytes);
    }
    if part.eq_ignore_ascii_case("\\m") {
        return Some(FileSizeUnit::Megabytes);
    }
    None
}

fn format_file_size_result(bytes: usize, unit: FileSizeUnit) -> String {
    match unit {
        FileSizeUnit::Bytes => bytes.to_string(),
        FileSizeUnit::Kilobytes => rounded_file_size_unit(bytes, 1_000).to_string(),
        FileSizeUnit::Megabytes => rounded_file_size_unit(bytes, 1_000_000).to_string(),
    }
}

fn rounded_file_size_unit(bytes: usize, divisor: usize) -> usize {
    bytes.saturating_add(divisor / 2) / divisor
}

fn user_info_property(value: &str) -> Option<DocumentInfoProperty> {
    Some(match value.to_ascii_uppercase().as_str() {
        "USERNAME" => DocumentInfoProperty::UserName,
        "USERINITIALS" => DocumentInfoProperty::UserInitials,
        "USERADDRESS" => DocumentInfoProperty::UserAddress,
        _ => return None,
    })
}

fn is_user_info_property(property: &DocumentInfoProperty) -> bool {
    matches!(
        property,
        DocumentInfoProperty::UserName
            | DocumentInfoProperty::UserInitials
            | DocumentInfoProperty::UserAddress
    )
}

fn doc_property_instruction_property(value: &str) -> Option<DocumentInfoProperty> {
    document_info_property(value).or_else(|| {
        (!value.is_empty()).then(|| DocumentInfoProperty::Custom(document_property_key(value)))
    })
}

fn document_info_property(value: &str) -> Option<DocumentInfoProperty> {
    Some(match document_property_key(value).as_str() {
        "TITLE" => DocumentInfoProperty::Title,
        "SUBJECT" => DocumentInfoProperty::Subject,
        "AUTHOR" | "CREATOR" => DocumentInfoProperty::Creator,
        "COMMENTS" | "COMMENT" | "DESCRIPTION" => DocumentInfoProperty::Description,
        "KEYWORDS" | "KEYWORD" => DocumentInfoProperty::Keywords,
        "CATEGORY" => DocumentInfoProperty::Category,
        "CONTENTSTATUS" => DocumentInfoProperty::ContentStatus,
        "LASTSAVEDBY" | "LASTMODIFIEDBY" | "LASTAUTHOR" => DocumentInfoProperty::LastModifiedBy,
        "CREATEDATE" => DocumentInfoProperty::CreatedDate,
        "SAVEDATE" => DocumentInfoProperty::SavedDate,
        "PRINTDATE" => DocumentInfoProperty::PrintDate,
        "VERSION" => DocumentInfoProperty::Version,
        "REVISIONNUMBER" => DocumentInfoProperty::Revision,
        "FILESIZE" => DocumentInfoProperty::FileSize,
        "APPLICATION" => DocumentInfoProperty::Extended(document_property_key("Application")),
        "APPVERSION" => DocumentInfoProperty::Extended(document_property_key("AppVersion")),
        "COMPANY" => DocumentInfoProperty::Extended(document_property_key("Company")),
        "DOCSECURITY" => DocumentInfoProperty::Extended(document_property_key("DocSecurity")),
        "HIDDENSLIDES" => DocumentInfoProperty::Extended(document_property_key("HiddenSlides")),
        "HYPERLINKBASE" => DocumentInfoProperty::Extended(document_property_key("HyperlinkBase")),
        "HYPERLINKSCHANGED" => {
            DocumentInfoProperty::Extended(document_property_key("HyperlinksChanged"))
        }
        "LINES" => DocumentInfoProperty::Extended(document_property_key("Lines")),
        "LINKSUPTODATE" => DocumentInfoProperty::Extended(document_property_key("LinksUpToDate")),
        "MANAGER" => DocumentInfoProperty::Extended(document_property_key("Manager")),
        "MMCLIPS" => DocumentInfoProperty::Extended(document_property_key("MMClips")),
        "NOTES" => DocumentInfoProperty::Extended(document_property_key("Notes")),
        "PAGES" | "NUMPAGES" => DocumentInfoProperty::Extended(document_property_key("Pages")),
        "PARAGRAPHS" => DocumentInfoProperty::Extended(document_property_key("Paragraphs")),
        "PRESENTATIONFORMAT" => {
            DocumentInfoProperty::Extended(document_property_key("PresentationFormat"))
        }
        "SCALECROP" => DocumentInfoProperty::Extended(document_property_key("ScaleCrop")),
        "SHAREDDOC" => DocumentInfoProperty::Extended(document_property_key("SharedDoc")),
        "SLIDES" => DocumentInfoProperty::Extended(document_property_key("Slides")),
        "WORDS" | "NUMWORDS" => DocumentInfoProperty::Extended(document_property_key("Words")),
        "CHARACTERS" | "NUMCHARS" => {
            DocumentInfoProperty::Extended(document_property_key("Characters"))
        }
        "CHARACTERSWITHSPACES" => {
            DocumentInfoProperty::Extended(document_property_key("CharactersWithSpaces"))
        }
        "TOTALTIME" | "EDITTIME" => {
            DocumentInfoProperty::Extended(document_property_key("TotalTime"))
        }
        "TEMPLATE" => DocumentInfoProperty::Extended(document_property_key("Template")),
        _ => return None,
    })
}

fn document_info_date_format_literal<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    document_info_literal_operand(first, parts)
}

fn document_info_literal_operand<'a>(
    first: &'a str,
    parts: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>,
) -> Option<String> {
    if let Some(format) = field_quoted_literal_token(first) {
        return (!format.is_empty()).then(|| format.to_string());
    }
    let mut values = vec![field_non_empty_non_switch_literal_token(first)?];
    while let Some(part) = parts.peek().copied() {
        if part.starts_with('\\') {
            break;
        }
        values.push(field_non_empty_non_switch_literal_token(parts.next()?)?);
    }
    Some(values.join(" "))
}

pub(crate) fn computed_revision_number_result(
    instruction: &str,
    core_properties: &CoreProperties,
) -> Option<String> {
    let text_format = revision_number_field_text_format(instruction)?;
    let revision = core_properties.revision.clone()?;
    Some(apply_field_text_format(revision, text_format))
}

pub(crate) fn computed_preserved_revision_number_result(
    instruction: &str,
    core_properties: &CoreProperties,
) -> Option<String> {
    let revision = core_properties.revision.as_deref()?;
    if revision.is_empty() || revision.trim() != revision {
        return None;
    }
    computed_revision_number_result(instruction, core_properties)
}

pub(crate) fn supports_revision_number_field_syntax(instruction: &str) -> bool {
    revision_number_field_text_format(instruction).is_some()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CoreTimestamp {
    year: i32,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

const MONTH_SHORT_NAMES: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const MONTH_LONG_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAY_SHORT_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const WEEKDAY_LONG_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

fn format_core_timestamp(value: &str, format: &str) -> Option<String> {
    let timestamp = parse_core_timestamp(value)?;
    let mut out = String::new();
    let chars: Vec<char> = format.chars().collect();
    let mut i = 0usize;
    while i < chars.len() {
        let ch = chars[i];
        if ch == '\'' {
            i += 1;
            while i < chars.len() && chars[i] != '\'' {
                out.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1;
            }
            continue;
        }
        if starts_with_ci(&chars, i, "AM/PM") {
            out.push_str(if timestamp.hour < 12 { "AM" } else { "PM" });
            i += 5;
            continue;
        }
        let run = repeated_run(&chars, i, ch);
        match ch {
            'y' | 'Y' => {
                if run >= 4 {
                    out.push_str(&format!("{:04}", timestamp.year));
                } else if run == 2 {
                    out.push_str(&format!("{:02}", timestamp.year.rem_euclid(100)));
                } else {
                    return None;
                }
            }
            'M' => match run {
                1 | 2 => push_numeric(&mut out, timestamp.month as u32, run)?,
                3 => out.push_str(MONTH_SHORT_NAMES.get(timestamp.month as usize - 1)?),
                4 => out.push_str(MONTH_LONG_NAMES.get(timestamp.month as usize - 1)?),
                _ => return None,
            },
            'd' | 'D' => match run {
                1 | 2 => push_numeric(&mut out, timestamp.day as u32, run)?,
                3 => out.push_str(WEEKDAY_SHORT_NAMES.get(weekday_index(&timestamp)?)?),
                4 => out.push_str(WEEKDAY_LONG_NAMES.get(weekday_index(&timestamp)?)?),
                _ => return None,
            },
            'H' => push_numeric(&mut out, timestamp.hour as u32, run)?,
            'h' => {
                let hour = timestamp.hour % 12;
                push_numeric(&mut out, if hour == 0 { 12 } else { hour } as u32, run)?;
            }
            'm' => push_numeric(&mut out, timestamp.minute as u32, run)?,
            's' | 'S' => push_numeric(&mut out, timestamp.second as u32, run)?,
            _ => {
                out.push(ch);
                i += 1;
                continue;
            }
        }
        i += run;
    }
    Some(out)
}

fn parse_core_timestamp(value: &str) -> Option<CoreTimestamp> {
    let value = value.trim();
    let date = value.get(0..10)?;
    let year = date.get(0..4)?.parse::<i32>().ok()?;
    (date.get(4..5)? == "-" && date.get(7..8)? == "-").then_some(())?;
    let month = date.get(5..7)?.parse::<u8>().ok()?;
    let day = date.get(8..10)?.parse::<u8>().ok()?;
    let mut hour = 0u8;
    let mut minute = 0u8;
    let mut second = 0u8;
    if value.len() >= 19 && matches!(value.as_bytes().get(10), Some(b'T' | b' ')) {
        hour = value.get(11..13)?.parse::<u8>().ok()?;
        minute = value.get(14..16)?.parse::<u8>().ok()?;
        second = value.get(17..19)?.parse::<u8>().ok()?;
        (value.get(13..14)? == ":" && value.get(16..17)? == ":").then_some(())?;
    }
    ((1..=12).contains(&month)
        && day >= 1
        && day <= days_in_month(year, month)?
        && hour <= 23
        && minute <= 59
        && second <= 59)
        .then_some(CoreTimestamp {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
}

fn days_in_month(year: i32, month: u8) -> Option<u8> {
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => return None,
    })
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn weekday_index(timestamp: &CoreTimestamp) -> Option<usize> {
    let month_offset = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let month_index = timestamp.month.checked_sub(1)? as usize;
    let mut year = timestamp.year;
    if timestamp.month < 3 {
        year -= 1;
    }
    Some(
        (year + year / 4 - year / 100
            + year / 400
            + month_offset[month_index]
            + timestamp.day as i32)
            .rem_euclid(7) as usize,
    )
}

fn repeated_run(chars: &[char], start: usize, ch: char) -> usize {
    chars[start..]
        .iter()
        .take_while(|next| **next == ch)
        .count()
}

fn starts_with_ci(chars: &[char], start: usize, pattern: &str) -> bool {
    let Some(slice) = chars.get(start..start + pattern.chars().count()) else {
        return false;
    };
    slice
        .iter()
        .zip(pattern.chars())
        .all(|(actual, expected)| actual.eq_ignore_ascii_case(&expected))
}

fn push_numeric(out: &mut String, value: u32, width: usize) -> Option<()> {
    match width {
        1 => out.push_str(&value.to_string()),
        2 => out.push_str(&format!("{value:02}")),
        _ => return None,
    }
    Some(())
}
