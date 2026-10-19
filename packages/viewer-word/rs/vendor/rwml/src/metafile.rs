use std::io::Read;

const MAX_METAFILE_INFLATE: usize = 1 << 20;
const MAX_METAFILE_RGBA: usize = 64 << 20;
const SRCCOPY: u32 = 0x00CC_0020;
const EMR_BITBLT: u32 = 76;
const EMR_STRETCHBLT: u32 = 77;
const EMR_SETDIBITSTODEVICE: u32 = 80;
const EMR_STRETCHDIBITS: u32 = 81;
const EMR_STRETCHDIBITS_FIXED_SIZE: usize = 80;
const META_DIBBITBLT: u16 = 0x0940;
const META_DIBSTRETCHBLT: u16 = 0x0B41;
const META_SETDIBTODEV: u16 = 0x0D33;
const META_STRETCHDIB: u16 = 0x0F43;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MetafileKind {
    Emf,
    Wmf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MetafileRaster {
    pub(crate) rgba: Vec<u8>,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
}

#[derive(Debug, Clone, Copy)]
struct MetafileFrame {
    left: i32,
    top: i32,
    logical_width: u32,
    logical_height: u32,
    width_px: u32,
    height_px: u32,
}

pub(crate) fn format_for_part(name: &str) -> Option<(MetafileKind, bool)> {
    let ext = name.rsplit('.').next()?.to_ascii_lowercase();
    match ext.as_str() {
        "emf" => Some((MetafileKind::Emf, false)),
        "wmf" => Some((MetafileKind::Wmf, false)),
        "emz" => Some((MetafileKind::Emf, true)),
        "wmz" => Some((MetafileKind::Wmf, true)),
        _ => None,
    }
}

pub(crate) fn is_gzip_payload(bytes: &[u8]) -> bool {
    bytes.len() >= 2 && bytes[0] == 0x1f && bytes[1] == 0x8b
}

pub(crate) fn dimensions(
    kind: MetafileKind,
    bytes: &[u8],
    compressed: bool,
) -> (Option<u32>, Option<u32>) {
    let inflated = if compressed && is_gzip_payload(bytes) {
        inflate_gzip_metafile(bytes)
    } else {
        None
    };
    let payload = match (compressed, inflated.as_deref()) {
        (true, Some(payload)) => payload,
        (true, None) => return (None, None),
        (false, _) => bytes,
    };
    metafile_frame(kind, payload)
        .map(|frame| (frame.width_px, frame.height_px))
        .unzip()
}

pub(crate) fn extract_raster(
    kind: MetafileKind,
    bytes: &[u8],
    compressed: bool,
) -> Option<MetafileRaster> {
    let inflated = if compressed && is_gzip_payload(bytes) {
        inflate_gzip_metafile(bytes)
    } else {
        None
    };
    let payload = match (compressed, inflated.as_deref()) {
        (true, Some(payload)) => payload,
        (true, None) => return None,
        (false, _) => bytes,
    };
    let frame = metafile_frame(kind, payload)?;
    let raster = match kind {
        MetafileKind::Emf => extract_emf_single_dib(payload, frame)?,
        MetafileKind::Wmf => extract_wmf_single_dib(payload, frame)?,
    };
    (raster.width_px == frame.width_px && raster.height_px == frame.height_px).then_some(raster)
}

fn inflate_gzip_metafile(bytes: &[u8]) -> Option<Vec<u8>> {
    let decoder = flate2::read::GzDecoder::new(bytes);
    let mut limited = decoder.take(MAX_METAFILE_INFLATE as u64 + 1);
    let mut out = Vec::new();
    limited.read_to_end(&mut out).ok()?;
    (!out.is_empty() && out.len() <= MAX_METAFILE_INFLATE).then_some(out)
}

fn metafile_frame(kind: MetafileKind, bytes: &[u8]) -> Option<MetafileFrame> {
    match kind {
        MetafileKind::Emf => emf_frame(bytes),
        MetafileKind::Wmf => wmf_frame(bytes),
    }
}

fn emf_frame(bytes: &[u8]) -> Option<MetafileFrame> {
    if bytes.len() < 60 || read_u32le(bytes, 0)? != 1 {
        return None;
    }
    let header_size = read_u32le(bytes, 4)? as usize;
    if header_size < 88
        || header_size > bytes.len()
        || bytes.get(40..44)? != b" EMF"
        || read_u32le(bytes, 44)? != 0x0001_0000
        || read_u16le(bytes, 58)? != 0
    {
        return None;
    }
    let left = read_i32le(bytes, 8)?;
    let top = read_i32le(bytes, 12)?;
    let (width_px, height_px) =
        inclusive_rect_dimensions(left, top, read_i32le(bytes, 16)?, read_i32le(bytes, 20)?)?;
    Some(MetafileFrame {
        left,
        top,
        logical_width: width_px,
        logical_height: height_px,
        width_px,
        height_px,
    })
}

fn wmf_frame(bytes: &[u8]) -> Option<MetafileFrame> {
    if bytes.len() < 22 || read_u32le(bytes, 0)? != 0x9AC6CDD7 {
        return None;
    }
    let units_per_inch = read_u16le(bytes, 14)? as u32;
    if units_per_inch == 0 {
        return None;
    }
    let left = read_i16le(bytes, 6)? as i32;
    let top = read_i16le(bytes, 8)? as i32;
    let width_units = (read_i16le(bytes, 10)? as i32).checked_sub(left)?;
    let height_units = (read_i16le(bytes, 12)? as i32).checked_sub(top)?;
    let width_px = scale_wmf_units(width_units, units_per_inch)?;
    let height_px = scale_wmf_units(height_units, units_per_inch)?;
    Some(MetafileFrame {
        left,
        top,
        logical_width: width_units.try_into().ok()?,
        logical_height: height_units.try_into().ok()?,
        width_px,
        height_px,
    })
}

fn extract_emf_single_dib(bytes: &[u8], frame: MetafileFrame) -> Option<MetafileRaster> {
    if bytes.len() < 88
        || read_u32le(bytes, 0)? != 1
        || read_u32le(bytes, 48)? as usize != bytes.len()
    {
        return None;
    }
    let declared_records = read_u32le(bytes, 52)? as usize;
    let mut offset = 0usize;
    let mut records = 0usize;
    let mut raster_records = 0usize;
    let mut raster = None;
    let mut saw_eof = false;
    while offset.checked_add(8)? <= bytes.len() {
        let record_type = read_u32le(bytes, offset)?;
        let record_size = read_u32le(bytes, offset + 4)? as usize;
        if record_size < 8 || record_size % 4 != 0 {
            return None;
        }
        let end = offset.checked_add(record_size)?;
        if end > bytes.len() {
            return None;
        }
        records = records.checked_add(1)?;
        let record = bytes.get(offset..end)?;
        match record_type {
            1 if offset == 0 && record_size >= 88 => {}
            EMR_BITBLT | EMR_STRETCHBLT | EMR_SETDIBITSTODEVICE | EMR_STRETCHDIBITS if !saw_eof => {
                raster_records += 1;
                if raster_records > 1 {
                    return None;
                }
                let candidate = match record_type {
                    EMR_BITBLT => extract_emf_source_blt_record(record, false),
                    EMR_STRETCHBLT => extract_emf_source_blt_record(record, true),
                    EMR_SETDIBITSTODEVICE => extract_emf_setdibits_to_device_record(record),
                    EMR_STRETCHDIBITS => extract_emf_stretchdibits_record(record),
                    _ => None,
                }?;
                if !emf_dib_covers_frame(record, record_type, frame, &candidate) {
                    return None;
                }
                raster = Some(candidate);
            }
            14 if !saw_eof
                && record_size == 20
                && read_u32le(record, 8)? == 0
                && matches!(read_u32le(record, 12)?, 0 | 16)
                && read_u32le(record, 16)? == 20 =>
            {
                saw_eof = true;
            }
            _ => return None,
        }
        offset = end;
        if saw_eof && offset != bytes.len() {
            return None;
        }
    }
    (offset == bytes.len() && saw_eof && records == declared_records && raster_records == 1)
        .then_some(raster?)
}

fn emf_dib_covers_frame(
    record: &[u8],
    record_type: u32,
    frame: MetafileFrame,
    raster: &MetafileRaster,
) -> bool {
    let Some(width) = i32::try_from(frame.logical_width).ok() else {
        return false;
    };
    let Some(height) = i32::try_from(frame.logical_height).ok() else {
        return false;
    };
    let Some(right) = frame
        .left
        .checked_add(width)
        .and_then(|value| value.checked_sub(1))
    else {
        return false;
    };
    let Some(bottom) = frame
        .top
        .checked_add(height)
        .and_then(|value| value.checked_sub(1))
    else {
        return false;
    };
    if read_i32le(record, 8) != Some(frame.left)
        || read_i32le(record, 12) != Some(frame.top)
        || read_i32le(record, 16) != Some(right)
        || read_i32le(record, 20) != Some(bottom)
        || read_i32le(record, 24) != Some(frame.left)
        || read_i32le(record, 28) != Some(frame.top)
        || raster.width_px != frame.width_px
        || raster.height_px != frame.height_px
    {
        return false;
    }
    match record_type {
        EMR_BITBLT | EMR_STRETCHBLT => {
            read_i32le(record, 32) == Some(width) && read_i32le(record, 36) == Some(height)
        }
        EMR_SETDIBITSTODEVICE => true,
        EMR_STRETCHDIBITS => {
            read_i32le(record, 72) == Some(width) && read_i32le(record, 76) == Some(height)
        }
        _ => false,
    }
}

fn extract_emf_source_blt_record(record: &[u8], stretches_source: bool) -> Option<MetafileRaster> {
    // [MS-EMF] 2.3.1.2 (EMR_BITBLT) and 2.3.1.6 (EMR_STRETCHBLT).
    let fixed_size = if stretches_source { 108 } else { 100 };
    if record.len() < fixed_size
        || read_u32le(record, 40)? != SRCCOPY
        || read_i32le(record, 44)? != 0
        || read_i32le(record, 48)? != 0
        || read_u32le(record, 52)? != 1.0f32.to_bits()
        || read_u32le(record, 56)? != 0.0f32.to_bits()
        || read_u32le(record, 60)? != 0.0f32.to_bits()
        || read_u32le(record, 64)? != 1.0f32.to_bits()
        || read_u32le(record, 68)? != 0.0f32.to_bits()
        || read_u32le(record, 72)? != 0.0f32.to_bits()
        || read_u32le(record, 76)? & 0xFF00_0000 != 0
        || read_u32le(record, 80)? != 0
    {
        return None;
    }
    let destination_width = read_i32le(record, 32)?;
    let destination_height = read_i32le(record, 36)?;
    if destination_width <= 0 || destination_height <= 0 {
        return None;
    }
    if stretches_source {
        let source_width = read_i32le(record, 100)?;
        let source_height = read_i32le(record, 104)?;
        if source_width <= 0
            || source_height <= 0
            || destination_width != source_width
            || destination_height != source_height
        {
            return None;
        }
    }
    let raster = extract_emf_dib_payload_at(record, fixed_size, 84, false)?;
    (destination_width as u32 == raster.width_px && destination_height as u32 == raster.height_px)
        .then_some(raster)
}

fn extract_emf_stretchdibits_record(record: &[u8]) -> Option<MetafileRaster> {
    if record.len() < EMR_STRETCHDIBITS_FIXED_SIZE
        || read_i32le(record, 32)? != 0
        || read_i32le(record, 36)? != 0
        || read_u32le(record, 64)? != 0
        || read_u32le(record, 68)? != SRCCOPY
    {
        return None;
    }
    let source_width = read_i32le(record, 40)?;
    let source_height = read_i32le(record, 44)?;
    let destination_width = read_i32le(record, 72)?;
    let destination_height = read_i32le(record, 76)?;
    if source_width <= 0
        || source_height <= 0
        || destination_width != source_width
        || destination_height != source_height
    {
        return None;
    }
    let raster = extract_emf_dib_payload(record, EMR_STRETCHDIBITS_FIXED_SIZE)?;
    (source_width as u32 == raster.width_px && source_height as u32 == raster.height_px)
        .then_some(raster)
}

fn extract_emf_setdibits_to_device_record(record: &[u8]) -> Option<MetafileRaster> {
    const FIXED_SIZE: usize = 76;
    if record.len() < FIXED_SIZE
        || read_i32le(record, 32)? != 0
        || read_i32le(record, 36)? != 0
        || read_u32le(record, 64)? != 0
        || read_u32le(record, 68)? != 0
    {
        return None;
    }
    let source_width = read_i32le(record, 40)?;
    let source_height = read_i32le(record, 44)?;
    let scan_count = read_u32le(record, 72)?;
    if source_width <= 0 || source_height <= 0 {
        return None;
    }
    let raster = extract_emf_dib_payload(record, FIXED_SIZE)?;
    (source_width as u32 == raster.width_px
        && source_height as u32 == raster.height_px
        && scan_count == raster.height_px)
        .then_some(raster)
}

fn extract_emf_dib_payload(record: &[u8], fixed_size: usize) -> Option<MetafileRaster> {
    extract_emf_dib_payload_at(record, fixed_size, 48, true)
}

fn extract_emf_dib_payload_at(
    record: &[u8],
    fixed_size: usize,
    fields_offset: usize,
    allow_monochrome: bool,
) -> Option<MetafileRaster> {
    if record.len() < fixed_size {
        return None;
    }
    let off_bmi = read_u32le(record, fields_offset)? as usize;
    let cb_bmi = read_u32le(record, fields_offset.checked_add(4)?)? as usize;
    let off_bits = read_u32le(record, fields_offset.checked_add(8)?)? as usize;
    let cb_bits = read_u32le(record, fields_offset.checked_add(12)?)? as usize;
    let bmi_end = off_bmi.checked_add(cb_bmi)?;
    let bits_end = off_bits.checked_add(cb_bits)?;
    if off_bmi != fixed_size || bmi_end != off_bits || bits_end != record.len() {
        return None;
    }
    let bmi = record.get(off_bmi..bmi_end)?;
    let bits = record.get(off_bits..bits_end)?;
    if !allow_monochrome && read_u16le(bmi, 14)? == 1 {
        return None;
    }
    decode_dib(bmi, bits)
}

fn extract_wmf_single_dib(bytes: &[u8], frame: MetafileFrame) -> Option<MetafileRaster> {
    if !wmf_single_dib_header_is_valid(bytes) {
        return None;
    }
    let mut offset = 40usize;
    let declared_max_record = read_u32le(bytes, 34)? as usize;
    let mut max_record_words = 0usize;
    let mut raster_records = 0usize;
    let mut raster = None;
    let mut saw_eof = false;
    while offset.checked_add(6)? <= bytes.len() {
        let record_words = read_u32le(bytes, offset)? as usize;
        if record_words < 3 {
            return None;
        }
        let record_size = record_words.checked_mul(2)?;
        let end = offset.checked_add(record_size)?;
        if end > bytes.len() {
            return None;
        }
        let function = read_u16le(bytes, offset + 4)?;
        max_record_words = max_record_words.max(record_words);
        let record = bytes.get(offset..end)?;
        match function {
            0 if !saw_eof && record_words == 3 => {
                saw_eof = true;
            }
            META_DIBBITBLT | META_DIBSTRETCHBLT | META_SETDIBTODEV | META_STRETCHDIB
                if !saw_eof =>
            {
                raster_records += 1;
                if raster_records > 1 {
                    return None;
                }
                let candidate = match function {
                    META_DIBBITBLT => extract_wmf_source_dib_blt_record(record, false),
                    META_DIBSTRETCHBLT => extract_wmf_source_dib_blt_record(record, true),
                    META_SETDIBTODEV => extract_wmf_setdib_to_device_record(record),
                    META_STRETCHDIB => extract_wmf_stretchdib_record(record),
                    _ => None,
                }?;
                if !wmf_dib_covers_frame(record, function, frame, &candidate) {
                    return None;
                }
                raster = Some(candidate);
            }
            _ => return None,
        }
        offset = end;
        if saw_eof && offset != bytes.len() {
            return None;
        }
    }
    (offset == bytes.len()
        && saw_eof
        && raster_records == 1
        && max_record_words == declared_max_record)
        .then_some(raster?)
}

fn wmf_single_dib_header_is_valid(bytes: &[u8]) -> bool {
    if bytes.len() < 40
        || bytes.len() % 2 != 0
        || read_u32le(bytes, 0) != Some(0x9AC6_CDD7)
        || read_u16le(bytes, 4) != Some(0)
        || read_u32le(bytes, 16) != Some(0)
        || !matches!(read_u16le(bytes, 22), Some(1) | Some(2))
        || read_u16le(bytes, 24) != Some(9)
        || read_u16le(bytes, 26) != Some(0x0300)
        || read_u32le(bytes, 28) != u32::try_from((bytes.len() - 22) / 2).ok()
        || read_u16le(bytes, 32) != Some(0)
        || read_u16le(bytes, 38) != Some(0)
    {
        return false;
    }
    let mut checksum = 0u16;
    for offset in (0..20).step_by(2) {
        let Some(word) = read_u16le(bytes, offset) else {
            return false;
        };
        checksum ^= word;
    }
    read_u16le(bytes, 20) == Some(checksum)
}

fn wmf_dib_covers_frame(
    record: &[u8],
    function: u16,
    frame: MetafileFrame,
    raster: &MetafileRaster,
) -> bool {
    let covers_frame = match function {
        META_DIBBITBLT => {
            read_i16le(record, 18) == i16::try_from(frame.top).ok()
                && read_i16le(record, 20) == i16::try_from(frame.left).ok()
                && read_i16le(record, 14) == i16::try_from(frame.logical_height).ok()
                && read_i16le(record, 16) == i16::try_from(frame.logical_width).ok()
        }
        META_DIBSTRETCHBLT => {
            read_i16le(record, 22) == i16::try_from(frame.top).ok()
                && read_i16le(record, 24) == i16::try_from(frame.left).ok()
                && read_i16le(record, 18) == i16::try_from(frame.logical_height).ok()
                && read_i16le(record, 20) == i16::try_from(frame.logical_width).ok()
        }
        META_SETDIBTODEV => {
            let (Ok(left), Ok(top), Ok(width), Ok(height)) = (
                u16::try_from(frame.left),
                u16::try_from(frame.top),
                u16::try_from(frame.logical_width),
                u16::try_from(frame.logical_height),
            ) else {
                return false;
            };
            read_u16le(record, 20) == Some(top)
                && read_u16le(record, 22) == Some(left)
                && read_u16le(record, 16) == Some(height)
                && read_u16le(record, 18) == Some(width)
        }
        META_STRETCHDIB => {
            read_i16le(record, 24) == i16::try_from(frame.top).ok()
                && read_i16le(record, 26) == i16::try_from(frame.left).ok()
                && read_i16le(record, 20) == i16::try_from(frame.logical_height).ok()
                && read_i16le(record, 22) == i16::try_from(frame.logical_width).ok()
        }
        _ => false,
    };
    covers_frame && raster.width_px == frame.width_px && raster.height_px == frame.height_px
}

fn extract_wmf_source_dib_blt_record(
    record: &[u8],
    stretches_source: bool,
) -> Option<MetafileRaster> {
    // [MS-WMF] 2.3.1.2 (META_DIBBITBLT) and 2.3.1.3 (META_DIBSTRETCHBLT).
    let fixed_size: usize = if stretches_source { 26 } else { 22 };
    if record.len() < fixed_size.checked_add(40)? || read_u32le(record, 6)? != SRCCOPY {
        return None;
    }
    let (source_height, source_width, source_y, source_x, destination_height, destination_width) =
        if stretches_source {
            (
                read_i16le(record, 10)?,
                read_i16le(record, 12)?,
                read_i16le(record, 14)?,
                read_i16le(record, 16)?,
                read_i16le(record, 18)?,
                read_i16le(record, 20)?,
            )
        } else {
            let destination_height = read_i16le(record, 14)?;
            let destination_width = read_i16le(record, 16)?;
            (
                destination_height,
                destination_width,
                read_i16le(record, 10)?,
                read_i16le(record, 12)?,
                destination_height,
                destination_width,
            )
        };
    if source_x != 0
        || source_y != 0
        || source_width <= 0
        || source_height <= 0
        || destination_width != source_width
        || destination_height != source_height
    {
        return None;
    }
    let raster = decode_packed_dib(record.get(fixed_size..)?)?;
    (source_width as u32 == raster.width_px && source_height as u32 == raster.height_px)
        .then_some(raster)
}

fn extract_wmf_stretchdib_record(record: &[u8]) -> Option<MetafileRaster> {
    const FIXED_SIZE: usize = 28;
    if record.len() < FIXED_SIZE
        || read_u32le(record, 6)? != SRCCOPY
        || read_u16le(record, 10)? != 0
        || read_i16le(record, 16)? != 0
        || read_i16le(record, 18)? != 0
    {
        return None;
    }
    let source_height = read_i16le(record, 12)?;
    let source_width = read_i16le(record, 14)?;
    let destination_height = read_i16le(record, 20)?;
    let destination_width = read_i16le(record, 22)?;
    if source_width <= 0
        || source_height <= 0
        || destination_width != source_width
        || destination_height != source_height
    {
        return None;
    }
    let raster = decode_packed_dib(record.get(FIXED_SIZE..)?)?;
    (source_width as u32 == raster.width_px && source_height as u32 == raster.height_px)
        .then_some(raster)
}

fn extract_wmf_setdib_to_device_record(record: &[u8]) -> Option<MetafileRaster> {
    const FIXED_SIZE: usize = 24;
    if record.len() < FIXED_SIZE
        || read_u16le(record, 6)? != 0
        || read_u16le(record, 10)? != 0
        || read_u16le(record, 12)? != 0
        || read_u16le(record, 14)? != 0
    {
        return None;
    }
    let scan_count = read_u16le(record, 8)? as u32;
    let source_height = read_u16le(record, 16)? as u32;
    let source_width = read_u16le(record, 18)? as u32;
    let raster = decode_packed_dib(record.get(FIXED_SIZE..)?)?;
    (scan_count == raster.height_px
        && source_height == raster.height_px
        && source_width == raster.width_px)
        .then_some(raster)
}

fn decode_packed_dib(dib: &[u8]) -> Option<MetafileRaster> {
    let header_len = bitmap_info_header_len(dib)?;
    decode_dib(dib.get(..header_len)?, dib.get(header_len..)?)
}

fn bitmap_info_header_len(dib: &[u8]) -> Option<usize> {
    if dib.len() < 40 || read_u32le(dib, 0)? != 40 {
        return None;
    }
    let bit_count = read_u16le(dib, 14)?;
    let compression = read_u32le(dib, 16)?;
    let colors_used = read_u32le(dib, 32)?;
    40usize.checked_add(dib_info_extra_len(bit_count, compression, colors_used)?)
}

fn decode_dib(bmi: &[u8], bits: &[u8]) -> Option<MetafileRaster> {
    if bmi.len() < 40 || read_u32le(bmi, 0)? != 40 {
        return None;
    }
    let width = read_i32le(bmi, 4)?;
    let height = read_i32le(bmi, 8)?;
    let planes = read_u16le(bmi, 12)?;
    let bit_count = read_u16le(bmi, 14)?;
    let compression = read_u32le(bmi, 16)?;
    let colors_used = read_u32le(bmi, 32)?;
    let info_extra_len = dib_info_extra_len(bit_count, compression, colors_used)?;
    let palette_end = 40usize.checked_add(info_extra_len)?;
    if bmi.len() != palette_end {
        return None;
    }
    let palette = bmi.get(40..palette_end)?;
    let bitfields = if compression == 3 {
        Some(bitfield_channels(palette, bit_count)?)
    } else {
        None
    };
    if width <= 0 || height == 0 || planes != 1 {
        return None;
    }
    let width = width as u32;
    let height_abs = height.unsigned_abs();
    let row_stride = dib_row_stride(width, bit_count)?;
    let needed = row_stride.checked_mul(height_abs as usize)?;
    if bits.len() != needed {
        return None;
    }
    let rgba_len = dib_rgba_len(width, height_abs)?;
    let mut rgba = vec![0u8; rgba_len];
    for y in 0..height_abs as usize {
        let src_y = if height < 0 {
            y
        } else {
            height_abs as usize - 1 - y
        };
        let row = bits.get(src_y * row_stride..src_y * row_stride + row_stride)?;
        for x in 0..width as usize {
            let dst = (y.checked_mul(width as usize)?.checked_add(x)?).checked_mul(4)?;
            let (red, green, blue) = if let Some(channels) = bitfields {
                let src = x.checked_mul((bit_count / 8) as usize)?;
                let value = match bit_count {
                    16 => read_u16le(row, src)? as u32,
                    32 => read_u32le(row, src)?,
                    _ => return None,
                };
                (
                    channels[0].decode(value),
                    channels[1].decode(value),
                    channels[2].decode(value),
                )
            } else {
                match bit_count {
                    1 | 4 | 8 => {
                        let index = match bit_count {
                            1 => (*row.get(x / 8)? >> (7 - x % 8)) & 1,
                            4 => {
                                let packed = *row.get(x / 2)?;
                                if x % 2 == 0 {
                                    packed >> 4
                                } else {
                                    packed & 0x0F
                                }
                            }
                            8 => *row.get(x)?,
                            _ => unreachable!(),
                        } as usize;
                        let entry = index.checked_mul(4)?;
                        (
                            *palette.get(entry + 2)?,
                            *palette.get(entry + 1)?,
                            *palette.get(entry)?,
                        )
                    }
                    24 | 32 => {
                        let src = x.checked_mul((bit_count / 8) as usize)?;
                        (*row.get(src + 2)?, *row.get(src + 1)?, *row.get(src)?)
                    }
                    _ => return None,
                }
            };
            rgba[dst] = red;
            rgba[dst + 1] = green;
            rgba[dst + 2] = blue;
            rgba[dst + 3] = 255;
        }
    }
    Some(MetafileRaster {
        rgba,
        width_px: width,
        height_px: height_abs,
    })
}

fn dib_rgba_len(width: u32, height: u32) -> Option<usize> {
    let len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(4)?;
    (len <= MAX_METAFILE_RGBA).then_some(len)
}

fn dib_info_extra_len(bit_count: u16, compression: u32, colors_used: u32) -> Option<usize> {
    match compression {
        0 => dib_palette_entries(bit_count, colors_used)?.checked_mul(4),
        3 if matches!(bit_count, 16 | 32) && colors_used == 0 => Some(12),
        _ => None,
    }
}

fn dib_palette_entries(bit_count: u16, colors_used: u32) -> Option<usize> {
    match bit_count {
        1 | 4 | 8 => {
            let maximum = 1usize.checked_shl(bit_count as u32)?;
            let entries = if colors_used == 0 {
                maximum
            } else {
                colors_used as usize
            };
            (entries > 0 && entries <= maximum).then_some(entries)
        }
        24 | 32 if colors_used == 0 => Some(0),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct BitfieldChannel {
    mask: u32,
    shift: u32,
    maximum: u32,
}

impl BitfieldChannel {
    fn new(mask: u32, bit_count: u16) -> Option<Self> {
        let allowed = if bit_count == 32 {
            u32::MAX
        } else {
            (1u32.checked_shl(bit_count as u32)?).checked_sub(1)?
        };
        if mask == 0 || mask & !allowed != 0 {
            return None;
        }
        let shift = mask.trailing_zeros();
        let maximum = mask >> shift;
        let contiguous = maximum == u32::MAX
            || maximum
                .checked_add(1)
                .is_some_and(|next| maximum & next == 0);
        contiguous.then_some(Self {
            mask,
            shift,
            maximum,
        })
    }

    fn decode(self, value: u32) -> u8 {
        let component = (value & self.mask) >> self.shift;
        ((component as u64 * 255 + self.maximum as u64 / 2) / self.maximum as u64) as u8
    }
}

fn bitfield_channels(mask_bytes: &[u8], bit_count: u16) -> Option<[BitfieldChannel; 3]> {
    let red = BitfieldChannel::new(read_u32le(mask_bytes, 0)?, bit_count)?;
    let green = BitfieldChannel::new(read_u32le(mask_bytes, 4)?, bit_count)?;
    let blue = BitfieldChannel::new(read_u32le(mask_bytes, 8)?, bit_count)?;
    if red.mask & green.mask != 0 || red.mask & blue.mask != 0 || green.mask & blue.mask != 0 {
        return None;
    }
    Some([red, green, blue])
}

fn dib_row_stride(width: u32, bit_count: u16) -> Option<usize> {
    let bits = (width as usize).checked_mul(bit_count as usize)?;
    bits.checked_add(31)?.checked_div(32)?.checked_mul(4)
}

fn inclusive_rect_dimensions(left: i32, top: i32, right: i32, bottom: i32) -> Option<(u32, u32)> {
    let width = right.checked_sub(left)?.checked_add(1)?;
    let height = bottom.checked_sub(top)?.checked_add(1)?;
    (width > 0 && height > 0).then_some((width as u32, height as u32))
}

fn scale_wmf_units(value: i32, units_per_inch: u32) -> Option<u32> {
    if value <= 0 {
        return None;
    }
    let value = value as u64;
    let units = units_per_inch as u64;
    let px = (value * 96 + units / 2) / units;
    (px > 0 && px <= u32::MAX as u64).then_some(px as u32)
}

fn read_u16le(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_i16le(bytes: &[u8], offset: usize) -> Option<i16> {
    let end = offset.checked_add(2)?;
    Some(i16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u32le(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_i32le(bytes: &[u8], offset: usize) -> Option<i32> {
    let end = offset.checked_add(4)?;
    Some(i32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::{
        bitmap_info_header_len, decode_dib, decode_packed_dib, dib_rgba_len, dimensions,
        extract_emf_stretchdibits_record, extract_raster, inflate_gzip_metafile, read_u16le,
        read_u32le, MetafileKind,
    };

    fn put_u16le(out: &mut [u8], offset: usize, value: u16) {
        out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32le(out: &mut [u8], offset: usize, value: u32) {
        out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_i16le(out: &mut [u8], offset: usize, value: i16) {
        out[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_i32le(out: &mut [u8], offset: usize, value: i32) {
        out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn bitmap_info_header(width: i32, height: i32, bit_count: u16) -> Vec<u8> {
        let mut bmi = vec![0; 40];
        put_u32le(&mut bmi, 0, 40);
        put_i32le(&mut bmi, 4, width);
        put_i32le(&mut bmi, 8, height);
        put_u16le(&mut bmi, 12, 1);
        put_u16le(&mut bmi, 14, bit_count);
        bmi
    }

    fn packed_palette_dib(bit_count: u16, width: i32, palette: &[[u8; 4]], bits: &[u8]) -> Vec<u8> {
        let mut dib = bitmap_info_header(width, -1, bit_count);
        put_u32le(&mut dib, 20, bits.len() as u32);
        put_u32le(&mut dib, 32, palette.len() as u32);
        for color in palette {
            dib.extend_from_slice(color);
        }
        dib.extend_from_slice(bits);
        dib
    }

    fn packed_bitfields_dib(bit_count: u16, width: i32, masks: [u32; 3], bits: &[u8]) -> Vec<u8> {
        let mut dib = bitmap_info_header(width, -1, bit_count);
        put_u32le(&mut dib, 16, 3);
        put_u32le(&mut dib, 20, bits.len() as u32);
        for mask in masks {
            dib.extend_from_slice(&mask.to_le_bytes());
        }
        dib.extend_from_slice(bits);
        dib
    }

    fn packed_rgb32_dib() -> Vec<u8> {
        let mut dib = bitmap_info_header(1, -1, 32);
        put_u32le(&mut dib, 20, 4);
        dib.extend_from_slice(&[0x33, 0x22, 0x11, 0x00]);
        dib
    }

    fn emf_with_setdibits_to_device(start_scan: u32, scan_count: u32) -> Vec<u8> {
        let dib = packed_rgb32_dib();
        let bmi_len = 40usize;
        let bits_len = dib.len() - bmi_len;
        let mut bytes = vec![0u8; 88];
        put_u32le(&mut bytes, 0, 1);
        put_u32le(&mut bytes, 4, 88);
        put_i32le(&mut bytes, 16, 0);
        put_i32le(&mut bytes, 20, 0);
        bytes[40..44].copy_from_slice(b" EMF");
        put_u32le(&mut bytes, 44, 0x0001_0000);
        let start = bytes.len();
        let record_size = 76 + dib.len();
        bytes.resize(start + record_size, 0);
        put_u32le(&mut bytes, start, 80);
        put_u32le(&mut bytes, start + 4, record_size as u32);
        put_i32le(&mut bytes, start + 40, 1);
        put_i32le(&mut bytes, start + 44, 1);
        put_u32le(&mut bytes, start + 48, 76);
        put_u32le(&mut bytes, start + 52, bmi_len as u32);
        put_u32le(&mut bytes, start + 56, (76 + bmi_len) as u32);
        put_u32le(&mut bytes, start + 60, bits_len as u32);
        put_u32le(&mut bytes, start + 68, start_scan);
        put_u32le(&mut bytes, start + 72, scan_count);
        bytes[start + 76..start + 76 + dib.len()].copy_from_slice(&dib);
        append_emf_eof(&mut bytes, 3);
        bytes
    }

    fn emf_with_stretchdib(raster_operation: u32, source_x: i32) -> Vec<u8> {
        let mut bytes = vec![0u8; 88];
        put_u32le(&mut bytes, 0, 1);
        put_u32le(&mut bytes, 4, 88);
        bytes[40..44].copy_from_slice(b" EMF");
        put_u32le(&mut bytes, 44, 0x0001_0000);
        bytes.extend_from_slice(&emf_stretchdibits_record(raster_operation, source_x));
        append_emf_eof(&mut bytes, 3);
        bytes
    }

    fn emf_with_source_blt(record_type: u32) -> Vec<u8> {
        emf_with_source_blt_dib(record_type, &packed_rgb32_dib())
    }

    fn emf_with_source_blt_dib(record_type: u32, dib: &[u8]) -> Vec<u8> {
        let fixed_size = match record_type {
            76 => 100,
            77 => 108,
            _ => panic!("unsupported source blit record type"),
        };
        let bmi_len = bitmap_info_header_len(dib).expect("test DIB header");
        let bits_len = dib.len() - bmi_len;
        let mut bytes = vec![0u8; 88];
        put_u32le(&mut bytes, 0, 1);
        put_u32le(&mut bytes, 4, 88);
        bytes[40..44].copy_from_slice(b" EMF");
        put_u32le(&mut bytes, 44, 0x0001_0000);
        let start = bytes.len();
        let record_size = fixed_size + dib.len();
        bytes.resize(start + record_size, 0);
        put_u32le(&mut bytes, start, record_type);
        put_u32le(&mut bytes, start + 4, record_size as u32);
        put_i32le(&mut bytes, start + 32, 1);
        put_i32le(&mut bytes, start + 36, 1);
        put_u32le(&mut bytes, start + 40, 0x00CC_0020);
        put_u32le(&mut bytes, start + 52, 1.0f32.to_bits());
        put_u32le(&mut bytes, start + 64, 1.0f32.to_bits());
        put_u32le(&mut bytes, start + 84, fixed_size as u32);
        put_u32le(&mut bytes, start + 88, bmi_len as u32);
        put_u32le(&mut bytes, start + 92, (fixed_size + bmi_len) as u32);
        put_u32le(&mut bytes, start + 96, bits_len as u32);
        if record_type == 77 {
            put_i32le(&mut bytes, start + 100, 1);
            put_i32le(&mut bytes, start + 104, 1);
        }
        bytes[start + fixed_size..start + record_size].copy_from_slice(dib);
        append_emf_eof(&mut bytes, 3);
        bytes
    }

    fn append_emf_eof(bytes: &mut Vec<u8>, record_count: u32) {
        let start = bytes.len();
        bytes.resize(start + 20, 0);
        put_u32le(bytes, start, 14);
        put_u32le(bytes, start + 4, 20);
        put_u32le(bytes, start + 16, 20);
        let byte_len = bytes.len() as u32;
        put_u32le(bytes, 48, byte_len);
        put_u32le(bytes, 52, record_count);
    }

    fn insert_emf_setpixel_before_eof(bytes: &mut Vec<u8>) {
        let eof = bytes.split_off(bytes.len() - 20);
        let start = bytes.len();
        bytes.resize(start + 20, 0);
        put_u32le(bytes, start, 15);
        put_u32le(bytes, start + 4, 20);
        put_u32le(bytes, start + 16, 0x0000_00FF);
        bytes.extend_from_slice(&eof);
        let byte_len = bytes.len() as u32;
        put_u32le(bytes, 48, byte_len);
        put_u32le(bytes, 52, 4);
    }

    fn wmf_with_setdib_to_device(start_scan: u16, scan_count: u16) -> Vec<u8> {
        let dib = packed_rgb32_dib();
        let mut bytes = vec![0u8; 40];
        put_u32le(&mut bytes, 0, 0x9AC6_CDD7);
        put_u16le(&mut bytes, 10, 1);
        put_u16le(&mut bytes, 12, 1);
        put_u16le(&mut bytes, 14, 96);
        let start = bytes.len();
        let record_size = 24 + dib.len();
        bytes.resize(start + record_size, 0);
        put_u32le(&mut bytes, start, (record_size / 2) as u32);
        put_u16le(&mut bytes, start + 4, 0x0D33);
        put_u16le(&mut bytes, start + 8, scan_count);
        put_u16le(&mut bytes, start + 10, start_scan);
        put_u16le(&mut bytes, start + 16, 1);
        put_u16le(&mut bytes, start + 18, 1);
        bytes[start + 24..start + 24 + dib.len()].copy_from_slice(&dib);
        let eof = bytes.len();
        bytes.resize(eof + 6, 0);
        put_u32le(&mut bytes, eof, 3);
        finalize_wmf_profile(&mut bytes, record_size / 2);
        bytes
    }

    fn emf_stretchdibits_record(raster_operation: u32, source_x: i32) -> Vec<u8> {
        let dib = packed_rgb32_dib();
        let mut record = vec![0u8; 80 + dib.len()];
        let record_len = record.len() as u32;
        put_u32le(&mut record, 0, 81);
        put_u32le(&mut record, 4, record_len);
        put_i32le(&mut record, 32, source_x);
        put_i32le(&mut record, 40, 1);
        put_i32le(&mut record, 44, 1);
        put_u32le(&mut record, 48, 80);
        put_u32le(&mut record, 52, 40);
        put_u32le(&mut record, 56, 120);
        put_u32le(&mut record, 60, 4);
        put_u32le(&mut record, 68, raster_operation);
        put_i32le(&mut record, 72, 1);
        put_i32le(&mut record, 76, 1);
        record[80..].copy_from_slice(&dib);
        record
    }

    fn wmf_with_stretchdib(raster_operation: u32, source_x: i16) -> Vec<u8> {
        let dib = packed_rgb32_dib();
        let mut bytes = vec![0u8; 40];
        put_u32le(&mut bytes, 0, 0x9AC6_CDD7);
        put_u16le(&mut bytes, 10, 1);
        put_u16le(&mut bytes, 12, 1);
        put_u16le(&mut bytes, 14, 96);
        let start = bytes.len();
        let record_size = 28 + dib.len();
        bytes.resize(start + record_size, 0);
        put_u32le(&mut bytes, start, (record_size / 2) as u32);
        put_u16le(&mut bytes, start + 4, 0x0F43);
        put_u32le(&mut bytes, start + 6, raster_operation);
        put_u16le(&mut bytes, start + 12, 1);
        put_u16le(&mut bytes, start + 14, 1);
        put_u16le(&mut bytes, start + 18, source_x as u16);
        put_u16le(&mut bytes, start + 20, 1);
        put_u16le(&mut bytes, start + 22, 1);
        bytes[start + 28..start + 28 + dib.len()].copy_from_slice(&dib);
        let eof = bytes.len();
        bytes.resize(eof + 6, 0);
        put_u32le(&mut bytes, eof, 3);
        finalize_wmf_profile(&mut bytes, record_size / 2);
        bytes
    }

    fn wmf_with_source_dib_blt(function: u16) -> Vec<u8> {
        wmf_with_source_dib_blt_dib(function, &packed_rgb32_dib())
    }

    fn wmf_with_source_dib_blt_dib(function: u16, dib: &[u8]) -> Vec<u8> {
        let fixed_size = match function {
            0x0940 => 22,
            0x0B41 => 26,
            _ => panic!("unsupported source DIB blit function"),
        };
        let mut bytes = vec![0u8; 40];
        put_u32le(&mut bytes, 0, 0x9AC6_CDD7);
        put_u16le(&mut bytes, 10, 1);
        put_u16le(&mut bytes, 12, 1);
        put_u16le(&mut bytes, 14, 96);
        let start = bytes.len();
        let record_size = fixed_size + dib.len();
        bytes.resize(start + record_size, 0);
        put_u32le(&mut bytes, start, (record_size / 2) as u32);
        put_u16le(&mut bytes, start + 4, function);
        put_u32le(&mut bytes, start + 6, 0x00CC_0020);
        match function {
            0x0940 => {
                put_u16le(&mut bytes, start + 14, 1);
                put_u16le(&mut bytes, start + 16, 1);
            }
            0x0B41 => {
                put_u16le(&mut bytes, start + 10, 1);
                put_u16le(&mut bytes, start + 12, 1);
                put_u16le(&mut bytes, start + 18, 1);
                put_u16le(&mut bytes, start + 20, 1);
            }
            _ => unreachable!(),
        }
        bytes[start + fixed_size..start + record_size].copy_from_slice(dib);
        let eof = bytes.len();
        bytes.resize(eof + 6, 0);
        put_u32le(&mut bytes, eof, 3);
        finalize_wmf_profile(&mut bytes, record_size / 2);
        bytes
    }

    fn wmf_source_dib_blt_without_bitmap(function: u16) -> Vec<u8> {
        let record_words = match function {
            0x0940 => 12,
            0x0B41 => 14,
            _ => panic!("unsupported source DIB blit function"),
        };
        let record_size = record_words * 2;
        let mut bytes = vec![0u8; 40 + record_size + 6];
        put_u32le(&mut bytes, 0, 0x9AC6_CDD7);
        put_u16le(&mut bytes, 10, 1);
        put_u16le(&mut bytes, 12, 1);
        put_u16le(&mut bytes, 14, 96);
        put_u32le(&mut bytes, 40, record_words as u32);
        put_u16le(&mut bytes, 44, function);
        put_u32le(&mut bytes, 46, 0x00CC_0020);
        match function {
            0x0940 => {
                put_u16le(&mut bytes, 54, 1);
                put_u16le(&mut bytes, 56, 1);
            }
            0x0B41 => {
                put_u16le(&mut bytes, 50, 1);
                put_u16le(&mut bytes, 52, 1);
                put_u16le(&mut bytes, 58, 1);
                put_u16le(&mut bytes, 60, 1);
            }
            _ => unreachable!(),
        }
        let eof = 40 + record_size;
        put_u32le(&mut bytes, eof, 3);
        finalize_wmf_profile(&mut bytes, record_words);
        bytes
    }

    fn finalize_wmf_profile(bytes: &mut [u8], max_record_words: usize) {
        put_u16le(bytes, 22, 1);
        put_u16le(bytes, 24, 9);
        put_u16le(bytes, 26, 0x0300);
        put_u32le(bytes, 28, ((bytes.len() - 22) / 2) as u32);
        put_u16le(bytes, 32, 0);
        put_u32le(bytes, 34, max_record_words as u32);
        put_u16le(bytes, 38, 0);
        let checksum = (0..20).step_by(2).fold(0u16, |value, offset| {
            value ^ read_u16le(bytes, offset).unwrap()
        });
        put_u16le(bytes, 20, checksum);
    }

    fn insert_wmf_setpixel_before_eof(bytes: &mut Vec<u8>) {
        let eof = bytes.split_off(bytes.len() - 6);
        let start = bytes.len();
        bytes.resize(start + 14, 0);
        put_u32le(bytes, start, 7);
        put_u16le(bytes, start + 4, 0x041F);
        put_u32le(bytes, start + 6, 0x0000_00FF);
        bytes.extend_from_slice(&eof);
        let max_record_words = read_u32le(bytes, 34).unwrap() as usize;
        finalize_wmf_profile(bytes, max_record_words.max(7));
    }

    #[test]
    fn rgb32_reserved_byte_decodes_as_opaque() {
        let bmi = bitmap_info_header(1, -1, 32);
        let raster = decode_dib(&bmi, &[0x33, 0x22, 0x11, 0x00]).expect("32-bit RGB DIB");

        assert_eq!(raster.rgba, [0x11, 0x22, 0x33, 0xFF]);
    }

    #[test]
    fn oversized_gzip_payload_is_rejected_instead_of_truncated() {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&vec![0x5A; (1 << 20) + 1]).unwrap();
        let compressed = encoder.finish().unwrap();

        assert!(inflate_gzip_metafile(&compressed).is_none());
    }

    #[test]
    fn decoded_rgba_allocation_has_a_hard_64_mib_ceiling() {
        assert_eq!(dib_rgba_len(4096, 4096), Some(64 << 20));
        assert_eq!(dib_rgba_len(4096, 4097), None);
        assert_eq!(dib_rgba_len(u32::MAX, u32::MAX), None);
    }

    #[test]
    fn emf_bitmap_offsets_cannot_overlap_fixed_record_fields() {
        let mut record = vec![0u8; 80];
        let bmi = bitmap_info_header(1, -1, 32);
        record[8..48].copy_from_slice(&bmi);
        put_u32le(&mut record, 48, 8);
        put_u32le(&mut record, 52, 40);
        put_u32le(&mut record, 56, 64);
        put_u32le(&mut record, 60, 4);
        record[64..68].copy_from_slice(&[0x33, 0x22, 0x11, 0x00]);

        assert!(extract_emf_stretchdibits_record(&record).is_none());
    }

    #[test]
    fn packed_8bit_palette_dib_decodes_indices() {
        let dib = packed_palette_dib(
            8,
            3,
            &[
                [0x00, 0x00, 0xFF, 0],
                [0x00, 0xFF, 0x00, 0],
                [0xFF, 0x00, 0x00, 0],
            ],
            &[0, 1, 2, 0],
        );

        let raster = decode_packed_dib(&dib).expect("8-bit palette DIB");
        assert_eq!(
            raster.rgba,
            [0xFF, 0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0xFF,]
        );
    }

    #[test]
    fn packed_4bit_palette_dib_decodes_odd_width_high_nibble_first() {
        let dib = packed_palette_dib(
            4,
            3,
            &[
                [0x00, 0x00, 0xFF, 0],
                [0x00, 0xFF, 0x00, 0],
                [0xFF, 0x00, 0x00, 0],
            ],
            &[0x01, 0x20, 0, 0],
        );

        let raster = decode_packed_dib(&dib).expect("4-bit palette DIB");
        assert_eq!(
            raster.rgba,
            [0xFF, 0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0xFF,]
        );
    }

    #[test]
    fn packed_1bit_palette_dib_decodes_non_byte_aligned_rows_msb_first() {
        let dib = packed_palette_dib(
            1,
            3,
            &[[0x00, 0x00, 0x00, 0], [0xFF, 0xFF, 0xFF, 0]],
            &[0b0100_0000, 0, 0, 0],
        );

        let raster = decode_packed_dib(&dib).expect("1-bit palette DIB");
        assert_eq!(
            raster.rgba,
            [0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0xFF,]
        );
    }

    #[test]
    fn palette_dib_rejects_missing_entries_and_out_of_range_indices() {
        let missing_palette = bitmap_info_header(1, -1, 8);
        assert!(decode_packed_dib(&missing_palette).is_none());

        let invalid_index = packed_palette_dib(
            8,
            1,
            &[[0x00, 0x00, 0x00, 0], [0xFF, 0xFF, 0xFF, 0]],
            &[2, 0, 0, 0],
        );
        assert!(decode_packed_dib(&invalid_index).is_none());
    }

    #[test]
    fn packed_16bit_bitfields_dib_decodes_rgb565() {
        let dib = packed_bitfields_dib(
            16,
            3,
            [0xF800, 0x07E0, 0x001F],
            &[0x00, 0xF8, 0xE0, 0x07, 0x1F, 0x00, 0, 0],
        );

        let raster = decode_packed_dib(&dib).expect("16-bit RGB565 DIB");
        assert_eq!(
            raster.rgba,
            [0xFF, 0x00, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0x00, 0x00, 0xFF, 0xFF,]
        );
    }

    #[test]
    fn packed_32bit_bitfields_dib_decodes_rgb_masks() {
        let dib = packed_bitfields_dib(
            32,
            1,
            [0x00FF_0000, 0x0000_FF00, 0x0000_00FF],
            &[0x33, 0x22, 0x11, 0x7F],
        );

        let raster = decode_packed_dib(&dib).expect("32-bit bitfields DIB");
        assert_eq!(raster.rgba, [0x11, 0x22, 0x33, 0xFF]);
    }

    #[test]
    fn bitfields_dib_rejects_overlapping_noncontiguous_and_out_of_range_masks() {
        for (bit_count, masks) in [
            (16, [0xF800, 0x0F00, 0x001F]),
            (16, [0xF800, 0x0560, 0x001F]),
            (16, [0x1_F000, 0x0F00, 0x001F]),
        ] {
            let dib = packed_bitfields_dib(bit_count, 1, masks, &[0, 0, 0, 0]);
            assert!(decode_packed_dib(&dib).is_none(), "masks={masks:?}");
        }
    }

    #[test]
    fn setdib_to_device_records_extract_full_frame_rasters() {
        for (kind, bytes) in [
            (MetafileKind::Emf, emf_with_setdibits_to_device(0, 1)),
            (MetafileKind::Wmf, wmf_with_setdib_to_device(0, 1)),
        ] {
            let raster = extract_raster(kind, &bytes, false).expect("full-frame SETDIB record");
            assert_eq!(raster.width_px, 1);
            assert_eq!(raster.height_px, 1);
            assert_eq!(raster.rgba, [0x11, 0x22, 0x33, 0xFF]);
        }
    }

    #[test]
    fn setdib_to_device_records_reject_partial_scan_ranges() {
        for (kind, bytes) in [
            (MetafileKind::Emf, emf_with_setdibits_to_device(1, 1)),
            (MetafileKind::Wmf, wmf_with_setdib_to_device(1, 1)),
        ] {
            assert!(extract_raster(kind, &bytes, false).is_none());
        }
    }

    #[test]
    fn source_bearing_bitblt_records_extract_identity_srccopy_rasters() {
        for (kind, bytes) in [
            (MetafileKind::Emf, emf_with_source_blt(76)),
            (MetafileKind::Emf, emf_with_source_blt(77)),
            (MetafileKind::Wmf, wmf_with_source_dib_blt(0x0940)),
            (MetafileKind::Wmf, wmf_with_source_dib_blt(0x0B41)),
        ] {
            let raster = extract_raster(kind, &bytes, false).expect("source-bearing BitBlt raster");
            assert_eq!(raster.width_px, 1);
            assert_eq!(raster.height_px, 1);
            assert_eq!(raster.rgba, [0x11, 0x22, 0x33, 0xFF]);
        }
    }

    #[test]
    fn source_bearing_bitblt_records_accept_matching_nonzero_frame_origins() {
        for record_type in [76, 77] {
            let mut emf = emf_with_source_blt(record_type);
            put_i32le(&mut emf, 8, 10);
            put_i32le(&mut emf, 12, 20);
            put_i32le(&mut emf, 16, 10);
            put_i32le(&mut emf, 20, 20);
            put_i32le(&mut emf, 88 + 8, 10);
            put_i32le(&mut emf, 88 + 12, 20);
            put_i32le(&mut emf, 88 + 16, 10);
            put_i32le(&mut emf, 88 + 20, 20);
            put_i32le(&mut emf, 88 + 24, 10);
            put_i32le(&mut emf, 88 + 28, 20);
            assert!(
                extract_raster(MetafileKind::Emf, &emf, false).is_some(),
                "record_type={record_type}"
            );
        }

        for (function, destination_y, destination_x) in [(0x0940, 18, 20), (0x0B41, 22, 24)] {
            let mut wmf = wmf_with_source_dib_blt(function);
            put_i16le(&mut wmf, 6, -10);
            put_i16le(&mut wmf, 8, -20);
            put_i16le(&mut wmf, 10, -9);
            put_i16le(&mut wmf, 12, -19);
            put_i16le(&mut wmf, 40 + destination_y, -20);
            put_i16le(&mut wmf, 40 + destination_x, -10);
            let max_record_words = read_u32le(&wmf, 34).unwrap() as usize;
            finalize_wmf_profile(&mut wmf, max_record_words);
            assert!(
                extract_raster(MetafileKind::Wmf, &wmf, false).is_some(),
                "function={function:#06x}"
            );
        }
    }

    #[test]
    fn source_bearing_bitblt_records_require_srccopy_and_zero_source_origins() {
        for (kind, mut bytes, raster_operation) in [
            (MetafileKind::Emf, emf_with_source_blt(76), 88 + 40),
            (MetafileKind::Emf, emf_with_source_blt(77), 88 + 40),
            (MetafileKind::Wmf, wmf_with_source_dib_blt(0x0940), 40 + 6),
            (MetafileKind::Wmf, wmf_with_source_dib_blt(0x0B41), 40 + 6),
        ] {
            put_u32le(&mut bytes, raster_operation, 0x0066_0046);
            assert!(extract_raster(kind, &bytes, false).is_none());
        }

        for (kind, bytes, source_offsets) in [
            (
                MetafileKind::Emf,
                emf_with_source_blt(76),
                [88 + 44, 88 + 48],
            ),
            (
                MetafileKind::Emf,
                emf_with_source_blt(77),
                [88 + 44, 88 + 48],
            ),
            (
                MetafileKind::Wmf,
                wmf_with_source_dib_blt(0x0940),
                [40 + 10, 40 + 12],
            ),
            (
                MetafileKind::Wmf,
                wmf_with_source_dib_blt(0x0B41),
                [40 + 14, 40 + 16],
            ),
        ] {
            for offset in source_offsets {
                let mut shifted = bytes.clone();
                if kind == MetafileKind::Emf {
                    put_i32le(&mut shifted, offset, 1);
                } else {
                    put_i16le(&mut shifted, offset, 1);
                }
                assert!(
                    extract_raster(kind, &shifted, false).is_none(),
                    "kind={kind:?}, offset={offset}"
                );
            }
        }
    }

    #[test]
    fn source_bearing_bitblt_records_reject_scaling_and_invalid_dimensions() {
        for (kind, bytes, dimension_offsets) in [
            (
                MetafileKind::Emf,
                emf_with_source_blt(76),
                [88 + 32, 88 + 36],
            ),
            (
                MetafileKind::Emf,
                emf_with_source_blt(77),
                [88 + 32, 88 + 36],
            ),
            (
                MetafileKind::Wmf,
                wmf_with_source_dib_blt(0x0940),
                [40 + 14, 40 + 16],
            ),
            (
                MetafileKind::Wmf,
                wmf_with_source_dib_blt(0x0B41),
                [40 + 18, 40 + 20],
            ),
        ] {
            for offset in dimension_offsets {
                let mut zero = bytes.clone();
                if kind == MetafileKind::Emf {
                    put_i32le(&mut zero, offset, 0);
                } else {
                    put_i16le(&mut zero, offset, 0);
                }
                assert!(extract_raster(kind, &zero, false).is_none());
            }
        }

        let mut emf_scaled = emf_with_source_blt(77);
        put_i32le(&mut emf_scaled, 88 + 100, 2);
        assert!(extract_raster(MetafileKind::Emf, &emf_scaled, false).is_none());

        let mut emf_negative = emf_with_source_blt(77);
        put_i32le(&mut emf_negative, 88 + 104, -1);
        assert!(extract_raster(MetafileKind::Emf, &emf_negative, false).is_none());

        let mut wmf_scaled = wmf_with_source_dib_blt(0x0B41);
        put_i16le(&mut wmf_scaled, 40 + 12, 2);
        assert!(extract_raster(MetafileKind::Wmf, &wmf_scaled, false).is_none());

        let mut wmf_negative = wmf_with_source_dib_blt(0x0B41);
        put_i16le(&mut wmf_negative, 40 + 10, -1);
        assert!(extract_raster(MetafileKind::Wmf, &wmf_negative, false).is_none());
    }

    #[test]
    fn emf_source_blits_require_exact_identity_source_state() {
        for (offset, invalid) in [
            (52, 0.0f32.to_bits()),
            (56, 1.0f32.to_bits()),
            (60, 1.0f32.to_bits()),
            (64, 0.0f32.to_bits()),
            (68, 1.0f32.to_bits()),
            (72, 1.0f32.to_bits()),
            (56, (-0.0f32).to_bits()),
            (52, f32::NAN.to_bits()),
        ] {
            let mut emf = emf_with_source_blt(76);
            put_u32le(&mut emf, 88 + offset, invalid);
            assert!(
                extract_raster(MetafileKind::Emf, &emf, false).is_none(),
                "offset={offset}, value={invalid:#010x}"
            );
        }

        let mut color_reserved = emf_with_source_blt(76);
        put_u32le(&mut color_reserved, 88 + 76, 0x0100_0000);
        assert!(extract_raster(MetafileKind::Emf, &color_reserved, false).is_none());

        let mut palette_usage = emf_with_source_blt(76);
        put_u32le(&mut palette_usage, 88 + 80, 1);
        assert!(extract_raster(MetafileKind::Emf, &palette_usage, false).is_none());
    }

    #[test]
    fn source_bearing_bitblt_records_require_exact_full_frame_coverage() {
        for record_type in [76, 77] {
            for destination_offset in [24, 28] {
                let mut shifted = emf_with_source_blt(record_type);
                put_i32le(&mut shifted, 88 + destination_offset, 1);
                assert!(extract_raster(MetafileKind::Emf, &shifted, false).is_none());
            }
        }

        let mut contradictory_bounds = emf_with_source_blt(76);
        put_i32le(&mut contradictory_bounds, 88 + 8, 100);
        put_i32le(&mut contradictory_bounds, 88 + 12, 100);
        put_i32le(&mut contradictory_bounds, 88 + 16, 100);
        put_i32le(&mut contradictory_bounds, 88 + 20, 100);
        assert!(extract_raster(MetafileKind::Emf, &contradictory_bounds, false).is_none());

        for (function, destination_offsets) in [(0x0940, [18, 20]), (0x0B41, [22, 24])] {
            for offset in destination_offsets {
                let mut shifted = wmf_with_source_dib_blt(function);
                put_i16le(&mut shifted, 40 + offset, 1);
                assert!(extract_raster(MetafileKind::Wmf, &shifted, false).is_none());
            }
        }
    }

    #[test]
    fn emf_source_blits_reject_monochrome_but_retain_other_palette_dibs() {
        let monochrome = packed_palette_dib(
            1,
            1,
            &[[0x00, 0x00, 0x00, 0], [0xFF, 0xFF, 0xFF, 0]],
            &[0, 0, 0, 0],
        );
        let indexed = packed_palette_dib(
            4,
            1,
            &[[0x00, 0x00, 0x00, 0], [0xFF, 0xFF, 0xFF, 0]],
            &[0, 0, 0, 0],
        );

        for record_type in [76, 77] {
            assert!(extract_raster(
                MetafileKind::Emf,
                &emf_with_source_blt_dib(record_type, &monochrome),
                false,
            )
            .is_none());
            assert!(extract_raster(
                MetafileKind::Emf,
                &emf_with_source_blt_dib(record_type, &indexed),
                false,
            )
            .is_some());
        }

        for function in [0x0940, 0x0B41] {
            assert!(extract_raster(
                MetafileKind::Wmf,
                &wmf_with_source_dib_blt_dib(function, &monochrome),
                false,
            )
            .is_some());
        }
    }

    #[test]
    fn emf_source_blits_require_canonical_contiguous_dib_payloads() {
        for (field_offset, invalid) in [(84, 104), (88, 36), (92, 136), (96, 3)] {
            let mut emf = emf_with_source_blt(76);
            put_u32le(&mut emf, 88 + field_offset, invalid);
            assert!(
                extract_raster(MetafileKind::Emf, &emf, false).is_none(),
                "field_offset={field_offset}"
            );
        }

        let mut tail = emf_with_source_blt(76);
        let eof = tail.split_off(tail.len() - 20);
        let record_size = read_u32le(&tail, 88 + 4).unwrap();
        tail.extend_from_slice(&[0; 4]);
        tail.extend_from_slice(&eof);
        put_u32le(&mut tail, 88 + 4, record_size + 4);
        let byte_len = tail.len() as u32;
        put_u32le(&mut tail, 48, byte_len);
        assert!(extract_raster(MetafileKind::Emf, &tail, false).is_none());
    }

    #[test]
    fn wmf_source_blits_reject_source_less_forms_and_nearby_functions() {
        for function in [0x0940, 0x0B41] {
            assert!(extract_raster(
                MetafileKind::Wmf,
                &wmf_source_dib_blt_without_bitmap(function),
                false,
            )
            .is_none());

            let mut nearby = wmf_with_source_dib_blt(function);
            put_u16le(&mut nearby, 40 + 4, function ^ 0x0100);
            assert!(extract_raster(MetafileKind::Wmf, &nearby, false).is_none());
        }
    }

    #[test]
    fn source_bearing_bitblt_records_reject_malformed_payloads_and_tails() {
        let mut malformed_emf = emf_with_source_blt(76);
        put_u16le(&mut malformed_emf, 88 + 100 + 14, 2);
        assert!(extract_raster(MetafileKind::Emf, &malformed_emf, false).is_none());

        let mut malformed_wmf = wmf_with_source_dib_blt(0x0940);
        put_u16le(&mut malformed_wmf, 40 + 22 + 14, 2);
        assert!(extract_raster(MetafileKind::Wmf, &malformed_wmf, false).is_none());

        let mut wmf_tail = wmf_with_source_dib_blt(0x0B41);
        let eof = wmf_tail.split_off(wmf_tail.len() - 6);
        let record_words = read_u32le(&wmf_tail, 40).unwrap() as usize;
        wmf_tail.extend_from_slice(&[0; 2]);
        wmf_tail.extend_from_slice(&eof);
        put_u32le(&mut wmf_tail, 40, (record_words + 1) as u32);
        finalize_wmf_profile(&mut wmf_tail, record_words + 1);
        assert!(extract_raster(MetafileKind::Wmf, &wmf_tail, false).is_none());
    }

    #[test]
    fn source_bearing_bitblt_profiles_reject_composition_and_duplicate_rasters() {
        let mut emf_composed = emf_with_source_blt(76);
        insert_emf_setpixel_before_eof(&mut emf_composed);
        assert!(extract_raster(MetafileKind::Emf, &emf_composed, false).is_none());

        let mut wmf_composed = wmf_with_source_dib_blt(0x0940);
        insert_wmf_setpixel_before_eof(&mut wmf_composed);
        assert!(extract_raster(MetafileKind::Wmf, &wmf_composed, false).is_none());

        let mut emf_duplicate = emf_with_source_blt(77);
        let record = emf_duplicate[88..emf_duplicate.len() - 20].to_vec();
        let eof = emf_duplicate.split_off(emf_duplicate.len() - 20);
        emf_duplicate.extend_from_slice(&record);
        emf_duplicate.extend_from_slice(&eof);
        let byte_len = emf_duplicate.len() as u32;
        put_u32le(&mut emf_duplicate, 48, byte_len);
        put_u32le(&mut emf_duplicate, 52, 4);
        assert!(extract_raster(MetafileKind::Emf, &emf_duplicate, false).is_none());

        let mut wmf_duplicate = wmf_with_source_dib_blt(0x0B41);
        let record = wmf_duplicate[40..wmf_duplicate.len() - 6].to_vec();
        let eof = wmf_duplicate.split_off(wmf_duplicate.len() - 6);
        wmf_duplicate.extend_from_slice(&record);
        wmf_duplicate.extend_from_slice(&eof);
        let max_record_words = read_u32le(&wmf_duplicate, 34).unwrap() as usize;
        finalize_wmf_profile(&mut wmf_duplicate, max_record_words);
        assert!(extract_raster(MetafileKind::Wmf, &wmf_duplicate, false).is_none());
    }

    #[test]
    fn source_bearing_bitblt_records_extract_from_gzip_wrappers() {
        for (kind, bytes) in [
            (MetafileKind::Emf, emf_with_source_blt(76)),
            (MetafileKind::Emf, emf_with_source_blt(77)),
            (MetafileKind::Wmf, wmf_with_source_dib_blt(0x0940)),
            (MetafileKind::Wmf, wmf_with_source_dib_blt(0x0B41)),
        ] {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&bytes).unwrap();
            let compressed = encoder.finish().unwrap();
            assert!(extract_raster(kind, &compressed, true).is_some());
        }
    }

    #[test]
    fn stretchdib_records_require_identity_srccopy_transfers() {
        assert!(
            extract_emf_stretchdibits_record(&emf_stretchdibits_record(0x00CC_0020, 0)).is_some()
        );
        assert!(extract_raster(
            MetafileKind::Wmf,
            &wmf_with_stretchdib(0x00CC_0020, 0),
            false,
        )
        .is_some());

        assert!(
            extract_emf_stretchdibits_record(&emf_stretchdibits_record(0x0066_0046, 0)).is_none()
        );
        assert!(
            extract_emf_stretchdibits_record(&emf_stretchdibits_record(0x00CC_0020, 1)).is_none()
        );
        assert!(extract_raster(
            MetafileKind::Wmf,
            &wmf_with_stretchdib(0x0066_0046, 0),
            false,
        )
        .is_none());
        assert!(extract_raster(
            MetafileKind::Wmf,
            &wmf_with_stretchdib(0x00CC_0020, 1),
            false,
        )
        .is_none());
    }

    #[test]
    fn emf_bounds_are_inclusive() {
        let mut bytes = vec![0u8; 88];
        put_u32le(&mut bytes, 0, 1);
        put_u32le(&mut bytes, 4, 88);
        put_i32le(&mut bytes, 8, 10);
        put_i32le(&mut bytes, 12, 20);
        put_i32le(&mut bytes, 16, 11);
        put_i32le(&mut bytes, 20, 22);
        bytes[40..44].copy_from_slice(b" EMF");
        put_u32le(&mut bytes, 44, 0x0001_0000);

        assert_eq!(
            dimensions(MetafileKind::Emf, &bytes, false),
            (Some(2), Some(3))
        );
    }

    #[test]
    fn emf_single_dib_profile_requires_terminal_eof() {
        let mut bytes = emf_with_setdibits_to_device(0, 1);
        bytes.truncate(bytes.len() - 20);

        assert!(extract_raster(MetafileKind::Emf, &bytes, false).is_none());
    }

    #[test]
    fn emf_eof_accepts_standard_empty_palette_offset() {
        let mut bytes = emf_with_setdibits_to_device(0, 1);
        let eof = bytes.len() - 20;
        put_u32le(&mut bytes, eof + 12, 16);

        assert!(extract_raster(MetafileKind::Emf, &bytes, false).is_some());
    }

    #[test]
    fn single_dib_profiles_reject_shifted_destinations() {
        let mut emf_set = emf_with_setdibits_to_device(0, 1);
        put_i32le(&mut emf_set, 88 + 24, 1);
        assert!(extract_raster(MetafileKind::Emf, &emf_set, false).is_none());

        let mut emf_stretch = emf_with_stretchdib(0x00CC_0020, 0);
        put_i32le(&mut emf_stretch, 88 + 28, 1);
        assert!(extract_raster(MetafileKind::Emf, &emf_stretch, false).is_none());

        let mut wmf_set = wmf_with_setdib_to_device(0, 1);
        put_u16le(&mut wmf_set, 40 + 22, 1);
        assert!(extract_raster(MetafileKind::Wmf, &wmf_set, false).is_none());

        let mut wmf_stretch = wmf_with_stretchdib(0x00CC_0020, 0);
        put_u16le(&mut wmf_stretch, 40 + 24, 1);
        assert!(extract_raster(MetafileKind::Wmf, &wmf_stretch, false).is_none());
    }

    #[test]
    fn single_dib_profiles_accept_matching_nonzero_frame_origins() {
        let mut emf = emf_with_setdibits_to_device(0, 1);
        put_i32le(&mut emf, 8, 10);
        put_i32le(&mut emf, 12, 20);
        put_i32le(&mut emf, 16, 10);
        put_i32le(&mut emf, 20, 20);
        put_i32le(&mut emf, 88 + 8, 10);
        put_i32le(&mut emf, 88 + 12, 20);
        put_i32le(&mut emf, 88 + 16, 10);
        put_i32le(&mut emf, 88 + 20, 20);
        put_i32le(&mut emf, 88 + 24, 10);
        put_i32le(&mut emf, 88 + 28, 20);
        assert!(extract_raster(MetafileKind::Emf, &emf, false).is_some());

        let mut wmf = wmf_with_setdib_to_device(0, 1);
        put_u16le(&mut wmf, 6, 10);
        put_u16le(&mut wmf, 8, 20);
        put_u16le(&mut wmf, 10, 11);
        put_u16le(&mut wmf, 12, 21);
        put_u16le(&mut wmf, 40 + 20, 20);
        put_u16le(&mut wmf, 40 + 22, 10);
        let max_record_words = read_u32le(&wmf, 34).unwrap() as usize;
        finalize_wmf_profile(&mut wmf, max_record_words);
        assert!(extract_raster(MetafileKind::Wmf, &wmf, false).is_some());
    }

    #[test]
    fn wmf_setdib_coordinates_do_not_alias_negative_frame_origins() {
        let mut wmf = wmf_with_setdib_to_device(0, 1);
        put_u16le(&mut wmf, 6, u16::MAX);
        put_u16le(&mut wmf, 10, 0);
        put_u16le(&mut wmf, 40 + 22, u16::MAX);
        let max_record_words = read_u32le(&wmf, 34).unwrap() as usize;
        finalize_wmf_profile(&mut wmf, max_record_words);

        assert!(extract_raster(MetafileKind::Wmf, &wmf, false).is_none());
    }

    #[test]
    fn emf_single_dib_profile_rejects_contradictory_record_bounds() {
        let mut emf = emf_with_setdibits_to_device(0, 1);
        put_i32le(&mut emf, 88 + 8, 100);
        put_i32le(&mut emf, 88 + 12, 100);
        put_i32le(&mut emf, 88 + 16, 100);
        put_i32le(&mut emf, 88 + 20, 100);

        assert!(extract_raster(MetafileKind::Emf, &emf, false).is_none());
    }

    #[test]
    fn single_dib_profiles_reject_later_vector_composition() {
        let mut emf = emf_with_setdibits_to_device(0, 1);
        insert_emf_setpixel_before_eof(&mut emf);
        assert!(extract_raster(MetafileKind::Emf, &emf, false).is_none());

        let mut wmf = wmf_with_setdib_to_device(0, 1);
        insert_wmf_setpixel_before_eof(&mut wmf);
        assert!(extract_raster(MetafileKind::Wmf, &wmf, false).is_none());
    }

    #[test]
    fn single_dib_profiles_reject_inconsistent_headers() {
        let mut emf_bytes = emf_with_setdibits_to_device(0, 1);
        put_u32le(&mut emf_bytes, 48, 1);
        assert!(extract_raster(MetafileKind::Emf, &emf_bytes, false).is_none());

        let mut emf_records = emf_with_setdibits_to_device(0, 1);
        put_u32le(&mut emf_records, 52, 2);
        assert!(extract_raster(MetafileKind::Emf, &emf_records, false).is_none());

        let mut wmf_size = wmf_with_setdib_to_device(0, 1);
        put_u32le(&mut wmf_size, 28, 1);
        assert!(extract_raster(MetafileKind::Wmf, &wmf_size, false).is_none());

        let mut wmf_checksum = wmf_with_setdib_to_device(0, 1);
        put_u16le(&mut wmf_checksum, 20, 0);
        assert!(extract_raster(MetafileKind::Wmf, &wmf_checksum, false).is_none());
    }

    #[test]
    fn placeable_wmf_profile_accepts_both_defined_header_types() {
        let memory = wmf_with_setdib_to_device(0, 1);
        assert!(extract_raster(MetafileKind::Wmf, &memory, false).is_some());

        let mut disk = wmf_with_setdib_to_device(0, 1);
        put_u16le(&mut disk, 22, 2);
        assert!(extract_raster(MetafileKind::Wmf, &disk, false).is_some());
    }

    #[test]
    fn emf_headers_require_standard_version_and_zero_reserved_field() {
        let mut version = emf_with_setdibits_to_device(0, 1);
        put_u32le(&mut version, 44, 0);
        assert_eq!(dimensions(MetafileKind::Emf, &version, false), (None, None));
        assert!(extract_raster(MetafileKind::Emf, &version, false).is_none());

        let mut reserved = emf_with_setdibits_to_device(0, 1);
        put_u16le(&mut reserved, 58, 1);
        assert_eq!(
            dimensions(MetafileKind::Emf, &reserved, false),
            (None, None)
        );
        assert!(extract_raster(MetafileKind::Emf, &reserved, false).is_none());
    }

    #[test]
    fn packed_dib_rejects_unexplained_payload_tail() {
        let mut dib = packed_rgb32_dib();
        dib.extend_from_slice(&[0; 4]);

        assert!(decode_packed_dib(&dib).is_none());
    }

    #[test]
    fn emf_dib_rejects_unexplained_record_gaps_and_tails() {
        let mut tail = emf_with_setdibits_to_device(0, 1);
        let eof = tail.split_off(tail.len() - 20);
        let record_size = read_u32le(&tail, 88 + 4).unwrap();
        tail.extend_from_slice(&[0; 4]);
        tail.extend_from_slice(&eof);
        put_u32le(&mut tail, 88 + 4, record_size + 4);
        let byte_len = tail.len() as u32;
        put_u32le(&mut tail, 48, byte_len);
        assert!(extract_raster(MetafileKind::Emf, &tail, false).is_none());

        let mut prefix_gap = emf_with_setdibits_to_device(0, 1);
        let record_size = read_u32le(&prefix_gap, 88 + 4).unwrap();
        let bmi_offset = read_u32le(&prefix_gap, 88 + 48).unwrap() as usize;
        let bits_offset = read_u32le(&prefix_gap, 88 + 56).unwrap() as usize;
        prefix_gap.splice(88 + bmi_offset..88 + bmi_offset, [0; 4]);
        put_u32le(&mut prefix_gap, 88 + 4, record_size + 4);
        put_u32le(&mut prefix_gap, 88 + 48, bmi_offset as u32 + 4);
        put_u32le(&mut prefix_gap, 88 + 56, bits_offset as u32 + 4);
        let byte_len = prefix_gap.len() as u32;
        put_u32le(&mut prefix_gap, 48, byte_len);
        assert!(extract_raster(MetafileKind::Emf, &prefix_gap, false).is_none());

        let mut gap = emf_with_setdibits_to_device(0, 1);
        let record_size = read_u32le(&gap, 88 + 4).unwrap();
        let bits_offset = read_u32le(&gap, 88 + 56).unwrap() as usize;
        let insert_at = 88 + bits_offset;
        gap.splice(insert_at..insert_at, [0; 4]);
        put_u32le(&mut gap, 88 + 4, record_size + 4);
        put_u32le(&mut gap, 88 + 56, bits_offset as u32 + 4);
        let byte_len = gap.len() as u32;
        put_u32le(&mut gap, 48, byte_len);
        assert!(extract_raster(MetafileKind::Emf, &gap, false).is_none());
    }

    #[test]
    fn placeable_wmf_profile_requires_zero_handle_and_reserved_fields() {
        let mut handle = wmf_with_setdib_to_device(0, 1);
        put_u16le(&mut handle, 4, 1);
        let max_record_words = read_u32le(&handle, 34).unwrap() as usize;
        finalize_wmf_profile(&mut handle, max_record_words);
        assert!(extract_raster(MetafileKind::Wmf, &handle, false).is_none());

        let mut reserved = wmf_with_setdib_to_device(0, 1);
        put_u32le(&mut reserved, 16, 1);
        let max_record_words = read_u32le(&reserved, 34).unwrap() as usize;
        finalize_wmf_profile(&mut reserved, max_record_words);
        assert!(extract_raster(MetafileKind::Wmf, &reserved, false).is_none());
    }
}
