use std::sync::Arc;
use eframe::egui::{self, Color32, RichText, Vec2};
use uuid::Uuid;

use crate::agent::runner::{default_agent_command, AgentProcess, AgentProcessStatus, AgentRunner};
use crate::diff::tracker::{DiffEvent, DiffStatus, DiffTracker, WorktreeDiff};
use crate::workspace::{AgentType, WorkspaceManager, Worktree, WorktreeStatus};

use super::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Diff,
    Workspaces,
    Agents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogStreamFilter {
    All,
    StdoutOnly,
    StderrOnly,
}

pub struct BrutusApp {
    pub workspace_manager: Arc<WorkspaceManager>,
    pub agent_runner: Arc<AgentRunner>,
    pub diff_tracker: Arc<DiffTracker>,

    pub active_tab: ActiveTab,
    pub show_terminal: bool,
    pub selected_worktree: Option<Uuid>,
    pub selected_agent: Option<Uuid>,
    pub selected_diff_file: Option<usize>,

    pub log_filter: LogStreamFilter,
    pub auto_scroll: bool,
    pub stdin_input: String,
    pub search_query: String,

    pub show_new_worktree_modal: bool,
    pub new_worktree_name: String,
    pub new_worktree_branch: String,
    pub new_worktree_agent: Option<AgentType>,

    pub show_spawn_agent_modal: bool,
    pub spawn_target_worktree: Option<Uuid>,
    pub spawn_agent_type: AgentType,
    pub spawn_custom_command: String,
    pub spawn_env_vars: String,

    pub show_delete_modal: bool,
    pub worktree_to_delete: Option<Uuid>,

    pub show_shortcuts_modal: bool,

    pub status_message: String,
    pub status_is_error: bool,
}

fn worktree_status_badge(status: WorktreeStatus) -> (&'static str, Color32) {
    match status {
        WorktreeStatus::Creating => ("[start]", Theme::WARN),
        WorktreeStatus::Ready => ("[ready]", Theme::SUCCESS),
        WorktreeStatus::Running => ("[run]", Theme::ACCENT),
        WorktreeStatus::Completed => ("[done]", Theme::INFO),
        WorktreeStatus::Failed => ("[fail]", Theme::ERROR),
        WorktreeStatus::CleaningUp => ("[clean]", Theme::TEXT_MUTED),
    }
}

fn agent_status_badge(status: &AgentProcessStatus) -> (&'static str, Color32) {
    match status {
        AgentProcessStatus::Starting => ("[start]", Theme::WARN),
        AgentProcessStatus::Running => ("[run]", Theme::SUCCESS),
        AgentProcessStatus::Exited(0) => ("[exit]", Theme::TEXT_MUTED),
        AgentProcessStatus::Exited(_) => ("[err]", Theme::ERROR),
        AgentProcessStatus::Failed(_) => ("[fail]", Theme::ERROR),
    }
}

fn ghost_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(text.to_owned())
            .size(12.0)
            .color(Theme::TEXT_MUTED),
    )
    .fill(Color32::TRANSPARENT)
    .stroke(Theme::hairline_subtle())
    .corner_radius(6.0)
}

fn accent_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(text.to_owned())
            .size(12.0)
            .color(Theme::BG)
            .strong(),
    )
    .fill(Theme::ACCENT)
    .stroke(egui::Stroke::NONE)
    .corner_radius(6.0)
}

/// Hiveory-style HUD corner-bracket CTA.
fn bracket_button(ui: &mut egui::Ui, text: &str) -> egui::Response {
    let btn = egui::Button::new(
        RichText::new(text.to_owned())
            .size(12.0)
            .color(Theme::TEXT)
            .strong(),
    )
    .fill(Theme::SURFACE)
    .stroke(Theme::hairline_bright())
    .corner_radius(0.0);

    let response = ui.add(btn);
    paint_bracket_corners(ui, response.rect, if response.hovered() {
        Theme::TEXT
    } else {
        Theme::BORDER_BRIGHT
    });
    response
}

fn paint_bracket_corners(ui: &egui::Ui, rect: egui::Rect, color: Color32) {
    let len = 5.0_f32;
    let stroke = egui::Stroke::new(1.5_f32, color);
    let p = ui.painter();
    let tl = rect.left_top();
    let tr = rect.right_top();
    let bl = rect.left_bottom();
    let br = rect.right_bottom();

    p.line_segment([tl, tl + egui::vec2(len, 0.0)], stroke);
    p.line_segment([tl, tl + egui::vec2(0.0, len)], stroke);
    p.line_segment([tr, tr + egui::vec2(-len, 0.0)], stroke);
    p.line_segment([tr, tr + egui::vec2(0.0, len)], stroke);
    p.line_segment([bl, bl + egui::vec2(len, 0.0)], stroke);
    p.line_segment([bl, bl + egui::vec2(0.0, -len)], stroke);
    p.line_segment([br, br + egui::vec2(-len, 0.0)], stroke);
    p.line_segment([br, br + egui::vec2(0.0, -len)], stroke);
}

fn section_label(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .monospace()
        .size(11.0)
        .color(Theme::TEXT_MUTED)
        .strong()
}

fn selected_row_frame(is_selected: bool) -> egui::Frame {
    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(8, 5))
        .corner_radius(8.0)
        .fill(if is_selected {
            Theme::SELECTED
        } else {
            Color32::TRANSPARENT
        })
        .stroke(if is_selected {
            egui::Stroke::new(1.0_f32, Theme::ACCENT_DIM)
        } else {
            egui::Stroke::NONE
        })
}

impl BrutusApp {
    pub fn new(
        workspace_manager: Arc<WorkspaceManager>,
        agent_runner: Arc<AgentRunner>,
        diff_tracker: Arc<DiffTracker>,
    ) -> Self {
        let initial_worktrees = workspace_manager.list_worktrees();
        let first_wt = initial_worktrees.first().map(|w| w.id);

        let initial_agent = first_wt.and_then(|wt_id| {
            agent_runner.list_processes_for_worktree(wt_id).first().map(|a| a.id)
        });

        for wt in &initial_worktrees {
            let tracker = diff_tracker.clone();
            let wt_id = wt.id;
            let wt_path = wt.path.clone();
            tokio::spawn(async move {
                let _ = tracker.start_watching(wt_id, wt_path).await;
            });
        }

        Self {
            workspace_manager,
            agent_runner,
            diff_tracker,
            active_tab: ActiveTab::Diff,
            show_terminal: true,
            selected_worktree: first_wt,
            selected_agent: initial_agent,
            selected_diff_file: Some(0),
            log_filter: LogStreamFilter::All,
            auto_scroll: true,
            stdin_input: String::new(),
            search_query: String::new(),
            show_new_worktree_modal: false,
            new_worktree_name: String::new(),
            new_worktree_branch: String::new(),
            new_worktree_agent: Some(AgentType::ClaudeCode),
            show_spawn_agent_modal: false,
            spawn_target_worktree: first_wt,
            spawn_agent_type: AgentType::ClaudeCode,
            spawn_custom_command: String::new(),
            spawn_env_vars: String::new(),
            show_delete_modal: false,
            worktree_to_delete: None,
            show_shortcuts_modal: false,
            status_message: "ready".to_string(),
            status_is_error: false,
        }
    }

    pub fn set_status(&mut self, msg: impl Into<String>, is_error: bool) {
        self.status_message = msg.into();
        self.status_is_error = is_error;
    }

    pub fn worktrees(&self) -> Vec<Worktree> {
        let mut list = self.workspace_manager.list_worktrees();
        if !self.search_query.is_empty() {
            let q = self.search_query.to_lowercase();
            list.retain(|w| w.name.to_lowercase().contains(&q) || w.branch.to_lowercase().contains(&q));
        }
        list
    }

    pub fn selected_worktree_obj(&self) -> Option<Worktree> {
        self.selected_worktree.and_then(|id| self.workspace_manager.get_worktree(id))
    }

    pub fn agents_for_selected_worktree(&self) -> Vec<AgentProcess> {
        if let Some(wt_id) = self.selected_worktree {
            self.agent_runner.list_processes_for_worktree(wt_id)
        } else {
            Vec::new()
        }
    }

    pub fn selected_agent_obj(&self) -> Option<AgentProcess> {
        self.selected_agent.and_then(|id| self.agent_runner.get_process(id))
    }

    pub fn selected_diff_obj(&self) -> Option<WorktreeDiff> {
        self.selected_worktree.and_then(|id| self.diff_tracker.get_diff(id))
    }

    fn open_terminal(&mut self, ctx: &egui::Context) {
        self.show_terminal = true;
        ctx.move_to_top(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("floating_terminal"),
        ));
    }

    fn check_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let input = ctx.input(|i| i.clone());

        if input.modifiers.command && input.key_pressed(egui::Key::N) {
            self.show_new_worktree_modal = true;
            self.new_worktree_name.clear();
            self.new_worktree_branch.clear();
        }

        if input.modifiers.command && input.key_pressed(egui::Key::A) {
            if self.selected_worktree.is_some() {
                self.spawn_target_worktree = self.selected_worktree;
                self.show_spawn_agent_modal = true;
            } else {
                self.set_status("select a worktree before spawning", true);
            }
        }

        if input.modifiers.command && input.key_pressed(egui::Key::R) {
            self.trigger_refresh_diff();
        }

        if input.modifiers.command && input.key_pressed(egui::Key::Num1) {
            self.open_terminal(ctx);
        } else if input.modifiers.command && input.key_pressed(egui::Key::Num2) {
            self.active_tab = ActiveTab::Diff;
        } else if input.modifiers.command && input.key_pressed(egui::Key::Num3) {
            self.active_tab = ActiveTab::Workspaces;
        } else if input.modifiers.command && input.key_pressed(egui::Key::Num4) {
            self.active_tab = ActiveTab::Agents;
        }

        if input.key_pressed(egui::Key::F1) {
            self.show_shortcuts_modal = true;
        }
    }

    fn trigger_refresh_diff(&mut self) {
        if let Some(wt_id) = self.selected_worktree {
            if let Some(wt) = self.workspace_manager.get_worktree(wt_id) {
                let _ = self.diff_tracker.sender().send(DiffEvent::RefreshRequested(wt_id));
                self.set_status(format!("refreshing diff for '{}'", wt.name), false);
            }
        } else {
            self.set_status("no worktree selected", true);
        }
    }
}

impl eframe::App for BrutusApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
        self.check_keyboard_shortcuts(ctx);

        egui::TopBottomPanel::top("top_header")
            .frame(
                egui::Frame::NONE
                    .fill(Theme::GLASS)
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .stroke(Theme::hairline()),
            )
            .show(ctx, |ui| {
                self.render_header(ui);
            });

        egui::TopBottomPanel::bottom("bottom_status_bar")
            .exact_height(22.0)
            .frame(
                egui::Frame::NONE
                    .fill(Theme::BG)
                    .inner_margin(egui::Margin::symmetric(12, 2))
                    .stroke(Theme::hairline_subtle()),
            )
            .show(ctx, |ui| {
                self.render_status_bar(ui);
            });

        egui::SidePanel::left("left_sidebar")
            .resizable(true)
            .default_width(280.0)
            .min_width(220.0)
            .max_width(420.0)
            .frame(
                egui::Frame::NONE
                    .fill(Theme::PANEL)
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .stroke(Theme::hairline_subtle()),
            )
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(Theme::BG)
                    .inner_margin(egui::Margin::symmetric(12, 10)),
            )
            .show(ctx, |ui| {
                self.render_central_content(ui);
            });

        self.render_terminal_window(ctx);
        self.render_modals(ctx);
    }
}

impl BrutusApp {
    fn render_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("BRUTUS")
                    .size(15.0)
                    .color(Theme::TEXT)
                    .strong(),
            );
            ui.label(
                RichText::new("agent")
                    .size(12.0)
                    .color(Theme::TEXT_DIM),
            );

            ui.add_space(14.0);

            if ui
                .selectable_label(
                    self.show_terminal,
                    RichText::new("terminal")
                        .size(13.0)
                        .color(if self.show_terminal {
                            Theme::ACCENT
                        } else {
                            Theme::TEXT_MUTED
                        }),
                )
                .on_hover_text("Toggle floating terminal (Ctrl/Cmd+1 to open)")
                .clicked()
            {
                if self.show_terminal {
                    self.show_terminal = false;
                } else {
                    self.open_terminal(ui.ctx());
                }
            }

            let tabs = [
                (ActiveTab::Diff, "diff"),
                (ActiveTab::Workspaces, "workspaces"),
                (ActiveTab::Agents, "agents"),
            ];

            for (tab, title) in tabs {
                let is_selected = self.active_tab == tab;
                let color = if is_selected {
                    Theme::ACCENT
                } else {
                    Theme::TEXT_MUTED
                };
                let label = RichText::new(title).size(13.0).color(color);
                let btn = egui::Button::new(label)
                    .fill(if is_selected {
                        Color32::from_rgba_premultiplied(0xff, 0xff, 0xff, 0x0a)
                    } else {
                        Color32::TRANSPARENT
                    })
                    .stroke(if is_selected {
                        egui::Stroke::new(1.0_f32, Theme::ACCENT_DIM)
                    } else {
                        egui::Stroke::NONE
                    })
                    .corner_radius(6.0);

                if ui.add(btn).clicked() {
                    self.active_tab = tab;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(ghost_button("help")).clicked() {
                    self.show_shortcuts_modal = true;
                }
                if ui.add(ghost_button("refresh")).clicked() {
                    self.trigger_refresh_diff();
                }
                if bracket_button(ui, "spawn").clicked() {
                    if self.selected_worktree.is_some() {
                        self.spawn_target_worktree = self.selected_worktree;
                        self.show_spawn_agent_modal = true;
                    } else {
                        self.set_status("select a worktree first", true);
                    }
                }
                if bracket_button(ui, "new").clicked() {
                    self.show_new_worktree_modal = true;
                    self.new_worktree_name.clear();
                    self.new_worktree_branch.clear();
                }

                let base_name = self
                    .workspace_manager
                    .base_path()
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "repo".to_string());
                ui.label(
                    RichText::new(format!(
                        "{}/{}",
                        base_name,
                        self.workspace_manager.base_branch()
                    ))
                    .size(12.0)
                    .color(Theme::TEXT_DIM),
                );
            });
        });
    }

    fn render_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let status_color = if self.status_is_error {
                Theme::ERROR
            } else {
                Theme::TEXT_MUTED
            };
            ui.label(
                RichText::new(&self.status_message)
                    .monospace()
                    .size(11.0)
                    .color(status_color),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let wt_count = self.workspace_manager.list_worktrees().len();
                let agent_count = self.agent_runner.list_processes().len();

                let active = if let Some(wt) = self.selected_worktree_obj() {
                    format!("active:{}[{}]", wt.name, wt.branch)
                } else {
                    "active:-".to_string()
                };

                ui.label(
                    RichText::new(format!(
                        "{}  wt:{}  agents:{}",
                        active, wt_count, agent_count
                    ))
                    .monospace()
                    .size(11.0)
                    .color(Theme::TEXT_DIM),
                );
            });
        });
    }
}

impl BrutusApp {
    fn render_sidebar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("/")
                    .monospace()
                    .size(12.0)
                    .color(Theme::TEXT_DIM),
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .hint_text("filter...")
                    .desired_width(ui.available_width() - 28.0)
                    .font(egui::TextStyle::Monospace),
            );
            if !self.search_query.is_empty() && ui.add(ghost_button("x")).clicked() {
                self.search_query.clear();
            }
        });

        ui.add_space(6.0);

        let worktrees = self.worktrees();
        ui.horizontal(|ui| {
            ui.label(section_label(format!("WORKTREES ({})", worktrees.len())));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if bracket_button(ui, "new").clicked() {
                    self.show_new_worktree_modal = true;
                    self.new_worktree_name.clear();
                    self.new_worktree_branch.clear();
                }
            });
        });

        ui.add_space(2.0);
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.cursor().top(),
            Theme::hairline_subtle(),
        );
        ui.add_space(4.0);

        let avail_height = ui.available_height() * 0.55;

        egui::ScrollArea::vertical()
            .id_salt("worktrees_scroll")
            .max_height(avail_height)
            .show(ui, |ui| {
                if worktrees.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("no worktrees — press new")
                            .monospace()
                            .size(11.0)
                            .color(Theme::TEXT_DIM),
                    );
                }

                for wt in worktrees {
                    let is_selected = self.selected_worktree == Some(wt.id);
                    let (badge, badge_color) = worktree_status_badge(wt.status);

                    selected_row_frame(is_selected).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(badge)
                                    .monospace()
                                    .size(10.0)
                                    .color(badge_color),
                            );

                            let label = ui.selectable_label(
                                is_selected,
                                RichText::new(&wt.name)
                                    .monospace()
                                    .size(12.0)
                                    .color(if is_selected {
                                        Theme::TEXT
                                    } else {
                                        Theme::TEXT_MUTED
                                    }),
                            );

                            if label.clicked() {
                                self.selected_worktree = Some(wt.id);
                                let agents = self.agent_runner.list_processes_for_worktree(wt.id);
                                self.selected_agent = agents.first().map(|a| a.id);
                                self.selected_diff_file = Some(0);
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui
                                    .add(ghost_button("del"))
                                    .on_hover_text("Delete worktree")
                                    .clicked()
                                {
                                    self.worktree_to_delete = Some(wt.id);
                                    self.show_delete_modal = true;
                                }
                                if ui
                                    .add(ghost_button("open"))
                                    .on_hover_text("Open folder")
                                    .clicked()
                                {
                                    if let Err(e) = self.workspace_manager.open_worktree_dir(wt.id) {
                                        self.set_status(format!("failed to open: {}", e), true);
                                    }
                                }
                            });
                        });

                        ui.horizontal(|ui| {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(&wt.branch)
                                    .monospace()
                                    .size(10.0)
                                    .color(Theme::TEXT_DIM),
                            );
                            if let Some(agent_t) = wt.agent_type {
                                ui.label(
                                    RichText::new(format!("[{}]", agent_t))
                                        .monospace()
                                        .size(10.0)
                                        .color(Theme::ACCENT_DIM),
                                );
                            }
                        });
                    });
                }
            });

        ui.add_space(8.0);

        let agents = self.agents_for_selected_worktree();
        ui.horizontal(|ui| {
            ui.label(section_label(format!("AGENTS ({})", agents.len())));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if bracket_button(ui, "spawn").clicked() {
                    if self.selected_worktree.is_some() {
                        self.spawn_target_worktree = self.selected_worktree;
                        self.show_spawn_agent_modal = true;
                    }
                }
            });
        });

        ui.add_space(2.0);
        ui.painter().hline(
            ui.max_rect().x_range(),
            ui.cursor().top(),
            Theme::hairline_subtle(),
        );
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .id_salt("agents_sidebar_scroll")
            .show(ui, |ui| {
                if agents.is_empty() {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("no agents — press spawn")
                            .monospace()
                            .size(11.0)
                            .color(Theme::TEXT_DIM),
                    );
                }

                for agent in agents {
                    let is_selected = self.selected_agent == Some(agent.id);
                    let (badge, badge_color) = agent_status_badge(&agent.status);

                    selected_row_frame(is_selected).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(badge)
                                    .monospace()
                                    .size(10.0)
                                    .color(badge_color),
                            );

                            let short_id = agent.id.to_string()[..8].to_string();
                            let title = format!("{} {}", agent.agent_type, short_id);

                            let label = ui.selectable_label(
                                is_selected,
                                RichText::new(title)
                                    .monospace()
                                    .size(12.0)
                                    .color(if is_selected {
                                        Theme::TEXT
                                    } else {
                                        Theme::TEXT_MUTED
                                    }),
                            );

                            if label.clicked() {
                                self.selected_agent = Some(agent.id);
                                self.open_terminal(ui.ctx());
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if matches!(
                                    agent.status,
                                    AgentProcessStatus::Running | AgentProcessStatus::Starting
                                ) {
                                    if ui.add(ghost_button("kill")).clicked() {
                                        let runner = self.agent_runner.clone();
                                        let agent_id = agent.id;
                                        tokio::spawn(async move {
                                            let _ = runner.kill_process(agent_id).await;
                                        });
                                        self.set_status(format!("killed {}", short_id), false);
                                    }
                                }
                            });
                        });

                        let status_str = match &agent.status {
                            AgentProcessStatus::Starting => "starting...".to_string(),
                            AgentProcessStatus::Running => "running".to_string(),
                            AgentProcessStatus::Exited(code) => format!("exit {}", code),
                            AgentProcessStatus::Failed(err) => format!("fail: {}", err),
                        };

                        ui.label(
                            RichText::new(status_str)
                                .monospace()
                                .size(10.0)
                                .color(Theme::TEXT_DIM),
                        );
                    });
                }
            });
    }
}

impl BrutusApp {
    fn render_central_content(&mut self, ui: &mut egui::Ui) {
        match self.active_tab {
            ActiveTab::Diff => self.render_diff_view(ui),
            ActiveTab::Workspaces => self.render_workspaces_overview(ui),
            ActiveTab::Agents => self.render_agents_overview(ui),
        }
    }

    fn render_terminal_window(&mut self, ctx: &egui::Context) {
        if !self.show_terminal {
            return;
        }

        let mut open = self.show_terminal;
        egui::Window::new("terminal")
            .id(egui::Id::new("floating_terminal"))
            .open(&mut open)
            .movable(true)
            .resizable(true)
            .collapsible(false)
            .default_pos(ctx.screen_rect().center() - egui::vec2(380.0, 210.0))
            .default_size(egui::vec2(760.0, 420.0))
            .min_size(egui::vec2(520.0, 260.0))
            .constrain_to(ctx.screen_rect())
            .show(ctx, |ui| {
                self.render_terminal_view(ui);
            });
        self.show_terminal = open;
    }

    fn render_terminal_view(&mut self, ui: &mut egui::Ui) {
        let agent = self.selected_agent_obj();

        ui.horizontal(|ui| {
            if let Some(ref a) = agent {
                let short_id = a.id.to_string()[..8].to_string();
                ui.label(
                    RichText::new(format!("{} #{}", a.agent_type, short_id))
                        .monospace()
                        .size(13.0)
                        .color(Theme::TEXT)
                        .strong(),
                );

                let (badge, badge_color) = agent_status_badge(&a.status);
                ui.label(
                    RichText::new(badge)
                        .monospace()
                        .size(11.0)
                        .color(badge_color),
                );
            } else {
                ui.label(
                    RichText::new("terminal")
                        .monospace()
                        .size(13.0)
                        .color(Theme::TEXT)
                        .strong(),
                );
                ui.label(
                    RichText::new("(no agent)")
                        .monospace()
                        .size(11.0)
                        .color(Theme::TEXT_DIM),
                );
            }
        });

        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(ref a) = agent {
                    if ui.add(ghost_button("copy")).clicked() {
                        let lines = a.output.get_lines();
                        let text = lines.into_iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
                        ui.ctx().copy_text(text);
                        self.set_status("copied to clipboard", false);
                    }
                    if ui.add(ghost_button("clear")).clicked() {
                        a.output.clear();
                        self.set_status("output cleared", false);
                    }
                }

                ui.checkbox(&mut self.auto_scroll, RichText::new("scroll").monospace().size(11.0));

                ui.selectable_value(
                    &mut self.log_filter,
                    LogStreamFilter::All,
                    RichText::new("all").monospace().size(11.0),
                );
                ui.selectable_value(
                    &mut self.log_filter,
                    LogStreamFilter::StdoutOnly,
                    RichText::new("out").monospace().size(11.0),
                );
                ui.selectable_value(
                    &mut self.log_filter,
                    LogStreamFilter::StderrOnly,
                    RichText::new("err").monospace().size(11.0),
                );
            });
        });

        ui.add_space(4.0);

        let log_frame = egui::Frame::NONE
            .fill(Theme::TERM_BG)
            .stroke(Theme::hairline())
            .corner_radius(8.0)
            .inner_margin(egui::Margin::same(8));
        let input_height = ui.spacing().interact_size.y.max(
            ui.text_style_height(&egui::TextStyle::Monospace)
                + 2.0 * ui.spacing().button_padding.y,
        );
        let log_height = (ui.available_height()
            - log_frame.total_margin().sum().y
            - input_height
            - 4.0
            - 2.0 * ui.spacing().item_spacing.y)
            .max(0.0);

        log_frame.show(ui, |ui| {
            let lines = if let Some(ref a) = agent {
                let all_lines = a.output.get_lines();
                match self.log_filter {
                    LogStreamFilter::All => all_lines,
                    LogStreamFilter::StdoutOnly => {
                        all_lines.into_iter().filter(|l| !l.is_stderr).collect()
                    }
                    LogStreamFilter::StderrOnly => {
                        all_lines.into_iter().filter(|l| l.is_stderr).collect()
                    }
                }
            } else {
                Vec::new()
            };

            ui.spacing_mut().item_spacing.y = 1.0;
            let row_height = ui
                .fonts(|fonts| fonts.row_height(&egui::FontId::monospace(12.0)))
                .max(ui.spacing().interact_size.y);

            egui::ScrollArea::both()
                .id_salt("terminal_log_scroll")
                .max_height(log_height)
                .min_scrolled_height(0.0)
                .auto_shrink([false, false])
                .stick_to_bottom(self.auto_scroll)
                .show_rows(ui, row_height, lines.len().max(1), |ui, visible_rows| {
                    if lines.is_empty() {
                        let msg = if agent.is_none() {
                            "select an agent or spawn one to view output"
                        } else {
                            "no output yet"
                        };
                        ui.label(
                            RichText::new(msg)
                                .monospace()
                                .size(12.0)
                                .color(Theme::TEXT_DIM),
                        );
                    } else {
                        for line in &lines[visible_rows] {
                            let (text_color, bg_color) = if line.is_stderr {
                                (Theme::TERM_STDERR, Theme::STDERR_BG)
                            } else {
                                (Theme::TERM_STDOUT, Color32::TRANSPARENT)
                            };

                            let time_str = line.timestamp.format("%H:%M:%S").to_string();

                            egui::Frame::NONE
                                .fill(bg_color)
                                .inner_margin(egui::Margin::symmetric(2, 0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(time_str)
                                                .monospace()
                                                .size(11.0)
                                                .color(Theme::TERM_TIMESTAMP),
                                        );
                                        ui.add(
                                            egui::Label::new(
                                                RichText::new(&line.text)
                                                    .monospace()
                                                    .size(12.0)
                                                    .color(text_color),
                                            )
                                            .extend(),
                                        );
                                    });
                                });
                        }
                    }
                });
        });

        ui.add_space(4.0);

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(">")
                    .monospace()
                    .strong()
                    .color(Theme::ACCENT),
            );

            let is_running = agent.as_ref().map_or(false, |a| {
                matches!(
                    a.status,
                    AgentProcessStatus::Running | AgentProcessStatus::Starting
                )
            });

            let text_edit = egui::TextEdit::singleline(&mut self.stdin_input)
                .hint_text(if is_running {
                    "stdin..."
                } else {
                    "agent not running"
                })
                .desired_width(ui.available_width() - 64.0)
                .font(egui::TextStyle::Monospace);

            let res = ui.add_enabled(is_running, text_edit);

            let enter_pressed = res.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            let send_clicked = ui
                .add_enabled(
                    is_running && !self.stdin_input.is_empty(),
                    ghost_button("send"),
                )
                .clicked();

            if (enter_pressed || send_clicked) && !self.stdin_input.is_empty() {
                if let Some(ref a) = agent {
                    let runner = self.agent_runner.clone();
                    let agent_id = a.id;
                    let input_text = std::mem::take(&mut self.stdin_input);

                    tokio::spawn(async move {
                        let _ = runner.send_input(agent_id, input_text).await;
                    });
                    res.request_focus();
                }
            }
        });
    }

    fn render_diff_view(&mut self, ui: &mut egui::Ui) {
        let diff = self.selected_diff_obj();

        ui.horizontal(|ui| {
            if let Some(wt) = self.selected_worktree_obj() {
                ui.label(
                    RichText::new(format!("diff {}", wt.name))
                        .monospace()
                        .size(13.0)
                        .color(Theme::TEXT)
                        .strong(),
                );
                ui.label(
                    RichText::new(format!("[{}]", wt.branch))
                        .monospace()
                        .size(11.0)
                        .color(Theme::TEXT_DIM),
                );
            } else {
                ui.label(
                    RichText::new("diff")
                        .monospace()
                        .size(13.0)
                        .color(Theme::TEXT)
                        .strong(),
                );
                ui.label(
                    RichText::new("(no worktree)")
                        .monospace()
                        .size(11.0)
                        .color(Theme::TEXT_DIM),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(ghost_button("refresh")).clicked() {
                    self.trigger_refresh_diff();
                }

                if let Some(ref d) = diff {
                    ui.label(
                        RichText::new(format!("+{} -{}", d.total_additions, d.total_deletions))
                            .monospace()
                            .size(11.0)
                            .color(Theme::SUCCESS),
                    );
                    ui.label(
                        RichText::new(format!("{} files", d.files.len()))
                            .monospace()
                            .size(11.0)
                            .color(Theme::TEXT_DIM),
                    );
                }
            });
        });

        ui.add_space(4.0);

        if let Some(ref diff) = diff {
            if diff.files.is_empty() {
                ui.add_space(24.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("no local changes")
                            .monospace()
                            .size(13.0)
                            .color(Theme::TEXT_MUTED),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("diffs appear when agents modify files")
                            .monospace()
                            .size(11.0)
                            .color(Theme::TEXT_DIM),
                    );
                });
                return;
            }

            ui.columns(2, |columns| {
                columns[0].set_width(240.0);
                columns[0].label(section_label("FILES"));
                columns[0].add_space(4.0);

                egui::ScrollArea::vertical()
                    .id_salt("diff_files_scroll")
                    .show(&mut columns[0], |ui| {
                        for (idx, file) in diff.files.iter().enumerate() {
                            let is_selected = self.selected_diff_file == Some(idx);

                            let (status_icon, status_color) = match file.status {
                                DiffStatus::Added => ("A", Theme::SUCCESS),
                                DiffStatus::Modified => ("M", Theme::WARN),
                                DiffStatus::Deleted => ("D", Theme::ERROR),
                                DiffStatus::Renamed => ("R", Theme::INFO),
                                DiffStatus::Copied => ("C", Theme::ACCENT),
                                DiffStatus::Untracked => ("?", Theme::TEXT_MUTED),
                            };

                            selected_row_frame(is_selected).show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(status_icon)
                                            .monospace()
                                            .size(11.0)
                                            .strong()
                                            .color(status_color),
                                    );

                                    let file_name = file.path.to_string_lossy();
                                    let btn = ui.selectable_label(
                                        is_selected,
                                        RichText::new(file_name.as_ref())
                                            .monospace()
                                            .size(11.0),
                                    );
                                    if btn.clicked() {
                                        self.selected_diff_file = Some(idx);
                                    }

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            if file.additions > 0 || file.deletions > 0 {
                                                ui.label(
                                                    RichText::new(format!(
                                                        "+{} -{}",
                                                        file.additions, file.deletions
                                                    ))
                                                    .monospace()
                                                    .size(10.0)
                                                    .color(Theme::TEXT_DIM),
                                                );
                                            }
                                        },
                                    );
                                });
                            });
                        }
                    });

                let sel_file = self.selected_diff_file.and_then(|idx| diff.files.get(idx));

                if let Some(file) = sel_file {
                    columns[1].horizontal(|ui| {
                        ui.label(
                            RichText::new(file.path.to_string_lossy())
                                .monospace()
                                .size(12.0)
                                .strong()
                                .color(Theme::TEXT),
                        );
                        ui.label(
                            RichText::new(format!("(+{} -{})", file.additions, file.deletions))
                                .monospace()
                                .size(11.0)
                                .color(Theme::SUCCESS),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(ghost_button("open")).clicked() {
                                let full_path = diff.worktree_path.join(&file.path);
                                let _ = open::that(full_path);
                            }
                        });
                    });

                    columns[1].add_space(4.0);

                    egui::Frame::NONE
                        .fill(Theme::TERM_BG)
                        .stroke(Theme::hairline())
                        .corner_radius(8.0)
                        .inner_margin(egui::Margin::same(8))
                        .show(&mut columns[1], |ui| {
                            egui::ScrollArea::both()
                                .id_salt("file_diff_content_scroll")
                                .show(ui, |ui| {
                                    if file.diff_output.is_empty() {
                                        ui.label(
                                            RichText::new("no text diff")
                                                .monospace()
                                                .size(11.0)
                                                .color(Theme::TEXT_DIM),
                                        );
                                    } else {
                                        ui.spacing_mut().item_spacing.y = 0.0;
                                        for line in file.diff_output.lines() {
                                            let (text_color, bg_color) = if line.starts_with('+')
                                                && !line.starts_with("+++")
                                            {
                                                (Theme::DIFF_ADD, Theme::DIFF_ADD_BG)
                                            } else if line.starts_with('-')
                                                && !line.starts_with("---")
                                            {
                                                (Theme::DIFF_DEL, Theme::DIFF_DEL_BG)
                                            } else if line.starts_with("@@") {
                                                (Theme::DIFF_HUNK, Theme::DIFF_HUNK_BG)
                                            } else if line.starts_with("diff")
                                                || line.starts_with("index")
                                            {
                                                (Theme::TEXT_MUTED, Color32::TRANSPARENT)
                                            } else {
                                                (Theme::TEXT, Color32::TRANSPARENT)
                                            };

                                            egui::Frame::NONE
                                                .fill(bg_color)
                                                .inner_margin(egui::Margin::symmetric(4, 0))
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        RichText::new(line)
                                                            .monospace()
                                                            .size(12.0)
                                                            .color(text_color),
                                                    );
                                                });
                                        }
                                    }
                                });
                        });
                } else {
                    columns[1].vertical_centered(|ui| {
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new("select a file")
                                .monospace()
                                .size(12.0)
                                .color(Theme::TEXT_DIM),
                        );
                    });
                }
            });
        } else {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("select a worktree to inspect diffs")
                        .monospace()
                        .size(12.0)
                        .color(Theme::TEXT_DIM),
                );
            });
        }
    }

    fn render_workspaces_overview(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("workspaces")
                    .size(14.0)
                    .color(Theme::TEXT)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if bracket_button(ui, "new").clicked() {
                    self.show_new_worktree_modal = true;
                    self.new_worktree_name.clear();
                    self.new_worktree_branch.clear();
                }
            });
        });

        ui.add_space(6.0);

        let worktrees = self.workspace_manager.list_worktrees();

        if worktrees.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("no worktrees")
                        .size(13.0)
                        .color(Theme::TEXT_MUTED),
                );
                ui.add_space(8.0);
                if bracket_button(ui, "create first worktree").clicked() {
                    self.show_new_worktree_modal = true;
                }
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for wt in worktrees {
                let agents = self.agent_runner.list_processes_for_worktree(wt.id);
                let (badge, badge_color) = worktree_status_badge(wt.status);

                egui::Frame::NONE
                    .fill(Theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .corner_radius(10.0)
                    .stroke(Theme::hairline_subtle())
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&wt.name)
                                    .size(13.0)
                                    .strong()
                                    .color(Theme::TEXT),
                            );
                            ui.label(
                                RichText::new(badge)
                                    .monospace()
                                    .size(11.0)
                                    .color(badge_color),
                            );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.add(ghost_button("del")).clicked() {
                                    self.worktree_to_delete = Some(wt.id);
                                    self.show_delete_modal = true;
                                }
                                if ui.add(ghost_button("open")).clicked() {
                                    let _ = self.workspace_manager.open_worktree_dir(wt.id);
                                }
                                if bracket_button(ui, "spawn").clicked() {
                                    self.selected_worktree = Some(wt.id);
                                    self.spawn_target_worktree = Some(wt.id);
                                    self.show_spawn_agent_modal = true;
                                }
                                if ui.add(ghost_button("diff")).clicked() {
                                    self.selected_worktree = Some(wt.id);
                                    self.active_tab = ActiveTab::Diff;
                                }
                            });
                        });

                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(format!(
                                "{} <- {}  |  {}  |  agents:{}",
                                wt.branch,
                                wt.base_branch,
                                wt.path.display(),
                                agents.len()
                            ))
                            .monospace()
                            .size(11.0)
                            .color(Theme::TEXT_DIM),
                        );
                    });

                ui.add_space(6.0);
            }
        });
    }

    fn render_agents_overview(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("agents")
                    .size(14.0)
                    .color(Theme::TEXT)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if bracket_button(ui, "spawn").clicked() {
                    self.show_spawn_agent_modal = true;
                }
            });
        });

        ui.add_space(6.0);

        let agents = self.agent_runner.list_processes();

        if agents.is_empty() {
            ui.add_space(24.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("no agents registered")
                        .size(13.0)
                        .color(Theme::TEXT_MUTED),
                );
            });
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for agent in agents {
                let wt_name = self
                    .workspace_manager
                    .get_worktree(agent.worktree_id)
                    .map(|w| w.name)
                    .unwrap_or_else(|| "unknown".to_string());
                let (badge, badge_color) = agent_status_badge(&agent.status);
                let short_id = agent.id.to_string()[..8].to_string();

                egui::Frame::NONE
                    .fill(Theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(12, 10))
                    .corner_radius(10.0)
                    .stroke(Theme::hairline_subtle())
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{} #{}", agent.agent_type, short_id))
                                    .size(13.0)
                                    .strong()
                                    .color(Theme::TEXT),
                            );
                            ui.label(
                                RichText::new(badge)
                                    .monospace()
                                    .size(11.0)
                                    .color(badge_color),
                            );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if matches!(
                                    agent.status,
                                    AgentProcessStatus::Running | AgentProcessStatus::Starting
                                ) {
                                    if ui.add(ghost_button("kill")).clicked() {
                                        let runner = self.agent_runner.clone();
                                        let aid = agent.id;
                                        tokio::spawn(async move {
                                            let _ = runner.kill_process(aid).await;
                                        });
                                        self.set_status("killed agent", false);
                                    }
                                }

                                if ui.add(ghost_button("term")).clicked() {
                                    self.selected_worktree = Some(agent.worktree_id);
                                    self.selected_agent = Some(agent.id);
                                    self.open_terminal(ui.ctx());
                                }
                            });
                        });

                        ui.add_space(2.0);
                        let cmd_str = agent.command.join(" ");
                        ui.label(
                            RichText::new(format!("wt:{}  |  {}", wt_name, cmd_str))
                                .monospace()
                                .size(11.0)
                                .color(Theme::TEXT_DIM),
                        );
                    });

                ui.add_space(6.0);
            }
        });
    }
}

impl BrutusApp {
    fn render_modals(&mut self, ctx: &egui::Context) {
        if self.show_new_worktree_modal {
            let mut open = self.show_new_worktree_modal;
            egui::Window::new("new worktree")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .fixed_size(Vec2::new(420.0, 260.0))
                .show(ctx, |ui| {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("create an isolated git worktree for parallel agents")
                            .monospace()
                            .size(11.0)
                            .color(Theme::TEXT_MUTED),
                    );
                    ui.add_space(8.0);

                    ui.label(section_label("name"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_worktree_name)
                            .hint_text("auth-refactor")
                            .font(egui::TextStyle::Monospace),
                    );

                    ui.add_space(6.0);
                    ui.label(section_label("branch (optional)"));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.new_worktree_branch)
                            .hint_text("defaults to brutus/<name>")
                            .font(egui::TextStyle::Monospace),
                    );

                    ui.add_space(6.0);
                    ui.label(section_label("auto-spawn agent"));
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.new_worktree_agent, None, "none");
                        ui.selectable_value(
                            &mut self.new_worktree_agent,
                            Some(AgentType::ClaudeCode),
                            "claude",
                        );
                        ui.selectable_value(
                            &mut self.new_worktree_agent,
                            Some(AgentType::Codex),
                            "codex",
                        );
                        ui.selectable_value(
                            &mut self.new_worktree_agent,
                            Some(AgentType::Opencode),
                            "opencode",
                        );
                    });

                    ui.add_space(14.0);

                    ui.horizontal(|ui| {
                        let can_create = !self.new_worktree_name.trim().is_empty();
                        if ui
                            .add_enabled(can_create, accent_button("create"))
                            .clicked()
                        {
                            let name = self.new_worktree_name.trim().to_string();
                            let branch = if self.new_worktree_branch.trim().is_empty() {
                                None
                            } else {
                                Some(self.new_worktree_branch.trim().to_string())
                            };
                            let auto_agent = self.new_worktree_agent;

                            let mgr = self.workspace_manager.clone();
                            let runner = self.agent_runner.clone();
                            let diff_tracker = self.diff_tracker.clone();
                            let create_name = name.clone();

                            tokio::spawn(async move {
                                match mgr.create_worktree(create_name, branch, auto_agent).await {
                                    Ok(wt) => {
                                        let _ =
                                            diff_tracker.start_watching(wt.id, wt.path.clone()).await;
                                        if let Some(agent_t) = auto_agent {
                                            let cmd = default_agent_command(agent_t);
                                            let _ = runner
                                                .spawn_agent(wt.id, agent_t, cmd, Vec::new())
                                                .await;
                                        }
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to create worktree: {}", e);
                                    }
                                }
                            });

                            self.set_status(format!("creating '{}'...", name), false);
                            self.show_new_worktree_modal = false;
                        }

                        if ui.add(ghost_button("cancel")).clicked() {
                            self.show_new_worktree_modal = false;
                        }
                    });
                });
            self.show_new_worktree_modal = open;
        }

        if self.show_spawn_agent_modal {
            let mut open = self.show_spawn_agent_modal;
            egui::Window::new("spawn agent")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .fixed_size(Vec2::new(450.0, 320.0))
                .show(ctx, |ui| {
                    ui.add_space(4.0);

                    ui.label(section_label("worktree"));
                    let worktrees = self.workspace_manager.list_worktrees();
                    egui::ComboBox::from_id_salt("spawn_wt_combo")
                        .selected_text(
                            self.spawn_target_worktree
                                .and_then(|id| self.workspace_manager.get_worktree(id))
                                .map(|w| format!("{} ({})", w.name, w.branch))
                                .unwrap_or_else(|| "select...".to_string()),
                        )
                        .show_ui(ui, |ui| {
                            for wt in &worktrees {
                                ui.selectable_value(
                                    &mut self.spawn_target_worktree,
                                    Some(wt.id),
                                    format!("{} ({})", wt.name, wt.branch),
                                );
                            }
                        });

                    ui.add_space(8.0);

                    ui.label(section_label("type"));
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut self.spawn_agent_type,
                            AgentType::ClaudeCode,
                            "claude",
                        );
                        ui.selectable_value(&mut self.spawn_agent_type, AgentType::Codex, "codex");
                        ui.selectable_value(
                            &mut self.spawn_agent_type,
                            AgentType::Opencode,
                            "opencode",
                        );
                        ui.selectable_value(
                            &mut self.spawn_agent_type,
                            AgentType::Custom,
                            "custom",
                        );
                    });

                    if self.spawn_agent_type == AgentType::Custom {
                        ui.add_space(6.0);
                        ui.label(section_label("command"));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.spawn_custom_command)
                                .hint_text("./scripts/my-agent")
                                .font(egui::TextStyle::Monospace),
                        );
                    }

                    ui.add_space(6.0);
                    ui.label(section_label("env (KEY=VALUE)"));
                    ui.add(
                        egui::TextEdit::multiline(&mut self.spawn_env_vars)
                            .hint_text("OPENAI_API_KEY=...")
                            .desired_rows(3)
                            .font(egui::TextStyle::Monospace),
                    );

                    ui.add_space(14.0);

                    ui.horizontal(|ui| {
                        let can_spawn = self.spawn_target_worktree.is_some()
                            && (self.spawn_agent_type != AgentType::Custom
                                || !self.spawn_custom_command.trim().is_empty());

                        let mut spawn_clicked = false;
                        ui.add_enabled_ui(can_spawn, |ui| {
                            spawn_clicked = bracket_button(ui, "spawn").clicked();
                        });

                        if spawn_clicked {
                            if let Some(wt_id) = self.spawn_target_worktree {
                                let agent_t = self.spawn_agent_type;
                                let command = if agent_t == AgentType::Custom {
                                    self.spawn_custom_command
                                        .split_whitespace()
                                        .map(|s| s.to_string())
                                        .collect()
                                } else {
                                    default_agent_command(agent_t)
                                };

                                let mut env_vars = Vec::new();
                                for line in self.spawn_env_vars.lines() {
                                    if let Some((k, v)) = line.split_once('=') {
                                        env_vars.push((k.trim().to_string(), v.trim().to_string()));
                                    }
                                }

                                let runner = self.agent_runner.clone();
                                tokio::spawn(async move {
                                    let _ =
                                        runner.spawn_agent(wt_id, agent_t, command, env_vars).await;
                                });

                                self.set_status(format!("spawned {}", agent_t), false);
                                self.selected_worktree = Some(wt_id);
                                self.open_terminal(ui.ctx());
                                self.show_spawn_agent_modal = false;
                            }
                        }

                        if ui.add(ghost_button("cancel")).clicked() {
                            self.show_spawn_agent_modal = false;
                        }
                    });
                });
            self.show_spawn_agent_modal = open;
        }

        if self.show_delete_modal {
            let mut open = self.show_delete_modal;
            egui::Window::new("confirm delete")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .fixed_size(Vec2::new(380.0, 160.0))
                .show(ctx, |ui| {
                    if let Some(wt_id) = self.worktree_to_delete {
                        let name = self
                            .workspace_manager
                            .get_worktree(wt_id)
                            .map(|w| w.name)
                            .unwrap_or_else(|| "selected".to_string());
                        ui.label(
                            RichText::new(format!("delete worktree '{}'?", name))
                                .monospace()
                                .size(12.0)
                                .strong()
                                .color(Theme::TEXT),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("stops agents and removes the worktree directory")
                                .monospace()
                                .size(11.0)
                                .color(Theme::TEXT_MUTED),
                        );

                        ui.add_space(14.0);

                        ui.horizontal(|ui| {
                            let del_btn = egui::Button::new(
                                RichText::new("delete")
                                    .monospace()
                                    .size(12.0)
                                    .color(Theme::TEXT)
                                    .strong(),
                            )
                            .fill(Theme::ERROR)
                            .stroke(egui::Stroke::NONE);

                            if ui.add(del_btn).clicked() {
                                let mgr = self.workspace_manager.clone();
                                let runner = self.agent_runner.clone();
                                let diff_tracker = self.diff_tracker.clone();

                                tokio::spawn(async move {
                                    let agents = runner.list_processes_for_worktree(wt_id);
                                    for a in agents {
                                        let _ = runner.kill_process(a.id).await;
                                    }
                                    let _ = mgr.remove_worktree(wt_id).await;
                                    let _ = diff_tracker.stop_watching(wt_id).await;
                                });

                                if self.selected_worktree == Some(wt_id) {
                                    self.selected_worktree = None;
                                    self.selected_agent = None;
                                }

                                self.set_status(format!("deleted '{}'", name), false);
                                self.show_delete_modal = false;
                            }

                            if ui.add(ghost_button("cancel")).clicked() {
                                self.show_delete_modal = false;
                            }
                        });
                    }
                });
            self.show_delete_modal = open;
        }

        if self.show_shortcuts_modal {
            let mut open = self.show_shortcuts_modal;
            egui::Window::new("shortcuts")
                .open(&mut open)
                .resizable(false)
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .fixed_size(Vec2::new(440.0, 280.0))
                .show(ctx, |ui| {
                    ui.label(section_label("KEYS"));
                    ui.add_space(4.0);

                    let shortcuts = [
                        ("ctrl/cmd+n", "new worktree"),
                        ("ctrl/cmd+a", "spawn agent"),
                        ("ctrl/cmd+r", "refresh diff"),
                        ("ctrl/cmd+1", "open floating terminal"),
                        ("ctrl/cmd+2", "diff"),
                        ("ctrl/cmd+3", "workspaces"),
                        ("ctrl/cmd+4", "agents"),
                        ("f1", "this help"),
                    ];

                    for (key, desc) in shortcuts {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(key)
                                    .monospace()
                                    .size(12.0)
                                    .color(Theme::ACCENT)
                                    .strong(),
                            );
                            ui.label(
                                RichText::new(desc)
                                    .monospace()
                                    .size(12.0)
                                    .color(Theme::TEXT_MUTED),
                            );
                        });
                    }

                    ui.add_space(14.0);
                    if ui.add(ghost_button("close")).clicked() {
                        self.show_shortcuts_modal = false;
                    }
                });
            self.show_shortcuts_modal = open;
        }
    }
}

pub fn run_gui(app: BrutusApp) -> anyhow::Result<()> {
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            desired_maximum_frame_latency: Some(1),
            wgpu_setup: eframe::egui_wgpu::WgpuSetupCreateNew {
                power_preference: eframe::wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            }
            .into(),
            ..Default::default()
        },
        viewport: egui::ViewportBuilder::default()
            .with_title("brutus")
            .with_inner_size([1280.0, 820.0])
            .with_min_inner_size([900.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "brutus",
        native_options,
        Box::new(|cc| {
            if let Some(render_state) = &cc.wgpu_render_state {
                let adapter = render_state.adapter.get_info();
                tracing::info!(
                    "Terminal renderer: wgpu / {:?} on {} ({:?})",
                    adapter.backend,
                    adapter.name,
                    adapter.device_type,
                );
            }
            Theme::apply(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("GUI application error: {}", e))
}
