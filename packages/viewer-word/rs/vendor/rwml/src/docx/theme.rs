//! Resolve flat Word table colors from the package theme without changing source parts.

use super::{attr_local, attr_local_trimmed, local, parse_rgb_hex_color};
use crate::model::Color;
use quick_xml::{
    events::{BytesStart, Event},
    Reader, Writer,
};
use std::collections::HashMap;

fn palette(xml: &str) -> HashMap<String, Color> {
    let mut colors = HashMap::new();
    let mut reader = Reader::from_str(xml);
    let mut scheme = false;
    let mut slot = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if local(e.name().as_ref()) == b"clrScheme" => scheme = true,
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if scheme => {
                let name = String::from_utf8_lossy(local(e.name().as_ref())).into_owned();
                if matches!(
                    name.as_str(),
                    "dk1"
                        | "lt1"
                        | "dk2"
                        | "lt2"
                        | "accent1"
                        | "accent2"
                        | "accent3"
                        | "accent4"
                        | "accent5"
                        | "accent6"
                        | "hlink"
                        | "folHlink"
                ) {
                    slot = Some(name);
                }
                let value = match local(e.name().as_ref()) {
                    b"srgbClr" => attr_local(&e, b"val"),
                    b"sysClr" => attr_local(&e, b"lastClr"),
                    _ => None,
                };
                if let (Some(slot), Some(color)) =
                    (&slot, value.and_then(|value| parse_rgb_hex_color(&value)))
                {
                    colors.insert(slot.clone(), color);
                }
            }
            Ok(Event::End(e)) if local(e.name().as_ref()) == b"clrScheme" => break,
            Ok(Event::End(e))
                if scheme
                    && slot
                        .as_deref()
                        .is_some_and(|slot| slot.as_bytes() == local(e.name().as_ref())) =>
            {
                slot = None
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    colors
}

fn theme_slot(name: &str) -> &str {
    match name {
        "background1" | "light1" => "lt1",
        "background2" | "light2" => "lt2",
        "text1" | "dark1" => "dk1",
        "text2" | "dark2" => "dk2",
        "hyperlink" => "hlink",
        "followedHyperlink" => "folHlink",
        other => other,
    }
}

fn theme_color(e: &BytesStart<'_>, theme: &HashMap<String, Color>, fill: bool) -> Option<Color> {
    let name = attr_local_trimmed(e, if fill { b"themeFill" } else { b"themeColor" })?;
    let color = *theme.get(&name).or_else(|| theme.get(theme_slot(&name)))?;
    let shade = attr_local_trimmed(
        e,
        if fill {
            b"themeFillShade"
        } else {
            b"themeShade"
        },
    )
    .and_then(|value| u8::from_str_radix(&value, 16).ok());
    let tint = attr_local_trimmed(e, if fill { b"themeFillTint" } else { b"themeTint" })
        .and_then(|value| u8::from_str_radix(&value, 16).ok());
    Some(luminance_transform(color, shade, tint))
}

fn luminance_transform(color: Color, shade: Option<u8>, tint: Option<u8>) -> Color {
    if shade.is_none() && tint.is_none() {
        return color;
    }
    let (r, g, b) = (
        color.r as f64 / 255.0,
        color.g as f64 / 255.0,
        color.b as f64 / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let mut luminance = (max + min) / 2.0;
    let saturation = if delta == 0.0 {
        0.0
    } else {
        delta / (1.0 - (2.0 * luminance - 1.0).abs())
    };
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    };
    // Word themeFillTint/themeFillShade adjust HSL luminance while preserving
    // hue and saturation; tint takes precedence if both are provided.
    // https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/8229a077-7fc8-4fba-96cc-c77b6a4fc768
    if let Some(shade) = shade.filter(|_| tint.is_none()) {
        luminance *= shade as f64 / 255.0;
    }
    if let Some(tint) = tint {
        luminance = luminance * tint as f64 / 255.0 + (1.0 - tint as f64 / 255.0);
    }
    let chroma = (1.0 - (2.0 * luminance - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - (hue.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hue as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let offset = luminance - chroma / 2.0;
    let channel = |value: f64| ((value + offset) * 255.0).round().clamp(0.0, 255.0) as u8;
    Color::rgb(channel(r), channel(g), channel(b))
}

fn resolved_element<'a>(e: BytesStart<'a>, colors: &HashMap<String, Color>) -> BytesStart<'a> {
    let is_shading = local(e.name().as_ref()) == b"shd";
    if !is_shading
        && !matches!(
            local(e.name().as_ref()),
            b"top" | b"bottom" | b"left" | b"right" | b"start" | b"end" | b"insideH" | b"insideV"
        )
    {
        return e;
    }
    let solid = is_shading && attr_local_trimmed(&e, b"val").as_deref() == Some("solid");
    if is_shading && attr_local_trimmed(&e, b"val").as_deref() == Some("nil") {
        return e;
    }
    let color = theme_color(&e, colors, is_shading && !solid).or_else(|| {
        solid
            .then(|| attr_local(&e, b"color").and_then(|value| parse_rgb_hex_color(&value)))
            .flatten()
    });
    let Some(color) = color else {
        return e;
    };
    let value = format!("{:02X}{:02X}{:02X}", color.r, color.g, color.b);
    let target = if is_shading { "fill" } else { "color" };
    let prefix = String::from_utf8_lossy(e.name().as_ref())
        .split_once(':')
        .map(|(prefix, _)| format!("{prefix}:"))
        .unwrap_or_default();
    let mut output = e.to_owned();
    output.clear_attributes();
    for attribute in e.attributes().with_checks(false).flatten() {
        let name = local(attribute.key.as_ref());
        if name == target.as_bytes()
            || (is_shading && matches!(name, b"themeFill" | b"themeFillTint" | b"themeFillShade"))
            || (solid && name == b"val")
        {
            continue;
        }
        output.push_attribute(attribute);
    }
    let key = format!("{prefix}{target}");
    output.push_attribute((key.as_str(), value.as_str()));
    if solid {
        let key = format!("{prefix}val");
        output.push_attribute((key.as_str(), "clear"));
    }
    output
}

pub(super) fn resolve(xml: &str, theme: &str, settings: &str) -> String {
    let mut colors = palette(theme);
    let base = colors.clone();
    let mut settings_reader = Reader::from_str(settings);
    loop {
        match settings_reader.read_event() {
            Ok(Event::Start(e)) | Ok(Event::Empty(e))
                if local(e.name().as_ref()) == b"clrSchemeMapping" =>
            {
                for (key, alias) in [
                    ("bg1", "background1"),
                    ("t1", "text1"),
                    ("bg2", "background2"),
                    ("t2", "text2"),
                    ("accent1", "accent1"),
                    ("accent2", "accent2"),
                    ("accent3", "accent3"),
                    ("accent4", "accent4"),
                    ("accent5", "accent5"),
                    ("accent6", "accent6"),
                    ("hyperlink", "hyperlink"),
                    ("followedHyperlink", "followedHyperlink"),
                ] {
                    if let Some(color) = attr_local_trimmed(&e, key.as_bytes())
                        .and_then(|name| base.get(theme_slot(&name)).copied())
                    {
                        colors.insert(alias.into(), color);
                    }
                }
                break;
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    let mut reader = Reader::from_str(xml);
    let mut writer = Writer::new(Vec::with_capacity(xml.len()));
    loop {
        let event = match reader.read_event() {
            Ok(Event::Start(e)) => Event::Start(resolved_element(e, &colors)),
            Ok(Event::Empty(e)) => Event::Empty(resolved_element(e, &colors)),
            Ok(Event::Eof) => break,
            Ok(event) => event,
            Err(_) => return xml.into(),
        };
        if writer.write_event(event).is_err() {
            return xml.into();
        }
    }
    String::from_utf8(writer.into_inner()).unwrap_or_else(|_| xml.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolves_theme_fill_before_stale_cached_rgb_and_preserves_clear() {
        let theme = r#"<a:theme><a:clrScheme><a:accent1><a:srgbClr val="204060"/></a:accent1><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1></a:clrScheme></a:theme>"#;
        let xml = r#"<w:tcPr><w:shd w:val="clear" w:fill="FFFFFF" w:themeFill="accent1" w:themeFillTint="80"/><w:shd w:themeFill="accent1" w:themeFillShade="80"/><w:shd w:val="nil" w:themeFill="accent1"/><w:shd w:val="solid" w:color="123456" w:fill="FFFFFF"/></w:tcPr>"#;
        let resolved = resolve(xml, theme, "");
        assert!(resolved.contains("w:fill=\"6F9FCF\""), "{resolved}");
        assert!(resolved.contains("w:fill=\"102030\""));
        assert!(resolved.contains("w:val=\"nil\" w:themeFill=\"accent1\""));
        assert!(resolved.contains("w:fill=\"123456\""));
    }

    #[test]
    fn paired_theme_colors_and_unresolved_cached_fill_remain_supported() {
        let theme = r#"<a:theme><a:clrScheme><a:accent1><a:srgbClr val="4F81BD"></a:srgbClr></a:accent1></a:clrScheme></a:theme>"#;
        let xml = r#"<w:tcPr><w:shd w:themeFill="accent1" w:themeFillTint="99"/><w:shd w:fill="ABCDEF" w:themeFill="missing"/></w:tcPr>"#;
        let resolved = resolve(xml, theme, "");
        assert!(resolved.contains("w:fill=\"95B3D7\""), "{resolved}");
        assert!(resolved.contains("w:fill=\"ABCDEF\" w:themeFill=\"missing\""));
        assert_eq!(
            luminance_transform(Color::rgb(0x20, 0x40, 0x60), Some(0), Some(128)),
            Color::rgb(111, 159, 207)
        );
        let mapped = resolve(
            r#"<w:shd w:themeFill="text1"/>"#,
            theme,
            r#"<w:settings><w:clrSchemeMapping w:t1="accent1"/></w:settings>"#,
        );
        assert!(mapped.contains("w:fill=\"4F81BD\""), "{mapped}");
    }
}
