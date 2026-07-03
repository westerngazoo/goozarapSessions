//! The crate's typed error.

use std::fmt;

/// Every fallible registry operation reports one of these. The crate never
/// panics on library paths; filesystem, (de)serialization, and lookup failures
/// surface here.
///
/// Messages are `String`-backed so the error stays `Clone`/`PartialEq` for test
/// assertions while remaining typed and meaningful.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// A filesystem operation failed.
    Io(String),
    /// A manifest could not be serialized.
    Serialize(String),
    /// A manifest could not be parsed.
    Deserialize(String),
    /// No model with the given id exists in the registry.
    NotFound(String),
    /// A model with the derived id already exists.
    AlreadyExists(String),
    /// A model name produced no usable id (e.g. it was empty or all symbols).
    InvalidName(String),
    /// Feature extraction from reference audio failed (propagated analysis error).
    Extract(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModelError::Io(m) => write!(f, "model i/o failed: {m}"),
            ModelError::Serialize(m) => write!(f, "model manifest serialization failed: {m}"),
            ModelError::Deserialize(m) => write!(f, "model manifest could not be read: {m}"),
            ModelError::NotFound(id) => write!(f, "no model '{id}' in the registry"),
            ModelError::AlreadyExists(id) => write!(f, "a model '{id}' already exists"),
            ModelError::InvalidName(n) => write!(f, "'{n}' is not a usable model name"),
            ModelError::Extract(m) => write!(f, "feature extraction failed: {m}"),
        }
    }
}

impl std::error::Error for ModelError {}
