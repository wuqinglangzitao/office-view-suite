//! Table module for PPTX generation
//!
//! This module provides comprehensive table support including:
//! - Cell content and formatting
//! - Row and column management
//! - Text alignment (horizontal and vertical)
//! - Cell backgrounds and borders
//! - Text wrapping
//! - XML generation for OOXML

mod builder;
mod cell;
mod format;
mod row;
mod style;
mod xml;

pub use builder::{Table, TableBuilder};
pub use cell::{CellAlign, CellVAlign, TableCell};
pub use format::generate_cell_xml;
pub use row::TableRow;
pub use style::{
    HEADER_TEXT, HELPER_HEADER_BG, IMPORT_HEADER_BG, header_cell, table_from_string_rows,
};
pub use xml::generate_table_xml;
