//! Source document typography controls used by browser preview.
//! MS-DOC: Dop95.copts80 / Dop97.doptypography. OOXML: settings/compat.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
/// Source controls that change glyph advances and trailing-space decoration.
pub struct PreviewTypography {
    /// Balance half-width/full-width spaces in fixed-pitch East Asian fonts.
    pub balance_single_byte_double_byte_width: bool,
    /// Include terminal spaces when painting an underline.
    pub underline_trailing_spaces: bool,
    /// Permit compression of full-width punctuation side bearings.
    pub compress_punctuation: bool,
}

pub(crate) fn from_dop(dop: &[u8]) -> PreviewTypography {
    if dop.len() < 88 {
        return PreviewTypography::default();
    }
    let Some(flags) = crate::util::u16le(dop, 84) else {
        return PreviewTypography::default();
    };
    PreviewTypography {
        balance_single_byte_double_byte_width: flags & 0x8000 == 0,
        underline_trailing_spaces: flags & 0x4000 == 0,
        compress_punctuation: crate::util::u16le(dop, 90)
            .is_some_and(|flags| matches!((flags >> 1) & 3, 1 | 2)),
    }
}

#[cfg(feature = "docx")]
pub(crate) fn from_settings(xml: &str) -> PreviewTypography {
    use quick_xml::{events::Event, name::ResolveResult, reader::NsReader};
    const W: &[u8] = b"http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    let mut result = PreviewTypography::default();
    let mut reader = NsReader::from_str(xml);
    let mut path = Vec::new();
    loop {
        match reader.read_resolved_event() {
            Ok((ns, event @ (Event::Start(_) | Event::Empty(_)))) => {
                let is_start = matches!(event, Event::Start(_));
                let e = match event {
                    Event::Start(e) | Event::Empty(e) => e,
                    _ => unreachable!(),
                };
                let name = if matches!(ns, ResolveResult::Bound(ref n) if n.as_ref() == W) {
                    e.local_name().as_ref().to_vec()
                } else {
                    Vec::new()
                };
                let value = e.attributes().flatten().find_map(|a| {
                    let (ns, local) = reader.resolver().resolve_attribute(a.key);
                    (matches!(ns, ResolveResult::Bound(ref n) if n.as_ref() == W)
                        && local.as_ref() == b"val")
                        .then(|| String::from_utf8_lossy(&a.value).into_owned())
                });
                let enabled = value
                    .as_deref()
                    .map(str::trim)
                    .map_or(true, |v| matches!(v, "1" | "true" | "on"));
                if path == [b"settings".to_vec(), b"compat".to_vec()] {
                    match name.as_slice() {
                        b"balanceSingleByteDoubleByteWidth" => {
                            result.balance_single_byte_double_byte_width = enabled
                        }
                        b"ulTrailSpace" => result.underline_trailing_spaces = enabled,
                        _ => {}
                    }
                } else if path == [b"settings".to_vec()] && name == b"characterSpacingControl" {
                    result.compress_punctuation = matches!(
                        value.as_deref(),
                        Some("compressPunctuation" | "compressPunctuationAndJapaneseKana")
                    );
                }
                if is_start {
                    path.push(name);
                }
            }
            Ok((_, Event::End(_))) => {
                path.pop();
            }
            Ok((_, Event::Eof)) => return result,
            Err(_) => return PreviewTypography::default(),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dop_compatibility_flags_are_inverted_and_bounded() {
        assert_eq!(from_dop(&[0; 84]), PreviewTypography::default());
        let mut dop = [0; 92];
        dop[90] = 3;
        assert_eq!(
            from_dop(&dop),
            PreviewTypography {
                balance_single_byte_double_byte_width: true,
                underline_trailing_spaces: true,
                compress_punctuation: true,
            }
        );
        dop[85] = 0xc0;
        dop[90] = 7;
        assert_eq!(from_dop(&dop), PreviewTypography::default());
    }

    #[cfg(feature = "docx")]
    #[test]
    fn settings_respect_namespaces_boolean_values_and_scope() {
        let xml = r#"<x:settings xmlns:x="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:f="fake">
            <x:characterSpacingControl x:val="compressPunctuation"/><x:compat>
            <x:balanceSingleByteDoubleByteWidth/><x:ulTrailSpace x:val="on"/>
            <f:ulTrailSpace x:val="false"/></x:compat></x:settings>"#;
        assert_eq!(
            from_settings(xml),
            from_dop(&{
                let mut d = [0; 92];
                d[90] = 3;
                d
            })
        );
        assert_eq!(
            from_settings(&xml.replace("x:val=\"on\"", "x:val=\"0\"")).underline_trailing_spaces,
            false
        );
        assert_eq!(
            from_settings(&xml.replace(
                "<x:balanceSingleByteDoubleByteWidth/>",
                "<x:balanceSingleByteDoubleByteWidth x:val=\"false\"/>"
            ))
            .balance_single_byte_double_byte_width,
            false
        );
        assert_eq!(
            from_settings("<settings><compat><ulTrailSpace/></compat></settings>"),
            PreviewTypography::default()
        );
    }
}
