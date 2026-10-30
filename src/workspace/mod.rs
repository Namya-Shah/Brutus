use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::process::Command as TokioCommand;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
    pub id: Uuid,
    pub name: String,
    pub path: PathBuf,
    pub branch: String,
    pub base_branch: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub agent_type: Option<AgentType>,
    pub status: WorktreeStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorktreeStatus {
    Creating,
    Ready,
    Running,
    Completed,
    Failed,
    CleaningUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentType {
    ClaudeCode,
    Codex,
    Opencode,
    Custom,
}

impl std::fmt::Display for AgentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentType::ClaudeCode => write!(f, "claude"),
            AgentType::Codex => write!(f, "codex"),
            AgentType::Opencode => write!(f, "opencode"),
            AgentType::Custom => write!(f, "custom"),
        }
    }
}

pub struct WorkspaceManager {
    base_path: PathBuf,
    base_branch: String,
    worktrees: parking_lot::RwLock<Vec<Worktree>>,
}

impl WorkspaceManager {
    pub fn new<P: AsRef<Path>>(base_path: P, base_branch: String) -> Result<Self> {
        let base_path = base_path.as_ref().to_path_buf();
        if !base_path.exists() {
            anyhow::bail!("Base path does not exist: {}", base_path.display());
        }

        let output = Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(&base_path)
            .output()
            .context("Failed to check if base path is a git repository")?;

        if !output.status.success() {
            anyhow::bail!("Base path is not a git repository: {}", base_path.display());
        }

        Ok(Self {
            base_path,
            base_branch,
            worktrees: parking_lot::RwLock::new(Vec::new()),
        })
    }

    pub async fn create_worktree(
        &self,
        name: String,
        branch: Option<String>,
        agent_type: Option<AgentType>,
    ) -> Result<Worktree> {
        let id = Uuid::new_v4();
        let branch_name = branch.unwrap_or_else(|| format!("brutus/{}", name));
        let worktree_path = self.base_path.join(".brutus").join(&name);

        if worktree_path.exists() {
            tokio::fs::remove_dir_all(&worktree_path).await
                .context("Failed to remove existing worktree directory")?;
        }

        tokio::fs::create_dir_all(worktree_path.parent().unwrap()).await
            .context("Failed to create .brutus directory")?;

        let mut worktree = Worktree {
            id,
            name: name.clone(),
            path: worktree_path.clone(),
            branch: branch_name.clone(),
            base_branch: self.base_branch.clone(),
            created_at: chrono::Utc::now(),
            agent_type,
            status: WorktreeStatus::Creating,
        };

        {
            let mut worktrees = self.worktrees.write();
            worktrees.push(worktree.clone());
        }

        self.run_git_worktree_add(&worktree).await?;

        worktree.status = WorktreeStatus::Ready;
        self.update_worktree(&worktree);

        Ok(worktree)
    }

    async fn run_git_worktree_add(&self, worktree: &Worktree) -> Result<()> {
        let output = TokioCommand::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                &worktree.branch,
                &worktree.path.to_string_lossy(),
                &worktree.base_branch,
            ])
            .current_dir(&self.base_path)
            .output()
            .await
            .context("Failed to execute git worktree add")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("git worktree add failed: {}", stderr);
        }

        Ok(())
    }

    pub async fn remove_worktree(&self, id: Uuid) -> Result<()> {
        let worktree = {
            let worktrees = self.worktrees.read();
            worktrees.iter().find(|w| w.id == id).cloned()
        };

        let Some(mut worktree) = worktree else {
            anyhow::bail!("Worktree not found: {}", id);
        };

        worktree.status = WorktreeStatus::CleaningUp;
        self.update_worktree(&worktree);

        let output = TokioCommand::new("git")
            .args(["worktree", "remove", "--force", &worktree.path.to_string_lossy()])
            .current_dir(&self.base_path)
            .output()
            .await
            .context("Failed to execute git worktree remove")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::warn!("git worktree remove failed: {}", stderr);
        }

        if worktree.path.exists() {
            tokio::fs::remove_dir_all(&worktree.path).await
                .context("Failed to remove worktree directory")?;
        }

        {
            let mut worktrees = self.worktrees.write();
            worktrees.retain(|w| w.id != id);
        }

        Ok(())
    }

    pub fn get_worktree(&self, id: Uuid) -> Option<Worktree> {
        let worktrees = self.worktrees.read();
        worktrees.iter().find(|w| w.id == id).cloned()
    }

    pub fn list_worktrees(&self) -> Vec<Worktree> {
        let worktrees = self.worktrees.read();
        worktrees.clone()
    }

    pub fn update_worktree(&self, worktree: &Worktree) {
        let mut worktrees = self.worktrees.write();
        if let Some(idx) = worktrees.iter().position(|w| w.id == worktree.id) {
            worktrees[idx] = worktree.clone();
        }
    }

    pub async fn run_agent_in_worktree(
        &self,
        id: Uuid,
        command: Vec<String>,
        env_vars: Vec<(String, String)>,
    ) -> Result<tokio::process::Child> {
        let worktree = self.get_worktree(id)
            .ok_or_else(|| anyhow::anyhow!("Worktree not found: {}", id))?;

        let mut cmd = TokioCommand::new(&command[0]);
        cmd.args(&command[1..])
            .current_dir(&worktree.path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let child = cmd.spawn().context("Failed to spawn agent process")?;

        {
            let mut wt = worktree.clone();
            wt.status = WorktreeStatus::Running;
            self.update_worktree(&wt);
        }

        Ok(child)
    }

    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    pub fn base_branch(&self) -> &str {
        &self.base_branch
    }
}