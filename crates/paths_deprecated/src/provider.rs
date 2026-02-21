//! Provider type definitions

/// Cloud provider enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    Gcp,
    Aws,
    Azure,
}
