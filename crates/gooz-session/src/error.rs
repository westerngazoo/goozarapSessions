//! The crate's single typed error.

use std::fmt;

/// Every fallible operation in this crate reports one of these. The crate never
/// panics on library paths: filesystem and (de)serialization failures surface
/// here with a human-readable message.
///
/// Messages are `String`-backed so the error stays `Clone`/`PartialEq` for test
/// assertions, while remaining typed and meaningful (the variant says *what*
/// failed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// Reading or writing the session file failed.
    Io(String),
    /// The session could not be serialized to JSON.
    Serialize(String),
    /// The session file was missing fields or was not valid session JSON.
    Deserialize(String),
    /// The arrangement was structurally invalid (bad span, stem index, or level).
    InvalidArrangement(String),
    /// Mixdown or WAV export failed (e.g. mismatched stem sample rates).
    Export(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SessionError::Io(m) => write!(f, "session i/o failed: {m}"),
            SessionError::Serialize(m) => write!(f, "session serialization failed: {m}"),
            SessionError::Deserialize(m) => write!(f, "session could not be read: {m}"),
            SessionError::InvalidArrangement(m) => write!(f, "invalid arrangement: {m}"),
            SessionError::Export(m) => write!(f, "export failed: {m}"),
        }
    }
}

impl std::error::Error for SessionError {}
