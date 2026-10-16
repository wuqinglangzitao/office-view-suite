//! Counter → label formatting for list autonumbers, by the `nfc` number-format
//! code ([MS-DOC] LVLF.nfc / [MS-OSHARED] 2.2.1.3 MSONFC).
//!
//! Only the value→string conversion lives here; choosing which level/counter to
//! format and substituting into the level template is the caller's job.

const MAX_KOREAN_REPEATS: usize = 1024;

/// Format a 1-based counter `n` per the `nfc` number-format code ([MS-OSHARED]
/// MSONFC). Unknown/forbidden codes fall back to decimal.
pub(crate) fn format(n: u32, nfc: u8) -> String {
    match nfc {
        0x00 => n.to_string(),                // msonfcArabic: 1, 2, 3
        0x01 => roman(n, true),               // upper roman
        0x02 => roman(n, false),              // lower roman
        0x03 => letter(n, true),              // upper letter A, B … AA
        0x04 => letter(n, false),             // lower letter
        0x05 => ordinal(n),                   // 1st, 2nd, 3rd
        0x0E => fullwidth(n),                 // decimalFullWidth: １, ２, ３
        0x0F => n.to_string(),                // decimalHalfWidth: 1, 2, 3
        0x12 => circled(n),                   // ①②③
        0x13 => fullwidth(n),                 // decimalFullWidth2: １, ２, ３
        0x16 => format!("{n:02}"),            // decimalZero: 01, 02 … 10
        0x1A => fullstop(n),                  // decimalEnclosedFullstop: ⒈⒉⒊
        0x1B => parenthesized(n),             // decimalEnclosedParen: ⑴⑵⑶
        0x18 => korean_ganada(n),             // 가, 나, 다 …
        0x19 => korean_chosung(n),            // ㄱ, ㄴ, ㄷ …
        0x29 | 0x2B | 0x2C => sino_korean(n), // koreanDigital / Legal / Digital2: 일이삼
        0x2A => korean_counting(n),           // koreanCounting: 하나, 둘, 셋 …
        0x17 | 0xFF => String::new(),         // bullet / none → no number
        _ => n.to_string(),                   // incl. Japanese/Chinese — decimal fallback
    }
}

/// Bijective base-26: 1→A, 26→Z, 27→AA, 52→AZ, 53→BA …
fn letter(n: u32, upper: bool) -> String {
    if n == 0 {
        return String::new();
    }
    let base = if upper { b'A' } else { b'a' };
    let mut n = n;
    let mut buf = Vec::new();
    while n > 0 {
        let rem = (n - 1) % 26;
        buf.push(base + rem as u8);
        n = (n - 1) / 26;
    }
    buf.reverse();
    String::from_utf8(buf).unwrap_or_default()
}

fn roman(n: u32, upper: bool) -> String {
    if n == 0 || n >= 4000 {
        return n.to_string();
    }
    const VALS: [(u32, &str); 13] = [
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
    ];
    let mut n = n;
    let mut out = String::new();
    for (v, s) in VALS {
        while n >= v {
            out.push_str(s);
            n -= v;
        }
    }
    if upper {
        out
    } else {
        out.to_lowercase()
    }
}

fn ordinal(n: u32) -> String {
    let suffix = match (n % 10, n % 100) {
        (1, 11) | (2, 12) | (3, 13) => "th",
        (1, _) => "st",
        (2, _) => "nd",
        (3, _) => "rd",
        _ => "th",
    };
    format!("{n}{suffix}")
}

/// Korean Ganada: 가, 나, 다, 라, 마, 바, 사, 아, 자, 차, 카, 타, 파, 하 (14
/// leading consonants in "가" syllable form), then wraps.
fn korean_ganada(n: u32) -> String {
    const G: [char; 14] = [
        '가', '나', '다', '라', '마', '바', '사', '아', '자', '차', '카', '타', '파', '하',
    ];
    if n == 0 {
        return String::new();
    }
    let idx = ((n - 1) % 14) as usize;
    let reps = (((n - 1) / 14) as usize + 1).min(MAX_KOREAN_REPEATS);
    G[idx].to_string().repeat(reps)
}

/// Korean Chosung (leading-jamo) numbering: ㄱ, ㄴ, ㄷ, …, ㅎ (14 consonants).
fn korean_chosung(n: u32) -> String {
    const C: [char; 14] = [
        'ㄱ', 'ㄴ', 'ㄷ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅅ', 'ㅇ', 'ㅈ', 'ㅊ', 'ㅋ', 'ㅌ', 'ㅍ', 'ㅎ',
    ];
    if n == 0 {
        return String::new();
    }
    let idx = ((n - 1) % 14) as usize;
    let reps = (((n - 1) / 14) as usize + 1).min(MAX_KOREAN_REPEATS);
    C[idx].to_string().repeat(reps)
}

/// Full-width decimal digits (U+FF10..U+FF19): `23` → `２３`.
fn fullwidth(n: u32) -> String {
    n.to_string()
        .chars()
        .map(|c| char::from_u32(0xFF10 + (c as u32 - '0' as u32)).unwrap_or(c))
        .collect()
}

/// Circled numbers ①②③ … (U+2460..U+2473 for 1..20), else decimal.
fn circled(n: u32) -> String {
    enclosed_1_to_20(n, 0x2460)
}

/// Numbers followed by a full stop ⒈⒉⒊ … (U+2488..U+249B), else decimal.
fn fullstop(n: u32) -> String {
    enclosed_1_to_20(n, 0x2488)
}

/// Parenthesized numbers ⑴⑵⑶ … (U+2474..U+2487), else decimal.
fn parenthesized(n: u32) -> String {
    enclosed_1_to_20(n, 0x2474)
}

fn enclosed_1_to_20(n: u32, base: u32) -> String {
    if (1..=20).contains(&n) {
        char::from_u32(base + n - 1)
            .map(String::from)
            .unwrap_or_else(|| n.to_string())
    } else {
        n.to_string()
    }
}

/// Sino-Korean numerals (일/이/삼 …) with positional 십/백/천.
fn sino_korean(n: u32) -> String {
    const D: [&str; 10] = ["영", "일", "이", "삼", "사", "오", "육", "칠", "팔", "구"];
    const U: [&str; 4] = ["", "십", "백", "천"];
    if n == 0 {
        return D[0].to_string();
    }
    if n >= 10_000 {
        return n.to_string(); // beyond 천: fall back to digits
    }
    let mut out = String::new();
    let digits = [(n / 1000) % 10, (n / 100) % 10, (n / 10) % 10, n % 10];
    for (i, &d) in digits.iter().enumerate() {
        if d == 0 {
            continue;
        }
        let unit_pos = 3 - i;
        // 십/백/천 with a leading 1 is written without the 일 (십 not 일십).
        if !(d == 1 && unit_pos > 0) {
            out.push_str(D[d as usize]);
        }
        out.push_str(U[unit_pos]);
    }
    out
}

/// Native Korean counting (하나, 둘, 셋 …). Practical range 1..99; the spec
/// notes Word only displays these for small integers, so larger values fall
/// back to decimal.
fn korean_counting(n: u32) -> String {
    const ONES: [&str; 10] = [
        "", "하나", "둘", "셋", "넷", "다섯", "여섯", "일곱", "여덟", "아홉",
    ];
    const TENS: [&str; 10] = [
        "", "열", "스물", "서른", "마흔", "쉰", "예순", "일흔", "여든", "아흔",
    ];
    // Combining forms used before a unit (한, 두, 세, 네) — but standalone list
    // labels use the full form, so keep the simple ones + tens concatenation.
    if n == 0 || n >= 100 {
        return n.to_string();
    }
    let t = (n / 10) as usize;
    let o = (n % 10) as usize;
    format!("{}{}", TENS[t], ONES[o])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arabic_and_letter_and_roman() {
        assert_eq!(format(1, 0), "1");
        assert_eq!(format(1, 3), "A");
        assert_eq!(format(26, 3), "Z");
        assert_eq!(format(27, 3), "AA");
        assert_eq!(format(28, 4), "ab");
        assert_eq!(format(4, 1), "IV");
        assert_eq!(format(9, 2), "ix");
        assert_eq!(format(2, 5), "2nd");
        assert_eq!(format(11, 5), "11th");
    }

    #[test]
    fn korean_formats() {
        // Ganada = nfc 0x18 (24), Chosung = 0x19 (25), koreanDigital = 0x29 (41).
        assert_eq!(format(1, 0x18), "가");
        assert_eq!(format(14, 0x18), "하");
        assert_eq!(format(15, 0x18), "가가");
        assert_eq!(format(1, 0x19), "ㄱ");
        assert_eq!(format(1, 0x29), "일");
        assert_eq!(format(10, 0x29), "십");
        assert_eq!(format(23, 0x29), "이십삼");
        assert_eq!(format(100, 0x29), "백");
        // koreanCounting = 0x2A (42).
        assert_eq!(format(1, 0x2A), "하나");
        assert_eq!(format(21, 0x2A), "스물하나");
    }

    #[test]
    fn special_formats() {
        assert_eq!(format(3, 0x16), "03"); // decimalZero
        assert_eq!(format(23, 0x0E), "２３"); // decimalFullWidth
        assert_eq!(format(23, 0x0F), "23"); // decimalHalfWidth
        assert_eq!(format(1, 0x12), "①"); // circled
        assert_eq!(format(23, 0x13), "２３"); // decimalFullWidth2
        assert_eq!(format(12, 0x1A), "⒓"); // decimalEnclosedFullstop
        assert_eq!(format(12, 0x1B), "⑿"); // decimalEnclosedParen
        assert_eq!(format(5, 0x17), ""); // bullet
        assert_eq!(format(5, 0xFF), ""); // none
    }

    #[test]
    fn korean_repeating_formats_are_bounded_for_large_start_values() {
        let ganada = format(14 * 4096, 0x18);
        let chosung = format(14 * 4096, 0x19);

        assert_eq!(ganada.chars().count(), 1024);
        assert_eq!(chosung.chars().count(), 1024);
    }
}
