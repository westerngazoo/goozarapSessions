//! Acceptance tests for R-0014 — model registry, realized by SPEC-0014.

use std::path::PathBuf;

use gooz_model::{MODEL_FORMAT_VERSION, ModelError, ModelKind, ModelRegistry};

fn temp_root(tag: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("gooz_model_{}_{}", std::process::id(), tag));
    // Fresh each run.
    let _ = std::fs::remove_dir_all(&p);
    p
}

// AC1 — open roots and creates the registry directory.
#[test]
fn ac1_open_creates_the_registry_dir() {
    let root = temp_root("open");
    let reg = ModelRegistry::open(&root).expect("opens");
    assert!(root.is_dir());
    assert_eq!(reg.root(), root.as_path());
    let _ = std::fs::remove_dir_all(&root);
}

// AC2 — create makes a model dir + manifest and returns id + dir.
#[test]
fn ac2_create_makes_dir_and_manifest() {
    let root = temp_root("create");
    let reg = ModelRegistry::open(&root).unwrap();
    let handle = reg
        .create("Warm Guitar", ModelKind::Timbre)
        .expect("creates");
    assert_eq!(handle.id(), "warm-guitar");
    assert!(handle.dir().is_dir());
    assert!(handle.dir().join("manifest.json").is_file());
    let _ = std::fs::remove_dir_all(&root);
}

// AC3 — manifest is inspectable JSON and round-trips.
#[test]
fn ac3_manifest_has_expected_fields() {
    let root = temp_root("manifest");
    let reg = ModelRegistry::open(&root).unwrap();
    reg.create("lofi drums", ModelKind::Beat).unwrap();
    let m = reg.get("lofi-drums").expect("gets");
    assert_eq!(m.format_version, MODEL_FORMAT_VERSION);
    assert_eq!(m.id, "lofi-drums");
    assert_eq!(m.name, "lofi drums");
    assert_eq!(m.kind, ModelKind::Beat);
    assert!(m.files.is_empty());

    let json = std::fs::read_to_string(reg.dir("lofi-drums").join("manifest.json")).unwrap();
    assert!(json.contains("\"kind\": \"beat\""));
    let _ = std::fs::remove_dir_all(&root);
}

// AC4 — list, get, dir.
#[test]
fn ac4_list_get_and_dir() {
    let root = temp_root("list");
    let reg = ModelRegistry::open(&root).unwrap();
    reg.create("b model", ModelKind::Timbre).unwrap();
    reg.create("a model", ModelKind::Lyric).unwrap();

    let all = reg.list().unwrap();
    assert_eq!(all.len(), 2);
    // Sorted by id.
    assert_eq!(all[0].id, "a-model");
    assert_eq!(all[1].id, "b-model");

    assert_eq!(reg.get("a-model").unwrap().kind, ModelKind::Lyric);
    assert_eq!(reg.dir("a-model"), root.join("a-model"));
    let _ = std::fs::remove_dir_all(&root);
}

// AC5 — track files, persisted across reopen.
#[test]
fn ac5_manifest_add_file_persists_across_reopen() {
    let root = temp_root("files");
    {
        let reg = ModelRegistry::open(&root).unwrap();
        reg.create("timbre", ModelKind::Timbre).unwrap();
        reg.manifest_add_file("timbre", "decoder.safetensors")
            .unwrap();
        // Idempotent: adding again does not duplicate.
        reg.manifest_add_file("timbre", "decoder.safetensors")
            .unwrap();
        reg.manifest_add_file("timbre", "features.json").unwrap();
    }
    let reg = ModelRegistry::open(&root).unwrap();
    let m = reg.get("timbre").unwrap();
    assert_eq!(m.files, vec!["decoder.safetensors", "features.json"]);
    let _ = std::fs::remove_dir_all(&root);
}

// AC6 — typed errors, no panic.
#[test]
fn ac6_duplicate_create_is_already_exists() {
    let root = temp_root("dup");
    let reg = ModelRegistry::open(&root).unwrap();
    reg.create("dup", ModelKind::Beat).unwrap();
    assert!(matches!(
        reg.create("dup", ModelKind::Beat),
        Err(ModelError::AlreadyExists(_))
    ));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn ac6_missing_model_is_not_found() {
    let root = temp_root("missing");
    let reg = ModelRegistry::open(&root).unwrap();
    assert!(matches!(reg.get("nope"), Err(ModelError::NotFound(_))));
    assert!(matches!(reg.remove("nope"), Err(ModelError::NotFound(_))));
    assert!(matches!(
        reg.manifest_add_file("nope", "x"),
        Err(ModelError::NotFound(_))
    ));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn ac6_empty_name_is_invalid() {
    let root = temp_root("badname");
    let reg = ModelRegistry::open(&root).unwrap();
    assert!(matches!(
        reg.create("   !!!  ", ModelKind::Timbre),
        Err(ModelError::InvalidName(_))
    ));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn ac6_remove_deletes_the_model() {
    let root = temp_root("remove");
    let reg = ModelRegistry::open(&root).unwrap();
    reg.create("gone", ModelKind::Timbre).unwrap();
    reg.remove("gone").unwrap();
    assert!(matches!(reg.get("gone"), Err(ModelError::NotFound(_))));
    assert!(reg.list().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(&root);
}

// Typed error implements std::error::Error with a useful Display.
#[test]
fn model_error_displays() {
    let err: &dyn std::error::Error = &ModelError::NotFound("x".into());
    assert!(err.to_string().contains('x'));
}
