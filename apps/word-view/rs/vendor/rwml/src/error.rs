//! Error type for `.doc` parsing.

/// Errors produced while opening or decoding a `.doc` file.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The input is not an OLE2 / CFB compound file (`.doc` is OLE2-based).
    #[error("not an OLE2/CFB file (.doc must start with the D0CF11E0 magic)")]
    NotOle2,

    /// The OLE2 container could not be opened.
    #[error("failed to open compound file: {0}")]
    Cfb(#[from] std::io::Error),

    /// A required stream (`WordDocument` / `0Table` / `1Table`) is missing.
    #[error("missing required stream: {0}")]
    MissingStream(&'static str),

    /// The File Information Block (FIB) is malformed (bad magic, truncated).
    #[error("malformed FIB: {0}")]
    Fib(&'static str),

    /// The piece table (CLX / PlcPcd) is malformed.
    #[error("malformed piece table: {0}")]
    PieceTable(String),

    /// The document is encrypted or XOR-obfuscated (`fEncrypted`). Extraction is
    /// refused rather than emitting scrambled bytes. `obfuscated` is `true` for
    /// the [MS-DOC] 2.2.6.1 XOR scheme, `false` for RC4/CryptoAPI encryption.
    #[error("encrypted document (obfuscated={obfuscated}) — extraction refused")]
    Encrypted {
        /// Whether the protection is XOR obfuscation (vs. real encryption).
        obfuscated: bool,
    },

    /// The document is an unsupported pre-Word-97 version (`nFib < 0x00C1`,
    /// i.e. Word 6/95): all-8-bit text with a different FIB/piece-table layout.
    #[error("unsupported Word 6/95 format (nFib=0x{0:04X})")]
    UnsupportedVersion(u16),

    /// The document parsed but contained no indexable text.
    #[error("no indexable text")]
    NoText,

    /// A modern `.docx` (OOXML / ZIP container) could not be read — a malformed
    /// ZIP, a missing `word/document.xml`, or `.docx` support not compiled in
    /// (the `docx` cargo feature is disabled).
    #[error("malformed or unsupported .docx: {0}")]
    Docx(String),

    /// Native PDF rendering failed.
    #[error("render failed: {0}")]
    Render(String),
}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, Error>;
