//! The local app-data session: the last project and the recent imports.
//!
//! Unlike the project export, which a user copies around, this file is the
//! window's own memory. It holds one [`project::Project`] document — the active
//! section, the decoded save, the research state and one config per section —
//! plus the metadata of the imports that were applied, so the next launch lands
//! on the same section with the same save and the import dialog can list what
//! came before.
//!
//! The file lives in the platform app-data directory, or wherever
//! `--state-dir`/`IDLEON_OPTIMIZER_STATE_DIR` points, and is written
//! atomically through a temporary sibling. A file that cannot be read back as a
//! session is renamed to `session.json.corrupt-<timestamp>` and the app starts
//! clean instead of refusing to open.
//!
//! Ported from the browser persistence the upstream spreads over
//! `js/ui/state-io.js` and `js/renderers/upgrade-eval.js`, where the research
//! state and shape layout survive a reload in `localStorage`; the session file
//! gives every section the same treatment and keeps the save with it.

use crate::project::{self, Config};
use crate::state::AppState;
use idleon_core::import as pipeline;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The session file's format marker.
pub const FORMAT: &str = "idleon-optimizer-session";
/// The session file's schema version.
pub const VERSION: u32 = 1;
/// The file name under the app-data directory.
pub const FILE: &str = "session.json";
/// The environment variable that moves the app-data directory.
pub const ENV_DIR: &str = "IDLEON_OPTIMIZER_STATE_DIR";
/// How many recent imports the dialog lists and the file keeps.
pub const RECENT_LIMIT: usize = 8;
/// How long an edit waits before the next session write.
pub const SAVE_DEBOUNCE: Duration = Duration::from_millis(250);

/// One applied import, as the dialog lists it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RecentImport {
    /// Unix seconds, formatted in the local zone when displayed.
    pub at: i64,
    /// The detected format, e.g. `it.json`, `project` or `section config`.
    pub format: String,
    /// The pasted text's label or the dropped file's name.
    pub source: String,
    pub characters: Option<usize>,
    pub research_level: Option<u32>,
    pub warnings: usize,
}

impl RecentImport {
    /// An entry stamped now.
    pub fn new(
        format: impl Into<String>,
        source: impl Into<String>,
        characters: Option<usize>,
        research_level: Option<u32>,
        warnings: usize,
    ) -> Self {
        Self {
            at: jiff::Timestamp::now().as_second(),
            format: format.into(),
            source: source.into(),
            characters,
            research_level,
            warnings,
        }
    }

    /// The line the dialog shows: format, local time and the source.
    pub fn line(&self) -> String {
        let mut line = format!("{} · {}", self.format, time_label(self.at));
        if let Some(characters) = self.characters {
            line.push_str(&format!(" · {characters} character(s)"));
        }
        if !self.source.is_empty() && self.source != self.format {
            line.push_str(&format!(" · {}", self.source));
        }
        line
    }
}

/// The project a saved session restores, decoded and prepared for applying.
pub struct SessionRestore {
    pub project: pipeline::ProjectImport,
    pub configs: Vec<Config>,
}

/// The window's local memory: where the file is and what it held.
pub struct Session {
    path: PathBuf,
    /// The most recent applied imports, newest first.
    pub recent: Vec<RecentImport>,
    /// The project waiting for the window to apply it, taken once at launch.
    pub restore: Option<SessionRestore>,
    /// Why the file was backed up or skipped, for the first status line.
    pub notice: Option<String>,
}

impl Session {
    /// Load the session at `path`; a missing file starts clean, and a corrupt
    /// one is backed up beside itself before the clean start.
    pub fn load(path: PathBuf) -> Self {
        let mut session = Self {
            path,
            recent: Vec::new(),
            restore: None,
            notice: None,
        };
        let text = match fs::read_to_string(&session.path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return session,
            Err(error) => {
                session.notice = Some(format!(
                    "The session file could not be read ({error}); starting clean."
                ));
                return session;
            }
        };
        match parse(&text) {
            Ok((restore, recent)) => {
                session.restore = Some(restore);
                session.recent = recent;
            }
            Err(reason) => {
                let backup = backup(&session.path);
                session.notice = Some(match backup {
                    Some(backup) => format!(
                        "The session file was corrupt ({reason}); backed it up to {} and started clean.",
                        backup.display()
                    ),
                    None => format!("The session file was corrupt ({reason}); starting clean."),
                });
            }
        }
        session
    }

    /// The default file under the platform app-data directory, or under
    /// `explicit` when the launch names one.
    pub fn path(explicit: Option<PathBuf>) -> PathBuf {
        if let Some(dir) = explicit {
            return dir.join(FILE);
        }
        if let Some(dir) = std::env::var_os(ENV_DIR) {
            return PathBuf::from(dir).join(FILE);
        }
        directories::ProjectDirs::from("", "", "idleon-optimizer")
            .map(|dirs| dirs.data_local_dir().join(FILE))
            .unwrap_or_else(|| PathBuf::from(".idleon-optimizer").join(FILE))
    }

    /// Add an applied import to the front of the list and drop the oldest.
    pub fn record(&mut self, recent: RecentImport) {
        self.recent.insert(0, recent);
        self.recent.truncate(RECENT_LIMIT);
    }

    /// Write the whole session: the project document plus the recent imports.
    pub fn save(&self, project: &Value) -> io::Result<()> {
        let text = serde_json::json!({
            "format": FORMAT,
            "version": VERSION,
            "project": project,
            "recentImports": self.recent,
        });
        let text = serde_json::to_string(&text).map_err(io::Error::other)?;
        write_atomic(&self.path, &text)
    }
}

/// The session document a text carries, or why it cannot be read back.
fn parse(text: &str) -> Result<(SessionRestore, Vec<RecentImport>), String> {
    let value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "the session is not an object".to_string())?;
    match object.get("format").and_then(Value::as_str) {
        Some(FORMAT) => {}
        Some(format) => return Err(format!("unknown format \"{format}\"")),
        None => return Err("the session carries no format marker".into()),
    }
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .ok_or_else(|| "the session carries no version".to_string())?;
    if version != u64::from(VERSION) {
        return Err(format!(
            "unsupported session version {version}; this build reads {VERSION}"
        ));
    }
    let project = object
        .get("project")
        .ok_or_else(|| "the session carries no project".to_string())?;
    Ok((restore(project)?, recent(object.get("recentImports"))))
}

/// Decode the embedded project and every section config it carries.
fn restore(value: &Value) -> Result<SessionRestore, String> {
    let mut document =
        pipeline::read(&value.to_string()).map_err(|error| format!("project: {error}"))?;
    if !matches!(document, pipeline::Document::Project(_)) {
        return Err("the session's project is not a project document".into());
    }
    let configs = project::prepare(&mut document).map_err(|error| format!("project: {error}"))?;
    let pipeline::Document::Project(project) = document else {
        unreachable!("the document was just checked to be a project")
    };
    Ok(SessionRestore {
        project: *project,
        configs,
    })
}

/// The recent list, skipping entries this build cannot read. The list is
/// cosmetic, so one bad row never costs the restored project.
fn recent(value: Option<&Value>) -> Vec<RecentImport> {
    let Some(entries) = value.and_then(Value::as_array) else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
        .take(RECENT_LIMIT)
        .collect()
}

/// Rename a corrupt session beside itself, keeping its bytes for inspection.
fn backup(path: &Path) -> Option<PathBuf> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    let stem = path.file_stem()?.to_string_lossy();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("json");
    for attempt in 0..32 {
        let suffix = if attempt == 0 {
            String::new()
        } else {
            format!("-{attempt}")
        };
        let candidate = path.with_file_name(format!("{stem}.corrupt-{stamp}{suffix}.{extension}"));
        if !candidate.exists() {
            return fs::rename(path, &candidate).ok().map(|()| candidate);
        }
    }
    None
}

/// Write through a temporary sibling so an interrupted save never leaves half
/// a session behind.
fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| FILE.to_string());
    let temporary = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    fs::write(&temporary, text)?;
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}

/// A Unix second as the local time the dialog shows.
pub fn time_label(at: i64) -> String {
    let Ok(timestamp) = jiff::Timestamp::from_second(at) else {
        return at.to_string();
    };
    let local = timestamp.to_zoned(jiff::tz::TimeZone::system());
    jiff::fmt::strtime::format("%b %-d, %H:%M", local.datetime())
        .unwrap_or_else(|_| timestamp.to_string())
}

/// The project a window state exports, as the session stores it.
pub fn project_value(state: &AppState) -> Option<Value> {
    serde_json::from_str(&project::export(state)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_core::project as core_project;
    use serde_json::json;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "idleon-session-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("scratch directory");
        dir
    }

    fn project_value() -> Value {
        let project = core_project::Project {
            version: core_project::VERSION,
            section: Some("killroy".into()),
            save: None,
            simulation: Default::default(),
            context: Default::default(),
            sections: [("killroy".to_string(), json!({ "weeks": 20 }))]
                .into_iter()
                .collect(),
            warnings: Vec::new(),
            origin: core_project::Origin::Project,
        };
        serde_json::from_str(&core_project::write(&project)).expect("the project writes JSON")
    }

    #[test]
    fn a_missing_session_starts_clean() {
        let dir = scratch("missing");
        let session = Session::load(dir.join(FILE));
        assert!(session.restore.is_none());
        assert!(session.notice.is_none());
        assert!(session.recent.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_round_trip_restores_the_project_and_the_recent_imports() {
        let dir = scratch("round-trip");
        let path = dir.join(FILE);
        let mut session = Session::load(path.clone());
        session.record(RecentImport::new(
            "save.json",
            "it.json",
            Some(10),
            Some(120),
            2,
        ));
        session.save(&project_value()).expect("the session saves");

        let restored = Session::load(path);
        assert!(restored.notice.is_none());
        let restore = restored.restore.expect("the project reads back");
        assert_eq!(restore.project.document.section.as_deref(), Some("killroy"));
        assert!(matches!(
            restore.configs.as_slice(),
            [Config::Killroy(crate::sections::killroy::State {
                weeks: 20
            })]
        ));
        assert_eq!(restored.recent.len(), 1);
        assert_eq!(restored.recent[0].format, "save.json");
        assert_eq!(restored.recent[0].characters, Some(10));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_corrupt_session_is_backed_up_and_the_window_starts_clean() {
        let dir = scratch("corrupt");
        let path = dir.join(FILE);
        fs::write(&path, "{not a session").expect("the corrupt file");
        let session = Session::load(path.clone());
        assert!(session.restore.is_none());
        assert!(session.recent.is_empty());
        let notice = session.notice.expect("the backup is reported");
        assert!(notice.contains("corrupt"), "{notice}");
        assert!(!path.exists(), "the corrupt file moved aside");
        let backups: Vec<PathBuf> = fs::read_dir(&dir)
            .expect("the scratch directory")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("session.corrupt-"))
            })
            .collect();
        assert_eq!(backups.len(), 1, "{backups:?}");
        assert_eq!(
            fs::read_to_string(&backups[0]).expect("the backup keeps its bytes"),
            "{not a session"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unknown_format_is_backed_up_too() {
        let dir = scratch("format");
        let path = dir.join(FILE);
        fs::write(
            &path,
            json!({ "format": "some-other-tool", "version": 1 }).to_string(),
        )
        .expect("the wrong file");
        let session = Session::load(path.clone());
        let notice = session.notice.expect("the mismatch is reported");
        assert!(notice.contains("unknown format"), "{notice}");
        assert!(!path.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_session_whose_project_cannot_apply_is_corrupt() {
        let dir = scratch("bad-project");
        let path = dir.join(FILE);
        fs::write(
            &path,
            json!({
                "format": FORMAT,
                "version": VERSION,
                "project": { "format": core_project::FORMAT, "version": 9 },
            })
            .to_string(),
        )
        .expect("the broken project file");
        let session = Session::load(path.clone());
        let notice = session.notice.expect("the broken project is reported");
        assert!(notice.contains("project"), "{notice}");
        assert!(!path.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn recent_imports_keep_the_newest_and_drop_the_oldest() {
        let dir = scratch("recent-cap");
        let mut session = Session::load(dir.join(FILE));
        for second in 0..(RECENT_LIMIT + 3) {
            session.record(RecentImport {
                at: second as i64,
                format: "it.json".into(),
                ..RecentImport::default()
            });
        }
        assert_eq!(session.recent.len(), RECENT_LIMIT);
        assert_eq!(
            session.recent.first().map(|entry| entry.at),
            Some((RECENT_LIMIT + 2) as i64)
        );
        assert_eq!(session.recent.last().map(|entry| entry.at), Some(3));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_recent_line_names_the_format_its_time_and_its_source() {
        let entry = RecentImport {
            at: 1_700_000_000,
            format: "save.json".into(),
            source: "it.json".into(),
            characters: Some(4),
            research_level: Some(120),
            warnings: 0,
        };
        let line = entry.line();
        assert!(line.contains("save.json"), "{line}");
        assert!(line.contains("it.json"), "{line}");
        assert!(line.contains("4 character(s)"), "{line}");
        assert!(line.contains(':'), "the time has a clock: {line}");
    }

    #[test]
    fn the_state_directory_override_ends_at_the_session_file() {
        let explicit = PathBuf::from("/tmp/idleon-state");
        assert_eq!(
            Session::path(Some(explicit)),
            PathBuf::from("/tmp/idleon-state/session.json")
        );
    }
}
