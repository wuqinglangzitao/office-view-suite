//! File Information Block (FIB) parsing.
//!
//! The FIB sits at offset 0 of the `WordDocument` stream and is the index into
//! everything else. Rather than hard-coding the well-known Word-97 byte offsets
//! we *navigate* the variable-length sub-structures (FibBase → csw·FibRgW97 →
//! cslw·FibRgLw97 → cbRgFcLcb·FibRgFcLcb) so non-standard writers are handled.
//!
//! Reference: [MS-DOC] 2.5.1 (Fib), 2.5.2–2.5.6 (FibRg*).

use crate::error::{Error, Result};
use crate::util::{u16le, u32le};

/// Parsed FIB fields needed for text extraction.
#[derive(Debug, Clone)]
pub(crate) struct Fib {
    /// `nFib` — format version. `< 0x00C1` is pre-Word-97 (Word 6/95).
    pub nfib: u16,
    /// `lid` — document language id; implies the ANSI codepage of compressed
    /// (8-bit) pieces (e.g. Korean `0x0412` → cp949).
    pub lid: u16,
    /// `fEncrypted` — the document is encrypted or XOR-obfuscated.
    pub encrypted: bool,
    /// `fObfuscated` — XOR obfuscation ([MS-DOC] 2.2.6.1) vs. real encryption.
    pub obfuscated: bool,
    /// `fComplex` — document uses a complex (multi-piece) piece table.
    pub complex: bool,
    /// `fWhichTblStm` — false → `0Table`, true → `1Table`.
    pub which_table_stream_one: bool,
    /// Byte offset of the CLX within the table stream.
    pub fc_clx: usize,
    /// Byte length of the CLX.
    pub lcb_clx: usize,
    /// `fcPlcfBtePapx`/`lcbPlcfBtePapx` — the bin table locating the PAPX FKPs
    /// (paragraph properties), used for table/list structure.
    pub fc_plcf_bte_papx: usize,
    pub lcb_plcf_bte_papx: usize,
    /// `fcPlcfBteChpx`/`lcbPlcfBteChpx` — the bin table locating the CHPX FKPs
    /// (character properties: bold/italic/font/…), for the rich document model.
    pub fc_plcf_bte_chpx: usize,
    pub lcb_plcf_bte_chpx: usize,
    /// `fcStshf`/`lcbStshf` — the style sheet (STSH), for paragraph style names
    /// and heading-level resolution.
    pub fc_stshf: usize,
    pub lcb_stshf: usize,
    /// `fcSttbfFfn`/`lcbSttbfFfn` — the font-name table (`SttbfFfn`), resolving a
    /// CHPX font index (`sprmCRgFtc0`) to a family name.
    pub fc_sttbf_ffn: usize,
    pub lcb_sttbf_ffn: usize,
    /// `fcPlfLst`/`lcbPlfLst` — the list-definition table (`PlfLst`).
    pub fc_plf_lst: usize,
    pub lcb_plf_lst: usize,
    /// `fcPlfLfo`/`lcbPlfLfo` — the list-format-override table (`PlfLfo`).
    pub fc_plf_lfo: usize,
    pub lcb_plf_lfo: usize,
    /// `fcPlcfHdd`/`lcbPlcfHdd` — story boundaries for the header document.
    pub fc_plcf_hdd: usize,
    pub lcb_plcf_hdd: usize,
    /// `fcPlcfandRef`/`lcbPlcfandRef` — annotation reference records.
    pub fc_plcfand_ref: usize,
    pub lcb_plcfand_ref: usize,
    /// `fcGrpXstAtnOwners`/`lcbGrpXstAtnOwners` — annotation author names.
    pub fc_grp_xst_atn_owners: usize,
    pub lcb_grp_xst_atn_owners: usize,
    /// Character counts partitioning the CP space across sub-documents.
    pub ccp_text: u32,
    pub ccp_ftn: u32,
    pub ccp_hdd: u32,
    pub ccp_atn: u32,
    pub ccp_edn: u32,
    pub ccp_txbx: u32,
}

impl Fib {
    pub(crate) fn parse(word: &[u8]) -> Result<Fib> {
        // FibBase.wIdent must be 0xA5EC.
        if u16le(word, 0).ok_or(Error::Fib("truncated header"))? != 0xA5EC {
            return Err(Error::Fib("bad wIdent (not 0xA5EC)"));
        }
        let nfib = u16le(word, 2).ok_or(Error::Fib("truncated nFib"))?;
        let lid = u16le(word, 0x14).unwrap_or(0);
        // FibBase flags word at 0x0A: bit 2 = fComplex, bit 8 = fEncrypted,
        // bit 9 = fWhichTblStm, bit 15 = fObfuscated.
        let flags = u16le(word, 0x0A).ok_or(Error::Fib("truncated flags"))?;
        let complex = (flags & 0x0004) != 0;
        let encrypted = (flags & 0x0100) != 0;
        let which_table_stream_one = (flags & 0x0200) != 0;
        let obfuscated = (flags & 0x8000) != 0;

        // Navigate the variable-length FIB layout.
        // FibBase is 32 bytes; then csw:u16, FibRgW97[csw], cslw:u16,
        // FibRgLw97[cslw], cbRgFcLcb:u16, FibRgFcLcb[...].
        let csw = u16le(word, 32).ok_or(Error::Fib("missing csw"))? as usize;
        let rglw_count_off = 34 + csw * 2; // cslw position
        let cslw = u16le(word, rglw_count_off).ok_or(Error::Fib("missing cslw"))? as usize;
        let rglw_off = rglw_count_off + 2; // FibRgLw97 start
        let fclcb_count_off = rglw_off + cslw * 4; // cbRgFcLcb position
        let fclcb_off = fclcb_count_off + 2; // FibRgFcLcb start

        // FibRgLw97: ccpText is field index 3, ccpFtn 4, ccpHdd 5, ccpAtn 7,
        // ccpEdn 8, ccpTxbx 9 (each u32). (index 6 is ccpMcr, unused here.)
        let lw = |i: usize| u32le(word, rglw_off + i * 4).unwrap_or(0);
        let ccp_text = lw(3);
        let ccp_ftn = lw(4);
        let ccp_hdd = lw(5);
        let ccp_atn = lw(7);
        let ccp_edn = lw(8);
        let ccp_txbx = lw(9);

        // FibRgFcLcb97: fcClx is fc/lcb pair index 33.
        let fc_clx = u32le(word, fclcb_off + 33 * 8).ok_or(Error::Fib("missing fcClx"))? as usize;
        let lcb_clx =
            u32le(word, fclcb_off + 33 * 8 + 4).ok_or(Error::Fib("missing lcbClx"))? as usize;

        // fcPlcfBtePapx is fc/lcb pair index 13 (paragraph-property bin table).
        let fc_plcf_bte_papx = u32le(word, fclcb_off + 13 * 8).unwrap_or(0) as usize;
        let lcb_plcf_bte_papx = u32le(word, fclcb_off + 13 * 8 + 4).unwrap_or(0) as usize;
        // fcPlcfBteChpx is pair index 12 (character-property bin table); fcStshf
        // is pair index 2 (style sheet).
        let fc_plcf_bte_chpx = u32le(word, fclcb_off + 12 * 8).unwrap_or(0) as usize;
        let lcb_plcf_bte_chpx = u32le(word, fclcb_off + 12 * 8 + 4).unwrap_or(0) as usize;
        // STSH is fc/lcb pair index 1 (pair 0 = fcStshfOrig, the stale fast-save
        // backup stylesheet — must not be used for istd→name).
        let fc_stshf = u32le(word, fclcb_off + 8).unwrap_or(0) as usize;
        let lcb_stshf = u32le(word, fclcb_off + 8 + 4).unwrap_or(0) as usize;
        // SttbfFfn = FibRgFcLcb97 index 15 (fc at +15*8, lcb at +15*8+4).
        let fc_sttbf_ffn = u32le(word, fclcb_off + 15 * 8).unwrap_or(0) as usize;
        let lcb_sttbf_ffn = u32le(word, fclcb_off + 15 * 8 + 4).unwrap_or(0) as usize;
        // fcPlfLst is pair index 73, fcPlfLfo index 74 (list tables).
        let fc_plf_lst = u32le(word, fclcb_off + 73 * 8).unwrap_or(0) as usize;
        let lcb_plf_lst = u32le(word, fclcb_off + 73 * 8 + 4).unwrap_or(0) as usize;
        let fc_plf_lfo = u32le(word, fclcb_off + 74 * 8).unwrap_or(0) as usize;
        let lcb_plf_lfo = u32le(word, fclcb_off + 74 * 8 + 4).unwrap_or(0) as usize;
        // PlcfHdd = FibRgFcLcb97 index 11.
        let fc_plcf_hdd = u32le(word, fclcb_off + 11 * 8).unwrap_or(0) as usize;
        let lcb_plcf_hdd = u32le(word, fclcb_off + 11 * 8 + 4).unwrap_or(0) as usize;
        // PlcfandRef = FibRgFcLcb97 index 4; GrpXstAtnOwners = index 36.
        let fc_plcfand_ref = u32le(word, fclcb_off + 4 * 8).unwrap_or(0) as usize;
        let lcb_plcfand_ref = u32le(word, fclcb_off + 4 * 8 + 4).unwrap_or(0) as usize;
        let fc_grp_xst_atn_owners = u32le(word, fclcb_off + 36 * 8).unwrap_or(0) as usize;
        let lcb_grp_xst_atn_owners = u32le(word, fclcb_off + 36 * 8 + 4).unwrap_or(0) as usize;

        Ok(Fib {
            nfib,
            lid,
            encrypted,
            obfuscated,
            complex,
            which_table_stream_one,
            fc_clx,
            lcb_clx,
            fc_plcf_bte_papx,
            lcb_plcf_bte_papx,
            fc_plcf_bte_chpx,
            lcb_plcf_bte_chpx,
            fc_stshf,
            lcb_stshf,
            fc_sttbf_ffn,
            lcb_sttbf_ffn,
            fc_plf_lst,
            lcb_plf_lst,
            fc_plf_lfo,
            lcb_plf_lfo,
            fc_plcf_hdd,
            lcb_plcf_hdd,
            fc_plcfand_ref,
            lcb_plcfand_ref,
            fc_grp_xst_atn_owners,
            lcb_grp_xst_atn_owners,
            ccp_text,
            ccp_ftn,
            ccp_hdd,
            ccp_atn,
            ccp_edn,
            ccp_txbx,
        })
    }

    /// The ANSI codepage implied by the document language (`lid`), used to
    /// decode compressed (8-bit) pieces. Mirrors LibreOffice ww8
    /// `GetCharSetFromLanguage`: Korean `0x0412` → 949, etc.
    pub(crate) fn ansi_codepage(&self) -> u16 {
        match self.lid & 0x03FF {
            0x12 => 949,                                                 // Korean (cp949/UHC)
            0x11 => 932,                                                 // Japanese
            0x04 if matches!(self.lid, 0x0404 | 0x0C04 | 0x1404) => 950, // Chinese (Traditional)
            0x04 => 936,                                                 // Chinese (Simplified)
            0x19 | 0x02 | 0x22 | 0x23 | 0x1A => 1251,                    // Cyrillic
            0x08 => 1253,                                                // Greek
            0x1F => 1254,                                                // Turkish
            0x0D => 1255,                                                // Hebrew
            0x01 => 1256,                                                // Arabic
            0x1E => 874,                                                 // Thai
            0x2A => 1258,                                                // Vietnamese
            _ => 1252,                                                   // Western (default)
        }
    }

    /// Total characters across all sub-documents (the full CP space). The six `ccp_*` fields
    /// are unbounded attacker-controlled `u32`s read straight from the FIB, so sum them in
    /// `usize` with saturating arithmetic — a plain `u32 + u32` panics (overflow-checks) /
    /// wraps on a crafted `.doc`.
    pub(crate) fn total_cp(&self) -> usize {
        [
            self.ccp_text,
            self.ccp_ftn,
            self.ccp_hdd,
            self.ccp_atn,
            self.ccp_edn,
            self.ccp_txbx,
        ]
        .into_iter()
        .fold(0usize, |acc, c| acc.saturating_add(c as usize))
    }

    /// Name of the table stream holding the CLX.
    pub(crate) fn table_stream(&self) -> &'static str {
        if self.which_table_stream_one {
            "1Table"
        } else {
            "0Table"
        }
    }
}

/// Read one `FibRgFcLcb97` fc/lcb pair by index from the raw FIB.
pub(crate) fn fc_lcb_pair(word: &[u8], index: usize) -> Option<(usize, usize)> {
    let fclcb_off = fclcb_offset(word)?;
    let pair_off = fclcb_off.checked_add(index.checked_mul(8)?)?;
    let fc = u32le(word, pair_off)? as usize;
    let lcb = u32le(word, pair_off.checked_add(4)?)? as usize;
    Some((fc, lcb))
}

fn fclcb_offset(word: &[u8]) -> Option<usize> {
    if u16le(word, 0)? != 0xA5EC {
        return None;
    }
    let csw = u16le(word, 32)? as usize;
    let rglw_count_off = 34usize.checked_add(csw.checked_mul(2)?)?;
    let cslw = u16le(word, rglw_count_off)? as usize;
    let rglw_off = rglw_count_off.checked_add(2)?;
    let fclcb_count_off = rglw_off.checked_add(cslw.checked_mul(4)?)?;
    fclcb_count_off.checked_add(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn total_cp_saturates_on_overflowing_ccp_fields() {
        // A crafted .doc can set every ccp_* field to u32::MAX; their sum overflows u32.
        // total_cp() must add in usize (no panic under overflow-checks, no wrap), not `u32 + u32`.
        let mut w = vec![0u8; 512];
        w[0..2].copy_from_slice(&0xA5ECu16.to_le_bytes()); // wIdent
        w[2..4].copy_from_slice(&0x00C1u16.to_le_bytes()); // nFib = Word 97
        w[34..36].copy_from_slice(&10u16.to_le_bytes()); // cslw=10 -> FibRgLw97 at off 36
        let rglw = 36;
        for idx in [3usize, 4, 5, 7, 8, 9] {
            // ccpText/Ftn/Hdd/Atn/Edn/Txbx
            w[rglw + idx * 4..rglw + idx * 4 + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        }
        let fib = Fib::parse(&w).expect("minimal FIB should parse");
        assert_eq!(fib.ccp_text, u32::MAX);
        // Computed in usize: 6 * u32::MAX (fits usize on 64-bit) — the point is no panic/wrap.
        assert_eq!(fib.total_cp(), (u32::MAX as usize) * 6);
    }
}
