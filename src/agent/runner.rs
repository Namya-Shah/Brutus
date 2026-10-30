use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::{error, info};

#[derive(Clone, Debug)]
pub struct AgentOutput {
    pub stdout: Arc<Mutex<VecDeque<String>>>,
    pub stderr: Arc<Mutex<VecDeque<String>>>,
    max_lines: usize,
}

impl AgentOutput {
    pub fn new(max_lines: usize) -> Self {
        Self {
            stdout: Arc::new(Mutex::new(VecDeque::with_capacity(max_lines))),
            stderr: Arc::new(Mutex::new(VecDeque::with_capacity(max_lines))),
            max_lines,
        }
    }

    async fn push(&self, buffer: &Arc<Mutex<VecDeque<String>>>, line: String) {
        let mut buf = buffer.lock().await;
        if buf.len() >= self.max_lines {
            buf.pop_front();
        }
        buf.push_back(line);
    }

    pub async fn push_stdout(&self, line: String) {
        self.push(&self.stdout, line).await;
    }

    pub async fn push_stderr(&self, line: String) {
        self.push(&self.stderr, line).await;
    }

    pub async fn drain_stdout(&self) -> Vec<String> {
        let mut buf = self.stdout.lock().await;
        buf.drain(..).collect()
    }

    pub async fn drain_stderr(&self) -> Vec<String> {
        let mut buf = self.stderr.lock().await;
        buf.drain(..).collect()
    }

    pub async fn try_pop_stdout(&self) -> Option<String> {
        let mut buf = self.stdout.lock().await;
        buf.pop_front()
    }

    pub async fn try_pop_stderr(&self) -> Option<String> {
        let mut buf = self.stderr.lock().await;
        buf.pop_front()
    }
}

#[derive(Debug, Clone)]
pub struct AgentProcess {
    pub id: uuid::Uuid,
    pub worktree_id: uuid::Uuid,
    pub agent_type: crate::workspace::AgentType,
    pub child: Arc<Mutex<Option<Child>>>,
    pub output: AgentOutput,
    pub command: Vec<String>,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub status: AgentProcessStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentProcessStatus {
    Starting,
    Running,
    Exited(i32),
    Failed(String),
}

pub struct AgentRunner {
    processes: parking_lot::RwLock<Vec<AgentProcess>>,
    max_output_lines: usize,
}

impl AgentRunner {
    pub fn new(max_output_lines: usize) -> Self {
        Self {
            processes: parking_lot::RwLock::new(Vec::new()),
            max_output_lines,
        }
    }

    pub async fn spawn_agent(
        &self,
        worktree_id: uuid::Uuid,
        agent_type: crate::workspace::AgentType,
        command: Vec<String>,
        env_vars: Vec<(String, String)>,
    ) -> Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let output = AgentOutput::new(self.max_output_lines);

        let mut cmd = tokio::process::Command::new(&command[0]);
        cmd.args(&command[1..])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        for (key, value) in env_vars {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().context("Failed to spawn agent process")?;

        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        let child = Arc::new(Mutex::new(Some(child)));

        let output_stdout = output.clone();
        let output_stderr = output.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                output_stdout.push_stdout(line).await;
            }
        });

        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                output_stderr.push_stderr(line).await;
            }
        });

        let process = AgentProcess {
            id,
            worktree_id,
            agent_type,
            child: child.clone(),
            output,
            command: command.clone(),
            started_at: chrono::Utc::now(),
            status: AgentProcessStatus::Starting,
        };

        {
            let mut processes = self.processes.write();
            processes.push(process);
        }

        let runner = self.clone();
        let process_id = id;
        tokio::spawn(async move {
            runner.monitor_process(process_id, child).await;
        });

        info!("Spawned agent {} for worktree {}", id, worktree_id);
        Ok(id)
    }

    async fn monitor_process(&self, process_id: uuid::Uuid, child: Arc<Mutex<Option<Child>>>) {
        let exit_status = {
            let mut child_guard = child.lock().await;
            if let Some(mut child) = child_guard.take() {
                match child.wait().await {
                    Ok(status) => status.code().unwrap_or(-1),
                    Err(e) => {
                        error!("Process {} failed to wait: {}", process_id, e);
                        -1
                    }
                }
            } else {
                -1
            }
        };

        self.update_process_status(process_id, AgentProcessStatus::Exited(exit_status));
        info!("Agent process {} exited with code {}", process_id, exit_status);
    }

    pub fn update_process_status(&self, id: uuid::Uuid, status: AgentProcessStatus) {
        let mut processes = self.processes.write();
        if let Some(proc) = processes.iter_mut().find(|p| p.id == id) {
            proc.status = status;
        }
    }

    pub fn get_process(&self, id: uuid::Uuid) -> Option<AgentProcess> {
        let processes = self.processes.read();
        processes.iter().find(|p| p.id == id).cloned()
    }

    pub fn list_processes(&self) -> Vec<AgentProcess> {
        let processes = self.processes.read();
        processes.clone()
    }

    pub fn list_processes_for_worktree(&self, worktree_id: uuid::Uuid) -> Vec<AgentProcess> {
        let processes = self.processes.read();
        processes.iter()
            .filter(|p| p.worktree_id == worktree_id)
            .cloned()
            .collect()
    }

    pub async fn kill_process(&self, id: uuid::Uuid) -> Result<()> {
        let process = self.get_process(id)
            .ok_or_else(|| anyhow::anyhow!("Process not found: {}", id))?;

        let mut child_guard = process.child.lock().await;
        if let Some(mut child) = child_guard.take() {
            child.kill().await.context("Failed to kill process")?;
        }

        self.update_process_status(id, AgentProcessStatus::Exited(-1));
        Ok(())
    }

    pub async fn send_input(&self, id: uuid::Uuid, input: String) -> Result<()> {
        let process = self.get_process(id)
            .ok_or_else(|| anyhow::anyhow!("Process not found: {}", id))?;

        let mut child_guard = process.child.lock().await;
        if let Some(child) = child_guard.as_mut() {
            if let Some(stdin) = child.stdin.as_mut() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(input.as_bytes()).await
                    .context("Failed to write to stdin")?;
                stdin.flush().await
                    .context("Failed to flush stdin")?;
            }
        }

        Ok(())
    }

    pub fn remove_process(&self, id: uuid::Uuid) {
        let mut processes = self.processes.write();
        processes.retain(|p| p.id != id);
    }
}

impl Clone for AgentRunner {
    fn clone(&self) -> Self {
        Self {
            processes: parking_lot::RwLock::new(Vec::new()),
            max_output_lines: self.max_output_lines,
        }
    }
}

pub fn default_agent_command(agent_type: crate::workspace::AgentType) -> Vec<String> {
    match agent_type {
        crate::workspace::AgentType::ClaudeCode => vec!["claude".to_string()],
        crate::workspace::AgentType::Codex => vec!["codex".to_string()],
        crate::workspace::AgentType::Opencode => vec!["opencode".to_string()],
        crate::workspace::AgentType::Custom => vec!["echo".to_string(), "Custom agent not configured".to_string()],
    }
}