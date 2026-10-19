//! Core traits and types for pptx-rs
//!
//! This module provides the foundational traits that enable trait-based
//! XML generation and consistent behavior across all PPTX elements.

mod dimension;
mod package_validation;
mod placement;
mod powerpoint_compat;
mod traits;
mod validation;
mod xml_utils;

pub use dimension::{Dimension, FlexPosition, FlexSize, SLIDE_HEIGHT_EMU, SLIDE_WIDTH_EMU};
pub use package_validation::{
    PackageValidationIssue, PackageValidationReport, REQUIRED_PACKAGE_PARTS, ValidationCategory,
    ValidationSeverity, validate_package, validate_package_bytes,
};
pub use placement::ElementPlacement;
pub use powerpoint_compat::{CompatReport, validate_powerpoint_structure};
pub use traits::{Positioned, Sized as ElementSized, ToXml};
pub use validation::{
    REQUIRED_PARTS_MINIMAL, REQUIRED_PARTS_REPAIR, RequiredPart, ValidationIssue,
    check_required_parts, check_required_parts_with_descriptions, clamp_ratio, clamp_unit_interval,
    validate_chart, validate_index, validate_non_empty, validate_non_empty_str, validate_positive,
    validate_well_formed_xml,
};
pub use xml_utils::{XmlWriter, append_escape_xml, append_i32, append_usize, escape_xml};
