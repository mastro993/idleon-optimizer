//! A local log and a crash record, so a problem outside a terminal leaves a trace.
//!
//! GPUI and GPUI Kit report recoverable errors through the `log` facade; without a
//! logger they vanish. This module installs one that appends warnings and
//! errors to `idleon-optimizer.log` in the app's state directory, and a panic
//! hook that appends the panic's message and location to the same file before
//! the default hook runs.
//!
//! Privacy: the file never leaves the machine, and nothing here writes save
//! values. Callers must log counts, ids and field names only (`docs/plan.md`
//! §5.5); this module adds a timestamp, a level and the logging module's path.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// The log's name inside the state directory.
pub(crate) const FILE: &str = "idleon-optimizer.log";

/// A log this large is rotated to `<name>.old` at launch, so the file never
/// grows without bound.
const ROTATE_BYTES: u64 = 1024 * 1024;

struct FileLog {
    file: Mutex<File>,
}

impl log::Log for FileLog {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Warn
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        write_line(
            &self.file,
            &format!("{} {}: {}", record.level(), record.target(), record.args()),
        );
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

/// Append one timestamped line; a poisoned lock or a failed write is ignored,
/// because diagnostics must never take the app down.
fn write_line(file: &Mutex<File>, line: &str) {
    let mut file = file.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let _ = writeln!(file, "{} {line}", jiff::Timestamp::now());
}

/// Rotate an oversized log, then open it for appending.
fn open(dir: &Path) -> std::io::Result<File> {
    fs::create_dir_all(dir)?;
    let path = dir.join(FILE);
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > ROTATE_BYTES) {
        let _ = fs::rename(&path, dir.join(format!("{FILE}.old")));
    }
    OpenOptions::new().create(true).append(true).open(path)
}

/// Install the file logger and the panic hook; returns the log's path, or
/// `None` when the directory cannot be written (the app then runs without one).
pub(crate) fn install(dir: &Path) -> Option<PathBuf> {
    let file = open(dir).ok()?;
    let for_panics = file.try_clone().ok().map(Mutex::new);
    // A second install (tests, or a host that already set a logger) is harmless.
    if log::set_boxed_logger(Box::new(FileLog {
        file: Mutex::new(file),
    }))
    .is_ok()
    {
        log::set_max_level(log::LevelFilter::Warn);
    }
    if let Some(file) = for_panics {
        let default = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            write_line(&file, &format!("PANIC {info}"));
            default(info);
        }));
    }
    Some(dir.join(FILE))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "idleon-diagnostics-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_line_is_appended_with_a_timestamp() {
        let dir = scratch("line");
        let file = Mutex::new(open(&dir).expect("the log opens"));
        write_line(&file, "WARN test: first");
        write_line(&file, "WARN test: second");
        let text = fs::read_to_string(dir.join(FILE)).expect("the log reads");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].ends_with("WARN test: first"));
        assert!(lines[1].ends_with("WARN test: second"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn an_oversized_log_is_rotated_at_open() {
        let dir = scratch("rotate");
        fs::create_dir_all(&dir).expect("scratch directory");
        fs::write(dir.join(FILE), vec![b'x'; (ROTATE_BYTES + 1) as usize]).expect("a big log");
        let _ = open(&dir).expect("the log opens");
        assert!(dir.join(format!("{FILE}.old")).exists());
        assert_eq!(fs::metadata(dir.join(FILE)).expect("the new log").len(), 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn an_unwritable_directory_installs_nothing() {
        // A path under a regular file cannot be created.
        let dir = scratch("unwritable");
        fs::create_dir_all(&dir).expect("scratch directory");
        let blocker = dir.join("file");
        fs::write(&blocker, b"x").expect("a file");
        assert!(open(&blocker.join("sub")).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
