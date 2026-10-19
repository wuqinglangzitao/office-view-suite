//! OPC (Open Packaging Convention) package handling

pub mod compress;
pub mod package;

pub use compress::{
    CompressionLevel, CompressionOptions, CompressionResult, PptxAnalysis, analyze_pptx,
    compress_pptx, compress_pptx_in_memory,
};
pub use package::Package;
