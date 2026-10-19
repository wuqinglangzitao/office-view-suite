use std::collections::HashMap;

use crate::annotation::{
    accept_field_number_format_switch, accept_field_text_format_switch,
    accept_general_format_switch, formula_field_expression_syntax, instruction_parts,
    strip_ascii_switch_prefix, FieldNumberFormat, FieldTextFormat,
};

use super::{
    apply_field_text_format, format_page_number, is_field_format_start,
    page_number_format_from_field_format, quoted_literal_text, PageNumberFormat,
};

pub(super) fn computed_formula_result(instruction: &str) -> Option<String> {
    computed_formula_result_with_bookmarks(instruction, None)
}

pub(super) fn computed_formula_result_with_bookmarks(
    instruction: &str,
    field_bookmarks: Option<&HashMap<String, String>>,
) -> Option<String> {
    let spec = formula_instruction(instruction)?;
    if spec.expression.is_empty() || spec.expression.contains(['\\', '"']) {
        return None;
    }
    let mut parser = FormulaParser::new(&spec.expression, field_bookmarks);
    let value = parser.parse()?;
    let text = match spec.number_format {
        Some(FormulaNumberFormat::Picture(format)) => format_formula_number(value, &format),
        Some(FormulaNumberFormat::General(format)) => format_formula_general_number(value, format),
        None => formula_number_text(value),
    }?;
    Some(apply_field_text_format(text, spec.text_format))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FormulaInstruction {
    pub(super) expression: String,
    pub(super) number_format: Option<FormulaNumberFormat>,
    pub(super) text_format: Option<FieldTextFormat>,
}

enum FormulaNumberFormatSwitch {
    Separate(usize),
    Compact { index: usize, picture: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FormulaNumberFormat {
    Picture(String),
    General(FieldNumberFormat),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FormulaFormatTail {
    number_format: Option<FieldNumberFormat>,
    text_format: Option<FieldTextFormat>,
}

pub(super) fn formula_instruction(instruction: &str) -> Option<FormulaInstruction> {
    let body = instruction.trim().strip_prefix('=')?.trim();
    if body.is_empty() {
        return None;
    }
    let tokens = instruction_parts(body);
    let Some(format_switch) = formula_number_format_switch(&tokens) else {
        let tail_index = tokens.iter().position(|part| is_field_format_start(part));
        if let Some(tail_index) = tail_index {
            if tail_index == 0 {
                return None;
            }
            let mut tail = tokens[tail_index..].iter().map(String::as_str);
            let formats = accept_formula_general_format_tail(&mut tail)?;
            return Some(FormulaInstruction {
                expression: tokens[..tail_index].join(" "),
                number_format: formats.number_format.map(FormulaNumberFormat::General),
                text_format: formats.text_format,
            });
        }
        return Some(FormulaInstruction {
            expression: body.to_string(),
            number_format: None,
            text_format: None,
        });
    };
    let (format_index, picture, tail_start) = match format_switch {
        FormulaNumberFormatSwitch::Separate(format_index) => {
            let (picture, tail_start) =
                formula_number_format_picture_operand(&tokens, format_index + 1)?;
            (format_index, picture, tail_start)
        }
        FormulaNumberFormatSwitch::Compact { index, picture } => {
            (index, formula_number_format_picture(&picture)?, index + 1)
        }
    };
    if format_index == 0 {
        return None;
    }
    let mut tail = tokens[tail_start..].iter().map(String::as_str);
    let text_format = accept_formula_text_format_tail(&mut tail)?;
    Some(FormulaInstruction {
        expression: tokens[..format_index].join(" "),
        number_format: Some(FormulaNumberFormat::Picture(picture)),
        text_format,
    })
}

pub(crate) fn supports_formula_field_syntax(instruction: &str) -> bool {
    formula_field_expression_syntax(instruction)
}

pub(crate) fn supports_context_free_formula_field_syntax(instruction: &str) -> bool {
    let Some(spec) = formula_instruction(instruction) else {
        return false;
    };
    !formula_expression_uses_defined_function(&spec.expression)
        && computed_formula_result(instruction).is_some()
}

fn formula_expression_uses_defined_function(expression: &str) -> bool {
    let chars: Vec<_> = expression.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        if !is_formula_identifier_start(chars[index]) {
            index += 1;
            continue;
        }
        let start = index;
        index += 1;
        while index < chars.len() && is_formula_identifier_continue(chars[index]) {
            index += 1;
        }
        let name: String = chars[start..index].iter().collect();
        let mut next = index;
        while next < chars.len() && chars[next].is_whitespace() {
            next += 1;
        }
        if name.eq_ignore_ascii_case("DEFINED") && chars.get(next) == Some(&'(') {
            return true;
        }
    }
    false
}

fn formula_number_format_switch(tokens: &[String]) -> Option<FormulaNumberFormatSwitch> {
    tokens.iter().enumerate().find_map(|(index, part)| {
        if part == "\\#" {
            return Some(FormulaNumberFormatSwitch::Separate(index));
        }
        let picture = strip_ascii_switch_prefix(part, "\\#")?;
        (!picture.is_empty()).then(|| FormulaNumberFormatSwitch::Compact {
            index,
            picture: picture.to_string(),
        })
    })
}

fn formula_number_format_picture(token: &str) -> Option<String> {
    if let Some(text) = quoted_literal_text(token) {
        return Some(text);
    }
    (!token.contains('"') && !token.starts_with('\\')).then(|| token.to_string())
}

fn formula_number_format_picture_operand(
    tokens: &[String],
    picture_index: usize,
) -> Option<(String, usize)> {
    let first = tokens.get(picture_index)?;
    if let Some(text) = quoted_literal_text(first) {
        return Some((text, picture_index + 1));
    }
    let mut values = vec![formula_unquoted_number_picture_token(first)?];
    let mut index = picture_index + 1;
    while let Some(part) = tokens.get(index) {
        if part.starts_with('\\') {
            break;
        }
        values.push(formula_unquoted_number_picture_token(part)?);
        index += 1;
    }
    Some((values.join(" "), index))
}

fn formula_unquoted_number_picture_token(token: &str) -> Option<&str> {
    (!token.contains('"') && !token.starts_with('\\')).then_some(token)
}

fn accept_formula_general_format_tail<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
) -> Option<FormulaFormatTail> {
    let mut formats = FormulaFormatTail::default();
    while let Some(part) = parts.next() {
        if accept_general_format_switch(part, parts, |format| {
            accept_field_number_format_switch(format, &mut formats.number_format)
                || accept_field_text_format_switch(format, &mut formats.text_format)
        })? {
            continue;
        }
        return None;
    }
    Some(formats)
}

fn accept_formula_text_format_tail<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
) -> Option<Option<FieldTextFormat>> {
    let mut text_format = None;
    while let Some(part) = parts.next() {
        if accept_general_format_switch(part, parts, |format| {
            accept_field_text_format_switch(format, &mut text_format)
        })? {
            continue;
        }
        return None;
    }
    Some(text_format)
}

#[derive(Debug, Clone)]
pub(super) struct FormulaParser<'a> {
    chars: Vec<char>,
    pos: usize,
    field_bookmarks: Option<&'a HashMap<String, String>>,
}

impl<'a> FormulaParser<'a> {
    pub(super) fn new(
        expression: &str,
        field_bookmarks: Option<&'a HashMap<String, String>>,
    ) -> Self {
        Self {
            chars: expression.chars().collect(),
            pos: 0,
            field_bookmarks,
        }
    }

    pub(super) fn parse(&mut self) -> Option<f64> {
        let value = self.parse_comparison()?;
        self.skip_ws();
        (self.pos == self.chars.len()).then_some(value)
    }

    fn parse_comparison(&mut self) -> Option<f64> {
        let lhs = self.parse_expression()?;
        self.skip_ws();
        let Some(operator) = self.parse_comparison_operator() else {
            return Some(lhs);
        };
        let rhs = self.parse_expression()?;
        eval_formula_comparison(lhs, rhs, operator)
    }

    fn parse_expression(&mut self) -> Option<f64> {
        let mut value = self.parse_term()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('+') => {
                    self.pos += 1;
                    value += self.parse_term()?;
                }
                Some('-') => {
                    self.pos += 1;
                    value -= self.parse_term()?;
                }
                _ => return Some(value),
            }
        }
    }

    fn parse_term(&mut self) -> Option<f64> {
        let mut value = self.parse_power()?;
        loop {
            self.skip_ws();
            match self.peek() {
                Some('*') => {
                    self.pos += 1;
                    value *= self.parse_power()?;
                }
                Some('/') => {
                    self.pos += 1;
                    let rhs = self.parse_power()?;
                    if rhs == 0.0 {
                        return None;
                    }
                    value /= rhs;
                }
                _ => return Some(value),
            }
        }
    }

    fn parse_power(&mut self) -> Option<f64> {
        let base = self.parse_factor()?;
        self.skip_ws();
        if self.peek() != Some('^') {
            return Some(base);
        }
        self.pos += 1;
        let exponent = self.parse_power()?;
        let value = base.powf(exponent);
        value.is_finite().then_some(value)
    }

    fn parse_factor(&mut self) -> Option<f64> {
        self.skip_ws();
        match self.peek()? {
            '+' => {
                self.pos += 1;
                self.parse_factor()
            }
            '-' => {
                self.pos += 1;
                self.parse_factor().map(|value| -value)
            }
            '(' => {
                self.pos += 1;
                let value = self.parse_comparison()?;
                self.skip_ws();
                if self.peek()? != ')' {
                    return None;
                }
                self.pos += 1;
                Some(value)
            }
            ch if ch.is_ascii_digit() || ch == '.' => self.parse_number(),
            ch if is_formula_identifier_start(ch) => self.parse_function(),
            _ => None,
        }
    }

    fn parse_number(&mut self) -> Option<f64> {
        let start = self.pos;
        let mut saw_digit = false;
        let mut saw_dot = false;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_digit() {
                saw_digit = true;
                self.pos += 1;
            } else if ch == '.' && !saw_dot {
                saw_dot = true;
                self.pos += 1;
            } else {
                break;
            }
        }
        if !saw_digit {
            return None;
        }
        if self.peek().is_some_and(|ch| matches!(ch, 'e' | 'E')) {
            self.pos += 1;
            if self.peek().is_some_and(|ch| matches!(ch, '+' | '-')) {
                self.pos += 1;
            }
            let exponent_start = self.pos;
            while self.peek().is_some_and(|ch| ch.is_ascii_digit()) {
                self.pos += 1;
            }
            if self.pos == exponent_start {
                return None;
            }
        }
        let value = self.chars[start..self.pos]
            .iter()
            .collect::<String>()
            .parse::<f64>()
            .ok()?;
        value.is_finite().then_some(value)
    }

    fn parse_function(&mut self) -> Option<f64> {
        let name = self.parse_identifier()?;
        let upper_name = name.to_ascii_uppercase();
        self.skip_ws();
        if self.peek() != Some('(') {
            return eval_formula_function(&upper_name, &[])
                .or_else(|| self.field_bookmark_formula_value(&name));
        }
        self.pos += 1;
        if upper_name == "IF" {
            return self.parse_if_function();
        }
        if upper_name == "DEFINED" {
            return self.parse_defined_function();
        }
        let arguments = self.parse_function_arguments()?;
        eval_formula_function(&upper_name, &arguments)
    }

    fn parse_if_function(&mut self) -> Option<f64> {
        let condition = self.parse_comparison()?;
        self.skip_ws();
        let separator = self.consume_argument_separator(None)?;
        if formula_truthy(condition) {
            let value = self.parse_comparison()?;
            self.skip_ws();
            self.consume_argument_separator(Some(separator))?;
            self.skip_unselected_argument_to_closing()?;
            Some(value)
        } else {
            self.skip_unselected_argument_to_separator(separator)?;
            let value = self.parse_comparison()?;
            self.skip_ws();
            if self.peek()? != ')' {
                return None;
            }
            self.pos += 1;
            Some(value)
        }
    }

    fn parse_defined_function(&mut self) -> Option<f64> {
        let start = self.pos;
        let mut depth = 0usize;
        while let Some(ch) = self.peek() {
            match ch {
                '(' => {
                    depth += 1;
                    self.pos += 1;
                }
                ')' if depth == 0 => {
                    let expression = self.chars[start..self.pos]
                        .iter()
                        .collect::<String>()
                        .trim()
                        .to_string();
                    if expression.is_empty() {
                        return None;
                    }
                    self.pos += 1;
                    if formula_expression_is_single_identifier(&expression)
                        && self.field_bookmark_exists(&expression)
                    {
                        return Some(1.0);
                    }
                    let mut parser = FormulaParser::new(&expression, self.field_bookmarks);
                    return Some(parser.parse().is_some() as u8 as f64);
                }
                ')' => {
                    depth = depth.checked_sub(1)?;
                    self.pos += 1;
                }
                _ => self.pos += 1,
            }
        }
        None
    }

    fn parse_comparison_operator(&mut self) -> Option<FormulaComparisonOperator> {
        match self.peek()? {
            '<' => {
                self.pos += 1;
                if self.peek() == Some('=') {
                    self.pos += 1;
                    Some(FormulaComparisonOperator::LessOrEqual)
                } else if self.peek() == Some('>') {
                    self.pos += 1;
                    Some(FormulaComparisonOperator::NotEqual)
                } else {
                    Some(FormulaComparisonOperator::Less)
                }
            }
            '>' => {
                self.pos += 1;
                if self.peek() == Some('=') {
                    self.pos += 1;
                    Some(FormulaComparisonOperator::GreaterOrEqual)
                } else {
                    Some(FormulaComparisonOperator::Greater)
                }
            }
            '=' => {
                self.pos += 1;
                Some(FormulaComparisonOperator::Equal)
            }
            _ => None,
        }
    }

    fn parse_identifier(&mut self) -> Option<String> {
        let start = self.pos;
        if !self.peek().is_some_and(is_formula_identifier_start) {
            return None;
        }
        self.pos += 1;
        while self.peek().is_some_and(is_formula_identifier_continue) {
            self.pos += 1;
        }
        Some(self.chars[start..self.pos].iter().collect())
    }

    fn parse_function_arguments(&mut self) -> Option<Vec<f64>> {
        let mut arguments = Vec::new();
        let mut separator = None;
        self.skip_ws();
        if self.peek()? == ')' {
            self.pos += 1;
            return Some(arguments);
        }
        loop {
            arguments.push(self.parse_comparison()?);
            self.skip_ws();
            match self.peek()? {
                ',' | ';' => {
                    let current = self.peek()?;
                    if separator
                        .replace(current)
                        .is_some_and(|seen| seen != current)
                    {
                        return None;
                    }
                    self.pos += 1;
                    self.skip_ws();
                }
                ')' => {
                    self.pos += 1;
                    return Some(arguments);
                }
                _ => return None,
            }
        }
    }

    fn consume_argument_separator(&mut self, expected: Option<char>) -> Option<char> {
        let separator = self.peek()?;
        if !matches!(separator, ',' | ';') {
            return None;
        }
        if expected.is_some_and(|expected| expected != separator) {
            return None;
        }
        self.pos += 1;
        self.skip_ws();
        Some(separator)
    }

    fn skip_unselected_argument_to_separator(&mut self, separator: char) -> Option<()> {
        let start = self.unselected_argument_start();
        let mut depth = 0usize;
        while let Some(ch) = self.peek() {
            match ch {
                '(' => {
                    depth += 1;
                    self.pos += 1;
                }
                ')' if depth == 0 => return None,
                ')' => {
                    depth = depth.checked_sub(1)?;
                    self.pos += 1;
                }
                ',' | ';' if depth == 0 => {
                    if ch != separator || self.unselected_argument_is_empty(start, self.pos) {
                        return None;
                    }
                    self.pos += 1;
                    self.skip_ws();
                    return Some(());
                }
                _ => self.pos += 1,
            }
        }
        None
    }

    fn skip_unselected_argument_to_closing(&mut self) -> Option<()> {
        let start = self.unselected_argument_start();
        let mut depth = 0usize;
        while let Some(ch) = self.peek() {
            match ch {
                '(' => {
                    depth += 1;
                    self.pos += 1;
                }
                ')' if depth == 0 => {
                    if self.unselected_argument_is_empty(start, self.pos) {
                        return None;
                    }
                    self.pos += 1;
                    return Some(());
                }
                ')' => {
                    depth = depth.checked_sub(1)?;
                    self.pos += 1;
                }
                ',' | ';' if depth == 0 => return None,
                _ => self.pos += 1,
            }
        }
        None
    }

    fn unselected_argument_start(&mut self) -> usize {
        self.skip_ws();
        self.pos
    }

    fn unselected_argument_is_empty(&self, start: usize, end: usize) -> bool {
        self.chars[start..end].iter().all(|ch| ch.is_whitespace())
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn field_bookmark_formula_value(&self, name: &str) -> Option<f64> {
        self.field_bookmarks?
            .get(name)?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
    }

    fn field_bookmark_exists(&self, name: &str) -> bool {
        self.field_bookmarks
            .is_some_and(|field_bookmarks| field_bookmarks.contains_key(name))
    }
}

fn is_formula_identifier_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn is_formula_identifier_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn formula_expression_is_single_identifier(expression: &str) -> bool {
    let mut chars = expression.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    is_formula_identifier_start(first) && chars.all(is_formula_identifier_continue)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormulaComparisonOperator {
    Equal,
    NotEqual,
    Less,
    LessOrEqual,
    Greater,
    GreaterOrEqual,
}

fn eval_formula_comparison(lhs: f64, rhs: f64, operator: FormulaComparisonOperator) -> Option<f64> {
    if !lhs.is_finite() || !rhs.is_finite() {
        return None;
    }
    let equal = (lhs - rhs).abs() < 1e-12;
    let matches = match operator {
        FormulaComparisonOperator::Equal => equal,
        FormulaComparisonOperator::NotEqual => !equal,
        FormulaComparisonOperator::Less => lhs < rhs && !equal,
        FormulaComparisonOperator::LessOrEqual => lhs < rhs || equal,
        FormulaComparisonOperator::Greater => lhs > rhs && !equal,
        FormulaComparisonOperator::GreaterOrEqual => lhs > rhs || equal,
    };
    Some(matches as u8 as f64)
}

pub(super) fn eval_formula_function(name: &str, arguments: &[f64]) -> Option<f64> {
    if !arguments.iter().all(|value| value.is_finite()) {
        return None;
    }
    match name {
        "ABS" if arguments.len() == 1 => Some(arguments[0].abs()),
        "AND" if !arguments.is_empty() => {
            Some((arguments.iter().all(|value| formula_truthy(*value))) as u8 as f64)
        }
        "AVERAGE" if !arguments.is_empty() => {
            Some(arguments.iter().sum::<f64>() / arguments.len() as f64)
        }
        "COUNT" if !arguments.is_empty() => Some(arguments.len() as f64),
        "FALSE" if arguments.is_empty() => Some(0.0),
        "IF" if arguments.len() == 3 => Some(if formula_truthy(arguments[0]) {
            arguments[1]
        } else {
            arguments[2]
        }),
        "INT" if arguments.len() == 1 => Some(arguments[0].floor()),
        "MAX" if !arguments.is_empty() => arguments.iter().copied().reduce(f64::max),
        "MIN" if !arguments.is_empty() => arguments.iter().copied().reduce(f64::min),
        "MOD" if arguments.len() == 2 => {
            if arguments[1].abs() < 1e-12 {
                None
            } else {
                let value = arguments[0] - arguments[1] * (arguments[0] / arguments[1]).floor();
                value.is_finite().then_some(value)
            }
        }
        "NOT" if arguments.len() == 1 => Some((!formula_truthy(arguments[0])) as u8 as f64),
        "OR" if !arguments.is_empty() => {
            Some((arguments.iter().any(|value| formula_truthy(*value))) as u8 as f64)
        }
        "PRODUCT" if !arguments.is_empty() => Some(arguments.iter().product()),
        "ROUND" if arguments.len() == 2 => {
            let digits = arguments[1].round();
            if (arguments[1] - digits).abs() > 1e-12 || !(-12.0..=12.0).contains(&digits) {
                return None;
            }
            let factor = 10_f64.powi(digits as i32);
            Some((arguments[0] * factor).round() / factor)
        }
        "SIGN" if arguments.len() == 1 => {
            if arguments[0].abs() < 1e-12 {
                Some(0.0)
            } else if arguments[0].is_sign_negative() {
                Some(-1.0)
            } else {
                Some(1.0)
            }
        }
        "SUM" if !arguments.is_empty() => Some(arguments.iter().sum()),
        "TRUE" if arguments.is_empty() => Some(1.0),
        _ => None,
    }
}

pub(super) fn formula_truthy(value: f64) -> bool {
    value.abs() >= 1e-12
}

pub(super) fn formula_number_text(value: f64) -> Option<String> {
    if !value.is_finite() {
        return None;
    }
    if value.abs() < 1e-12 {
        return Some("0".to_string());
    }
    let rounded = value.round();
    if (value - rounded).abs() < 1e-12 && rounded >= i64::MIN as f64 && rounded <= i64::MAX as f64 {
        return Some((rounded as i64).to_string());
    }
    let mut text = format!("{value:.12}");
    while text.contains('.') && text.ends_with('0') {
        text.pop();
    }
    if text.ends_with('.') {
        text.pop();
    }
    Some(text)
}

pub(super) fn format_formula_number(value: f64, picture: &str) -> Option<String> {
    if !value.is_finite() || picture.is_empty() {
        return None;
    }
    if picture.contains(';') {
        let (selected, value) = select_formula_number_section(value, picture)?;
        return format_formula_number_section(value, selected, false);
    }
    format_formula_number_section(value, picture, true)
}

pub(super) fn format_formula_general_number(
    value: f64,
    format: FieldNumberFormat,
) -> Option<String> {
    match format {
        FieldNumberFormat::DollarText => format_formula_dollar_text(value),
        _ => {
            let integer = formula_integer_value(value)?;
            let page_format = page_number_format_from_field_format(format);
            format_page_number(integer, Some(page_format))
        }
    }
}

fn formula_integer_value(value: f64) -> Option<usize> {
    if !value.is_finite() {
        return None;
    }
    let rounded = value.round();
    if (value - rounded).abs() >= 1e-12 || rounded < 0.0 || rounded > usize::MAX as f64 {
        return None;
    }
    Some(rounded as usize)
}

fn format_formula_dollar_text(value: f64) -> Option<String> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let cents_total = (value * 100.0).round();
    if cents_total < 0.0 || cents_total > usize::MAX as f64 {
        return None;
    }
    let cents_total = cents_total as usize;
    let whole = cents_total / 100;
    let cents = cents_total % 100;
    let whole_text = format_page_number(whole, Some(PageNumberFormat::CardText))?;
    Some(format!("{whole_text} and {cents:02}/100"))
}

fn format_formula_number_section(
    value: f64,
    picture: &str,
    allow_sign_control: bool,
) -> Option<String> {
    let picture = parse_formula_number_picture(picture)?;
    let (sign_control, prefix) = formula_number_sign_control(&picture.prefix, allow_sign_control);
    let (integer_picture, fraction_picture) = match picture.core.split_once('.') {
        Some((integer, fraction)) if !fraction.contains('.') => (integer, Some(fraction)),
        Some(_) => return None,
        None => (picture.core.as_str(), None),
    };
    if !valid_formula_integer_picture(integer_picture) {
        return None;
    }
    let fraction_picture = fraction_picture.unwrap_or("");
    if !valid_formula_fraction_picture(fraction_picture) {
        return None;
    }
    let max_fraction_digits = fraction_picture.len();
    if max_fraction_digits > 12 {
        return None;
    }
    let required_integer_digits = integer_picture.chars().filter(|ch| *ch == '0').count();
    let required_fraction_digits = if fraction_picture.contains('x') {
        fraction_picture.len()
    } else {
        fraction_picture.chars().filter(|ch| *ch == '0').count()
    };
    let rounded_abs = if value.abs() < 1e-12 {
        0.0
    } else {
        value.abs()
    };
    let fixed = format!("{rounded_abs:.max_fraction_digits$}");
    let (integer_text, fraction_text) = fixed.split_once('.').unwrap_or((fixed.as_str(), ""));
    let mut integer_text = integer_text.to_string();
    if integer_picture.is_empty() {
        integer_text.clear();
    } else {
        while integer_text.len() < required_integer_digits {
            integer_text.insert(0, '0');
        }
        if integer_picture.contains('x') {
            let keep_digits = integer_picture
                .chars()
                .filter(|ch| matches!(ch, '0' | '#' | 'x'))
                .count();
            if integer_text.len() > keep_digits {
                integer_text = integer_text[integer_text.len() - keep_digits..].to_string();
            }
        }
        if integer_picture.contains(',') {
            integer_text = grouped_decimal_digits(&integer_text);
        }
    }
    let mut fraction_text = fraction_text.to_string();
    while fraction_text.len() > required_fraction_digits && fraction_text.ends_with('0') {
        fraction_text.pop();
    }
    let mut text = prefix.to_string();
    text.push_str(&integer_text);
    if !fraction_text.is_empty() {
        text.push('.');
        text.push_str(&fraction_text);
    }
    text.push_str(&picture.suffix);
    let has_nonzero_digit = text.chars().any(|ch| ch.is_ascii_digit() && ch != '0');
    if let Some(sign_control) = sign_control {
        text.insert(
            0,
            formula_number_sign_control_prefix(value, has_nonzero_digit, sign_control),
        );
    } else if value.is_sign_negative() && has_nonzero_digit {
        text.insert(0, '-');
    }
    Some(text)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FormulaNumberSignControl {
    Plus,
    Minus,
}

fn formula_number_sign_control(
    prefix: &str,
    allow_sign_control: bool,
) -> (Option<FormulaNumberSignControl>, &str) {
    if !allow_sign_control {
        return (None, prefix);
    }
    if let Some(prefix) = prefix.strip_prefix('+') {
        return (Some(FormulaNumberSignControl::Plus), prefix);
    }
    if let Some(prefix) = prefix.strip_prefix('-') {
        return (Some(FormulaNumberSignControl::Minus), prefix);
    }
    (None, prefix)
}

fn formula_number_sign_control_prefix(
    value: f64,
    has_nonzero_digit: bool,
    sign_control: FormulaNumberSignControl,
) -> char {
    if !has_nonzero_digit {
        return ' ';
    }
    match sign_control {
        FormulaNumberSignControl::Plus if value.is_sign_negative() => '-',
        FormulaNumberSignControl::Plus => '+',
        FormulaNumberSignControl::Minus if value.is_sign_negative() => '-',
        FormulaNumberSignControl::Minus => ' ',
    }
}

fn select_formula_number_section(value: f64, picture: &str) -> Option<(&str, f64)> {
    let sections: Vec<_> = picture.split(';').collect();
    let selected = match sections.as_slice() {
        [positive, negative] => {
            if value.is_sign_negative() {
                *negative
            } else {
                *positive
            }
        }
        [positive, negative, zero] => {
            if value.abs() < 1e-12 {
                *zero
            } else if value.is_sign_negative() {
                *negative
            } else {
                *positive
            }
        }
        _ => return None,
    };
    if selected.is_empty() || selected.contains(';') {
        return None;
    }
    Some((selected, value.abs()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FormulaNumberPicture {
    prefix: String,
    core: String,
    suffix: String,
}

fn parse_formula_number_picture(picture: &str) -> Option<FormulaNumberPicture> {
    if picture.contains('`') {
        return None;
    }
    // Emit a trailing single-quoted literal (e.g. `#,##0 'items'`) verbatim. Leading
    // or interspersed quotes stay unsupported (bail -> cached) so the literal never
    // perturbs sign control or digit counting in the numeric core.
    let (picture, trailing_literal) = peel_trailing_quoted_literal(picture)?;
    if picture.contains('\'') {
        return None;
    }
    let mut first = picture.find(['0', '#', 'x'])?;
    if first > 0 && picture.as_bytes()[first - 1] == b'.' {
        first -= 1;
    }
    let last = picture.rfind(['0', '#', 'x'])?;
    let prefix = &picture[..first];
    let core = &picture[first..=last];
    let suffix = &picture[last + 1..];
    if core.is_empty() || !valid_formula_number_affix(prefix) || !valid_formula_number_affix(suffix)
    {
        return None;
    }
    Some(FormulaNumberPicture {
        prefix: prefix.to_string(),
        core: core.to_string(),
        suffix: format!("{suffix}{trailing_literal}"),
    })
}

fn peel_trailing_quoted_literal(picture: &str) -> Option<(&str, String)> {
    let Some(head) = picture.strip_suffix('\'') else {
        return Some((picture, String::new()));
    };
    let start = head.rfind('\'')?;
    let literal = &head[start + 1..];
    if literal.chars().any(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some((&head[..start], literal.to_string()))
}

fn valid_formula_number_affix(affix: &str) -> bool {
    affix.chars().all(|ch| {
        (ch == ' ' || ch.is_ascii_graphic())
            && !matches!(ch, '0' | '#' | ',' | '.' | '"' | '\\' | 'x')
    })
}

fn valid_formula_integer_picture(picture: &str) -> bool {
    if picture.is_empty() {
        return true;
    }
    if !picture
        .chars()
        .all(|ch| matches!(ch, '0' | '#' | ',' | 'x'))
    {
        return false;
    }
    let x_count = picture.chars().filter(|ch| *ch == 'x').count();
    if x_count > 1 {
        return false;
    }
    if x_count == 1 {
        return !picture.contains(',')
            && picture.starts_with('x')
            && picture.chars().all(|ch| matches!(ch, '0' | '#' | 'x'));
    }
    let groups: Vec<_> = picture.split(',').collect();
    if groups.len() == 1 {
        return picture.chars().any(|ch| matches!(ch, '0' | '#'));
    }
    if groups
        .iter()
        .any(|group| group.is_empty() || !group.chars().all(|ch| matches!(ch, '0' | '#')))
    {
        return false;
    }
    let Some((first, rest)) = groups.split_first() else {
        return false;
    };
    (1..=3).contains(&first.len()) && rest.iter().all(|group| group.len() == 3)
}

fn valid_formula_fraction_picture(picture: &str) -> bool {
    let x_count = picture.chars().filter(|ch| *ch == 'x').count();
    if x_count > 1 || (x_count == 1 && !picture.ends_with('x')) {
        return false;
    }
    let mut seen_optional = false;
    for ch in picture.chars() {
        match ch {
            '0' if seen_optional => return false,
            '0' => {}
            '#' => seen_optional = true,
            'x' => seen_optional = true,
            _ => return false,
        }
    }
    true
}

fn grouped_decimal_digits(digits: &str) -> String {
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}
