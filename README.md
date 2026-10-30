# Brutus

A fast, parallel agent IDE and workspace manager written in Rust. Run multiple CLI coding agents (Claude Code, Codex, OpenCode, custom scripts) in parallel, isolated across separate Git worktrees, with real-time terminal streaming and diff inspection.

## Features

- **Parallel Agent Execution**: Run multiple coding agents simultaneously in isolated Git worktrees
- **Real-time Terminal Streaming**: Live stdout/stderr streaming with ring buffers
- **Live Diff Tracking**: Continuous `git diff` monitoring with file system watchers
- **Interactive TUI**: Multi-pane terminal UI built with ratatui
- **Workspace Management**: Automatic Git worktree lifecycle (create, monitor, cleanup)

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
git clone <repository>
cd brutus
cargo build --release
```

## Usage

```bash
# Run in current directory (must be a git repository)
brutus

# Specify a different repository path
brutus -p /path/to/repo

# Use a different base branch
brutus -b develop

# Enable debug logging
brutus --debug
```

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Switch focus between panes |
| `1-4` | Switch tabs (Workspaces, Agents, Diffs, Logs) |
| `j` / `k` / `↑` / `↓` | Navigate lists |
| `Enter` | Select item |
| `n` | Create new worktree |
| `a` | Spawn agent in selected worktree |
| `k` | Kill selected agent |
| `r` | Refresh diff |
| `d` | Delete selected worktree |
| `:` | Enter command mode |
| `?` | Show help |
| `Ctrl+Q` | Quit |

## Command Mode

Press `:` to enter command mode. Available commands:

- `new <name>` - Create new worktree
- `spawn <claude|codex|opencode>` - Spawn agent in selected worktree
- `kill` - Kill selected agent
- `delete` - Delete selected worktree
- `refresh` - Refresh diff for selected worktree
- `help` - Show command help

## Architecture

```
src/
├── main.rs              # Entry point, CLI args, terminal setup
├── lib.rs               # Module exports
├── workspace/
│   └── mod.rs           # Worktree lifecycle manager
├── agent/
│   └── runner.rs        # Parallel agent process runner
├── diff/
│   └── tracker.rs       # Live diff tracker with file watching
└── tui/
    └── app.rs           # Multi-pane TUI application
```

## Requirements

- Rust 1.75+
- Git 2.30+ (for worktree support)
- Terminal with true color support

## Supported Agents

- **Claude Code** (`claude` command)
- **Codex** (`codex` command)
- **OpenCode** (`opencode` command)
- **Custom** (any CLI command)

## License

MIT OR Apache-2.0