use anyhow::{Context, Result};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command as TokioCommand;
use tokio::sync::{mpsc, Mutex as TokioMutex};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: PathBuf,
    pub status: DiffStatus,
    pub additions: usize,
    pub deletions: usize,
    pub diff_output: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Untracked,
}

impl std::fmt::Display for DiffStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiffStatus::Added => write!(f, "A"),
            DiffStatus::Modified => write!(f, "M"),
            DiffStatus::Deleted => write!(f, "D"),
            DiffStatus::Renamed => write!(f, "R"),
            DiffStatus::Copied => write!(f, "C"),
            DiffStatus::Untracked => write!(f, "?"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorktreeDiff {
    pub worktree_id: uuid::Uuid,
    pub worktree_path: PathBuf,
    pub files: Vec<FileDiff>,
    pub last_updated: chrono::DateTime<chrono::Utc>,
    pub total_additions: usize,
    pub total_deletions: usize,
}

#[derive(Clone)]
pub struct DiffTracker {
    watchers: Arc<parking_lot::RwLock<Vec<(uuid::Uuid, RecommendedWatcher)>>>,
    worktree_paths: Arc<parking_lot::RwLock<HashMap<uuid::Uuid, PathBuf>>>,
    diffs: Arc<parking_lot::RwLock<Vec<WorktreeDiff>>>,
    tx: mpsc::UnboundedSender<DiffEvent>,
    rx: Arc<TokioMutex<Option<mpsc::UnboundedReceiver<DiffEvent>>>>,
}

#[derive(Debug)]
pub enum DiffEvent {
    FileChanged(uuid::Uuid, PathBuf),
    WorktreeRemoved(uuid::Uuid),
    RefreshRequested(uuid::Uuid),
}

impl DiffTracker {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            watchers: Arc::new(parking_lot::RwLock::new(Vec::new())),
            worktree_paths: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            diffs: Arc::new(parking_lot::RwLock::new(Vec::new())),
            tx,
            rx: Arc::new(TokioMutex::new(Some(rx))),
        }
    }

    pub fn sender(&self) -> mpsc::UnboundedSender<DiffEvent> {
        self.tx.clone()
    }

    pub async fn start_watching(&self, worktree_id: uuid::Uuid, path: PathBuf) -> Result<()> {
        let tx = self.tx.clone();

        self.worktree_paths.write().insert(worktree_id, path.clone());

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    ) {
                        for p in event.paths {
                            if p.extension().map_or(true, |ext| ext != "git") {
                                let _ = tx.send(DiffEvent::FileChanged(worktree_id, p));
                            }
                        }
                    }
                }
                Err(e) => warn!("Watcher error for worktree {}: {}", worktree_id, e),
            },
            notify::Config::default(),
        )
        .context("Failed to create file watcher")?;

        watcher
            .watch(&path, RecursiveMode::Recursive)
            .context("Failed to watch worktree path")?;

        {
            let mut watchers = self.watchers.write();
            watchers.push((worktree_id, watcher));
        }

        info!("Started watching worktree {} at {}", worktree_id, path.display());

        // Queue initial diff calculation
        let _ = self.tx.send(DiffEvent::RefreshRequested(worktree_id));

        Ok(())
    }

    pub async fn stop_watching(&self, worktree_id: uuid::Uuid) -> Result<()> {
        let watcher = {
            let mut watchers = self.watchers.write();
            watchers
                .iter()
                .position(|(id, _)| *id == worktree_id)
                .map(|idx| watchers.remove(idx).1)
        };

        if let Some(mut watcher) = watcher {
            let _ = watcher.unwatch(Path::new("."));
        }

        self.worktree_paths.write().remove(&worktree_id);

        {
            let mut diffs = self.diffs.write();
            diffs.retain(|d| d.worktree_id != worktree_id);
        }

        info!("Stopped watching worktree {}", worktree_id);
        Ok(())
    }

    pub async fn compute_diff(&self, worktree_id: uuid::Uuid, path: &Path) -> Result<WorktreeDiff> {
        let output = TokioCommand::new("git")
            .args(["diff", "--numstat", "HEAD"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to run git diff --numstat")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();
        let mut total_additions = 0;
        let mut total_deletions = 0;

        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                let additions = parts[0].parse().unwrap_or(0);
                let deletions = parts[1].parse().unwrap_or(0);
                let file_path = PathBuf::from(parts[2]);

                total_additions += additions;
                total_deletions += deletions;

                let status = self.get_file_status(path, &file_path).await.unwrap_or(DiffStatus::Modified);
                let diff_output = self.get_file_diff(path, &file_path).await.unwrap_or_default();

                files.push(FileDiff {
                    path: file_path,
                    status,
                    additions,
                    deletions,
                    diff_output,
                });
            }
        }

        let untracked = self.get_untracked_files(path).await.unwrap_or_default();
        for file_path in untracked {
            let diff_output = self.get_untracked_file_content(path, &file_path).await.unwrap_or_default();
            let additions = diff_output.lines().count();
            total_additions += additions;
            files.push(FileDiff {
                path: file_path,
                status: DiffStatus::Untracked,
                additions,
                deletions: 0,
                diff_output,
            });
        }

        let diff = WorktreeDiff {
            worktree_id,
            worktree_path: path.to_path_buf(),
            files,
            last_updated: chrono::Utc::now(),
            total_additions,
            total_deletions,
        };

        Ok(diff)
    }

    async fn get_file_status(&self, repo_path: &Path, file_path: &Path) -> Result<DiffStatus> {
        let output = TokioCommand::new("git")
            .args(["status", "--porcelain", &file_path.to_string_lossy()])
            .current_dir(repo_path)
            .output()
            .await
            .context("Failed to run git status")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let status_char = stdout.chars().next().unwrap_or(' ');

        Ok(match status_char {
            'A' => DiffStatus::Added,
            'M' => DiffStatus::Modified,
            'D' => DiffStatus::Deleted,
            'R' => DiffStatus::Renamed,
            'C' => DiffStatus::Copied,
            '?' => DiffStatus::Untracked,
            _ => DiffStatus::Modified,
        })
    }

    async fn get_file_diff(&self, repo_path: &Path, file_path: &Path) -> Result<String> {
        let output = TokioCommand::new("git")
            .args(["diff", "HEAD", "--", &file_path.to_string_lossy()])
            .current_dir(repo_path)
            .output()
            .await
            .context("Failed to run git diff for file")?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    async fn get_untracked_files(&self, repo_path: &Path) -> Result<Vec<PathBuf>> {
        let output = TokioCommand::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo_path)
            .output()
            .await
            .context("Failed to run git status for untracked")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();

        for line in stdout.lines() {
            if line.starts_with("??") {
                let file_path = line[3..].trim();
                files.push(PathBuf::from(file_path));
            }
        }

        Ok(files)
    }

    async fn get_untracked_file_content(&self, repo_path: &Path, file_path: &Path) -> Result<String> {
        let full_path = repo_path.join(file_path);
        if full_path.is_file() {
            let content = tokio::fs::read_to_string(&full_path).await?;
            let mut diff = format!("--- /dev/null\n+++ b/{}\n", file_path.display());
            for line in content.lines() {
                diff.push('+');
                diff.push_str(line);
                diff.push('\n');
            }
            Ok(diff)
        } else {
            Ok(String::new())
        }
    }

    pub async fn update_diff(&self, worktree_id: uuid::Uuid, path: &Path) -> Result<()> {
        let diff = self.compute_diff(worktree_id, path).await?;
        let mut diffs = self.diffs.write();

        if let Some(idx) = diffs.iter().position(|d| d.worktree_id == worktree_id) {
            diffs[idx] = diff;
        } else {
            diffs.push(diff);
        }

        Ok(())
    }

    pub fn get_diff(&self, worktree_id: uuid::Uuid) -> Option<WorktreeDiff> {
        let diffs = self.diffs.read();
        diffs.iter().find(|d| d.worktree_id == worktree_id).cloned()
    }

    pub fn get_all_diffs(&self) -> Vec<WorktreeDiff> {
        let diffs = self.diffs.read();
        diffs.clone()
    }

    pub fn get_worktree_path(&self, worktree_id: uuid::Uuid) -> Option<PathBuf> {
        self.worktree_paths.read().get(&worktree_id).cloned()
    }

    pub async fn run_event_loop(&self) -> Result<()> {
        let mut rx = self
            .rx
            .lock()
            .await
            .take()
            .ok_or_else(|| anyhow::anyhow!("Event loop already running"))?;

        while let Some(event) = rx.recv().await {
            match event {
                DiffEvent::FileChanged(worktree_id, _) => {
                    if let Some(worktree_dir) = self.get_worktree_path(worktree_id) {
                        debug!("Updating diff for worktree {}: {}", worktree_id, worktree_dir.display());
                        if let Err(e) = self.update_diff(worktree_id, &worktree_dir).await {
                            error!("Failed to update diff for worktree {}: {}", worktree_id, e);
                        }
                    }
                }
                DiffEvent::WorktreeRemoved(worktree_id) => {
                    if let Err(e) = self.stop_watching(worktree_id).await {
                        error!("Failed to stop watching worktree {}: {}", worktree_id, e);
                    }
                }
                DiffEvent::RefreshRequested(worktree_id) => {
                    if let Some(worktree_dir) = self.get_worktree_path(worktree_id) {
                        if let Err(e) = self.update_diff(worktree_id, &worktree_dir).await {
                            error!("Failed to refresh diff for worktree {}: {}", worktree_id, e);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for DiffTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_status_formatting() {
        assert_eq!(DiffStatus::Added.to_string(), "A");
        assert_eq!(DiffStatus::Modified.to_string(), "M");
        assert_eq!(DiffStatus::Deleted.to_string(), "D");
        assert_eq!(DiffStatus::Renamed.to_string(), "R");
        assert_eq!(DiffStatus::Copied.to_string(), "C");
        assert_eq!(DiffStatus::Untracked.to_string(), "?");
    }

    #[test]
    fn test_tracker_sync_queries() {
        let tracker = DiffTracker::new();
        let wt_id = uuid::Uuid::new_v4();
        assert!(tracker.get_diff(wt_id).is_none());
        assert!(tracker.get_all_diffs().is_empty());
        assert!(tracker.get_worktree_path(wt_id).is_none());
    }
}

