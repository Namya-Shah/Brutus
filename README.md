# Brutus

A fast, parallel agent IDE and workspace manager written in Rust. Run multiple CLI coding agents (Claude Code, Codex, OpenCode, custom scripts) in parallel, isolated across separate Git worktrees, with real-time terminal streaming, interactive stdin, and live git diff inspection in a native cross-platform GUI.

## Features

- **Cross-Platform Native Desktop GUI**: Built with `egui` and `eframe` for macOS, Windows, and Linux as a single, lightweight binary with zero web/node runtime dependencies.
- **Parallel Agent Execution**: Run multiple coding agents simultaneously across isolated Git worktrees.
- **Floating Terminal**: Move and resize the terminal anywhere within Brutus while browsing other views, with live stdout/stderr color-coded streaming, auto-scrolling, stdout/stderr filtering, clipboard copy, and interactive stdin prompt.
- **GPU Terminal Rendering**: All agent output shares a wgpu renderer that prefers a high-performance GPU, with only visible log rows laid out each frame. This accelerates terminal drawing; AI model inference uses the backend configured by each agent.
- **Live Git Diff Tracking**: Continuous `git diff` monitoring with file system watchers, changed file status badges (`[M]`, `[A]`, `[D]`, `[?]`), and colored syntax/diff highlighting.
- **Git Worktree Lifecycle Management**: Automatic creation, auto-discovery of existing worktrees, direct folder opening in native file managers, and safe cleanup.
- **Modern Dark IDE Layout**: Multi-pane interface with responsive sidebar, tabs, quick action bars, and modal dialogs.

## Supported Platforms

- **macOS** (wgpu / Metal / Cocoa)
- **Windows** (wgpu / DirectX 12 / Win32)
- **Linux** (wgpu / Vulkan / Wayland / X11)

## Installation & Build

### From Source

```bash
git clone <repository>
cd brutus
cargo build --release
```

The compiled native executable will be located in `target/release/brutus` (or `brutus.exe` on Windows).

### Install to Cargo Bin

```bash
cargo install --path .
```

## Usage

```bash
# Launch in the current repository
brutus

# Specify a target repository path
brutus -p /path/to/repo

# Specify base branch for new worktrees (defaults to main)
brutus -b main

# Enable debug logging
brutus --debug
```

## GUI Navigation & Layout

- **Header / App Bar**: Terminal visibility toggle, workspace tabs, quick action buttons (`➕ New Worktree`, `🚀 Spawn Agent`, `🔄 Refresh Diff`, `❓ Shortcuts`), active repository and base branch badge.
- **Left Sidebar**:
  - Filter / Search bar for worktrees.
  - **Worktrees List**: Active worktrees with status indicators (`Ready`, `Running`, `Failed`, `Creating`), branch names, folder opening, and deletion.
  - **Agents List**: Spawned agents for the selected worktree with status pills and `⏹ Kill` controls.
- **📟 Floating Terminal**: Drag the title bar to move it anywhere within the app, including over the sidebar, and drag its edges or corners to resize it. The terminal stays open across workspace tabs and remembers its position and size while Brutus is running. Close it with `×` or the header toggle; reopen it with the toggle, `Ctrl/Cmd + 1`, or an agent's terminal action. Includes live output from the selected agent (stdout in bright white/green, stderr in soft red), stdout/stderr filter, auto-scroll toggle, output copy/clear, horizontal scrolling for long lines, and an interactive `> ` stdin bar.
- **Central Workspace**:
  - **🔀 Live Git Diff Tab**: Split view showing changed files (`+Additions`, `-Deletions`, Git status `A/M/D/R/?`) and full colorized diff with syntax highlighting.
  - **📁 Workspaces Tab**: Overview card grid of all git worktrees, branch details, base branches, and disk paths.
  - **🤖 Agents Monitor Tab**: Global table of all active and past agents across all worktrees.
- **Status Bar**: Real-time feedback, active worktree and branch, active agent, and worktree/agent counters.

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Ctrl + N` / `Cmd + N` | Create new Git worktree |
| `Ctrl + A` / `Cmd + A` | Spawn coding agent in active worktree |
| `Ctrl + R` / `Cmd + R` | Refresh live git diff |
| `Ctrl + 1` / `Cmd + 1` | Open floating terminal |
| `Ctrl + 2` / `Cmd + 2` | Switch to Live Git Diff tab |
| `Ctrl + 3` / `Cmd + 3` | Switch to Workspaces Overview tab |
| `Ctrl + 4` / `Cmd + 4` | Switch to Agents Monitor tab |
| `F1` | Open keyboard shortcuts help modal |

## Architecture

```
src/
├── main.rs              # Entry point, CLI arguments, Tokio background runtime, GUI launcher
├── lib.rs               # Library root and module exports
├── gui/
│   ├── mod.rs           # GUI module exports
│   └── app.rs           # Native eframe/egui desktop application, widgets, panels, and modals
├── workspace/
│   └── mod.rs           # Git worktree lifecycle manager and auto-discovery
├── agent/
│   └── runner.rs        # Parallel agent process runner, stream buffers, and stdin sender
└── diff/
    └── tracker.rs       # Live diff computation and file system change watcher
```

## Supported Agents

- **Claude Code** (`claude` command)
- **Codex** (`codex` command)
- **OpenCode** (`opencode` command)
- **Custom** (any CLI command or script)

## License

MIT OR Apache-2.0
