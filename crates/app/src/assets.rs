//! Manifest-backed game artwork.
//!
//! The images are Legends of Idleon game artwork, committed under
//! `assets/game/` with a credit note. The app never assumes a path exists: it
//! reads `assets/game/manifest.json`, resolves a reference only when the file
//! is on disk, and reports the rest as missing so the UI can draw a labelled
//! placeholder.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The manifest file inside the asset folder.
pub(crate) const MANIFEST_FILE: &str = "manifest.json";
/// The default asset folder, relative to the repository root.
pub(crate) const DEFAULT_DIR: &str = "assets/game";
/// Overrides the asset folder, for packaged builds and harnesses.
pub(crate) const ENV_DIR: &str = "IDLEON_ASSETS_DIR";

/// What an image reference resolves to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Artwork<'a> {
    /// The file exists and the manifest lists it.
    Ready(&'a Path),
    /// The reference is not in the manifest, or the file is gone.
    Missing,
}

/// The manifest's entries that are on disk; a missing or unreadable manifest
/// has none, so every reference falls back to its placeholder.
#[derive(Clone, Debug, Default)]
pub(crate) struct Manifest {
    files: BTreeMap<String, PathBuf>,
}

impl Manifest {
    /// Load the manifest in `dir`; a missing or unreadable one is a state, not an error.
    pub(crate) fn load(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let path = dir.join(MANIFEST_FILE);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        Self::parse(&dir, &text).unwrap_or_default()
    }

    /// Load the first candidate folder that has a manifest.
    ///
    /// `IDLEON_ASSETS_DIR` wins when set, so a packaged build can point at its
    /// own copy. Otherwise the folder next to the executable (on macOS, the
    /// app bundle's `Contents/Resources` first) and the working directory are
    /// tried in that order; a debug build also tries the repository root.
    pub(crate) fn load_default() -> Self {
        if let Some(dir) = std::env::var_os(ENV_DIR) {
            return Self::load(dir);
        }
        let mut candidates = Vec::new();
        if let Ok(executable) = std::env::current_exe()
            && let Some(parent) = executable.parent()
        {
            // `Contents/MacOS` holds only code: data there is signed as nested
            // code and breaks the seal when unpacked without its xattrs.
            #[cfg(target_os = "macos")]
            candidates.push(parent.join("../Resources").join(DEFAULT_DIR));
            candidates.push(parent.join(DEFAULT_DIR));
        }
        if let Ok(cwd) = std::env::current_dir() {
            candidates.push(cwd.join(DEFAULT_DIR));
        }
        // The repository root is a development convenience; a release build
        // must not carry the builder's checkout path.
        #[cfg(debug_assertions)]
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
        for entry in entries {
            let Some(reference) = entry["path"].as_str().and_then(normalize) else {
                continue;
            };
            let absolute = dir.join(&reference);
            if absolute.is_file() {
                files.insert(reference, absolute);
            }
        }
        Ok(Self { files })
    }

    /// What `reference` resolves to. A file removed after the import is missing.
    pub(crate) fn artwork(&self, reference: &str) -> Artwork<'_> {
        normalize(reference)
            .and_then(|reference| self.files.get(&reference))
            .map_or(Artwork::Missing, |path| Artwork::Ready(path))
    }
}

/// Normalize a reference to a manifest path.
///
/// Accepts the data-table form (`./assets/combat-sim/...`) and the manifest form
/// (`combat-sim/...`), and rejects absolute paths and parent traversal.
pub(crate) fn normalize(reference: &str) -> Option<String> {
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
    fn references_accept_table_paths_and_reject_escapes() {
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
    fn a_missing_manifest_resolves_nothing() {
        let dir = scratch("missing-manifest");
        let manifest = Manifest::load(&dir);
        assert_eq!(
            manifest.artwork("combat-sim/player/0.png"),
            Artwork::Missing
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_broken_manifest_resolves_nothing_and_never_panics() {
        for (name, text) in [("not-json", "{not json"), ("no-files", r#"{"version":1}"#)] {
            let dir = scratch(name);
            fs::create_dir_all(dir.join("combat-sim/player")).expect("art folder");
            fs::write(dir.join("combat-sim/player/0.png"), b"png").expect("art file");
            fs::write(dir.join(MANIFEST_FILE), text).expect("manifest file");
            let manifest = Manifest::load(&dir);
            assert_eq!(
                manifest.artwork("combat-sim/player/0.png"),
                Artwork::Missing,
                "{name}"
            );
            let _ = fs::remove_dir_all(dir);
        }
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
    fn every_combat_replay_reference_is_in_the_manifest() {
        // The replay resolves map art and monster sheets by the reference the
        // extracted table carries; a file the manifest cannot resolve draws a
        // placeholder, so the committed set must cover every reference.
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(DEFAULT_DIR);
        let manifest = Manifest::load(dir);
        let mut missing = Vec::new();
        for map in idleon_combat::encounter::maps() {
            for reference in map.art.bg.iter().chain(map.art.plat.iter()) {
                let path = format!("combat-sim/{reference}");
                if manifest.artwork(&path) == Artwork::Missing {
                    missing.push(path);
                }
            }
            for monster in &map.mobs {
                let Some(sprite) = &monster.sprite else {
                    continue;
                };
                let path = format!("combat-sim/{}", sprite.file);
                if manifest.artwork(&path) == Artwork::Missing {
                    missing.push(path);
                }
            }
        }
        missing.sort_unstable();
        missing.dedup();
        assert!(
            missing.is_empty(),
            "replay files not in the manifest: {missing:?}"
        );
    }
}
