use super::*;

pub(super) fn unquote_field_text(text: &str) -> String {
    let bytes = text.as_bytes();
    if bytes.len() >= 2 && bytes[0] == b'"' && *bytes.last().unwrap_or(&0) == b'"' {
        text[1..text.len() - 1].to_string()
    } else {
        text.to_string()
    }
}

pub(crate) fn computed_display_result(instruction: &str) -> Option<String> {
    computed_advance_result(instruction)
        .or_else(|| computed_eq_result(instruction))
        .or_else(|| computed_symbol_result(instruction))
}

pub(crate) fn supports_display_field_syntax(instruction: &str) -> bool {
    if computed_display_result(instruction).is_some() {
        return true;
    }
    if symbol_field_syntax(instruction).is_some() {
        return true;
    }
    let Some(spec) = eq_instruction(instruction) else {
        return false;
    };
    supports_eq_displace_syntax(&spec.expression) || supports_eq_script_syntax(&spec.expression)
}

pub(crate) fn supports_computed_symbol_field_syntax(instruction: &str) -> bool {
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Display(ref kind) if kind == "SYMBOL"
    ) && computed_symbol_result(instruction).is_some()
}

pub(crate) fn supports_context_free_display_field_syntax(instruction: &str) -> bool {
    matches!(
        FieldKind::from_instruction(instruction),
        FieldKind::Display(ref kind) if kind == "EQ" || kind == "ADVANCE"
    ) && computed_display_result(instruction).is_some()
}

fn computed_advance_result(instruction: &str) -> Option<String> {
    advance_field_syntax(instruction).then_some(())?;
    Some(String::new())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EqInstruction {
    expression: String,
    text_format: Option<FieldTextFormat>,
}

fn computed_eq_result(instruction: &str) -> Option<String> {
    let spec = eq_instruction(instruction)?;
    Some(apply_field_text_format(
        eq_display_text(&spec.expression)?,
        spec.text_format,
    ))
}

fn eq_instruction(instruction: &str) -> Option<EqInstruction> {
    let tokens = instruction_parts(instruction);
    let mut parts = tokens.iter().map(String::as_str);
    let kind = parts.next()?;
    if !kind.eq_ignore_ascii_case("EQ") {
        return None;
    }
    let mut expression_parts = Vec::new();
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if accept_field_format_switch_for_tail(part, &mut parts, &mut text_format)? {
            continue;
        }
        expression_parts.push(part);
    }
    if expression_parts.is_empty() {
        return None;
    }
    Some(EqInstruction {
        expression: expression_parts.join(" "),
        text_format,
    })
}

fn eq_display_text(expression: &str) -> Option<String> {
    eq_displace_text(expression)
        .or_else(|| eq_array_text(expression))
        .or_else(|| eq_fraction_text(expression))
        .or_else(|| eq_radical_text(expression))
        .or_else(|| eq_script_text(expression))
        .or_else(|| eq_integral_text(expression))
        .or_else(|| eq_overstrike_text(expression))
        .or_else(|| eq_bracket_text(expression))
        .or_else(|| eq_box_text(expression))
        .or_else(|| eq_list_text(expression))
}

fn eq_displace_text(expression: &str) -> Option<String> {
    let mut body = strip_ascii_switch_prefix(expression, "\\d")?.trim_start();
    let mut has_option = false;
    loop {
        if let Some((_value, rest)) = consume_eq_numeric_prefix_option(body, "\\fo")
            .or_else(|| consume_eq_numeric_prefix_option(body, "\\ba"))
        {
            has_option = true;
            body = rest.trim_start();
        } else if let Some(rest) = consume_eq_prefix_switch(body, "\\li") {
            has_option = true;
            body = rest.trim_start();
        } else {
            break;
        }
    }
    if !has_option {
        return None;
    }
    let inner = take_eq_enclosed_operand(body)?;
    if inner.trim().is_empty() {
        Some(String::new())
    } else {
        eq_operand_text(inner)
    }
}

fn supports_eq_displace_syntax(expression: &str) -> bool {
    let Some(mut body) = strip_ascii_switch_prefix(expression, "\\d") else {
        return false;
    };
    body = body.trim_start();
    let mut has_option = false;
    loop {
        if let Some((_value, rest)) = consume_eq_numeric_prefix_option(body, "\\fo")
            .or_else(|| consume_eq_numeric_prefix_option(body, "\\ba"))
        {
            has_option = true;
            body = rest.trim_start();
        } else if let Some(rest) = consume_eq_prefix_switch(body, "\\li") {
            has_option = true;
            body = rest.trim_start();
        } else {
            break;
        }
    }
    let Some(inner) = take_eq_enclosed_operand(body) else {
        return false;
    };
    has_option && (inner.trim().is_empty() || eq_operand_text(inner).is_some())
}

fn eq_array_text(expression: &str) -> Option<String> {
    let mut body = strip_ascii_switch_prefix(expression, "\\a")?.trim_start();
    let mut columns = 1usize;
    loop {
        if let Some(rest) = consume_eq_prefix_switch(body, "\\al")
            .or_else(|| consume_eq_prefix_switch(body, "\\ac"))
            .or_else(|| consume_eq_prefix_switch(body, "\\ar"))
        {
            body = rest.trim_start();
        } else if let Some((value, rest)) = consume_eq_numeric_prefix_option(body, "\\co") {
            columns = eq_column_count(value)?;
            body = rest.trim_start();
        } else if let Some((_value, rest)) = consume_eq_numeric_prefix_option(body, "\\vs")
            .or_else(|| consume_eq_numeric_prefix_option(body, "\\hs"))
        {
            body = rest.trim_start();
        } else {
            break;
        }
    }
    let inner = body.strip_prefix('(')?.strip_suffix(')')?;
    let operands = split_eq_list_operands(inner)?;
    let mut cells = Vec::with_capacity(operands.len());
    for operand in operands {
        cells.push(eq_operand_text(operand)?);
    }
    Some(
        cells
            .chunks(columns)
            .map(|row| row.join("\t"))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn eq_fraction_text(expression: &str) -> Option<String> {
    let body = strip_ascii_switch_prefix(expression, "\\f")?;
    let inner = body.strip_prefix('(')?.strip_suffix(')')?;
    let (numerator, denominator) = split_eq_fraction_operands(inner)?;
    Some(format!(
        "{}/{}",
        eq_operand_text(numerator)?,
        eq_operand_text(denominator)?
    ))
}

fn eq_radical_text(expression: &str) -> Option<String> {
    let body = strip_ascii_switch_prefix(expression, "\\r")?;
    let inner = body.strip_prefix('(')?.strip_suffix(')')?;
    let operands = split_eq_radical_operands(inner)?;
    let text = match operands {
        (radicand, None) => format!("√{}", eq_operand_text(radicand)?),
        (degree, Some(radicand)) => {
            format!(
                "{}√{}",
                eq_operand_text(degree)?,
                eq_operand_text(radicand)?
            )
        }
    };
    Some(text)
}

fn eq_script_text(expression: &str) -> Option<String> {
    let mut body = expression.trim_start();
    let mut text = String::new();
    loop {
        let rest = strip_ascii_switch_prefix(body, "\\s")?;
        let (segment, remaining) = eq_script_segment(rest.trim_start())?;
        text.push_str(&segment);
        body = remaining.trim_start();
        if body.is_empty() {
            return Some(text);
        }
    }
}

fn supports_eq_script_syntax(expression: &str) -> bool {
    let mut body = expression.trim_start();
    loop {
        let Some(rest) = strip_ascii_switch_prefix(body, "\\s") else {
            return false;
        };
        let Some(remaining) = eq_script_syntax_segment(rest.trim_start()) else {
            return false;
        };
        body = remaining.trim_start();
        if body.is_empty() {
            return true;
        }
    }
}

fn eq_script_syntax_segment(mut body: &str) -> Option<&str> {
    let mut saw_option = false;
    loop {
        if body.is_empty() || consume_eq_prefix_switch(body, "\\s").is_some() {
            return saw_option.then_some(body);
        }
        if let Some(rest) = consume_eq_script_syntax_option(body, "\\up", false)
            .or_else(|| consume_eq_script_syntax_option(body, "\\do", false))
            .or_else(|| consume_eq_script_syntax_option(body, "\\ai", true))
            .or_else(|| consume_eq_script_syntax_option(body, "\\di", true))
        {
            body = rest.trim_start();
            saw_option = true;
            continue;
        }
        return None;
    }
}

fn consume_eq_script_syntax_option<'a>(
    value: &'a str,
    option: &str,
    allow_empty: bool,
) -> Option<&'a str> {
    let (_points, rest) = consume_eq_numeric_prefix_option(value, option)?;
    let (operand, rest) = take_eq_parenthesized_operand(rest)?;
    if operand.trim().is_empty() {
        return allow_empty.then_some(rest);
    }
    eq_operand_text(operand)?;
    Some(rest)
}

fn eq_script_segment(mut body: &str) -> Option<(String, &str)> {
    let mut text = String::new();
    let mut saw_option = false;
    loop {
        if body.is_empty() || consume_eq_prefix_switch(body, "\\s").is_some() {
            return saw_option.then_some((text, body));
        }
        if let Some((marker, segment, rest)) = consume_eq_script_visible_option(body, "\\up", '^')
            .or_else(|| consume_eq_script_visible_option(body, "\\do", '_'))
        {
            text.push_str(&eq_script_marker(marker, eq_operand_text(segment)?));
            body = rest.trim_start();
            saw_option = true;
            continue;
        }
        if let Some((segment, rest)) = consume_eq_script_layout_option(body, "\\ai")
            .or_else(|| consume_eq_script_layout_option(body, "\\di"))
        {
            if !segment.trim().is_empty() {
                text.push_str(&eq_operand_text(segment)?);
            }
            saw_option = true;
            body = rest.trim_start();
            continue;
        }
        return None;
    }
}

fn consume_eq_script_visible_option<'a>(
    value: &'a str,
    option: &str,
    marker: char,
) -> Option<(char, &'a str, &'a str)> {
    let (_points, rest) = consume_eq_numeric_prefix_option(value, option)?;
    let (operand, rest) = take_eq_parenthesized_operand(rest)?;
    Some((marker, operand, rest))
}

fn consume_eq_script_layout_option<'a>(value: &'a str, option: &str) -> Option<(&'a str, &'a str)> {
    let (_points, rest) = consume_eq_numeric_prefix_option(value, option)?;
    let (operand, rest) = take_eq_parenthesized_operand(rest)?;
    Some((operand, rest))
}

fn eq_script_marker(marker: char, text: String) -> String {
    if text.chars().count() == 1 {
        format!("{marker}{text}")
    } else {
        format!("{marker}{{{text}}}")
    }
}

fn eq_integral_text(expression: &str) -> Option<String> {
    let mut body = strip_ascii_switch_prefix(expression, "\\i")?.trim_start();
    let mut symbol = '∫';
    loop {
        if let Some(rest) = consume_eq_prefix_switch(body, "\\su") {
            symbol = 'Σ';
            body = rest.trim_start();
        } else if let Some(rest) = consume_eq_prefix_switch(body, "\\pr") {
            symbol = 'Π';
            body = rest.trim_start();
        } else if let Some(rest) = consume_eq_prefix_switch(body, "\\in") {
            body = rest.trim_start();
        } else if let Some((ch, rest)) = consume_eq_bracket_option(body, "\\fc")
            .or_else(|| consume_eq_bracket_option(body, "\\vc"))
        {
            symbol = ch;
            body = rest.trim_start();
        } else {
            break;
        }
    }
    let inner = body.strip_prefix('(')?.strip_suffix(')')?;
    let operands = split_eq_list_operands(inner)?;
    if operands.len() != 3 {
        return None;
    }
    let lower = eq_operand_text(operands[0])?;
    let upper = eq_operand_text(operands[1])?;
    let integrand = eq_operand_text(operands[2])?;
    Some(format!(
        "{}_{}^{} {}",
        symbol,
        eq_limit_text(lower),
        eq_limit_text(upper),
        integrand
    ))
}

fn eq_limit_text(text: String) -> String {
    if text.chars().count() == 1 {
        text
    } else {
        format!("{{{text}}}")
    }
}

fn eq_overstrike_text(expression: &str) -> Option<String> {
    let mut body = strip_ascii_switch_prefix(expression, "\\o")?.trim_start();
    while let Some(rest) = consume_eq_prefix_switch(body, "\\al")
        .or_else(|| consume_eq_prefix_switch(body, "\\ac"))
        .or_else(|| consume_eq_prefix_switch(body, "\\ar"))
    {
        body = rest.trim_start();
    }
    let inner = body.strip_prefix('(')?.strip_suffix(')')?;
    let operands = split_eq_list_operands(inner)?;
    let mut text = String::new();
    for operand in operands {
        text.push_str(&eq_operand_text(operand)?);
    }
    (!text.is_empty()).then_some(text)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EqBracketPair {
    left: char,
    right: char,
}

fn eq_bracket_text(expression: &str) -> Option<String> {
    let mut body = strip_ascii_switch_prefix(expression, "\\b")?.trim_start();
    let mut brackets = EqBracketPair {
        left: '(',
        right: ')',
    };
    loop {
        if let Some((ch, rest)) = consume_eq_bracket_option(body, "\\bc") {
            brackets = eq_matching_brackets(ch);
            body = rest.trim_start();
        } else if let Some((ch, rest)) = consume_eq_bracket_option(body, "\\lc") {
            brackets.left = ch;
            body = rest.trim_start();
        } else if let Some((ch, rest)) = consume_eq_bracket_option(body, "\\rc") {
            brackets.right = ch;
            body = rest.trim_start();
        } else {
            break;
        }
    }
    let inner = take_eq_enclosed_operand(body)?;
    Some(format!(
        "{}{}{}",
        brackets.left,
        eq_operand_text(inner)?,
        brackets.right
    ))
}

fn eq_box_text(expression: &str) -> Option<String> {
    let inner = eq_enclosed_operand_with_prefix_switches(
        expression,
        "\\x",
        &["\\to", "\\bo", "\\le", "\\ri"],
    )?;
    eq_operand_text(inner)
}

fn eq_list_text(expression: &str) -> Option<String> {
    let body = strip_ascii_switch_prefix(expression, "\\l")?.trim_start();
    let inner = body.strip_prefix('(')?.strip_suffix(')')?;
    let operands = split_eq_list_operands(inner)?;
    let mut parts = Vec::with_capacity(operands.len());
    for operand in operands {
        parts.push(eq_operand_text(operand)?);
    }
    (!parts.is_empty()).then_some(parts.join(","))
}

fn consume_eq_bracket_option<'a>(value: &'a str, option: &str) -> Option<(char, &'a str)> {
    let rest = strip_ascii_switch_prefix(value, option)?;
    consume_eq_bracket_char(rest)
}

fn consume_eq_bracket_char(value: &str) -> Option<(char, &str)> {
    let rest = value.trim_start();
    let rest = rest.strip_prefix('\\').unwrap_or(rest);
    let ch = rest.chars().next()?;
    if ch.is_whitespace() {
        return None;
    }
    Some((ch, &rest[ch.len_utf8()..]))
}

fn eq_matching_brackets(ch: char) -> EqBracketPair {
    match ch {
        '{' => EqBracketPair {
            left: '{',
            right: '}',
        },
        '[' => EqBracketPair {
            left: '[',
            right: ']',
        },
        '(' => EqBracketPair {
            left: '(',
            right: ')',
        },
        '<' => EqBracketPair {
            left: '<',
            right: '>',
        },
        _ => EqBracketPair {
            left: ch,
            right: ch,
        },
    }
}

fn eq_enclosed_operand_with_prefix_switches<'a>(
    expression: &'a str,
    switch: &str,
    options: &[&str],
) -> Option<&'a str> {
    let mut body = strip_ascii_switch_prefix(expression, switch)?.trim_start();
    loop {
        let mut consumed = false;
        for option in options {
            if let Some(rest) = consume_eq_prefix_switch(body, option) {
                body = rest.trim_start();
                consumed = true;
                break;
            }
        }
        if !consumed {
            break;
        }
    }
    take_eq_enclosed_operand(body)
}

fn eq_column_count(value: f32) -> Option<usize> {
    (value.fract() == 0.0 && value >= 1.0 && value <= usize::MAX as f32).then_some(value as usize)
}

fn eq_operand_text(operand: &str) -> Option<String> {
    let operand = operand.trim();
    if operand.is_empty() {
        return None;
    }
    let text = unquote_field_text(operand);
    if let Some(nested) = eq_display_text(&text) {
        return Some(format!("({nested})"));
    }
    let text = unescape_eq_literal_operand(&text)?;
    (!text.is_empty()).then_some(text)
}

fn unescape_eq_literal_operand(operand: &str) -> Option<String> {
    let mut out = String::with_capacity(operand.len());
    let mut chars = operand.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next()? {
            ',' => out.push(','),
            ';' => out.push(';'),
            '(' => out.push('('),
            ')' => out.push(')'),
            '\\' => out.push('\\'),
            _ => return None,
        }
    }
    Some(out)
}

fn computed_symbol_result(instruction: &str) -> Option<String> {
    let spec = symbol_field_syntax(instruction)?;
    let font = spec.font.as_deref();
    let font_code = legacy_symbol_font_code(font, spec.code);
    let text = if spec.unicode {
        char::from_u32(spec.code)?.to_string()
    } else if symbol_font_matches(font, "symbol") {
        symbol_font_char(font_code)?.to_string()
    } else if symbol_font_matches(font, "wingdings") {
        wingdings_font_char(font_code)?.to_string()
    } else {
        ansi_char(spec.code)?.to_string()
    };
    Some(apply_field_text_format(text, spec.text_format))
}

pub(crate) fn computed_run_symbol_char(font: Option<&str>, value: &str) -> Option<char> {
    let code = run_symbol_code(value)?;
    let font_code = legacy_symbol_font_code(font, code);

    if symbol_font_matches(font, "symbol") {
        symbol_font_char(font_code)
    } else if symbol_font_matches(font, "wingdings") {
        wingdings_font_char(font_code)
    } else {
        char::from_u32(code)
    }
}

fn legacy_symbol_font_code(font: Option<&str>, code: u32) -> u32 {
    let legacy_font = symbol_font_matches(font, "symbol") || symbol_font_matches(font, "wingdings");
    if legacy_font && code > 0xFF {
        code & 0xFF
    } else {
        code
    }
}

fn run_symbol_code(value: &str) -> Option<u32> {
    let value = value.trim();
    let hex = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    (!hex.is_empty() && hex.chars().all(|ch| ch.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(hex, 16).ok())
        .flatten()
}

fn ansi_char(code: u32) -> Option<char> {
    match code {
        0x80 => Some('\u{20AC}'),
        0x82 => Some('\u{201A}'),
        0x83 => Some('\u{0192}'),
        0x84 => Some('\u{201E}'),
        0x85 => Some('\u{2026}'),
        0x86 => Some('\u{2020}'),
        0x87 => Some('\u{2021}'),
        0x88 => Some('\u{02C6}'),
        0x89 => Some('\u{2030}'),
        0x8A => Some('\u{0160}'),
        0x8B => Some('\u{2039}'),
        0x8C => Some('\u{0152}'),
        0x8E => Some('\u{017D}'),
        0x91 => Some('\u{2018}'),
        0x92 => Some('\u{2019}'),
        0x93 => Some('\u{201C}'),
        0x94 => Some('\u{201D}'),
        0x95 => Some('\u{2022}'),
        0x96 => Some('\u{2013}'),
        0x97 => Some('\u{2014}'),
        0x98 => Some('\u{02DC}'),
        0x99 => Some('\u{2122}'),
        0x9A => Some('\u{0161}'),
        0x9B => Some('\u{203A}'),
        0x9C => Some('\u{0153}'),
        0x9E => Some('\u{017E}'),
        0x9F => Some('\u{0178}'),
        _ => char::from_u32(code),
    }
}

fn symbol_font_matches(font: Option<&str>, expected: &str) -> bool {
    let Some(font) = font else {
        return false;
    };
    font.chars()
        .filter(|ch| !ch.is_whitespace() && *ch != '-' && *ch != '_')
        .flat_map(char::to_lowercase)
        .collect::<String>()
        == expected
}

fn symbol_font_char(code: u32) -> Option<char> {
    Some(match code {
        0x41 => '\u{0391}',
        0x42 => '\u{0392}',
        0x43 => '\u{03A7}',
        0x44 => '\u{0394}',
        0x45 => '\u{0395}',
        0x46 => '\u{03A6}',
        0x47 => '\u{0393}',
        0x48 => '\u{0397}',
        0x49 => '\u{0399}',
        0x4B => '\u{039A}',
        0x4C => '\u{039B}',
        0x4D => '\u{039C}',
        0x4E => '\u{039D}',
        0x4F => '\u{039F}',
        0x50 => '\u{03A0}',
        0x51 => '\u{0398}',
        0x52 => '\u{03A1}',
        0x53 => '\u{03A3}',
        0x54 => '\u{03A4}',
        0x55 => '\u{03A5}',
        0x57 => '\u{03A9}',
        0x58 => '\u{039E}',
        0x59 => '\u{03A8}',
        0x5A => '\u{0396}',
        0x61 => '\u{03B1}',
        0x62 => '\u{03B2}',
        0x63 => '\u{03C7}',
        0x64 => '\u{03B4}',
        0x65 => '\u{03B5}',
        0x66 => '\u{03C6}',
        0x67 => '\u{03B3}',
        0x68 => '\u{03B7}',
        0x69 => '\u{03B9}',
        0x6B => '\u{03BA}',
        0x6C => '\u{03BB}',
        0x6D => '\u{03BC}',
        0x6E => '\u{03BD}',
        0x6F => '\u{03BF}',
        0x70 => '\u{03C0}',
        0x71 => '\u{03B8}',
        0x72 => '\u{03C1}',
        0x73 => '\u{03C3}',
        0x74 => '\u{03C4}',
        0x75 => '\u{03C5}',
        0x77 => '\u{03C9}',
        0x78 => '\u{03BE}',
        0x79 => '\u{03C8}',
        0x7A => '\u{03B6}',
        0xB7 => '\u{2022}',
        0xD3 => '\u{00A9}',
        _ => return None,
    })
}

fn wingdings_font_char(code: u32) -> Option<char> {
    Some(match code {
        0x41 => '\u{270C}',
        0x4A => '\u{263A}',
        0xFC => '\u{2713}',
        0xFB => '\u{2611}',
        0xFE => '\u{2612}',
        0xA8 => '\u{25CA}',
        0xD8 => '\u{27A2}',
        0xE0 => '\u{2794}',
        0xE8 => '\u{27A3}',
        0x6C => '\u{25CF}',
        0x6E => '\u{25A0}',
        0x75 => '\u{25C6}',
        _ => return None,
    })
}
