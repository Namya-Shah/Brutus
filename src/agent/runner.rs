use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Child;
use tokio::sync::Mutex as TokioMutex;
use tracing::{error, info};

#[derive(Clone, Debug)]
pub struct OutputLine {
    pub text: String,
    pub is_stderr: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Clone, Debug)]
pub struct AgentOutput {
    pub stdout: Arc<parking_lot::Mutex<VecDeque<String>>>,
    pub stderr: Arc<parking_lot::Mutex<VecDeque<String>>>,
    pub all: Arc<parking_lot::Mutex<VecDeque<OutputLine>>>,
    max_lines: usize,
}

impl AgentOutput {
    pub fn new(max_lines: usize) -> Self {
        Self {
            stdout: Arc::new(parking_lot::Mutex::new(VecDeque::with_capacity(max_lines))),
            stderr: Arc::new(parking_lot::Mutex::new(VecDeque::with_capacity(max_lines))),
            all: Arc::new(parking_lot::Mutex::new(VecDeque::with_capacity(max_lines))),
            max_lines,
        }
    }

    pub fn push_stdout(&self, line: String) {
        let entry = OutputLine {
            text: line.clone(),
            is_stderr: false,
            timestamp: chrono::Utc::now(),
        };

        {
            let mut buf = self.stdout.lock();
            if buf.len() >= self.max_lines {
                buf.pop_front();
            }
            buf.push_back(line);
        }

        {
            let mut all = self.all.lock();
            if all.len() >= self.max_lines {
                all.pop_front();
            }
            all.push_back(entry);
        }
    }

    pub fn push_stderr(&self, line: String) {
        let entry = OutputLine {
            text: line.clone(),
            is_stderr: true,
            timestamp: chrono::Utc::now(),
        };

        {
            let mut buf = self.stderr.lock();
            if buf.len() >= self.max_lines {
                buf.pop_front();
            }
            buf.push_back(line);
        }

        {
            let mut all = self.all.lock();
            if all.len() >= self.max_lines {
                all.pop_front();
            }
            all.push_back(entry);
        }
    }

    pub fn get_lines(&self) -> Vec<OutputLine> {
        let all = self.all.lock();
        all.iter().cloned().collect()
    }

    pub fn get_stdout(&self) -> Vec<String> {
        let buf = self.stdout.lock();
        buf.iter().cloned().collect()
    }

    pub fn get_stderr(&self) -> Vec<String> {
        let buf = self.stderr.lock();
        buf.iter().cloned().collect()
    }

    pub fn drain_stdout(&self) -> Vec<String> {
        let mut buf = self.stdout.lock();
        buf.drain(..).collect()
    }

    pub fn drain_stderr(&self) -> Vec<String> {
        let mut buf = self.stderr.lock();
        buf.drain(..).collect()
    }

    pub fn clear(&self) {
        self.stdout.lock().clear();
        self.stderr.lock().clear();
        self.all.lock().clear();
    }
}

#[derive(Debug, Clone)]
pub struct AgentProcess {
    pub id: uuid::Uuid,
    pub worktree_id: uuid::Uuid,
    pub agent_type: crate::workspace::AgentType,
    pub child: Arc<TokioMutex<Option<Child>>>,
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

#[derive(Clone)]
pub struct AgentRunner {
    processes: Arc<parking_lot::RwLock<Vec<AgentProcess>>>,
    max_output_lines: usize,
}

impl AgentRunner {
    pub fn new(max_output_lines: usize) -> Self {
        Self {
            processes: Arc::new(parking_lot::RwLock::new(Vec::new())),
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

        let child = Arc::new(TokioMutex::new(Some(child)));

        let output_stdout = output.clone();
        let output_stderr = output.clone();

        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                output_stdout.push_stdout(line);
            }
        });

        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                output_stderr.push_stderr(line);
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

    async fn monitor_process(&self, process_id: uuid::Uuid, child: Arc<TokioMutex<Option<Child>>>) {
        // Small delay to ensure process status reflects running
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        self.update_process_status(process_id, AgentProcessStatus::Running);

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
        processes
            .iter()
            .filter(|p| p.worktree_id == worktree_id)
            .cloned()
            .collect()
    }

    pub async fn kill_process(&self, id: uuid::Uuid) -> Result<()> {
        let process = self
            .get_process(id)
            .ok_or_else(|| anyhow::anyhow!("Process not found: {}", id))?;

        let mut child_guard = process.child.lock().await;
        if let Some(mut child) = child_guard.take() {
            child.kill().await.context("Failed to kill process")?;
        }

        self.update_process_status(id, AgentProcessStatus::Exited(-1));
        Ok(())
    }

    pub async fn send_input(&self, id: uuid::Uuid, input: String) -> Result<()> {
        let process = self
            .get_process(id)
            .ok_or_else(|| anyhow::anyhow!("Process not found: {}", id))?;

        let mut child_guard = process.child.lock().await;
        if let Some(child) = child_guard.as_mut() {
            if let Some(stdin) = child.stdin.as_mut() {
                use tokio::io::AsyncWriteExt;
                let data = if input.ends_with('\n') {
                    input
                } else {
                    format!("{}\n", input)
                };
                stdin
                    .write_all(data.as_bytes())
                    .await
                    .context("Failed to write to stdin")?;
                stdin.flush().await.context("Failed to flush stdin")?;
            }
        }

        Ok(())
    }

    pub fn remove_process(&self, id: uuid::Uuid) {
        let mut processes = self.processes.write();
        processes.retain(|p| p.id != id);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_output_streaming_and_filtering() {
        let output = AgentOutput::new(10);
        output.push_stdout("Hello from agent".to_string());
        output.push_stderr("Warning occurred".to_string());
        output.push_stdout("Processing step completed".to_string());

        let lines = output.get_lines();
        assert_eq!(lines.len(), 3);
        assert!(!lines[0].is_stderr);
        assert_eq!(lines[0].text, "Hello from agent");
        assert!(lines[1].is_stderr);
        assert_eq!(lines[1].text, "Warning occurred");
        assert!(!lines[2].is_stderr);
        assert_eq!(lines[2].text, "Processing step completed");

        let stdout = output.get_stdout();
        assert_eq!(stdout.len(), 2);
        assert_eq!(stdout[0], "Hello from agent");
        assert_eq!(stdout[1], "Processing step completed");

        let stderr = output.get_stderr();
        assert_eq!(stderr.len(), 1);
        assert_eq!(stderr[0], "Warning occurred");

        output.clear();
        assert!(output.get_lines().is_empty());
    }

    #[test]
    fn test_agent_output_max_lines_capacity() {
        let output = AgentOutput::new(3);
        output.push_stdout("Line 1".to_string());
        output.push_stdout("Line 2".to_string());
        output.push_stdout("Line 3".to_string());
        output.push_stdout("Line 4".to_string());

        let lines = output.get_lines();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].text, "Line 2");
        assert_eq!(lines[1].text, "Line 3");
        assert_eq!(lines[2].text, "Line 4");
    }

    #[test]
    fn test_default_agent_command() {
        assert_eq!(default_agent_command(crate::workspace::AgentType::ClaudeCode), vec!["claude"]);
        assert_eq!(default_agent_command(crate::workspace::AgentType::Codex), vec!["codex"]);
        assert_eq!(default_agent_command(crate::workspace::AgentType::Opencode), vec!["opencode"]);
    }
}

