//! The per-song model registry: model directories and their manifests.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::ModelError;

/// The manifest file format version stamped into every model.
pub const MODEL_FORMAT_VERSION: u32 = 1;

/// The manifest file name written inside each model directory.
const MANIFEST: &str = "manifest.json";

/// The family of influence model a directory holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelKind {
    /// A DDSP-style timbre decoder (R-0016/R-0017).
    Timbre,
    /// Beat-conditioning vectors (R-0018).
    Beat,
    /// A LoRA adapter on a small lyric LM (R-0024).
    Lyric,
}

/// The inspectable description of one model, stored as `manifest.json` inside
/// the model's directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelManifest {
    /// The manifest format version.
    pub format_version: u32,
    /// The model's registry id (its directory name).
    pub id: String,
    /// The human name the model was created with.
    pub name: String,
    /// Which family of model this is.
    pub kind: ModelKind,
    /// Creation time, seconds since the Unix epoch (`0` if unavailable).
    pub created_unix: u64,
    /// Artifact file names this model owns, relative to its directory.
    pub files: Vec<String>,
}

/// A handle to a freshly created model: its id and directory, ready for a caller
/// to write artifacts into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelHandle {
    id: String,
    dir: PathBuf,
}

impl ModelHandle {
    /// The model's registry id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The model's directory (write artifacts here).
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// A registry of models rooted at a directory — in practice `<session>/models/`.
#[derive(Debug, Clone)]
pub struct ModelRegistry {
    root: PathBuf,
}

impl ModelRegistry {
    /// Opens (creating if absent) a registry rooted at `root`.
    pub fn open(root: impl AsRef<Path>) -> Result<ModelRegistry, ModelError> {
        let root = root.as_ref().to_path_buf();
        std::fs::create_dir_all(&root).map_err(|e| ModelError::Io(e.to_string()))?;
        Ok(ModelRegistry { root })
    }

    /// The registry root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The directory a model id resolves to (no existence guarantee).
    pub fn dir(&self, id: &str) -> PathBuf {
        self.root.join(id)
    }

    /// Creates a new model directory with a written manifest and returns a
    /// handle. Errors if the derived id is empty or already exists.
    pub fn create(&self, name: &str, kind: ModelKind) -> Result<ModelHandle, ModelError> {
        let id = id_from_name(name)?;
        let dir = self.dir(&id);
        if dir.exists() {
            return Err(ModelError::AlreadyExists(id));
        }
        std::fs::create_dir_all(&dir).map_err(|e| ModelError::Io(e.to_string()))?;
        let manifest = ModelManifest {
            format_version: MODEL_FORMAT_VERSION,
            id: id.clone(),
            name: name.to_string(),
            kind,
            created_unix: now_unix(),
            files: Vec::new(),
        };
        write_manifest(&dir, &manifest)?;
        Ok(ModelHandle { id, dir })
    }

    /// Reads one model's manifest, or [`ModelError::NotFound`].
    pub fn get(&self, id: &str) -> Result<ModelManifest, ModelError> {
        let path = self.dir(id).join(MANIFEST);
        if !path.exists() {
            return Err(ModelError::NotFound(id.to_string()));
        }
        read_manifest(&self.dir(id))
    }

    /// Every model's manifest, sorted by id.
    pub fn list(&self) -> Result<Vec<ModelManifest>, ModelError> {
        let mut out = Vec::new();
        let entries = std::fs::read_dir(&self.root).map_err(|e| ModelError::Io(e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| ModelError::Io(e.to_string()))?;
            if !entry.path().is_dir() {
                continue;
            }
            if entry.path().join(MANIFEST).exists() {
                out.push(read_manifest(&entry.path())?);
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Records an artifact file name in a model's manifest (idempotent).
    pub fn manifest_add_file(&self, id: &str, file: &str) -> Result<(), ModelError> {
        let dir = self.dir(id);
        if !dir.join(MANIFEST).exists() {
            return Err(ModelError::NotFound(id.to_string()));
        }
        let mut manifest = read_manifest(&dir)?;
        if !manifest.files.iter().any(|f| f == file) {
            manifest.files.push(file.to_string());
            write_manifest(&dir, &manifest)?;
        }
        Ok(())
    }

    /// Removes a model directory, or [`ModelError::NotFound`].
    pub fn remove(&self, id: &str) -> Result<(), ModelError> {
        let dir = self.dir(id);
        if !dir.exists() {
            return Err(ModelError::NotFound(id.to_string()));
        }
        std::fs::remove_dir_all(&dir).map_err(|e| ModelError::Io(e.to_string()))
    }
}

/// Derives a filesystem-safe, session-local id from a model name: lowercase,
/// non-alphanumeric runs collapse to a single `-`, ends trimmed. An empty result
/// is [`ModelError::InvalidName`].
fn id_from_name(name: &str) -> Result<String, ModelError> {
    let mut id = String::with_capacity(name.len());
    let mut prev_dash = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            id.push(ch.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            id.push('-');
            prev_dash = true;
        }
    }
    let id = id.trim_matches('-').to_string();
    if id.is_empty() {
        return Err(ModelError::InvalidName(name.to_string()));
    }
    Ok(id)
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn write_manifest(dir: &Path, manifest: &ModelManifest) -> Result<(), ModelError> {
    let json =
        serde_json::to_string_pretty(manifest).map_err(|e| ModelError::Serialize(e.to_string()))?;
    std::fs::write(dir.join(MANIFEST), json).map_err(|e| ModelError::Io(e.to_string()))
}

fn read_manifest(dir: &Path) -> Result<ModelManifest, ModelError> {
    let json =
        std::fs::read_to_string(dir.join(MANIFEST)).map_err(|e| ModelError::Io(e.to_string()))?;
    serde_json::from_str(&json).map_err(|e| ModelError::Deserialize(e.to_string()))
}
