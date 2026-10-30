use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use std::io::Stdout;
use std::sync::Arc;
use uuid::Uuid;

use crate::agent::runner::{AgentProcess, AgentProcessStatus, AgentRunner};
use crate::diff::tracker::{DiffTracker, WorktreeDiff};
use crate::workspace::{Worktree, WorktreeStatus, WorkspaceManager, AgentType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPane {
    AgentList,
    TerminalOutput,
    DiffView,
    Controls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppTab {
    Workspaces,
    Agents,
    Diffs,
    Logs,
}

pub struct App {
    pub workspace_manager: Arc<WorkspaceManager>,
    pub agent_runner: Arc<AgentRunner>,
    pub diff_tracker: Arc<DiffTracker>,
    
    pub focus: FocusPane,
    pub active_tab: AppTab,
    
    pub worktree_list_state: ListState,
    pub agent_list_state: ListState,
    pub diff_list_state: ListState,
    
    pub selected_worktree: Option<Uuid>,
    pub selected_agent: Option<Uuid>,
    pub selected_diff_file: Option<usize>,
    
    pub terminal_scroll: usize,
    pub diff_scroll: usize,
    
    pub input_buffer: String,
    pub input_mode: bool,
    
    pub status_message: String,
    pub should_quit: bool,
}

impl App {
    pub fn new(
        workspace_manager: Arc<WorkspaceManager>,
        agent_runner: Arc<AgentRunner>,
        diff_tracker: Arc<DiffTracker>,
    ) -> Self {
        let mut worktree_list_state = ListState::default();
        worktree_list_state.select(Some(0));
        
        let mut agent_list_state = ListState::default();
        agent_list_state.select(Some(0));
        
        let mut diff_list_state = ListState::default();
        diff_list_state.select(Some(0));

        Self {
            workspace_manager,
            agent_runner,
            diff_tracker,
            focus: FocusPane::AgentList,
            active_tab: AppTab::Workspaces,
            worktree_list_state,
            agent_list_state,
            diff_list_state,
            selected_worktree: None,
            selected_agent: None,
            selected_diff_file: None,
            terminal_scroll: 0,
            diff_scroll: 0,
            input_buffer: String::new(),
            input_mode: false,
            status_message: "Welcome to Brutus! Press '?' for help".to_string(),
            should_quit: false,
        }
    }

    pub fn worktrees(&self) -> Vec<Worktree> {
        self.workspace_manager.list_worktrees()
    }

    pub fn agents(&self) -> Vec<AgentProcess> {
        self.agent_runner.list_processes()
    }

    pub fn agents_for_selected_worktree(&self) -> Vec<AgentProcess> {
        if let Some(wt_id) = self.selected_worktree {
            self.agent_runner.list_processes_for_worktree(wt_id)
        } else {
            Vec::new()
        }
    }

    pub fn selected_worktree(&self) -> Option<Worktree> {
        self.selected_worktree.and_then(|id| self.workspace_manager.get_worktree(id))
    }

    pub fn selected_agent(&self) -> Option<AgentProcess> {
        self.selected_agent.and_then(|id| self.agent_runner.get_process(id))
    }

    pub fn selected_diff(&self) -> Option<WorktreeDiff> {
        self.selected_worktree.and_then(|id| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(self.diff_tracker.get_diff(id))
        })
    }

    pub fn next_worktree(&mut self) {
        let worktrees = self.worktrees();
        if worktrees.is_empty() { return; }
        let i = self.worktree_list_state.selected().unwrap_or(0);
        let next = (i + 1) % worktrees.len();
        self.worktree_list_state.select(Some(next));
        self.selected_worktree = Some(worktrees[next].id);
        self.update_agent_selection();
    }

    pub fn prev_worktree(&mut self) {
        let worktrees = self.worktrees();
        if worktrees.is_empty() { return; }
        let i = self.worktree_list_state.selected().unwrap_or(0);
        let prev = if i == 0 { worktrees.len() - 1 } else { i - 1 };
        self.worktree_list_state.select(Some(prev));
        self.selected_worktree = Some(worktrees[prev].id);
        self.update_agent_selection();
    }

    fn update_agent_selection(&mut self) {
        let agents = self.agents_for_selected_worktree();
        if agents.is_empty() {
            self.agent_list_state.select(None);
            self.selected_agent = None;
        } else {
            self.agent_list_state.select(Some(0));
            self.selected_agent = Some(agents[0].id);
        }
    }

    pub fn next_agent(&mut self) {
        let agents = self.agents_for_selected_worktree();
        if agents.is_empty() { return; }
        let i = self.agent_list_state.selected().unwrap_or(0);
        let next = (i + 1) % agents.len();
        self.agent_list_state.select(Some(next));
        self.selected_agent = Some(agents[next].id);
    }

    pub fn prev_agent(&mut self) {
        let agents = self.agents_for_selected_worktree();
        if agents.is_empty() { return; }
        let i = self.agent_list_state.selected().unwrap_or(0);
        let prev = if i == 0 { agents.len() - 1 } else { i - 1 };
        self.agent_list_state.select(Some(prev));
        self.selected_agent = Some(agents[prev].id);
    }

    pub fn next_diff_file(&mut self) {
        if let Some(diff) = self.selected_diff() {
            if diff.files.is_empty() { return; }
            let i = self.diff_list_state.selected().unwrap_or(0);
            let next = (i + 1) % diff.files.len();
            self.diff_list_state.select(Some(next));
            self.selected_diff_file = Some(next);
        }
    }

    pub fn prev_diff_file(&mut self) {
        if let Some(diff) = self.selected_diff() {
            if diff.files.is_empty() { return; }
            let i = self.diff_list_state.selected().unwrap_or(0);
            let prev = if i == 0 { diff.files.len() - 1 } else { i - 1 };
            self.diff_list_state.select(Some(prev));
            self.selected_diff_file = Some(prev);
        }
    }

    pub fn focus_next(&mut self) {
        self.focus = match self.focus {
            FocusPane::AgentList => FocusPane::TerminalOutput,
            FocusPane::TerminalOutput => FocusPane::DiffView,
            FocusPane::DiffView => FocusPane::Controls,
            FocusPane::Controls => FocusPane::AgentList,
        };
    }

    pub fn focus_prev(&mut self) {
        self.focus = match self.focus {
            FocusPane::AgentList => FocusPane::Controls,
            FocusPane::TerminalOutput => FocusPane::AgentList,
            FocusPane::DiffView => FocusPane::TerminalOutput,
            FocusPane::Controls => FocusPane::DiffView,
        };
    }

    pub fn next_tab(&mut self) {
        self.active_tab = match self.active_tab {
            AppTab::Workspaces => AppTab::Agents,
            AppTab::Agents => AppTab::Diffs,
            AppTab::Diffs => AppTab::Logs,
            AppTab::Logs => AppTab::Workspaces,
        };
    }

    pub fn prev_tab(&mut self) {
        self.active_tab = match self.active_tab {
            AppTab::Workspaces => AppTab::Logs,
            AppTab::Agents => AppTab::Workspaces,
            AppTab::Diffs => AppTab::Agents,
            AppTab::Logs => AppTab::Diffs,
        };
    }

    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        if self.input_mode {
            return self.handle_input_key(key).await;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.should_quit = true;
                }
            }
            KeyCode::Char('?') => {
                self.status_message = "Keys: Tab/Shift-Tab=switch pane, 1-4=tabs, j/k=navigate, Enter=select, n=new worktree, a=spawn agent, k=kill, r=refresh diff, q=quit".to_string();
            }
            KeyCode::Tab => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus_prev();
                } else {
                    self.focus_next();
                }
            }
            KeyCode::Char('1') => self.active_tab = AppTab::Workspaces,
            KeyCode::Char('2') => self.active_tab = AppTab::Agents,
            KeyCode::Char('3') => self.active_tab = AppTab::Diffs,
            KeyCode::Char('4') => self.active_tab = AppTab::Logs,
            KeyCode::Char('n') => self.create_worktree_dialog().await?,
            KeyCode::Char('a') => self.spawn_agent_dialog().await?,
            KeyCode::Char('k') => self.kill_selected_agent().await?,
            KeyCode::Char('r') => self.refresh_diff().await?,
            KeyCode::Char('d') => self.delete_selected_worktree().await?,
            KeyCode::Down | KeyCode::Char('j') => self.handle_down(),
            KeyCode::Up => self.handle_up(),
            KeyCode::Enter => self.handle_enter().await?,
            KeyCode::PageDown => self.scroll_down(),
            KeyCode::PageUp => self.scroll_up(),
            KeyCode::Char(':') => self.enter_command_mode(),
            _ => {}
        }
        Ok(())
    }

    fn handle_down(&mut self) {
        match self.focus {
            FocusPane::AgentList => match self.active_tab {
                AppTab::Workspaces => self.next_worktree(),
                AppTab::Agents => self.next_agent(),
                AppTab::Diffs => self.next_diff_file(),
                _ => {}
            },
            FocusPane::TerminalOutput => self.terminal_scroll = self.terminal_scroll.saturating_add(1),
            FocusPane::DiffView => self.diff_scroll = self.diff_scroll.saturating_add(1),
            _ => {}
        }
    }

    fn handle_up(&mut self) {
        match self.focus {
            FocusPane::AgentList => match self.active_tab {
                AppTab::Workspaces => self.prev_worktree(),
                AppTab::Agents => self.prev_agent(),
                AppTab::Diffs => self.prev_diff_file(),
                _ => {}
            },
            FocusPane::TerminalOutput => self.terminal_scroll = self.terminal_scroll.saturating_sub(1),
            FocusPane::DiffView => self.diff_scroll = self.diff_scroll.saturating_sub(1),
            _ => {}
        }
    }

    async fn handle_enter(&mut self) -> Result<()> {
        match self.active_tab {
            AppTab::Workspaces => {
                if let Some(wt) = self.selected_worktree() {
                    self.status_message = format!("Selected worktree: {}", wt.name);
                }
            }
            AppTab::Agents => {
                if let Some(agent) = self.selected_agent() {
                    self.status_message = format!("Selected agent: {} ({:?})", agent.id, agent.status);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn scroll_down(&mut self) {
        match self.focus {
            FocusPane::TerminalOutput => self.terminal_scroll = self.terminal_scroll.saturating_add(10),
            FocusPane::DiffView => self.diff_scroll = self.diff_scroll.saturating_add(10),
            _ => {}
        }
    }

    fn scroll_up(&mut self) {
        match self.focus {
            FocusPane::TerminalOutput => self.terminal_scroll = self.terminal_scroll.saturating_sub(10),
            FocusPane::DiffView => self.diff_scroll = self.diff_scroll.saturating_sub(10),
            _ => {}
        }
    }

    fn enter_command_mode(&mut self) {
        self.input_mode = true;
        self.input_buffer.clear();
        self.status_message = "Command: ".to_string();
    }

    async fn handle_input_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.input_mode = false;
                self.input_buffer.clear();
                self.status_message = "Command cancelled".to_string();
            }
            KeyCode::Enter => {
                let cmd = self.input_buffer.trim().to_string();
                self.execute_command(cmd).await?;
                self.input_mode = false;
                self.input_buffer.clear();
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                self.input_buffer.push(c);
            }
            _ => {}
        }
        Ok(())
    }

    async fn execute_command(&mut self, cmd: String) -> Result<()> {
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        if parts.is_empty() { return Ok(()); }

        match parts[0] {
            "new" | "n" => {
                let name = parts.get(1).unwrap_or(&"new-worktree");
                self.create_worktree(name.to_string()).await?;
            }
            "spawn" | "s" => {
                if let Some(wt_id) = self.selected_worktree {
                    let agent_type = parts.get(1).and_then(|s| match *s {
                        "claude" => Some(AgentType::ClaudeCode),
                        "codex" => Some(AgentType::Codex),
                        "opencode" => Some(AgentType::Opencode),
                        _ => None,
                    }).unwrap_or(AgentType::ClaudeCode);
                    self.spawn_agent(wt_id, agent_type).await?;
                }
            }
            "kill" => self.kill_selected_agent().await?,
            "delete" | "del" => self.delete_selected_worktree().await?,
            "refresh" => self.refresh_diff().await?,
            "help" => self.status_message = "Commands: new <name>, spawn <claude|codex|opencode>, kill, delete, refresh, help".to_string(),
            _ => self.status_message = format!("Unknown command: {}", parts[0]),
        }
        Ok(())
    }

    async fn create_worktree_dialog(&mut self) -> Result<()> {
        self.input_mode = true;
        self.input_buffer.clear();
        self.status_message = "New worktree name: ".to_string();
        Ok(())
    }

    async fn create_worktree(&mut self, name: String) -> Result<()> {
        let worktree = self.workspace_manager.create_worktree(name, None, None).await?;
        self.selected_worktree = Some(worktree.id);
        self.worktree_list_state.select(Some(self.worktrees().len() - 1));
        self.status_message = format!("Created worktree: {}", worktree.name);
        Ok(())
    }

    async fn spawn_agent_dialog(&mut self) -> Result<()> {
        if self.selected_worktree.is_none() {
            self.status_message = "No worktree selected".to_string();
            return Ok(());
        }
        self.input_mode = true;
        self.input_buffer.clear();
        self.status_message = "Agent type (claude/codex/opencode): ".to_string();
        Ok(())
    }

    async fn spawn_agent(&mut self, worktree_id: Uuid, agent_type: AgentType) -> Result<()> {
        let command = crate::agent::runner::default_agent_command(agent_type);
        let id = self.agent_runner.spawn_agent(worktree_id, agent_type, command, Vec::new()).await?;
        self.status_message = format!("Spawned agent {} in worktree {}", id, worktree_id);
        self.update_agent_selection();
        Ok(())
    }

    async fn kill_selected_agent(&mut self) -> Result<()> {
        if let Some(agent_id) = self.selected_agent {
            self.agent_runner.kill_process(agent_id).await?;
            self.status_message = format!("Killed agent {}", agent_id);
            self.update_agent_selection();
        } else {
            self.status_message = "No agent selected".to_string();
        }
        Ok(())
    }

    async fn delete_selected_worktree(&mut self) -> Result<()> {
        if let Some(wt_id) = self.selected_worktree {
            let agents = self.agent_runner.list_processes_for_worktree(wt_id);
            for agent in agents {
                self.agent_runner.kill_process(agent.id).await?;
            }
            self.workspace_manager.remove_worktree(wt_id).await?;
            self.diff_tracker.stop_watching(wt_id).await?;
            self.selected_worktree = None;
            self.worktree_list_state.select(if self.worktrees().is_empty() { None } else { Some(0) });
            self.status_message = format!("Deleted worktree {}", wt_id);
        } else {
            self.status_message = "No worktree selected".to_string();
        }
        Ok(())
    }

    async fn refresh_diff(&mut self) -> Result<()> {
        if let Some(wt_id) = self.selected_worktree {
            if let Some(wt) = self.workspace_manager.get_worktree(wt_id) {
                self.diff_tracker.sender().send(crate::diff::tracker::DiffEvent::RefreshRequested(wt_id))?;
                self.status_message = format!("Refreshing diff for {}", wt.name);
            }
        } else {
            self.status_message = "No worktree selected".to_string();
        }
        Ok(())
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(area);

        self.render_tabs(frame, main_layout[0]);
        self.render_main_content(frame, main_layout[1]);
        self.render_status_bar(frame, main_layout[2]);
    }

    fn render_tabs(&self, frame: &mut Frame, area: Rect) {
        let tabs: Vec<Line> = vec!["Workspaces", "Agents", "Diffs", "Logs"]
            .into_iter()
            .map(|t| Line::from(t))
            .collect();
        let selected = match self.active_tab {
            AppTab::Workspaces => 0,
            AppTab::Agents => 1,
            AppTab::Diffs => 2,
            AppTab::Logs => 3,
        };

        let tabs_widget = Tabs::new(tabs)
            .select(selected)
            .block(Block::default().borders(Borders::ALL).title(" Brutus "))
            .style(Style::default().fg(Color::White))
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            .divider(symbols::line::VERTICAL);

        frame.render_widget(tabs_widget, area);
    }

    fn render_main_content(&mut self, frame: &mut Frame, area: Rect) {
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        self.render_left_pane(frame, horizontal[0]);
        self.render_right_pane(frame, horizontal[1]);
    }

    fn render_left_pane(&mut self, frame: &mut Frame, area: Rect) {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        self.render_worktree_list(frame, vertical[0]);
        self.render_agent_list(frame, vertical[1]);
    }

    fn render_worktree_list(&mut self, frame: &mut Frame, area: Rect) {
        let worktrees = self.worktrees();
        let items: Vec<ListItem> = worktrees.iter().map(|wt| {
            let status_style = match wt.status {
                WorktreeStatus::Creating => Style::default().fg(Color::Yellow),
                WorktreeStatus::Ready => Style::default().fg(Color::Green),
                WorktreeStatus::Running => Style::default().fg(Color::Blue),
                WorktreeStatus::Completed => Style::default().fg(Color::Cyan),
                WorktreeStatus::Failed => Style::default().fg(Color::Red),
                WorktreeStatus::CleaningUp => Style::default().fg(Color::Magenta),
            };
            let agent_str = wt.agent_type.map(|a| format!(" [{}]", a)).unwrap_or_default();
            let name = wt.name.clone();
            let status_fmt = format!(" ({:?})", wt.status);
            ListItem::new(Line::from(vec![
                Span::styled(name, Style::default().fg(Color::White)),
                Span::styled(agent_str, Style::default().fg(Color::DarkGray)),
                Span::styled(status_fmt, status_style),
            ]))
        }).collect();

        let title = if self.focus == FocusPane::AgentList && self.active_tab == AppTab::Workspaces {
            " Worktrees (focused) "
        } else {
            " Worktrees "
        };

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, area, &mut self.worktree_list_state);
    }

    fn render_agent_list(&mut self, frame: &mut Frame, area: Rect) {
        let agents = self.agents_for_selected_worktree();
        let items: Vec<ListItem> = agents.iter().map(|agent| {
            let status_style = match agent.status {
                AgentProcessStatus::Starting => Style::default().fg(Color::Yellow),
                AgentProcessStatus::Running => Style::default().fg(Color::Green),
                AgentProcessStatus::Exited(code) => {
                    if code == 0 { Style::default().fg(Color::Cyan) } else { Style::default().fg(Color::Red) }
                }
                AgentProcessStatus::Failed(_) => Style::default().fg(Color::Red),
            };
            let agent_type_str = format!("{:?} ", agent.agent_type);
            let id_str = agent.id.to_string()[..8].to_string();
            let status_fmt = format!(" ({:?})", agent.status);
            ListItem::new(Line::from(vec![
                Span::styled(agent_type_str, Style::default().fg(Color::DarkGray)),
                Span::styled(id_str, Style::default().fg(Color::White)),
                Span::styled(status_fmt, status_style),
            ]))
        }).collect();

        let title = if self.focus == FocusPane::AgentList && self.active_tab == AppTab::Agents {
            " Agents (focused) "
        } else {
            " Agents "
        };

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, area, &mut self.agent_list_state);
    }

    fn render_right_pane(&mut self, frame: &mut Frame, area: Rect) {
        let vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
            .split(area);

        self.render_terminal_output(frame, vertical[0]);
        self.render_diff_view(frame, vertical[1]);
    }

    fn render_terminal_output(&mut self, frame: &mut Frame, area: Rect) {
        let title = if self.focus == FocusPane::TerminalOutput {
            " Terminal Output (focused) "
        } else {
            " Terminal Output "
        };

        let mut lines = Vec::new();
        
        if let Some(agent) = self.selected_agent() {
            let output = agent.output.clone();
            let rt = tokio::runtime::Handle::current();
            let stdout_lines = rt.block_on(async { output.drain_stdout().await });
            let stderr_lines = rt.block_on(async { output.drain_stderr().await });
            
            for line in stdout_lines {
                lines.push(Line::from(Span::styled(line, Style::default().fg(Color::Green))));
            }
            for line in stderr_lines {
                lines.push(Line::from(Span::styled(line, Style::default().fg(Color::Red))));
            }
        } else {
            lines.push(Line::from("No agent selected. Select an agent to view output."));
        }

        let paragraph = Paragraph::new(Text::from(lines))
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false })
            .scroll((self.terminal_scroll as u16, 0));

        frame.render_widget(paragraph, area);
    }

    fn render_diff_view(&mut self, frame: &mut Frame, area: Rect) {
        let title = if self.focus == FocusPane::DiffView {
            " Live Diff (focused) "
        } else {
            " Live Diff "
        };

        let mut content = Vec::new();

        if let Some(diff) = self.selected_diff() {
            if diff.files.is_empty() {
                content.push(Line::from("No changes detected"));
            } else {
                let file_items: Vec<ListItem> = diff.files.iter().enumerate().map(|(i, file)| {
                    let style = if Some(i) == self.selected_diff_file {
                        Style::default().bg(Color::Blue).fg(Color::White).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };
                    let status_str = format!("{} ", file.status);
                    let path_str = file.path.to_string_lossy().to_string();
                    let stats_str = format!(" +{} -{}", file.additions, file.deletions);
                    ListItem::new(Line::from(vec![
                        Span::styled(status_str, Style::default().fg(Color::Yellow)),
                        Span::styled(path_str, style),
                        Span::styled(stats_str, Style::default().fg(Color::DarkGray)),
                    ]).style(style))
                }).collect();

                let list = List::new(file_items)
                    .block(Block::default().borders(Borders::ALL).title(title))
                    .highlight_style(Style::default().bg(Color::Blue).fg(Color::White))
                    .highlight_symbol("▶ ");

                frame.render_stateful_widget(list, area, &mut self.diff_list_state);
                return;
            }
        } else {
            content.push(Line::from("No worktree selected"));
        }

        let paragraph = Paragraph::new(Text::from(content))
            .block(Block::default().borders(Borders::ALL).title(title))
            .wrap(Wrap { trim: false })
            .scroll((self.diff_scroll as u16, 0));

        frame.render_widget(paragraph, area);
    }

    fn render_status_bar(&self, frame: &mut Frame, area: Rect) {
        let focus_str = format!("Focus: {:?}", self.focus);
        let tab_str = format!("Tab: {:?}", self.active_tab);
        let wt_count = self.worktrees().len();
        let agent_count = self.agents().len();

        let status = Paragraph::new(Line::from(vec![
            Span::styled(self.status_message.clone(), Style::default().fg(Color::White)),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled(focus_str, Style::default().fg(Color::Cyan)),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled(tab_str, Style::default().fg(Color::Cyan)),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("Worktrees: {}", wt_count), Style::default().fg(Color::Yellow)),
            Span::styled(" | ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("Agents: {}", agent_count), Style::default().fg(Color::Yellow)),
        ]))
        .block(Block::default().borders(Borders::ALL));

        frame.render_widget(status, area);
    }
}

pub async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    mut app: App,
) -> Result<()> {
    use crossterm::event::{Event, poll, read};
    
    use std::time::Duration;
    use tokio::time::interval;

    let mut tick_interval = interval(Duration::from_millis(100));

    loop {
        terminal.draw(|frame| app.render(frame))?;

        tokio::select! {
            _ = tick_interval.tick() => {
                if app.should_quit {
                    break;
                }
            }
            _ = async {
                if poll(Duration::from_millis(50)).unwrap_or(false) {
                    if let Ok(event) = read() {
                        if let Event::Key(key) = event {
                            let _ = app.handle_key(key).await;
                        }
                    }
                }
            } => {}
        }
    }

    Ok(())
}