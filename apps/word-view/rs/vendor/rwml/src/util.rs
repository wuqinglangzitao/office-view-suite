//! Bounds-checked little-endian primitives. Every read returns `None` on
//! out-of-bounds so the parser degrades to an error instead of panicking on
//! malformed input.

#[inline]
pub(crate) fn u16le(b: &[u8], off: usize) -> Option<u16> {
    let s = b.get(off..off + 2)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

#[inline]
pub(crate) fn u32le(b: &[u8], off: usize) -> Option<u32> {
    let s = b.get(off..off + 4)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
