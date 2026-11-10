//! Manifest-backed upstream artwork.
//!
//! Upstream images are game artwork, committed under `assets/upstream/` with a
//! credit note. The app never assumes a path exists: it reads
//! `assets/upstream/manifest.json` (written by `script/import-upstream-assets.mjs`),
//! resolves a reference only when the file is on disk, and reports the rest as
//! missing so the UI can draw a labelled placeholder.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The manifest file inside the asset folder.
pub const MANIFEST_FILE: &str = "manifest.json";
/// The default asset folder, relative to the repository root.
pub const DEFAULT_DIR: &str = "assets/upstream";
/// Overrides the asset folder, for packaged builds and harnesses.
pub const ENV_DIR: &str = "IDLEON_ASSETS_DIR";

/// Whether the asset folder is usable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// A manifest loaded; individual files may still be missing.
    Ready,
    /// No manifest in any candidate folder.
    NotInstalled,
    /// A manifest exists but cannot be read.
    Invalid(String),
}

/// What an image reference resolves to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Artwork<'a> {
    /// The file exists and the manifest lists it.
    Ready(&'a Path),
    /// The reference is not in the manifest, or the file is gone.
    Missing,
}

/// The manifest's entries and the totals it declares.
#[derive(Clone, Debug)]
pub struct Manifest {
    files: BTreeMap<String, PathBuf>,
    total_files: usize,
    total_bytes: u64,
    missing_files: usize,
    status: Status,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            files: BTreeMap::new(),
            total_files: 0,
            total_bytes: 0,
            missing_files: 0,
            status: Status::NotInstalled,
        }
    }
}

impl Manifest {
    /// Load the manifest in `dir`; a missing or unreadable one is a state, not an error.
    pub fn load(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let path = dir.join(MANIFEST_FILE);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match Self::parse(&dir, &text) {
            Ok(manifest) => manifest,
            Err(error) => Self {
                status: Status::Invalid(error),
                ..Self::default()
            },
        }
    }

    /// Load the first candidate folder that has a manifest.
    ///
    /// `IDLEON_ASSETS_DIR` wins when set, so a packaged build can point at its
    /// own copy. Otherwise the folder next to the executable, the working
    /// directory and the repository root are tried in that order.
    pub fn load_default() -> Self {
        if let Some(dir) = std::env::var_os(ENV_DIR) {
            return Self::load(dir);
        }
        let mut candidates = Vec::new();
        if let Ok(executable) = std::env::current_exe()
            && let Some(parent) = executable.parent()
        {
            candidates.push(parent.join(DEFAULT_DIR));
        }
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join(DEFAULT_DIR));
        }
        candidates.push(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(DEFAULT_DIR),
        );
        for candidate in candidates {
            if candidate.join(MANIFEST_FILE).is_file() {
                return Self::load(candidate);
            }
        }
        Self::default()
    }

    /// Parse a manifest and keep only the files that are actually on disk.
    fn parse(dir: &Path, text: &str) -> Result<Self, String> {
        let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let entries = value["files"]
            .as_array()
            .ok_or_else(|| "manifest has no files array".to_string())?;
        let mut files = BTreeMap::new();
        let mut missing_files = 0;
        for entry in entries {
            let Some(path) = entry["path"].as_str() else {
                missing_files += 1;
                continue;
            };
            let Some(reference) = normalize(path) else {
                missing_files += 1;
                continue;
            };
            let absolute = dir.join(&reference);
            if absolute.is_file() {
                files.insert(reference, absolute);
            } else {
                missing_files += 1;
            }
        }
        Ok(Self {
            total_files: value["totalFiles"]
                .as_u64()
                .map_or(entries.len(), |total| total as usize),
            total_bytes: value["totalBytes"].as_u64().unwrap_or(0),
            missing_files,
            files,
            status: Status::Ready,
        })
    }

    /// The short line the status bar shows.
    pub fn status_label(&self) -> String {
        match &self.status {
            Status::Ready if self.missing_files == 0 => format!(
                "Art: {} files · {:.1} MB",
                self.files.len(),
                self.total_bytes as f64 / 1024.0 / 1024.0
            ),
            Status::Ready => format!(
                "Art: {}/{} files",
                self.files.len(),
                self.total_files.max(self.files.len())
            ),
            Status::NotInstalled => "Art: not installed".into(),
            Status::Invalid(_) => "Art: manifest invalid".into(),
        }
    }

    /// What `reference` resolves to. A file removed after the import is missing.
    pub fn artwork(&self, reference: &str) -> Artwork<'_> {
        normalize(reference)
            .and_then(|reference| self.files.get(&reference))
            .map_or(Artwork::Missing, |path| Artwork::Ready(path))
    }
}

/// Normalize a reference to a manifest path.
///
/// Accepts the upstream form (`./assets/combat-sim/...`) and the manifest form
/// (`combat-sim/...`), and rejects absolute paths and parent traversal.
pub fn normalize(reference: &str) -> Option<String> {
    let reference = reference.trim().replace('\\', "/");
    let reference = reference.split(['?', '#']).next().unwrap_or_default();
    let reference = reference.trim_start_matches("./");
    let reference = reference.strip_prefix("assets/").unwrap_or(reference);
    if reference.is_empty() || reference.starts_with('/') {
        return None;
    }
    if reference
        .split('/')
        .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return None;
    }
    Some(reference.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "idleon-assets-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch directory");
        dir
    }

    fn write_manifest(dir: &Path, files: &[(&str, usize)]) {
        let entries: Vec<Value> = files
            .iter()
            .map(
                |(path, bytes)| serde_json::json!({ "path": path, "bytes": bytes, "sha256": "00" }),
            )
            .collect();
        let manifest = serde_json::json!({
            "version": 1,
            "files": entries,
            "totalFiles": files.len(),
            "totalBytes": files.iter().map(|(_, bytes)| bytes).sum::<usize>(),
        });
        fs::write(
            dir.join(MANIFEST_FILE),
            serde_json::to_string(&manifest).expect("manifest JSON"),
        )
        .expect("manifest file");
    }

    #[test]
    fn references_accept_upstream_paths_and_reject_escapes() {
        assert_eq!(
            normalize("./assets/combat-sim/player/0.png").as_deref(),
            Some("combat-sim/player/0.png")
        );
        assert_eq!(
            normalize("assets/royal-guardian/RGres0.png?v=2").as_deref(),
            Some("royal-guardian/RGres0.png")
        );
        assert_eq!(
            normalize("combat-sim\\ui\\research\\ResBut0.png").as_deref(),
            Some("combat-sim/ui/research/ResBut0.png")
        );
        assert_eq!(
            normalize("  ./assets/sneaking/NjR0.png  ").as_deref(),
            Some("sneaking/NjR0.png")
        );
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("../assets/secret.png"), None);
        assert_eq!(normalize("assets/../secret.png"), None);
        assert_eq!(normalize("/etc/passwd"), None);
        assert_eq!(normalize("assets//empty.png"), None);
    }

    #[test]
    fn a_missing_manifest_is_not_installed() {
        let dir = scratch("missing-manifest");
        let manifest = Manifest::load(&dir);
        assert_eq!(manifest.status_label(), "Art: not installed");
        assert_eq!(
            manifest.artwork("combat-sim/player/0.png"),
            Artwork::Missing
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_broken_manifest_is_invalid_and_never_panics() {
        let dir = scratch("broken-manifest");
        fs::write(dir.join(MANIFEST_FILE), "{not json").expect("manifest file");
        let manifest = Manifest::load(&dir);
        assert_eq!(manifest.status_label(), "Art: manifest invalid");
        assert_eq!(
            manifest.artwork("combat-sim/player/0.png"),
            Artwork::Missing
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn only_files_on_disk_resolve_and_deleted_ones_are_missing() {
        let dir = scratch("resolve");
        fs::create_dir_all(dir.join("combat-sim/player")).expect("art folder");
        fs::write(dir.join("combat-sim/player/0.png"), b"png").expect("art file");
        write_manifest(
            &dir,
            &[
                ("combat-sim/player/0.png", 3),
                ("combat-sim/player/1.png", 3),
            ],
        );
        let manifest = Manifest::load(&dir);
        assert_eq!(manifest.status_label(), "Art: 1/2 files");
        assert_eq!(
            manifest.artwork("./assets/combat-sim/player/0.png"),
            Artwork::Ready(&dir.join("combat-sim/player/0.png"))
        );
        assert_eq!(
            manifest.artwork("combat-sim/player/1.png"),
            Artwork::Missing
        );
        assert_eq!(manifest.artwork("../manifest.json"), Artwork::Missing);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_manifest_without_a_files_array_is_invalid() {
        let dir = scratch("no-files");
        fs::write(dir.join(MANIFEST_FILE), r#"{"version":1}"#).expect("manifest file");
        let manifest = Manifest::load(&dir);
        assert_eq!(manifest.status_label(), "Art: manifest invalid");
        let _ = fs::remove_dir_all(dir);
    }
}
