//! Character-property (CHPX) reading — per-run bold/italic/underline/strike/
//! hidden, for the rich document model.
//!
//! The `PlcfBteChpx` bin table (FIB `fcPlcfBteChpx`, in the table stream) points
//! to 512-byte **CHPX FKP** pages in the `WordDocument` stream. Each FKP maps FC
//! ranges to a `Chpx` (`cb` byte + `grpprl`); we scan the grpprl for the
//! character `sprm`s that affect extracted/rendered text.
//!
//! The FKP shape is the PAPX FKP's sibling, with one difference: the per-run
//! offset array `rgb` is a single byte per run (a word offset; `0` = default
//! properties), where PAPX uses a 13-byte `BxPap`.
//!
//! Reference: [MS-DOC] 2.8.26 (PlcBteChpx), 2.9.32 (ChpxFkp), 2.9.31 (Chpx),
//! 2.6.1 (character sprms).

use crate::model::{Color, VertAlign};
use crate::util::{u16le, u32le};

const FKP_SIZE: usize = 512;
/// Upper bound on CHPX bin-table page iterations *and* accumulated entries — far above any
/// real `.doc` (character property runs), but it bounds a crafted bin-table that would
/// otherwise amplify a small table into billions of entries (memory/CPU DoS).
const MAX_FKP_ENTRIES: usize = 1 << 20;

// Test-lowerable copy of the cap (see papx.rs); production always uses `MAX_FKP_ENTRIES`.
#[cfg(test)]
thread_local! {
    static TEST_MAX_FKP: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_FKP_ENTRIES) };
}
#[cfg(test)]
fn set_test_max_fkp(n: usize) {
    TEST_MAX_FKP.with(|c| c.set(n));
}
fn max_fkp_entries() -> usize {
    #[cfg(test)]
    {
        TEST_MAX_FKP.with(|c| c.get())
    }
    #[cfg(not(test))]
    {
        MAX_FKP_ENTRIES
    }
}

// Character sprms (sgc = 2). Toggle operands are 1 byte: 0 off, 1 on, 0x80
// inherit-from-style, 0x81 invert-style.
const SPRM_C_F_BOLD: u16 = 0x0835;
const SPRM_C_F_ITALIC: u16 = 0x0836;
const SPRM_C_F_STRIKE: u16 = 0x0837;
const SPRM_C_F_SMALL_CAPS: u16 = 0x083A;
const SPRM_C_F_CAPS: u16 = 0x083B;
const SPRM_C_F_VANISH: u16 = 0x083C; // hidden text (NOT 0x0838 — that is Outline)
const SPRM_C_F_SPEC: u16 = 0x0855; // run's special char is a real object (1-byte)
const SPRM_C_F_BIDI: u16 = 0x085A; // right-to-left run layout (ToggleOperand)
const SPRM_C_HIGHLIGHT: u16 = 0x2A0C; // one-byte Ico highlight palette
const SPRM_C_ISTD: u16 = 0x4A30; // apply a character style
const SPRM_C_ISTD_PERMUTE: u16 = 0xCA31; // conditionally remap character style
const SPRM_C_DEFAULT: u16 = 0x2A32; // reset direct character properties
const SPRM_C_PLAIN: u16 = 0x2A33; // reset to the paragraph style
const SPRM_C_KUL: u16 = 0x2A3E; // underline kind (0 = none)
const SPRM_C_PIC_LOCATION: u16 = 0x6A03; // fcPic into the Data stream (4-byte)
const SPRM_C_HPS: u16 = 0x4A43; // font size, half-points (2-byte)
const SPRM_C_RG_FTC0: u16 = 0x4A4F; // font index into SttbfFfn (2-byte)
const SPRM_C_MAJORITY: u16 = 0xCA47; // conditional reset to paragraph style
const SPRM_C_ISS: u16 = 0x2A48; // 0 normal, 1 superscript, 2 subscript
const SPRM_C_CV: u16 = 0x6870; // 24-bit color COLORREF (4-byte)
const SPRM_C_ICO: u16 = 0x2A42; // legacy 0–16 palette color index (1-byte)

// Mixed-property PRCs can contain these non-character variable operands.
const SPRM_P_CHG_TABS: u16 = 0xC615;
const SPRM_T_DEF_TABLE: u16 = 0xD608;
const TC80_LEN: usize = 20;

// Prm0 uses compact `isprm` values rather than the 16-bit Sprm encodings above.
const ISPRM_C_F_BOLD: u8 = 0x55;
const ISPRM_C_F_ITALIC: u8 = 0x56;
const ISPRM_C_F_STRIKE: u8 = 0x57;
const ISPRM_C_F_SMALL_CAPS: u8 = 0x5A;
const ISPRM_C_F_CAPS: u8 = 0x5B;
const ISPRM_C_F_VANISH: u8 = 0x5C;

/// Map a legacy `sprmCIco` palette index (0–16) to RGB ([MS-DOC] Ico).
pub(crate) fn ico_color(i: u8) -> Option<Color> {
    let (r, g, b) = match i {
        0 | 1 => (0, 0, 0),       // auto / black
        2 => (0, 0, 0xFF),        // blue
        3 => (0, 0xFF, 0xFF),     // cyan
        4 => (0, 0xFF, 0),        // green
        5 => (0xFF, 0, 0xFF),     // magenta
        6 => (0xFF, 0, 0),        // red
        7 => (0xFF, 0xFF, 0),     // yellow
        8 => (0xFF, 0xFF, 0xFF),  // white
        9 => (0, 0, 0x80),        // dark blue
        10 => (0, 0x80, 0x80),    // dark cyan
        11 => (0, 0x80, 0),       // dark green
        12 => (0x80, 0, 0x80),    // dark magenta
        13 => (0x80, 0, 0),       // dark red
        14 => (0x80, 0x80, 0),    // dark yellow
        15 => (0x80, 0x80, 0x80), // dark grey
        16 => (0xC0, 0xC0, 0xC0), // light grey
        _ => return None,
    };
    Some(Color { r, g, b })
}

/// Map a legacy `sprmCHighlight` Ico to the existing Word highlight names.
pub(crate) fn highlight_name(i: u8) -> Option<&'static str> {
    match i {
        1 => Some("black"),
        2 => Some("blue"),
        3 => Some("cyan"),
        4 => Some("green"),
        5 => Some("magenta"),
        6 => Some("red"),
        7 => Some("yellow"),
        8 => Some("white"),
        9 => Some("darkBlue"),
        10 => Some("darkCyan"),
        11 => Some("darkGreen"),
        12 => Some("darkMagenta"),
        13 => Some("darkRed"),
        14 => Some("darkYellow"),
        15 => Some("darkGray"),
        16 => Some("lightGray"),
        _ => None,
    }
}

/// Resolved character properties scanned out of one CHPX grpprl.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Chp {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub hidden: bool,
    /// Font size in half-points (`sprmCHps`), if set.
    pub size_half_pt: Option<u16>,
    /// Font index into `SttbfFfn` (`sprmCRgFtc0`), resolved to a name by the
    /// assembler; `None` = inherit.
    pub ftc: Option<u16>,
    /// Text color (`sprmCCv` 24-bit, or legacy `sprmCIco` palette), if set.
    pub color: Option<Color>,
    /// Highlight Ico (`sprmCHighlight`), including explicit clear (`0`).
    pub highlight: Option<u8>,
    /// Direct `sprmCIss`, including explicit baseline.
    pub vert_align: Option<VertAlign>,
    /// Literal direct `sprmCFSmallCaps`; style-relative operands stay unknown.
    pub small_caps: Option<bool>,
    /// Literal direct `sprmCFCaps`; style-relative operands stay unknown.
    pub caps: Option<bool>,
    /// Literal direct `sprmCFBiDi`; style-relative operands stay unknown.
    pub rtl: Option<bool>,
    /// For a special-char run (`fSpec`) that is an inline picture, the `fcPic`
    /// offset into the `Data` stream.
    pub pic: Option<u32>,
}

impl Chp {
    /// Apply the bounded literal character-toggle subset of a PCD `Prm0`.
    ///
    /// Direct character formatting appends this modifier after CHPX, so a
    /// recognized literal operand replaces the corresponding CHPX value.
    pub(crate) fn apply_pcd_prm0(&mut self, raw: u16) {
        if raw & 1 != 0 {
            return;
        }
        let value = match (raw >> 8) as u8 {
            0 => false,
            1 => true,
            _ => return,
        };
        match ((raw >> 1) & 0x7F) as u8 {
            ISPRM_C_F_BOLD => self.bold = value,
            ISPRM_C_F_ITALIC => self.italic = value,
            ISPRM_C_F_STRIKE => self.strike = value,
            ISPRM_C_F_SMALL_CAPS => self.small_caps = Some(value),
            ISPRM_C_F_CAPS => self.caps = Some(value),
            ISPRM_C_F_VANISH => self.hidden = value,
            _ => {}
        }
    }

    /// Apply either the compact `Prm0` subset or a precompiled complex
    /// `Prm1` character overlay.
    pub(crate) fn apply_pcd_prm(&mut self, raw: u16, prm1_patches: &[Option<PcdPrm1Patch>]) {
        if raw & 1 == 0 {
            self.apply_pcd_prm0(raw);
            return;
        }
        if let Some(Some(patch)) = prm1_patches.get(usize::from(raw >> 1)) {
            patch.apply(self);
        }
    }

    /// Collapse distinct legacy encodings that materialize as the same shared
    /// model properties before run comparison.
    pub(crate) fn normalize_model_defaults(&mut self) {
        if self.highlight == Some(0) {
            self.highlight = None;
        }
        if self.vert_align == Some(VertAlign::Baseline) {
            self.vert_align = None;
        }
        if self.small_caps == Some(false) {
            self.small_caps = None;
        }
        if self.caps == Some(false) {
            self.caps = None;
        }
        if self.rtl == Some(false) {
            self.rtl = None;
        }
    }
}

/// Sparse deterministic character effects compiled once from one CLX PRC.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PcdPrm1Patch {
    bold: Option<bool>,
    italic: Option<bool>,
    underline: Option<bool>,
    strike: Option<bool>,
    hidden: Option<bool>,
    highlight: Option<u8>,
    vert_align: Option<VertAlign>,
    small_caps: Option<bool>,
    caps: Option<bool>,
    rtl: Option<bool>,
}

impl PcdPrm1Patch {
    fn apply(self, chp: &mut Chp) {
        if let Some(value) = self.bold {
            chp.bold = value;
        }
        if let Some(value) = self.italic {
            chp.italic = value;
        }
        if let Some(value) = self.underline {
            chp.underline = value;
        }
        if let Some(value) = self.strike {
            chp.strike = value;
        }
        if let Some(value) = self.hidden {
            chp.hidden = value;
        }
        if let Some(value) = self.highlight {
            chp.highlight = Some(value);
        }
        if let Some(value) = self.vert_align {
            chp.vert_align = Some(value);
        }
        if let Some(value) = self.small_caps {
            chp.small_caps = Some(value);
        }
        if let Some(value) = self.caps {
            chp.caps = Some(value);
        }
        if let Some(value) = self.rtl {
            chp.rtl = Some(value);
        }
    }
}

/// Compile every retained CLX PRC once. `None` is an inert malformed or
/// style-dependent group; callers then preserve the CHPX-derived result.
pub(crate) fn compile_pcd_prm1_patches(prcs: &[Vec<u8>]) -> Vec<Option<PcdPrm1Patch>> {
    prcs.iter()
        .map(|grpprl| compile_pcd_prm1_patch(grpprl))
        .collect()
}

fn compile_pcd_prm1_patch(grpprl: &[u8]) -> Option<PcdPrm1Patch> {
    let mut patch = PcdPrm1Patch::default();
    let mut pos = 0usize;
    while pos < grpprl.len() {
        let sprm = u16le(grpprl, pos)?;
        let operand_start = pos.checked_add(2)?;
        let operand_len = pcd_prm1_operand_len(sprm, grpprl, operand_start)?;
        let operand_end = operand_start.checked_add(operand_len)?;
        let operand = grpprl.get(operand_start..operand_end)?;

        if ((sprm >> 10) & 0x7) == 2 {
            // ToggleOperand style-relative values require effective style
            // state even when the specific character property is unmodeled.
            if ((sprm >> 13) & 0x7) == 0 && matches!(operand[0], 0x80 | 0x81) {
                return None;
            }
            match sprm {
                // These require effective character/paragraph style state.
                SPRM_C_ISTD | SPRM_C_ISTD_PERMUTE | SPRM_C_DEFAULT | SPRM_C_PLAIN
                | SPRM_C_MAJORITY => {
                    return None;
                }
                SPRM_C_F_BOLD => patch.bold = Some(literal_toggle(operand)?),
                SPRM_C_F_ITALIC => patch.italic = Some(literal_toggle(operand)?),
                SPRM_C_F_STRIKE => patch.strike = Some(literal_toggle(operand)?),
                SPRM_C_F_VANISH => patch.hidden = Some(literal_toggle(operand)?),
                SPRM_C_F_SMALL_CAPS => patch.small_caps = Some(literal_toggle(operand)?),
                SPRM_C_F_CAPS => patch.caps = Some(literal_toggle(operand)?),
                SPRM_C_F_BIDI => patch.rtl = Some(literal_toggle(operand)?),
                SPRM_C_KUL => patch.underline = Some(valid_kul(operand[0])? != 0),
                SPRM_C_HIGHLIGHT => {
                    let value = operand[0];
                    if value > 16 {
                        return None;
                    }
                    patch.highlight = Some(value);
                }
                SPRM_C_ISS => {
                    patch.vert_align = Some(match operand[0] {
                        0 => VertAlign::Baseline,
                        1 => VertAlign::Super,
                        2 => VertAlign::Sub,
                        _ => return None,
                    });
                }
                _ => {}
            }
        }
        pos = operand_end;
    }
    Some(patch)
}

fn literal_toggle(operand: &[u8]) -> Option<bool> {
    match operand.first().copied()? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn valid_kul(value: u8) -> Option<u8> {
    matches!(
        value,
        0x00 | 0x01
            | 0x02
            | 0x03
            | 0x04
            | 0x06
            | 0x07
            | 0x09
            | 0x0A
            | 0x0B
            | 0x14
            | 0x17
            | 0x19
            | 0x1A
            | 0x1B
            | 0x27
            | 0x2B
            | 0x37
    )
    .then_some(value)
}

fn pcd_prm1_operand_len(sprm: u16, data: &[u8], operand_start: usize) -> Option<usize> {
    match (sprm >> 13) & 0x7 {
        0 | 1 => Some(1),
        2 | 4 | 5 => Some(2),
        3 => Some(4),
        7 => Some(3),
        6 if sprm == SPRM_T_DEF_TABLE => tdef_table_operand_len(data, operand_start),
        6 if sprm == SPRM_P_CHG_TABS => pchg_tabs_operand_len(data, operand_start),
        6 => 1usize.checked_add(usize::from(*data.get(operand_start)?)),
        _ => None,
    }
}

fn tdef_table_operand_len(data: &[u8], operand_start: usize) -> Option<usize> {
    let cb = usize::from(u16le(data, operand_start)?);
    let total_len = cb.checked_add(1)?;
    let operand_end = operand_start.checked_add(total_len)?;
    let operand = data.get(operand_start..operand_end)?;

    let column_count = usize::from(*operand.get(2)?);
    if column_count > 63 {
        return None;
    }
    let center_count = column_count.checked_add(1)?;
    let centers_end = 3usize.checked_add(center_count.checked_mul(2)?)?;
    let centers = operand.get(3..centers_end)?;
    let mut previous = None;
    for center in centers.chunks_exact(2) {
        let value = i16::from_le_bytes([center[0], center[1]]);
        if previous.is_some_and(|previous| previous > value) {
            return None;
        }
        previous = Some(value);
    }
    if (total_len.checked_sub(centers_end)?) % TC80_LEN != 0 {
        return None;
    }
    Some(total_len)
}

pub(crate) fn pchg_tabs_operand_len(data: &[u8], operand_start: usize) -> Option<usize> {
    let cb = usize::from(*data.get(operand_start)?);
    if cb < 2 {
        return None;
    }
    let del_count_offset = operand_start.checked_add(1)?;
    let del_count = usize::from(*data.get(del_count_offset)?);
    if del_count > 64 {
        return None;
    }
    let add_count_offset = del_count_offset
        .checked_add(1)?
        .checked_add(del_count.checked_mul(4)?)?;
    let add_count = usize::from(*data.get(add_count_offset)?);
    if add_count > 64 {
        return None;
    }
    let remainder_len = add_count_offset
        .checked_add(1)?
        .checked_add(add_count.checked_mul(3)?)?
        .checked_sub(del_count_offset)?;
    if cb != u8::MAX as usize && cb != remainder_len {
        return None;
    }
    remainder_len.checked_add(1)
}

#[derive(Debug, Clone, Copy)]
struct ChpEntry {
    fc_start: u32,
    fc_lim: u32,
    chp: Chp,
}

/// All represented character-property ranges, sorted and non-overlapping by
/// FC, for point lookup by a character's FC.
#[derive(Debug, Default)]
pub(crate) struct ChpxTable {
    entries: Vec<ChpEntry>,
}

impl ChpxTable {
    /// The character properties at `WordDocument` byte offset `fc`. Default
    /// (all-off) when no CHPX range covers `fc`.
    pub(crate) fn chp_at(&self, fc: u32) -> Chp {
        let i = self.entries.partition_point(|e| e.fc_lim <= fc);
        self.entries
            .get(i)
            .filter(|e| e.fc_start <= fc)
            .map(|e| e.chp)
            .unwrap_or_default()
    }

    /// The `fcPic` (offset into the `Data` stream) for an inline-picture run at
    /// `fc`, if this run is a picture.
    pub(crate) fn pic_at(&self, fc: u32) -> Option<u32> {
        self.chp_at(fc).pic
    }
}

/// Parse the CHPX bin table + FKP pages. Returns an empty table (not an error)
/// when the structures are absent or malformed — runs then degrade to default
/// (unstyled) properties.
pub(crate) fn parse(word: &[u8], table: &[u8], fc_plcf: usize, lcb_plcf: usize) -> ChpxTable {
    let mut entries = Vec::new();
    if lcb_plcf < 4 {
        return ChpxTable { entries };
    }
    let Some(plc_end) = fc_plcf.checked_add(lcb_plcf) else {
        return ChpxTable { entries };
    };
    let Some(plc) = table.get(fc_plcf..plc_end) else {
        return ChpxTable { entries };
    };
    // PlcBteChpx: (n+1) FCs then n PnFkpChpx (4 bytes each). n = (lcb-4)/8.
    // A PLC size that does not yield a whole number of entries is malformed.
    let payload_len = plc.len() - 4;
    if payload_len % 8 != 0 {
        return ChpxTable { entries };
    }
    let declared_n = payload_len / 8;
    if declared_n == 0 {
        return ChpxTable { entries };
    }
    // The Pn array follows every declared FC, even when processing is capped.
    let Some(pn_base) = declared_n
        .checked_add(1)
        .and_then(|count| count.checked_mul(4))
    else {
        return ChpxTable { entries };
    };

    // Bound page iterations and accumulated entries: a crafted .doc can make
    // `n` huge and point every page number at one valid FKP page, amplifying a
    // small table into billions of entries (memory/CPU DoS).
    let cap = max_fkp_entries();
    let n = declared_n.min(cap);

    // Validate every outer range that this bounded parse will process before
    // accepting any page, so malformed order cannot yield a partial table.
    for i in 0..n {
        let Some(fc_start) = u32le(plc, i * 4) else {
            return ChpxTable {
                entries: Vec::new(),
            };
        };
        let Some(fc_lim) = u32le(plc, (i + 1) * 4) else {
            return ChpxTable {
                entries: Vec::new(),
            };
        };
        if fc_start >= fc_lim {
            return ChpxTable {
                entries: Vec::new(),
            };
        }
    }

    for i in 0..n {
        if entries.len() >= cap {
            break;
        }
        let Some(pn_raw) = u32le(plc, pn_base + i * 4) else {
            break;
        };
        let page = (pn_raw & 0x003F_FFFF) as usize; // low 22 bits = page number
        let Some(off) = page.checked_mul(FKP_SIZE) else {
            continue;
        };
        let Some(fc_start) = u32le(plc, i * 4) else {
            break;
        };
        let Some(fc_lim) = u32le(plc, (i + 1) * 4) else {
            break;
        };
        parse_fkp(
            word,
            off,
            fc_start,
            fc_lim,
            cap - entries.len(),
            &mut entries,
        );
    }
    ChpxTable { entries }
}

/// Parse one 512-byte CHPX FKP at `page_off`, appending its runs.
fn parse_fkp(
    word: &[u8],
    page_off: usize,
    outer_start: u32,
    outer_lim: u32,
    budget: usize,
    out: &mut Vec<ChpEntry>,
) {
    if budget == 0 {
        return;
    }
    let Some(page_end) = page_off.checked_add(FKP_SIZE) else {
        return;
    };
    let Some(page) = word.get(page_off..page_end) else {
        return;
    };
    let crun = page[FKP_SIZE - 1] as usize;
    // rgfc is (crun+1) u32; rgb is crun single bytes.
    if !(1..=0x65).contains(&crun) {
        return;
    }
    let rgfc_end = 4 * (crun + 1);
    let rgb_start = rgfc_end;
    let rgb_end = rgb_start + crun;
    if rgb_end > FKP_SIZE - 1 {
        return;
    }

    // Every run is [rgfc[i], rgfc[i+1]); malformed pages are ignored as a
    // unit so sorting or duplicates cannot create ambiguous lookup ranges.
    for i in 0..crun {
        let Some(fc_start) = u32le(page, i * 4) else {
            return;
        };
        let Some(fc_lim) = u32le(page, (i + 1) * 4) else {
            return;
        };
        if fc_start >= fc_lim {
            return;
        }
    }

    let mut remaining = budget;
    for i in 0..crun {
        if remaining == 0 {
            break;
        }
        let Some(run_start) = u32le(page, i * 4) else {
            break;
        };
        let Some(run_lim) = u32le(page, (i + 1) * 4) else {
            break;
        };
        let fc_start = run_start.max(outer_start);
        let fc_lim = run_lim.min(outer_lim);
        if fc_start >= fc_lim {
            continue;
        }
        let Some(&b) = page.get(rgb_start + i) else {
            break;
        };
        let chp = if b == 0 {
            Chp::default()
        } else {
            parse_chpx(page, b as usize * 2, rgb_end).unwrap_or_default()
        };
        out.push(ChpEntry {
            fc_start,
            fc_lim,
            chp,
        });
        remaining -= 1;
    }
}

/// Read a `Chpx` (cb byte + grpprl) at `off` within an FKP page.
fn parse_chpx(page: &[u8], off: usize, metadata_end: usize) -> Option<Chp> {
    // Nonzero rgb offsets point after rgfc+rgb, and neither the size byte nor
    // grpprl may consume the final crun byte.
    if off < metadata_end || off >= FKP_SIZE - 1 {
        return None;
    }
    let cb = *page.get(off)? as usize;
    let data_start = off.checked_add(1)?;
    let data_end = data_start.checked_add(cb)?;
    if data_end > FKP_SIZE - 1 {
        return None;
    }
    Some(scan_grpprl(page.get(data_start..data_end)?))
}

/// Walk a CHPX grpprl, extracting the styling toggles. Stops on an unsizeable
/// or truncated sprm.
fn scan_grpprl(gp: &[u8]) -> Chp {
    let mut chp = Chp::default();
    let mut fspec = false;
    let mut picloc = None;
    let mut pos = 0;
    while pos + 2 <= gp.len() {
        let Some(sprm) = u16le(gp, pos) else { break };
        let op = pos + 2;
        let Some(len) = operand_len(sprm, gp, op) else {
            break;
        };
        let Some(operand_end) = op.checked_add(len) else {
            break;
        };
        let Some(operand) = gp.get(op..operand_end) else {
            break;
        };
        let toggle = || matches!(operand.first().copied().unwrap_or(0), 0x01 | 0x81);
        match sprm {
            SPRM_C_F_BOLD => chp.bold = toggle(),
            SPRM_C_F_ITALIC => chp.italic = toggle(),
            SPRM_C_F_STRIKE => chp.strike = toggle(),
            SPRM_C_F_SMALL_CAPS => apply_direct_toggle(&mut chp.small_caps, operand[0]),
            SPRM_C_F_CAPS => apply_direct_toggle(&mut chp.caps, operand[0]),
            SPRM_C_F_VANISH => chp.hidden = toggle(),
            SPRM_C_F_SPEC => fspec = operand.first().copied().unwrap_or(0) != 0,
            SPRM_C_F_BIDI => apply_direct_toggle(&mut chp.rtl, operand[0]),
            SPRM_C_KUL => chp.underline = operand.first().copied().unwrap_or(0) != 0,
            SPRM_C_PIC_LOCATION => picloc = u32le(operand, 0),
            SPRM_C_HPS => chp.size_half_pt = u16le(operand, 0),
            SPRM_C_RG_FTC0 => chp.ftc = u16le(operand, 0),
            SPRM_C_HIGHLIGHT => {
                let value = operand[0];
                if value <= 16 {
                    chp.highlight = Some(value);
                }
            }
            SPRM_C_ISS => {
                let value = match operand[0] {
                    0 => Some(VertAlign::Baseline),
                    1 => Some(VertAlign::Super),
                    2 => Some(VertAlign::Sub),
                    _ => None,
                };
                if value.is_some() {
                    chp.vert_align = value;
                }
            }
            // These operators make the properties below style-derived. MS-DOC
            // explicitly preserves right-to-left layout across them.
            SPRM_C_ISTD | SPRM_C_ISTD_PERMUTE | SPRM_C_MAJORITY => {
                chp.vert_align = None;
                chp.small_caps = None;
                chp.caps = None;
            }
            SPRM_C_PLAIN if operand[0] == 0 => {
                chp.vert_align = None;
                chp.small_caps = None;
                chp.caps = None;
            }
            SPRM_C_CV => {
                // COLORREF: bytes [R, G, B, reserved].
                chp.color = Some(Color {
                    r: operand[0],
                    g: operand[1],
                    b: operand[2],
                });
            }
            // Legacy palette color, only when no 24-bit `sprmCCv` was seen.
            SPRM_C_ICO if chp.color.is_none() => {
                chp.color = ico_color(operand.first().copied().unwrap_or(0));
            }
            _ => {}
        }
        pos = operand_end;
    }
    // A picture run sets both fSpec and a picture location.
    chp.pic = if fspec { picloc } else { None };
    chp
}

fn apply_direct_toggle(state: &mut Option<bool>, operand: u8) {
    match operand {
        0 => *state = Some(false),
        1 => *state = Some(true),
        0x80 | 0x81 => *state = None,
        _ => {}
    }
}

/// Operand length for a sprm, from its `spra` field ([MS-DOC] 2.2.5). Character
/// sprms never use the `sprmTDefTable` special case, so the generic `spra == 6`
/// path (a leading length byte) suffices.
fn operand_len(sprm: u16, data: &[u8], op: usize) -> Option<usize> {
    match (sprm >> 13) & 0x7 {
        0 | 1 => Some(1),
        2 | 4 | 5 => Some(2),
        3 => Some(4),
        7 => Some(3),
        6 => Some(1 + *data.get(op)? as usize),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOLD_ON: &[u8] = &[0x35, 0x08, 0x01];
    const ITALIC_ON: &[u8] = &[0x36, 0x08, 0x01];

    fn plc(boundaries: &[u32], pages: &[u32]) -> Vec<u8> {
        assert_eq!(boundaries.len(), pages.len() + 1);
        let pn_base = boundaries.len() * 4;
        let mut bytes = vec![0; pn_base + pages.len() * 4];
        for (i, value) in boundaries.iter().chain(pages).enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    fn fkp(boundaries: &[u32], grpprls: &[Option<&[u8]>]) -> [u8; FKP_SIZE] {
        assert_eq!(boundaries.len(), grpprls.len() + 1);
        assert!(grpprls.len() <= 0x65);
        let crun = grpprls.len();
        let rgb_start = boundaries.len() * 4;
        let mut page = [0u8; FKP_SIZE];
        for (i, value) in boundaries.iter().enumerate() {
            page[i * 4..i * 4 + 4].copy_from_slice(&value.to_le_bytes());
        }
        let mut payload = (rgb_start + crun + 1) & !1;
        for (i, grpprl) in grpprls.iter().enumerate() {
            let Some(grpprl) = grpprl else { continue };
            assert!(grpprl.len() <= u8::MAX as usize);
            assert!(payload + 1 + grpprl.len() < FKP_SIZE - 1);
            page[rgb_start + i] = (payload / 2) as u8;
            page[payload] = grpprl.len() as u8;
            page[payload + 1..payload + 1 + grpprl.len()].copy_from_slice(grpprl);
            payload = (payload + 1 + grpprl.len() + 1) & !1;
        }
        page[FKP_SIZE - 1] = crun as u8;
        page
    }

    fn word_with_pages(pages: &[(usize, [u8; FKP_SIZE])]) -> Vec<u8> {
        let page_count = pages
            .iter()
            .map(|(number, _)| number + 1)
            .max()
            .unwrap_or(0);
        let mut word = vec![0; page_count * FKP_SIZE];
        for (number, page) in pages {
            let start = number * FKP_SIZE;
            word[start..start + FKP_SIZE].copy_from_slice(page);
        }
        word
    }

    #[test]
    fn scans_bold_italic_hidden() {
        // sprmCFBold=1, sprmCFItalic=1, sprmCFVanish=1.
        let chp = scan_grpprl(&[0x35, 0x08, 0x01, 0x36, 0x08, 0x01, 0x3C, 0x08, 0x01]);
        assert!(chp.bold && chp.italic && chp.hidden);
        assert!(!chp.strike && !chp.underline);
    }

    #[test]
    fn bold_off_and_inherit() {
        assert!(!scan_grpprl(&[0x35, 0x08, 0x00]).bold); // explicit off
        assert!(!scan_grpprl(&[0x35, 0x08, 0x80]).bold); // inherit-from-style
        assert!(scan_grpprl(&[0x35, 0x08, 0x81]).bold); // invert (of default-off)
    }

    #[test]
    fn underline_and_strike() {
        let chp = scan_grpprl(&[0x3E, 0x2A, 0x01, 0x37, 0x08, 0x01]);
        assert!(chp.underline && chp.strike);
    }

    #[test]
    fn highlight_palette_maps_to_word_names() {
        let expected = [
            None,
            Some("black"),
            Some("blue"),
            Some("cyan"),
            Some("green"),
            Some("magenta"),
            Some("red"),
            Some("yellow"),
            Some("white"),
            Some("darkBlue"),
            Some("darkCyan"),
            Some("darkGreen"),
            Some("darkMagenta"),
            Some("darkRed"),
            Some("darkYellow"),
            Some("darkGray"),
            Some("lightGray"),
        ];
        for (value, expected_name) in expected.into_iter().enumerate() {
            let value = value as u8;
            assert_eq!(scan_grpprl(&[0x0C, 0x2A, value]).highlight, Some(value));
            assert_eq!(highlight_name(value), expected_name);
        }
        assert_eq!(highlight_name(17), None);
        assert_eq!(highlight_name(u8::MAX), None);
    }

    #[test]
    fn highlight_uses_last_valid_value_and_explicit_zero_clears() {
        let highlighted = scan_grpprl(&[0x0C, 0x2A, 7, 0x0C, 0x2A, 14, 0x0C, 0x2A, 4]);
        assert_eq!(highlighted.highlight, Some(4));

        let cleared = scan_grpprl(&[0x0C, 0x2A, 7, 0x0C, 0x2A, 0]);
        assert_eq!(cleared.highlight, Some(0));
        assert_eq!(cleared.highlight.and_then(highlight_name), None);
    }

    #[test]
    fn invalid_or_truncated_highlight_preserves_prior_valid_value() {
        let chp = scan_grpprl(&[
            0x0C, 0x2A, 7, // yellow
            0x0C, 0x2A, 17, // invalid Ico
            0x0C, 0x2A, // truncated modifier
        ]);
        assert_eq!(chp.highlight, Some(7));
    }

    #[test]
    fn vertical_alignment_values_are_bounded() {
        for (value, expected) in [VertAlign::Baseline, VertAlign::Super, VertAlign::Sub]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                scan_grpprl(&[0x48, 0x2A, value as u8]).vert_align,
                Some(expected)
            );
        }
        assert_eq!(scan_grpprl(&[0x48, 0x2A, 3]).vert_align, None);
        assert_eq!(scan_grpprl(&[0x48, 0x2A, u8::MAX]).vert_align, None);
    }

    #[test]
    fn vertical_alignment_uses_last_valid_value_and_explicit_baseline() {
        let chp = scan_grpprl(&[
            0x48, 0x2A, 1, // superscript
            0x48, 0x2A, 2, // subscript
            0x48, 0x2A, 0, // explicit baseline
        ]);
        assert_eq!(chp.vert_align, Some(VertAlign::Baseline));
    }

    #[test]
    fn invalid_or_truncated_vertical_alignment_preserves_prior_valid_value() {
        let chp = scan_grpprl(&[
            0x48, 0x2A, 1, // superscript
            0x48, 0x2A, 3, // invalid Iss
            0x33, 0x2A, 1, // invalid sprmCPlain operand
            0x48, 0x2A, // truncated modifier
        ]);
        assert_eq!(chp.vert_align, Some(VertAlign::Super));
    }

    #[test]
    fn style_and_reset_operators_discard_stale_direct_vertical_alignment() {
        let cplain = scan_grpprl(&[
            0x48, 0x2A, 1, // superscript
            0x33, 0x2A, 0, // sprmCPlain
        ]);
        assert_eq!(cplain.vert_align, None);

        let cistd = scan_grpprl(&[
            0x48, 0x2A, 1, // superscript
            0x30, 0x4A, 0x0A, 0x00, // sprmCIstd
        ]);
        assert_eq!(cistd.vert_align, None);

        let cistd_permute = scan_grpprl(&[
            0x48, 0x2A, 1, // superscript
            0x31, 0xCA, 0x07, 0x00, 0x0A, 0x00, 0x0A, 0x00, 0x0A, 0x00,
        ]);
        assert_eq!(cistd_permute.vert_align, None);

        let cmajority = scan_grpprl(&[
            0x48, 0x2A, 1, // superscript
            0x47, 0xCA, 0x03, 0x48, 0x2A, 0x01, // matching Iss comparison
        ]);
        assert_eq!(cmajority.vert_align, None);

        let direct_after_reset = scan_grpprl(&[
            0x48, 0x2A, 1, // superscript
            0x33, 0x2A, 0, // sprmCPlain
            0x48, 0x2A, 2, // later direct subscript
        ]);
        assert_eq!(direct_after_reset.vert_align, Some(VertAlign::Sub));
    }

    #[test]
    fn capitalization_literal_toggles_are_bounded() {
        assert_eq!(scan_grpprl(&[0x3A, 0x08, 0]).small_caps, Some(false));
        assert_eq!(scan_grpprl(&[0x3A, 0x08, 1]).small_caps, Some(true));
        assert_eq!(scan_grpprl(&[0x3A, 0x08, 2]).small_caps, None);
        assert_eq!(scan_grpprl(&[0x3A, 0x08, u8::MAX]).small_caps, None);

        assert_eq!(scan_grpprl(&[0x3B, 0x08, 0]).caps, Some(false));
        assert_eq!(scan_grpprl(&[0x3B, 0x08, 1]).caps, Some(true));
        assert_eq!(scan_grpprl(&[0x3B, 0x08, 2]).caps, None);
        assert_eq!(scan_grpprl(&[0x3B, 0x08, u8::MAX]).caps, None);
    }

    #[test]
    fn capitalization_uses_last_valid_literal_values() {
        let chp = scan_grpprl(&[
            0x3A, 0x08, 1, // small caps on
            0x3A, 0x08, 0, // small caps off
            0x3B, 0x08, 0, // caps off
            0x3B, 0x08, 1, // caps on
        ]);
        assert_eq!(chp.small_caps, Some(false));
        assert_eq!(chp.caps, Some(true));
    }

    #[test]
    fn style_relative_capitalization_discards_stale_direct_values() {
        let chp = scan_grpprl(&[
            0x3A, 0x08, 1, // small caps on
            0x3B, 0x08, 1, // caps on
            0x3A, 0x08, 0x80, // small caps from style
            0x3B, 0x08, 0x81, // caps opposite style
        ]);
        assert_eq!(chp.small_caps, None);
        assert_eq!(chp.caps, None);

        let later_literal = scan_grpprl(&[
            0x3A, 0x08, 0x80, // small caps from style
            0x3B, 0x08, 0x81, // caps opposite style
            0x3A, 0x08, 1, // later literal small caps on
            0x3B, 0x08, 0, // later literal caps off
        ]);
        assert_eq!(later_literal.small_caps, Some(true));
        assert_eq!(later_literal.caps, Some(false));
    }

    fn prm0(isprm: u8, value: u8) -> u16 {
        (u16::from(value) << 8) | (u16::from(isprm) << 1)
    }

    #[test]
    fn pcd_prm0_applies_six_literal_character_toggles() {
        for (isprm, expected) in [
            (
                ISPRM_C_F_BOLD,
                Chp {
                    bold: true,
                    ..Chp::default()
                },
            ),
            (
                ISPRM_C_F_ITALIC,
                Chp {
                    italic: true,
                    ..Chp::default()
                },
            ),
            (
                ISPRM_C_F_STRIKE,
                Chp {
                    strike: true,
                    ..Chp::default()
                },
            ),
            (
                ISPRM_C_F_SMALL_CAPS,
                Chp {
                    small_caps: Some(true),
                    ..Chp::default()
                },
            ),
            (
                ISPRM_C_F_CAPS,
                Chp {
                    caps: Some(true),
                    ..Chp::default()
                },
            ),
            (
                ISPRM_C_F_VANISH,
                Chp {
                    hidden: true,
                    ..Chp::default()
                },
            ),
        ] {
            let mut chp = Chp::default();
            chp.apply_pcd_prm0(prm0(isprm, 1));
            assert_eq!(chp, expected);
        }

        let mut chp = Chp {
            bold: true,
            italic: true,
            strike: true,
            hidden: true,
            small_caps: Some(true),
            caps: Some(true),
            ..Chp::default()
        };
        for isprm in [
            ISPRM_C_F_BOLD,
            ISPRM_C_F_ITALIC,
            ISPRM_C_F_STRIKE,
            ISPRM_C_F_SMALL_CAPS,
            ISPRM_C_F_CAPS,
            ISPRM_C_F_VANISH,
        ] {
            chp.apply_pcd_prm0(prm0(isprm, 0));
        }
        assert!(!chp.bold && !chp.italic && !chp.strike && !chp.hidden);
        assert_eq!(chp.small_caps, Some(false));
        assert_eq!(chp.caps, Some(false));
    }

    #[test]
    fn pcd_prm1_compiles_bounded_character_effects_in_source_order() {
        let grpprl = [
            0x35, 0x08, 0, // bold off
            0x35, 0x08, 1, // bold on (last wins)
            0x36, 0x08, 0, // italic off
            0x37, 0x08, 1, // strike on
            0x3C, 0x08, 0, // hidden off
            0x3A, 0x08, 1, // small caps on
            0x3B, 0x08, 0, // caps off
            0x5A, 0x08, 1, // RTL on
            0x3E, 0x2A, 0x0B, // wavy underline
            0x0C, 0x2A, 14, // dark yellow highlight
            0x48, 0x2A, 2, // subscript
        ];
        let patch = compile_pcd_prm1_patch(&grpprl).unwrap();
        let color = Color {
            r: 0x11,
            g: 0x22,
            b: 0x33,
        };
        let mut chp = Chp {
            italic: true,
            hidden: true,
            highlight: Some(7),
            vert_align: Some(VertAlign::Super),
            small_caps: Some(false),
            caps: Some(true),
            rtl: Some(false),
            size_half_pt: Some(24),
            color: Some(color),
            ..Chp::default()
        };

        patch.apply(&mut chp);

        assert!(chp.bold);
        assert!(!chp.italic);
        assert!(chp.underline);
        assert!(chp.strike);
        assert!(!chp.hidden);
        assert_eq!(chp.small_caps, Some(true));
        assert_eq!(chp.caps, Some(false));
        assert_eq!(chp.rtl, Some(true));
        assert_eq!(chp.highlight, Some(14));
        assert_eq!(chp.vert_align, Some(VertAlign::Sub));
        assert_eq!(chp.size_half_pt, Some(24));
        assert_eq!(chp.color, Some(color));
    }

    #[test]
    fn pcd_prm1_preserves_explicit_clear_values() {
        let patch = compile_pcd_prm1_patch(&[
            0x35, 0x08, 1, 0x35, 0x08, 0, // bold on then off
            0x3E, 0x2A, 1, 0x3E, 0x2A, 0, // underline then none
            0x0C, 0x2A, 7, 0x0C, 0x2A, 0, // highlight then clear
            0x48, 0x2A, 1, 0x48, 0x2A, 0, // superscript then baseline
        ])
        .unwrap();
        let mut chp = Chp {
            bold: true,
            underline: true,
            highlight: Some(7),
            vert_align: Some(VertAlign::Super),
            ..Chp::default()
        };

        patch.apply(&mut chp);

        assert!(!chp.bold);
        assert!(!chp.underline);
        assert_eq!(chp.highlight, Some(0));
        assert_eq!(chp.vert_align, Some(VertAlign::Baseline));
    }

    #[test]
    fn pcd_prm1_accepts_only_defined_underline_values() {
        for value in [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x06, 0x07, 0x09, 0x0A, 0x0B, 0x14, 0x17, 0x19, 0x1A,
            0x1B, 0x27, 0x2B, 0x37,
        ] {
            assert!(
                compile_pcd_prm1_patch(&[0x3E, 0x2A, value]).is_some(),
                "valid Kul 0x{value:02X} was rejected"
            );
        }
        for value in [0x05, 0x08, 0x0C, 0x13, 0x38, u8::MAX] {
            assert!(
                compile_pcd_prm1_patch(&[0x3E, 0x2A, value]).is_none(),
                "undefined Kul 0x{value:02X} was accepted"
            );
        }
    }

    #[test]
    fn pcd_prm1_skips_complete_mixed_property_operands() {
        let patch = compile_pcd_prm1_patch(&[
            0x05, 0x24, 1, // paragraph keep-with-next
            0x15, 0xC6, 2, 0, 0, // short PChgTabs with two-byte remainder
            0x08, 0xD6, 4, 0, 0, 0, 0, // zero-column TDefTable
            0x15, 0xC6, 0xFF, // extended PChgTabs
            1, 0, 0, 0, 0, // one delete/close record
            1, 0, 0, 0, // one add record
            0x35, 0x08, 1, // bold on
            0x36, 0x08, 1, // italic on
        ])
        .unwrap();
        let mut chp = Chp::default();
        patch.apply(&mut chp);
        assert!(chp.bold && chp.italic);
    }

    #[test]
    fn pcd_prm1_rejects_malformed_or_style_dependent_groups_atomically() {
        let groups: &[&[u8]] = &[
            &[0x35, 0x08],                            // truncated toggle
            &[0x35, 0x08, 2],                         // invalid toggle
            &[0x35, 0x08, 0x80],                      // style-relative toggle
            &[0x35, 0x08, 0x81],                      // inverse-style toggle
            &[0x38, 0x08, 0x80, 0x35, 0x08, 1],       // unmodeled style-relative outline
            &[0x0C, 0x2A, 17],                        // invalid highlight Ico
            &[0x48, 0x2A, 3],                         // invalid vertical alignment
            &[0x32, 0x2A, 0],                         // sprmCDefault
            &[0x33, 0x2A, 0],                         // sprmCPlain
            &[0x30, 0x4A, 0x0A, 0x00],                // sprmCIstd
            &[0x31, 0xCA, 0],                         // sprmCIstdPermute
            &[0x47, 0xCA, 0],                         // sprmCMajority
            &[0x15, 0xC6, 0],                         // invalid short PChgTabs cb
            &[0x15, 0xC6, 1, 0],                      // invalid short PChgTabs cb
            &[0x15, 0xC6, 2, 1, 0],                   // impossible PChgTabs counts
            &[0x15, 0xC6, 3, 0, 0, 0],                // mismatched PChgTabs cb
            &[0x15, 0xC6, 0xFF, 65],                  // excessive extended tab count
            &[0x08, 0xD6, 0x03, 0x00, 0, 0],          // undersized TDefTable
            &[0x08, 0xD6, 0x04, 0x00, 64, 0, 0],      // excessive TDefTable columns
            &[0x08, 0xD6, 0x05, 0x00, 0, 0, 0, 0],    // partial TC80
            &[0x08, 0xD6, 0x06, 0x00, 1, 5, 0, 4, 0], // descending centers
            &[0x08, 0xD6, 0x03, 0x00, 0x00],          // truncated TDefTable
        ];
        for grpprl in groups {
            assert!(
                compile_pcd_prm1_patch(grpprl).is_none(),
                "malformed/style-dependent grpprl was partially compiled: {grpprl:02X?}"
            );
        }
    }

    #[test]
    fn pcd_prm_dispatch_preserves_prm0_and_missing_prm1_behavior() {
        let patches = compile_pcd_prm1_patches(&[vec![0x35, 0x08, 1], vec![0x35, 0x08, 0x80]]);

        let mut complex = Chp::default();
        complex.apply_pcd_prm(1, &patches);
        assert!(complex.bold);

        let original = Chp {
            bold: true,
            ..Chp::default()
        };
        for raw in [3, 5] {
            let mut chp = original;
            chp.apply_pcd_prm(raw, &patches);
            assert_eq!(chp, original);
        }

        let mut compact = Chp::default();
        compact.apply_pcd_prm(prm0(ISPRM_C_F_ITALIC, 1), &patches);
        assert!(compact.italic);
    }

    #[test]
    fn pcd_prm0_ignores_complex_relative_unknown_and_invalid_values() {
        let original = Chp {
            bold: true,
            caps: Some(true),
            ..Chp::default()
        };
        for raw in [
            0,
            1,
            prm0(0x54, 1),
            prm0(ISPRM_C_F_BOLD, 2),
            prm0(ISPRM_C_F_BOLD, 0x80),
            prm0(ISPRM_C_F_BOLD, 0x81),
        ] {
            let mut chp = original;
            chp.apply_pcd_prm0(raw);
            assert_eq!(chp, original, "raw PRM 0x{raw:04X} changed CHP");
        }
    }

    #[test]
    fn invalid_or_truncated_capitalization_preserves_prior_literal_values() {
        let chp = scan_grpprl(&[
            0x3A, 0x08, 1, // small caps on
            0x3B, 0x08, 1, // caps on
            0x3A, 0x08, 2, // invalid small caps
            0x3B, 0x08, 0xFF, // invalid caps
            0x3A, 0x08, // truncated modifier
        ]);
        assert_eq!(chp.small_caps, Some(true));
        assert_eq!(chp.caps, Some(true));
    }

    #[test]
    fn style_and_reset_operators_discard_stale_direct_capitalization() {
        let resets: &[&[u8]] = &[
            // sprmCPlain
            &[0x33, 0x2A, 0],
            // sprmCIstd
            &[0x30, 0x4A, 0x0A, 0x00],
            // sprmCIstdPermute
            &[0x31, 0xCA, 0x07, 0x00, 0x0A, 0x00, 0x0A, 0x00, 0x0A, 0x00],
            // sprmCMajority comparing both properties as on
            &[0x47, 0xCA, 0x06, 0x3A, 0x08, 1, 0x3B, 0x08, 1],
        ];
        for reset in resets {
            let mut grpprl = vec![
                0x3A, 0x08, 1, // small caps on
                0x3B, 0x08, 1, // caps on
            ];
            grpprl.extend_from_slice(reset);
            let chp = scan_grpprl(&grpprl);
            assert_eq!(chp.small_caps, None);
            assert_eq!(chp.caps, None);
        }

        let direct_after_reset = scan_grpprl(&[
            0x3A, 0x08, 1, // small caps on
            0x3B, 0x08, 1, // caps on
            0x33, 0x2A, 0, // sprmCPlain
            0x3A, 0x08, 0, // later direct small caps off
            0x3B, 0x08, 1, // later direct caps on
        ]);
        assert_eq!(direct_after_reset.small_caps, Some(false));
        assert_eq!(direct_after_reset.caps, Some(true));
    }

    #[test]
    fn run_rtl_literal_toggles_are_bounded() {
        assert_eq!(scan_grpprl(&[0x5A, 0x08, 0]).rtl, Some(false));
        assert_eq!(scan_grpprl(&[0x5A, 0x08, 1]).rtl, Some(true));
        assert_eq!(scan_grpprl(&[0x5A, 0x08, 2]).rtl, None);
        assert_eq!(scan_grpprl(&[0x5A, 0x08, u8::MAX]).rtl, None);
    }

    #[test]
    fn run_rtl_uses_source_order_and_style_relative_unknown_state() {
        let explicit = scan_grpprl(&[
            0x5A, 0x08, 1, // RTL on
            0x5A, 0x08, 0, // RTL off
        ]);
        assert_eq!(explicit.rtl, Some(false));

        for style_relative in [0x80, 0x81] {
            let unresolved = scan_grpprl(&[
                0x5A,
                0x08,
                1, // RTL on
                0x5A,
                0x08,
                style_relative,
            ]);
            assert_eq!(unresolved.rtl, None);
        }

        let recovered = scan_grpprl(&[
            0x5A, 0x08, 0x80, // RTL from style
            0x5A, 0x08, 1, // later literal RTL on
        ]);
        assert_eq!(recovered.rtl, Some(true));
    }

    #[test]
    fn invalid_or_truncated_run_rtl_preserves_prior_literal_state() {
        let chp = scan_grpprl(&[
            0x5A, 0x08, 1, // RTL on
            0x5A, 0x08, 2, // invalid toggle
            0x5A, 0x08, // truncated modifier
        ]);
        assert_eq!(chp.rtl, Some(true));
    }

    #[test]
    fn style_and_reset_operators_preserve_direct_run_rtl() {
        let operators: &[&[u8]] = &[
            // sprmCPlain
            &[0x33, 0x2A, 0],
            // sprmCIstd
            &[0x30, 0x4A, 0x0A, 0x00],
            // sprmCIstdPermute
            &[0x31, 0xCA, 0x07, 0x00, 0x0A, 0x00, 0x0A, 0x00, 0x0A, 0x00],
            // sprmCMajority
            &[0x47, 0xCA, 0x03, 0x35, 0x08, 1],
        ];
        for operator in operators {
            let mut grpprl = vec![0x5A, 0x08, 1];
            grpprl.extend_from_slice(operator);
            assert_eq!(scan_grpprl(&grpprl).rtl, Some(true));
        }

        let explicit_off = scan_grpprl(&[
            0x5A, 0x08, 0, // RTL off
            0x33, 0x2A, 0, // sprmCPlain
        ]);
        assert_eq!(explicit_off.rtl, Some(false));
    }

    #[test]
    fn skips_unknown_sprm_by_spra() {
        // A 2-byte-operand sprm (spra=2, e.g. sprmCHps 0x4A43) then bold.
        let chp = scan_grpprl(&[0x43, 0x4A, 0xAA, 0xBB, 0x35, 0x08, 0x01]);
        assert!(chp.bold);
    }

    #[test]
    fn truncated_operands_preserve_prior_complete_properties() {
        let chp = scan_grpprl(&[
            0x35, 0x08, 0x01, // bold on
            0x35, 0x08, // truncated bold toggle
        ]);
        assert!(chp.bold);

        let chp = scan_grpprl(&[
            0x43, 0x4A, 0x18, 0x00, // 12 pt
            0x43, 0x4A, 0x20, // truncated font size
        ]);
        assert_eq!(chp.size_half_pt, Some(24));

        let chp = scan_grpprl(&[
            0x36, 0x08, 0x01, // italic on
            0x00, 0xC0, 0x04, 0xAA, // variable operand declares four bytes
        ]);
        assert!(chp.italic);
    }

    #[test]
    fn declared_plc_count_locates_page_numbers_when_processing_is_capped() {
        let word = word_with_pages(&[(1, fkp(&[100, 200], &[Some(BOLD_ON)]))]);
        let plc = plc(&[100, 200, 300], &[1, 0]);

        set_test_max_fkp(1);
        let table = parse(&word, &plc, 0, plc.len());
        set_test_max_fkp(MAX_FKP_ENTRIES);

        assert!(table.chp_at(150).bold);
        assert_eq!(table.entries.len(), 1);
    }

    #[test]
    fn cumulative_entry_budget_is_exact() {
        let boundaries = (0..=0x65).collect::<Vec<_>>();
        let grpprls = vec![None; 0x65];
        let word = word_with_pages(&[(0, fkp(&boundaries, &grpprls))]);
        let plc = plc(&[0, 0x65], &[0]);

        set_test_max_fkp(64);
        let table = parse(&word, &plc, 0, plc.len());
        set_test_max_fkp(MAX_FKP_ENTRIES);

        assert_eq!(table.entries.len(), 64);
    }

    #[test]
    fn malformed_or_unordered_plc_is_ignored() {
        let word = word_with_pages(&[(0, fkp(&[100, 200], &[Some(BOLD_ON)]))]);

        let mut noncanonical = plc(&[100, 200], &[0]);
        noncanonical.push(0);
        assert!(parse(&word, &noncanonical, 0, noncanonical.len())
            .entries
            .is_empty());

        let unordered = plc(&[200, 100], &[0]);
        assert!(parse(&word, &unordered, 0, unordered.len())
            .entries
            .is_empty());
    }

    #[test]
    fn malformed_fkp_ranges_are_ignored() {
        for boundaries in [[100, 200, 150], [100, 100, 200]] {
            let word = word_with_pages(&[(0, fkp(&boundaries, &[None, None]))]);
            let plc = plc(&[100, 200], &[0]);
            assert!(parse(&word, &plc, 0, plc.len()).entries.is_empty());
        }
    }

    #[test]
    fn lookup_respects_run_starts_and_uncovered_gaps() {
        let word = word_with_pages(&[
            (0, fkp(&[120, 180], &[Some(BOLD_ON)])),
            (1, fkp(&[220, 280], &[Some(ITALIC_ON)])),
        ]);
        let plc = plc(&[100, 200, 300], &[0, 1]);
        let table = parse(&word, &plc, 0, plc.len());

        assert_eq!(table.chp_at(119), Chp::default());
        assert!(table.chp_at(120).bold);
        assert!(table.chp_at(179).bold);
        assert_eq!(table.chp_at(180), Chp::default());
        assert_eq!(table.chp_at(219), Chp::default());
        assert!(table.chp_at(220).italic);
        assert!(table.chp_at(279).italic);
        assert_eq!(table.chp_at(280), Chp::default());
    }

    #[test]
    fn chpx_offsets_cannot_target_metadata_or_crun() {
        let mut metadata = [0u8; FKP_SIZE];
        metadata[0..4].copy_from_slice(&100u32.to_le_bytes());
        metadata[4..8].copy_from_slice(&0x0108_3503u32.to_le_bytes());
        metadata[8] = 2; // offset 4, inside rgfc
        metadata[FKP_SIZE - 1] = 1;
        let word = word_with_pages(&[(0, metadata)]);
        let metadata_plc = plc(&[100, 0x0108_3503], &[0]);
        assert_eq!(
            parse(&word, &metadata_plc, 0, metadata_plc.len()).chp_at(100),
            Chp::default()
        );

        let mut overlaps_crun = [0u8; FKP_SIZE];
        overlaps_crun[0..4].copy_from_slice(&100u32.to_le_bytes());
        overlaps_crun[4..8].copy_from_slice(&200u32.to_le_bytes());
        overlaps_crun[8] = 254; // offset 508
        overlaps_crun[508] = 3;
        overlaps_crun[509] = 0x35;
        overlaps_crun[510] = 0x08;
        overlaps_crun[FKP_SIZE - 1] = 1; // also the apparent bold operand
        let word = word_with_pages(&[(0, overlaps_crun)]);
        let crun_plc = plc(&[100, 200], &[0]);
        assert_eq!(
            parse(&word, &crun_plc, 0, crun_plc.len()).chp_at(150),
            Chp::default()
        );
    }

    #[test]
    fn lookup_by_fc() {
        let t = ChpxTable {
            entries: vec![
                ChpEntry {
                    fc_start: 0,
                    fc_lim: 100,
                    chp: Chp {
                        bold: true,
                        ..Chp::default()
                    },
                },
                ChpEntry {
                    fc_start: 100,
                    fc_lim: 200,
                    chp: Chp {
                        italic: true,
                        ..Chp::default()
                    },
                },
            ],
        };
        assert!(t.chp_at(50).bold);
        assert!(t.chp_at(150).italic);
        assert!(!t.chp_at(250).bold && !t.chp_at(250).italic); // past end = default
    }

    #[test]
    fn bin_table_page_iterations_are_capped() {
        let word = word_with_pages(&[(0, fkp(&[0, 10], &[None]))]);
        let boundaries = (0..=10).collect::<Vec<_>>();
        let pages = vec![0; 10];
        let plc = plc(&boundaries, &pages);

        set_test_max_fkp(3);
        let table = parse(&word, &plc, 0, plc.len());
        set_test_max_fkp(MAX_FKP_ENTRIES);

        assert_eq!(table.entries.len(), 3);
        assert_eq!(table.entries[0].fc_start, 0);
        assert_eq!(table.entries[2].fc_lim, 3);
    }
}
