//! Watches a package's sources, and every local crate it builds from, for
//! `cargo styleguide dev` and `serve --watch`.

use anyhow::{Context as _, Result};
use notify::{EventKind, RecursiveMode, Watcher as _};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{Receiver, RecvTimeoutError, channel},
    time::Duration,
};

/// How long to wait for an editor's burst of writes to settle.
const SETTLE: Duration = Duration::from_millis(300);

pub struct Watcher {
    _watcher: notify::RecommendedWatcher,
    events: Receiver<notify::Result<notify::Event>>,
    ignored: Vec<PathBuf>,
}

impl Watcher {
    /// Watches every package in `metadata` that lives on disk rather than in
    /// a registry, ignoring changes under `ignored` (the target and site
    /// directories, which the build itself writes).
    pub fn new(metadata: &Value, ignored: Vec<PathBuf>) -> Result<Self> {
        let (sender, events) = channel();
        let mut watcher = notify::recommended_watcher(sender).context("starting file watcher")?;
        let dirs = local_package_dirs(metadata);
        for dir in &dirs {
            watcher
                .watch(dir, RecursiveMode::Recursive)
                .with_context(|| format!("watching {}", dir.display()))?;
        }
        eprintln!(
            "watching {}",
            dirs.iter()
                .map(|dir| dir.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        Ok(Self {
            _watcher: watcher,
            events,
            ignored: ignored
                .into_iter()
                .map(|path| path.canonicalize().unwrap_or(path))
                .collect(),
        })
    }

    /// Waits up to `timeout` for a source change and returns the changed
    /// paths once the writes settle, or nothing if none came.
    pub fn changed(&self, timeout: Duration) -> Vec<PathBuf> {
        let mut changed = Vec::new();
        let mut wait = timeout;
        loop {
            match self.events.recv_timeout(wait) {
                Ok(Ok(event)) if self.relevant(&event) => {
                    changed.extend(event.paths);
                    wait = SETTLE;
                }
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                    return changed;
                }
            }
        }
    }

    fn relevant(&self, event: &notify::Event) -> bool {
        matches!(
            event.kind,
            EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)
        ) && event.paths.iter().any(|path| self.is_source(path))
    }

    fn is_source(&self, path: &Path) -> bool {
        if self.ignored.iter().any(|ignored| path.starts_with(ignored)) {
            return false;
        }
        // Skip version control and editor scratch files: `.git/…`, `.foo.swp`,
        // `foo~`, `4913`.
        let hidden = path.components().any(|part| {
            part.as_os_str()
                .to_str()
                .is_some_and(|part| part.starts_with('.') && part.len() > 1 && part != "..")
        });
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        !hidden && !name.ends_with('~') && !name.ends_with(".swp") && name != "4913"
    }
}

/// The directories of local (path and workspace) packages, without ones
/// nested inside another.
fn local_package_dirs(metadata: &Value) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = metadata["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|package| package["source"].is_null())
        .filter_map(|package| package["manifest_path"].as_str())
        .filter_map(|manifest| Path::new(manifest).parent().map(Path::to_path_buf))
        .collect();
    dirs.sort();
    dirs.dedup();
    let all = dirs.clone();
    dirs.retain(|dir| {
        !all.iter()
            .any(|other| other != dir && dir.starts_with(other))
    });
    dirs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_package_dirs_are_watched_once() {
        let metadata = serde_json::json!({ "packages": [
            { "source": null, "manifest_path": "/ws/Cargo.toml" },
            { "source": null, "manifest_path": "/ws/crates/a/Cargo.toml" },
            { "source": null, "manifest_path": "/other/b/Cargo.toml" },
            { "source": "registry+https://github.com/rust-lang/crates.io-index", "manifest_path": "/registry/c/Cargo.toml" },
        ]});
        assert_eq!(
            local_package_dirs(&metadata),
            [PathBuf::from("/other/b"), PathBuf::from("/ws")]
        );
    }
}
