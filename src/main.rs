use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::stdout;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::signal;
use tracing_subscriber::{EnvFilter, fmt};

use brutus::agent::runner::AgentRunner;
use brutus::diff::tracker::DiffTracker;
use brutus::tui::app::{App, run_app};
use brutus::workspace::WorkspaceManager;

#[derive(Parser, Debug)]
#[command(name = "brutus", version, about = "Fast, parallel agent IDE and workspace manager")]
struct Args {
    /// Base repository path
    #[arg(short, long, default_value = ".")]
    path: PathBuf,

    /// Base branch to create worktrees from
    #[arg(short, long, default_value = "main")]
    branch: String,

    /// Maximum output lines per agent
    #[arg(long, default_value = "10000")]
    max_output_lines: usize,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    init_logging(args.debug);

    let base_path = std::fs::canonicalize(&args.path)
        .map_err(|e| anyhow::anyhow!("Failed to canonicalize path {}: {}", args.path.display(), e))?;

    tracing::info!("Starting Brutus with base path: {}", base_path.display());
    tracing::info!("Base branch: {}", args.branch);

    let workspace_manager = Arc::new(WorkspaceManager::new(&base_path, args.branch.clone())?);
    let agent_runner = Arc::new(AgentRunner::new(args.max_output_lines));
    let diff_tracker = Arc::new(DiffTracker::new());

    let diff_tracker_clone = diff_tracker.clone();
    tokio::spawn(async move {
        if let Err(e) = diff_tracker_clone.run_event_loop().await {
            tracing::error!("Diff tracker event loop error: {}", e);
        }
    });

    enable_raw_mode()?;
    execute!(stdout(), EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(workspace_manager.clone(), agent_runner.clone(), diff_tracker.clone());

    let app_handle = tokio::spawn(async move {
        run_app(&mut terminal, app).await
    });

    tokio::select! {
        result = app_handle => {
            result??;
        }
        _ = signal::ctrl_c() => {
            tracing::info!("Received Ctrl-C, shutting down...");
        }
    }

    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    tracing::info!("Brutus shutdown complete");
    Ok(())
}

fn init_logging(debug: bool) {
    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();
}