//! The song session model and its JSON serialization.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::SessionError;

/// The session file format version stamped into every [`Song`].
pub const FORMAT_VERSION: u32 = 1;

/// A song's musical settings: tempo and the pitch grid it snaps to.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Beats per minute.
    pub bpm: f64,
    /// Beats per bar.
    pub beats_per_bar: f64,
    /// The pitch grid root, in Hz.
    pub root_hz: f64,
    /// Harmonic-series odd-limit (the smooth↔tense grid size).
    pub odd_limit: u64,
}

/// What a [`Stem`] is — a rendered guitar riff, a drum beat, or something else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StemKind {
    /// A hum→riff guitar stem (R-0008).
    Riff,
    /// A Euclidean drum beat (R-0009).
    Beat,
    /// Any other rendered part.
    Other,
}

/// A recorded audio take (the raw hum/beatbox captured from the mic).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Take {
    /// A human label for the take.
    pub name: String,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// The captured mono samples.
    pub samples: Vec<f32>,
}

/// A rendered, loopable part on the timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stem {
    /// A human label for the stem.
    pub name: String,
    /// What kind of part this is.
    pub kind: StemKind,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Loop length in whole bars.
    pub bars: u32,
    /// The rendered mono samples.
    pub samples: Vec<f32>,
}

/// A savable song: settings, takes, stems, and a reference to its (future)
/// per-song influence model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Song {
    /// The session file format version.
    pub format_version: u32,
    /// The song's name.
    pub name: String,
    /// Tempo and grid settings.
    pub settings: Settings,
    /// Recorded takes.
    pub takes: Vec<Take>,
    /// Rendered stems on the timeline.
    pub stems: Vec<Stem>,
    /// Opaque reference to the song's influence model (M4); `None` until trained.
    pub model_ref: Option<String>,
}

impl Song {
    /// A new, empty song at the current [`FORMAT_VERSION`].
    ///
    /// ```
    /// use gooz_session::{Settings, Song};
    ///
    /// let settings = Settings { bpm: 92.0, beats_per_bar: 4.0, root_hz: 220.0, odd_limit: 9 };
    /// let song = Song::new("session 001", settings);
    /// assert!(song.takes.is_empty() && song.stems.is_empty());
    /// ```
    pub fn new(name: impl Into<String>, settings: Settings) -> Song {
        Song {
            format_version: FORMAT_VERSION,
            name: name.into(),
            settings,
            takes: Vec::new(),
            stems: Vec::new(),
            model_ref: None,
        }
    }

    /// Appends a take (builder style).
    pub fn with_take(mut self, take: Take) -> Song {
        self.takes.push(take);
        self
    }

    /// Appends a stem (builder style).
    pub fn with_stem(mut self, stem: Stem) -> Song {
        self.stems.push(stem);
        self
    }

    /// Serializes the song to pretty JSON.
    ///
    /// ```
    /// use gooz_session::{Settings, Song};
    ///
    /// let s = Settings { bpm: 92.0, beats_per_bar: 4.0, root_hz: 220.0, odd_limit: 9 };
    /// let song = Song::new("demo", s);
    /// let json = song.to_json().unwrap();
    /// assert_eq!(Song::from_json(&json).unwrap(), song);
    /// ```
    pub fn to_json(&self) -> Result<String, SessionError> {
        serde_json::to_string_pretty(self).map_err(|e| SessionError::Serialize(e.to_string()))
    }

    /// Parses a song from JSON produced by [`Song::to_json`].
    pub fn from_json(json: &str) -> Result<Song, SessionError> {
        serde_json::from_str(json).map_err(|e| SessionError::Deserialize(e.to_string()))
    }

    /// Writes the song to `path` as JSON.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), SessionError> {
        let json = self.to_json()?;
        std::fs::write(path, json).map_err(|e| SessionError::Io(e.to_string()))
    }

    /// Reads a song from a JSON file written by [`Song::save`].
    pub fn load(path: impl AsRef<Path>) -> Result<Song, SessionError> {
        let json = std::fs::read_to_string(path).map_err(|e| SessionError::Io(e.to_string()))?;
        Song::from_json(&json)
    }
}
