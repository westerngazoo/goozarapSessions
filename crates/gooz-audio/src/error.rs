//! The engine's typed error.

use std::fmt;

/// Every fallible audio operation reports one of these.
///
/// Fieldless and `Copy`, mirroring the other crates' error style. The library
/// never panics on these paths — an absent device or an unsupported format is
/// reported, not aborted on.
///
/// ```
/// use gooz_audio::AudioError;
///
/// assert_eq!(AudioError::NoInputDevice, AudioError::NoInputDevice);
/// assert!(!AudioError::UnsupportedConfig.to_string().is_empty());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError {
    /// No default input device is available.
    NoInputDevice,
    /// No default output device is available.
    NoOutputDevice,
    /// The device cannot provide an `f32` stream we support (includes a failed
    /// default-config query).
    UnsupportedConfig,
    /// The backend failed to build the stream.
    StreamBuild,
    /// The backend failed to start the stream.
    StreamPlay,
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            AudioError::NoInputDevice => "no default input device is available",
            AudioError::NoOutputDevice => "no default output device is available",
            AudioError::UnsupportedConfig => "the device does not support a usable f32 stream",
            AudioError::StreamBuild => "the audio stream could not be built",
            AudioError::StreamPlay => "the audio stream could not be started",
        };
        f.write_str(message)
    }
}

impl std::error::Error for AudioError {}
