//! Core model types for Entangled.

mod code_block;
mod content;
mod properties;
mod reference_id;
mod reference_map;
mod reference_name;
mod tangle;

pub use code_block::CodeBlock;
pub use content::{Content, PlainText, RawContent};
pub use properties::{parse_properties, extract_quarto_options, Properties, Property, QuartoOptions};
pub use reference_id::ReferenceId;
pub use reference_map::ReferenceMap;
pub use reference_name::ReferenceName;
pub use tangle::{tangle_annotated, tangle_naked, tangle_ref, CycleDetector};
