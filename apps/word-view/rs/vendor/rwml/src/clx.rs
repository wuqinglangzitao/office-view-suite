//! CLX / piece-table parsing.
//!
//! The CLX (in the table stream at `[fcClx, fcClx+lcbClx)`) is zero or more
//! `Prc` blocks (`0x01`) followed by one `Pcdt` (`0x02`) whose body is a
//! `PlcPcd` — the piece table. The piece table maps character positions to byte
//! offsets in the `WordDocument` stream and records, per piece, whether the
//! text is 1-byte ANSI (`fCompressed`) or 2-byte UTF-16LE, plus the PCD's
//! additional formatting modifier.
//!
//! Reference: [MS-DOC] 2.8.35 (Pcdt), 2.8.34 (Prc), 2.9.177 (PlcPcd), 2.9.176 (Pcd).

use crate::error::{Error, Result};
use crate::util::{u16le, u32le};

/// Upper bound on the piece count — far beyond any real `.doc` (a piece per fast-save edit;
/// real documents have at most thousands), but it bounds a crafted PlcPcd that would
/// otherwise declare millions of overlapping pieces to amplify decoding (see `parse_plcpcd`).
const MAX_PIECES: usize = 1 << 20;
/// A `Prm1.igrpprl` is 15 bits, so only this many ordered PRCs are addressable.
const MAX_PRCS: usize = 1 << 15;
/// [MS-DOC] `PrcData.cbGrpprl` upper bound.
const MAX_PRC_GRPPRL_LEN: usize = 0x3FA2;

/// One text piece resolved from the piece table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Piece {
    /// Character count of this piece (`cp[i+1] - cp[i]`).
    pub cch: usize,
    /// Byte offset of the piece text in the `WordDocument` stream.
    pub fc: usize,
    /// `true` = 1-byte ANSI (cp1252), `false` = 2-byte UTF-16LE.
    pub compressed: bool,
    /// Raw PCD `Prm`; interpretation depends on its low `fComplex` bit.
    pub prm: u16,
}

/// Parsed CLX structures retained by the legacy reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedClx {
    pub pieces: Vec<Piece>,
    /// Ordered `PrcData.GrpPrl` payloads addressed by `Prm1.igrpprl`.
    pub prcs: Vec<Vec<u8>>,
}

/// Parse the CLX, returning its ordered PRCs and piece list.
pub(crate) fn parse(clx: &[u8]) -> Result<ParsedClx> {
    parse_with_limits(clx, MAX_PRCS, MAX_PRC_GRPPRL_LEN)
}

fn parse_with_limits(clx: &[u8], max_prcs: usize, max_prc_len: usize) -> Result<ParsedClx> {
    let mut pos = 0usize;
    let mut prcs = Vec::new();
    loop {
        let marker = *clx
            .get(pos)
            .ok_or_else(|| Error::PieceTable("CLX truncated before Pcdt".into()))?;
        match marker {
            0x01 => {
                // Prc: 1-byte clxt + 2-byte cbGrpprl + cbGrpprl bytes.
                let cb = u16le(clx, pos + 1)
                    .ok_or_else(|| Error::PieceTable("truncated Prc".into()))?
                    as i16;
                let cb = usize::try_from(cb)
                    .map_err(|_| Error::PieceTable("negative Prc length".into()))?;
                if cb > max_prc_len {
                    return Err(Error::PieceTable(format!(
                        "Prc grpprl length {cb} exceeds limit {max_prc_len}"
                    )));
                }
                if prcs.len() >= max_prcs {
                    return Err(Error::PieceTable(format!(
                        "Prc count exceeds limit {max_prcs}"
                    )));
                }
                let start = pos
                    .checked_add(3)
                    .ok_or_else(|| Error::PieceTable("Prc length overflow".into()))?;
                let end = start
                    .checked_add(cb)
                    .ok_or_else(|| Error::PieceTable("Prc length overflow".into()))?;
                let grpprl = clx
                    .get(start..end)
                    .ok_or_else(|| Error::PieceTable("Prc out of CLX bounds".into()))?;
                prcs.push(grpprl.to_vec());
                pos = end;
            }
            0x02 => {
                // Pcdt: 1-byte clxt + 4-byte lcb + PlcPcd[lcb].
                let lcb = u32le(clx, pos + 1)
                    .ok_or_else(|| Error::PieceTable("truncated Pcdt".into()))?
                    as usize;
                let start = pos + 5;
                let plc = clx
                    .get(start..start.saturating_add(lcb))
                    .ok_or_else(|| Error::PieceTable("PlcPcd out of CLX bounds".into()))?;
                return Ok(ParsedClx {
                    pieces: parse_plcpcd(plc)?,
                    prcs,
                });
            }
            other => {
                return Err(Error::PieceTable(format!(
                    "unexpected CLX marker 0x{other:02x}"
                )))
            }
        }
    }
}

/// PlcPcd = (n+1) CP entries (u32) followed by n PCD entries (8 bytes each).
fn parse_plcpcd(plc: &[u8]) -> Result<Vec<Piece>> {
    parse_plcpcd_with_limit(plc, MAX_PIECES)
}

fn parse_plcpcd_with_limit(plc: &[u8], max_pieces: usize) -> Result<Vec<Piece>> {
    if plc.len() < 4 || (plc.len() - 4) % 12 != 0 {
        return Err(Error::PieceTable(format!(
            "bad PlcPcd length {}",
            plc.len()
        )));
    }
    // Cap the piece count: `n` scales with the (uncapped) table stream, and pieces may
    // overlap (all point at the same WordDocument bytes), so an unbounded count is one half
    // of an N×W decode-amplification DoS (the other half is bounded in `text`/`assemble`).
    // Mirrors the FKP/FFN/style caps elsewhere; far above any real document's run count.
    let declared_n = (plc.len() - 4) / 12;
    let processed_n = declared_n.min(max_pieces);
    // Capping materialization must not move the descriptor array within the PLC.
    let pcd_base = (declared_n + 1) * 4;
    let mut pieces = Vec::with_capacity(processed_n);
    for i in 0..processed_n {
        let cp0 = u32le(plc, i * 4).unwrap_or(0) as i64;
        let cp1 = u32le(plc, (i + 1) * 4).unwrap_or(0) as i64;
        let cch = (cp1 - cp0).max(0) as usize;

        // PCD: [0..2] flags, [2..6] FcCompressed, [6..8] prm.
        let pcd_off = pcd_base + i * 8;
        let fc_compressed =
            u32le(plc, pcd_off + 2).ok_or_else(|| Error::PieceTable("truncated PCD".into()))?;
        let compressed = (fc_compressed & 0x4000_0000) != 0;
        let fc30 = (fc_compressed & 0x3FFF_FFFF) as usize;
        let fc = if compressed { fc30 / 2 } else { fc30 };
        let prm =
            u16le(plc, pcd_off + 6).ok_or_else(|| Error::PieceTable("truncated PCD".into()))?;
        pieces.push(Piece {
            cch,
            fc,
            compressed,
            prm,
        });
    }
    Ok(pieces)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a one-piece Pcdt: CPs `[0, 5]` then one PCD with `fc=0x100`,
    /// uncompressed.
    fn one_piece_pcdt() -> Vec<u8> {
        let mut plc = Vec::new();
        plc.extend_from_slice(&0u32.to_le_bytes());
        plc.extend_from_slice(&5u32.to_le_bytes());
        plc.extend_from_slice(&0u16.to_le_bytes()); // PCD flags
        plc.extend_from_slice(&0x100u32.to_le_bytes()); // FcCompressed (uncompressed)
        plc.extend_from_slice(&0x01AAu16.to_le_bytes()); // Prm0: sprmCFBold on
        let mut clx = vec![0x02u8];
        clx.extend_from_slice(&(plc.len() as u32).to_le_bytes());
        clx.extend_from_slice(&plc);
        clx
    }

    #[test]
    fn parses_single_uncompressed_piece() {
        let parsed = parse(&one_piece_pcdt()).unwrap();
        assert!(parsed.prcs.is_empty());
        assert_eq!(
            parsed.pieces,
            vec![Piece {
                cch: 5,
                fc: 0x100,
                compressed: false,
                prm: 0x01AA,
            }]
        );
    }

    #[test]
    fn capped_piece_prefix_uses_declared_pcd_array_offset() {
        let mut plc = Vec::new();
        for cp in [0u32, 2, 5, 9] {
            plc.extend_from_slice(&cp.to_le_bytes());
        }
        for (fc, prm) in [(0x100u32, 0x01AAu16), (0x200, 0x01AC), (0x300, 0)] {
            plc.extend_from_slice(&0u16.to_le_bytes());
            plc.extend_from_slice(&fc.to_le_bytes());
            plc.extend_from_slice(&prm.to_le_bytes());
        }

        assert_eq!(
            parse_plcpcd_with_limit(&plc, 1).unwrap(),
            vec![Piece {
                cch: 2,
                fc: 0x100,
                compressed: false,
                prm: 0x01AA,
            }]
        );
    }

    #[test]
    fn rejects_malformed_or_truncated_plcpcd() {
        assert!(parse_plcpcd(&[0; 5]).is_err());

        let mut truncated_pcd = Vec::new();
        truncated_pcd.extend_from_slice(&0u32.to_le_bytes());
        truncated_pcd.extend_from_slice(&1u32.to_le_bytes());
        truncated_pcd.extend_from_slice(&0u16.to_le_bytes());
        truncated_pcd.extend_from_slice(&0x100u32.to_le_bytes());
        truncated_pcd.push(0xAA);
        assert!(parse_plcpcd(&truncated_pcd).is_err());

        let mut clx = vec![0x02];
        clx.extend_from_slice(&16u32.to_le_bytes());
        clx.extend_from_slice(&[0; 4]);
        assert!(parse(&clx).is_err());
    }

    #[test]
    fn skips_leading_prc_block() {
        // Prc: 0x01 + cbGrpprl(2) + 2 bytes, then the Pcdt.
        let mut clx = vec![0x01u8, 0x02, 0x00, 0xAA, 0xBB];
        clx.extend_from_slice(&one_piece_pcdt());
        let parsed = parse(&clx).unwrap();
        assert_eq!(parsed.prcs, vec![vec![0xAA, 0xBB]]);
        assert_eq!(parsed.pieces.len(), 1);
        assert_eq!(parsed.pieces[0].fc, 0x100);
    }

    #[test]
    fn retains_ordered_and_empty_prc_payloads() {
        let mut clx = vec![
            0x01, 0x00, 0x00, // empty PRC at index 0
            0x01, 0x03, 0x00, 0x35, 0x08, 0x01, // bold PRC at index 1
        ];
        clx.extend_from_slice(&one_piece_pcdt());

        assert_eq!(
            parse(&clx).unwrap().prcs,
            vec![Vec::new(), vec![0x35, 0x08, 0x01]]
        );
    }

    #[test]
    fn validates_signed_size_and_count_prc_boundaries() {
        assert!(parse(&[0x01, 0xFF, 0xFF]).is_err());
        assert!(parse_with_limits(&[0x01, 0x03, 0x00, 1, 2, 3], 1, 2).is_err());
        assert!(parse(&[0x01, 0x02, 0x00, 1]).is_err());

        let mut maximum = vec![0x01];
        maximum.extend_from_slice(&(MAX_PRC_GRPPRL_LEN as i16).to_le_bytes());
        maximum.resize(3 + MAX_PRC_GRPPRL_LEN, 0);
        maximum.extend_from_slice(&one_piece_pcdt());
        assert_eq!(parse(&maximum).unwrap().prcs[0].len(), MAX_PRC_GRPPRL_LEN);

        let mut oversized = vec![0x01];
        oversized.extend_from_slice(&((MAX_PRC_GRPPRL_LEN + 1) as i16).to_le_bytes());
        assert!(parse(&oversized).is_err());

        let mut maximum_count = Vec::with_capacity(MAX_PRCS * 3 + one_piece_pcdt().len());
        for _ in 0..MAX_PRCS {
            maximum_count.extend_from_slice(&[0x01, 0x00, 0x00]);
        }
        maximum_count.extend_from_slice(&one_piece_pcdt());
        assert_eq!(parse(&maximum_count).unwrap().prcs.len(), MAX_PRCS);

        let mut excess = vec![
            0x01, 0x00, 0x00, // index 0
            0x01, 0x00, 0x00, // exceeds the test cap
        ];
        excess.extend_from_slice(&one_piece_pcdt());
        assert!(parse_with_limits(&excess, 1, MAX_PRC_GRPPRL_LEN).is_err());
    }

    #[test]
    fn rejects_unknown_marker() {
        assert!(parse(&[0x99]).is_err());
    }
}
