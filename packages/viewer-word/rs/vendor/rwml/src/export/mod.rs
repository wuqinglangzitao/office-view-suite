//! Exporters: pure folds over the [`crate::model::DocModel`] into Markdown (GFM)
//! and semantic HTML. They never touch the binary format.

pub(crate) mod html;
pub(crate) mod markdown;
