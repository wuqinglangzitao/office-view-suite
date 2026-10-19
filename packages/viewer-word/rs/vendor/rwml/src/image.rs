//! Inline picture extraction from the `Data` stream.
//!
//! A picture run points (via `sprmCPicLocation`) at an `fcPic` offset into the
//! `Data` OLE stream, where a `PICF` header precedes the picture payload. In
//! Word 97+ that payload is usually an Office Art / Escher container wrapping the
//! blip; rather than walk Escher exactly, we scan a bounded window for a known
//! raster magic (PNG/JPEG/GIF) — the pragmatic approach POI's `matchSignature`
//! also takes. Metafiles (WMF/EMF, often DEFLATE-compressed) and OLE objects are
//! left as placeholders.
//!
//! Reference: [MS-DOC] 2.9.158 (PICF); [MS-ODRAW] (Office Art blips).

use crate::model::Image;
use crate::util::{u16le, u32le};

#[cfg(any(feature = "docx", feature = "render"))]
pub(crate) const MIME_RAW_RGBA: &str = "image/x-rwml-rgba";

/// Extract an inline picture at `fc_pic` in the `Data` stream. Returns a
/// placeholder [`Image`] (no bytes) if nothing recognizable is found.
pub(crate) fn extract(data: &[u8], fc_pic: u32) -> Image {
    extract_bytes(data, fc_pic as usize).unwrap_or_default()
}

fn extract_bytes(data: &[u8], fc: usize) -> Option<Image> {
    // PICF: lcb (total size) u32@0, cbHeader u16@4.
    let lcb = u32le(data, fc)? as usize;
    let cb_header = u16le(data, fc + 4)? as usize;
    if cb_header < 8 || lcb < cb_header {
        return None;
    }
    let start = fc.checked_add(cb_header)?;
    let end = fc.checked_add(lcb)?.min(data.len());
    let region = data.get(start..end)?;
    let (off, mime) = find_raster(region)?;
    let bytes = region[off..].to_vec();
    let (width_px, height_px) = dims(&bytes, mime).unzip();
    Some(Image {
        alt: None,
        bytes: Some(bytes),
        mime: Some(mime.to_string()),
        width_px,
        height_px,
        rotation_degrees: None,
        floating_offset_emu: None,
    })
}

/// Intrinsic pixel dimensions parsed from an image header (PNG/JPEG/GIF/BMP/TIFF/WebP).
/// Best-effort: returns `None` for unknown or truncated headers. Bounds-checked,
/// no allocation — used by the renderer/writer to size embedded pictures.
pub(crate) fn dims(bytes: &[u8], mime: &str) -> Option<(u32, u32)> {
    match mime {
        "image/png" => png_dims(bytes),
        "image/jpeg" => jpeg_dims(bytes),
        "image/gif" => gif_dims(bytes),
        "image/bmp" => bmp_dims(bytes),
        "image/tiff" => tiff_dims(bytes),
        "image/webp" => webp_dims(bytes),
        _ => None,
    }
}

fn png_dims(b: &[u8]) -> Option<(u32, u32)> {
    // 8-byte signature, then IHDR: len(4)+"IHDR"(4)+width(4 BE)+height(4 BE).
    if b.len() < 24 || b[0..8] != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return None;
    }
    let w = u32::from_be_bytes([b[16], b[17], b[18], b[19]]);
    let h = u32::from_be_bytes([b[20], b[21], b[22], b[23]]);
    (w > 0 && h > 0).then_some((w, h))
}

fn gif_dims(b: &[u8]) -> Option<(u32, u32)> {
    // "GIF8" + 2 version bytes, then logical-screen width/height (2 LE each) @6.
    if b.len() < 10 || &b[0..4] != b"GIF8" {
        return None;
    }
    let w = u16::from_le_bytes([b[6], b[7]]) as u32;
    let h = u16::from_le_bytes([b[8], b[9]]) as u32;
    (w > 0 && h > 0).then_some((w, h))
}

fn bmp_dims(b: &[u8]) -> Option<(u32, u32)> {
    // "BM" + file header; BITMAPINFOHEADER width@18 height@22 (signed 4 LE).
    if b.len() < 26 || &b[0..2] != b"BM" {
        return None;
    }
    let w = i32::from_le_bytes([b[18], b[19], b[20], b[21]]).unsigned_abs();
    let h = i32::from_le_bytes([b[22], b[23], b[24], b[25]]).unsigned_abs();
    (w > 0 && h > 0).then_some((w, h))
}

fn tiff_dims(b: &[u8]) -> Option<(u32, u32)> {
    let little = match b.get(0..2)? {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    if tiff_u16(b, 2, little)? != 42 {
        return None;
    }
    let ifd = tiff_u32(b, 4, little)? as usize;
    let count = tiff_u16(b, ifd, little)? as usize;
    let entries = ifd.checked_add(2)?;
    let end = entries.checked_add(count.checked_mul(12)?)?;
    if end.checked_add(4)? > b.len() {
        return None;
    }

    let (mut width, mut height) = (None, None);
    for i in 0..count {
        let off = entries + i * 12;
        let tag = tiff_u16(b, off, little)?;
        let typ = tiff_u16(b, off + 2, little)?;
        let count = tiff_u32(b, off + 4, little)?;
        let value = match (typ, count) {
            (3, 1) => tiff_u16(b, off + 8, little).map(u32::from),
            (4, 1) => tiff_u32(b, off + 8, little),
            _ => None,
        };
        match tag {
            256 => width = value,
            257 => height = value,
            _ => {}
        }
    }
    let (w, h) = (width?, height?);
    (w > 0 && h > 0).then_some((w, h))
}

fn tiff_u16(b: &[u8], off: usize, little: bool) -> Option<u16> {
    let end = off.checked_add(2)?;
    let bytes: [u8; 2] = b.get(off..end)?.try_into().ok()?;
    Some(if little {
        u16::from_le_bytes(bytes)
    } else {
        u16::from_be_bytes(bytes)
    })
}

fn tiff_u32(b: &[u8], off: usize, little: bool) -> Option<u32> {
    let end = off.checked_add(4)?;
    let bytes: [u8; 4] = b.get(off..end)?.try_into().ok()?;
    Some(if little {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    })
}

fn jpeg_dims(b: &[u8]) -> Option<(u32, u32)> {
    // SOI then segment scan; a SOFn marker (C0..CF except C4/C8/CC) carries
    // height@+5 (2 BE) and width@+7 (2 BE).
    if b.len() < 4 || b[0] != 0xFF || b[1] != 0xD8 {
        return None;
    }
    let mut i = 2usize;
    while i + 9 < b.len() {
        if b[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = b[i + 1];
        if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as u32;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as u32;
            return (w > 0 && h > 0).then_some((w, h));
        }
        // Standalone markers (SOI/EOI/RSTn) have no length; others do.
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        let seg_len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        if seg_len < 2 {
            return None;
        }
        i += 2 + seg_len;
    }
    None
}

fn webp_dims(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 20 || &b[0..4] != b"RIFF" || &b[8..12] != b"WEBP" {
        return None;
    }
    let mut i = 12usize;
    while i + 8 <= b.len() {
        let tag = &b[i..i + 4];
        let len = u32::from_le_bytes([b[i + 4], b[i + 5], b[i + 6], b[i + 7]]) as usize;
        let start = i + 8;
        let end = start.checked_add(len)?;
        if end > b.len() {
            return None;
        }
        let data = &b[start..end];
        match tag {
            b"VP8X" if data.len() >= 10 => {
                let w = 1 + u32::from_le_bytes([data[4], data[5], data[6], 0]);
                let h = 1 + u32::from_le_bytes([data[7], data[8], data[9], 0]);
                return Some((w, h));
            }
            b"VP8L" if data.len() >= 5 && data[0] == 0x2f => {
                let w = 1 + ((((data[2] & 0x3f) as u32) << 8) | data[1] as u32);
                let h = 1
                    + ((((data[4] & 0x0f) as u32) << 10)
                        | ((data[3] as u32) << 2)
                        | (((data[2] & 0xc0) as u32) >> 6));
                return Some((w, h));
            }
            b"VP8 " if data.len() >= 10 && data[3..6] == [0x9d, 0x01, 0x2a] => {
                let w = u16::from_le_bytes([data[6], data[7]]) as u32 & 0x3fff;
                let h = u16::from_le_bytes([data[8], data[9]]) as u32 & 0x3fff;
                return (w > 0 && h > 0).then_some((w, h));
            }
            _ => {}
        }
        i = end + (len & 1);
    }
    None
}

/// Scan the picture payload for a raster magic. In Word 97+ the payload is an
/// Office Art / Escher container, so the file magic can sit well past the start
/// (after the SpContainer / FOPT / blip headers); scan the whole bounded region.
///
/// The formats are tried in order of signature reliability, each scanned over the
/// **whole** region before falling through to the next — a stray early match for a
/// weak magic must not pre-empt a real later image:
///
///   * PNG — an 8-byte signature, effectively unambiguous.
///   * GIF — `GIF8` (covers both `GIF87a` and `GIF89a`).
///   * WebP — RIFF container with `WEBP` form type.
///   * JPEG — validated: the `FF D8` SOI must be immediately followed by another
///     marker (`FF` + a marker byte `>= 0xC0`). A bare `FF D8 FF` is only three
///     bytes and turns up by chance inside Escher/OLE binary, so without the
///     marker check the scanner extracts megabytes of garbage (observed on real
///     private corpus files as `FF D8 FF 85`, where `0x85` is not a JPEG marker).
fn find_raster(region: &[u8]) -> Option<(usize, &'static str)> {
    const PNG: &[u8] = &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    const GIF: &[u8] = b"GIF8";
    if let Some(i) = find(region, PNG) {
        return Some((i, "image/png"));
    }
    if let Some(i) = find(region, GIF) {
        return Some((i, "image/gif"));
    }
    if let Some(i) = find_webp(region) {
        return Some((i, "image/webp"));
    }
    find_jpeg(region).map(|i| (i, "image/jpeg"))
}

/// First index of `needle` in `haystack` (naive scan — payloads are small and
/// scanned at most a few times).
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| &haystack[i..i + needle.len()] == needle)
}

/// First index of a *validated* JPEG SOI: `FF D8 FF <marker>` with the marker
/// byte in `0xC0..=0xFF` (a real JPEG segment marker), rejecting the chance
/// `FF D8 FF` triples that litter binary Escher containers.
fn find_jpeg(region: &[u8]) -> Option<usize> {
    if region.len() < 4 {
        return None;
    }
    (0..=region.len() - 4).find(|&i| {
        region[i] == 0xFF && region[i + 1] == 0xD8 && region[i + 2] == 0xFF && region[i + 3] >= 0xC0
    })
}

fn find_webp(region: &[u8]) -> Option<usize> {
    if region.len() < 12 {
        return None;
    }
    (0..=region.len() - 12)
        .find(|&i| &region[i..i + 4] == b"RIFF" && &region[i + 8..i + 12] == b"WEBP")
}

/// A `data:` URI for an extracted image, for self-contained HTML previews.
pub(crate) fn data_uri(img: &Image) -> Option<String> {
    let bytes = img.bytes.as_ref()?;
    let mime = img.mime.as_deref()?;
    Some(format!("data:{mime};base64,{}", base64(bytes)))
}

/// Minimal standard base64 encoder (no padding-free variants), so the crate
/// stays dependency-light.
fn base64(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18 & 63) as usize] as char);
        out.push(TABLE[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn rejects_stray_jpeg_magic_without_valid_marker() {
        // `FF D8 FF 85` — a chance triple seen in real Escher binary; 0x85 is not
        // a JPEG marker, so it must NOT be extracted as an image.
        assert_eq!(find_raster(&[0xFF, 0xD8, 0xFF, 0x85, 1, 2, 3, 4]), None);
        // A genuine JPEG SOI (`FF D8 FF E0`, JFIF APP0) is accepted.
        assert_eq!(
            find_raster(&[0x00, 0xFF, 0xD8, 0xFF, 0xE0, 0, 16]),
            Some((1, "image/jpeg"))
        );
    }

    #[test]
    fn png_anywhere_beats_a_later_or_earlier_jpeg_triple() {
        // A stray `FF D8 FF E0` precedes a real PNG: PNG (the stronger signature)
        // must win even though the JPEG match sits at a smaller offset.
        let mut region = vec![0xFF, 0xD8, 0xFF, 0xE0, 0, 0, 0, 0];
        let png_at = region.len();
        region.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 9, 9]);
        assert_eq!(find_raster(&region), Some((png_at, "image/png")));
    }

    #[test]
    fn finds_webp_and_reads_vp8x_dimensions() {
        let webp = [
            b'R', b'I', b'F', b'F', 22, 0, 0, 0, b'W', b'E', b'B', b'P', b'V', b'P', b'8', b'X',
            10, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 2, 0, 0,
        ];
        let mut region = vec![0, 1, 2];
        let webp_at = region.len();
        region.extend_from_slice(&webp);

        assert_eq!(find_raster(&region), Some((webp_at, "image/webp")));
        assert_eq!(dims(&webp, "image/webp"), Some((2, 3)));
    }

    #[test]
    fn reads_tiff_ifd_dimensions() {
        let tiff = [
            b'I', b'I', 42, 0, 8, 0, 0, 0, 2, 0, 0, 1, 4, 0, 1, 0, 0, 0, 2, 0, 0, 0, 1, 1, 4, 0, 1,
            0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(dims(&tiff, "image/tiff"), Some((2, 3)));
        assert_eq!(dims(&tiff[..12], "image/tiff"), None);
    }

    #[test]
    fn finds_png_after_blip_header() {
        // Simulate a PICF (cbHeader=8, lcb covers a small payload) whose payload
        // has 33 bytes of blip header then a PNG signature.
        let mut data = vec![0u8; 4];
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        let payload_len = 33 + png.len();
        let lcb = 8 + payload_len;
        data[0..4].copy_from_slice(&(lcb as u32).to_le_bytes());
        data.extend_from_slice(&8u16.to_le_bytes()); // cbHeader
        data.extend_from_slice(&[0u8; 2]); // pad to cbHeader=8
        data.extend_from_slice(&[0u8; 33]); // blip header
        data.extend_from_slice(&png);
        let img = extract_bytes(&data, 0).unwrap();
        assert_eq!(img.mime.as_deref(), Some("image/png"));
        assert!(img.bytes.unwrap().starts_with(&png));
    }
}
