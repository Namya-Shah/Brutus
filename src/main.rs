use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{fmt, EnvFilter};

use brutus::agent::runner::AgentRunner;
use brutus::diff::tracker::DiffTracker;
use brutus::gui::app::{run_gui, BrutusApp};
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

fn main() -> Result<()> {
    let args = Args::parse();

    init_logging(args.debug);

    let base_path = std::fs::canonicalize(&args.path)
        .map_err(|e| anyhow::anyhow!("Failed to canonicalize path {}: {}", args.path.display(), e))?;

    tracing::info!("Starting Brutus GUI with base path: {}", base_path.display());
    tracing::info!("Base branch: {}", args.branch);

    // Build Tokio runtime for background async services (agent process watchers, diff filesystem events)
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let _guard = rt.enter();

    let workspace_manager = Arc::new(WorkspaceManager::new(&base_path, args.branch.clone())?);
    let agent_runner = Arc::new(AgentRunner::new(args.max_output_lines));
    let diff_tracker = Arc::new(DiffTracker::new());

    let diff_tracker_clone = diff_tracker.clone();
    tokio::spawn(async move {
        if let Err(e) = diff_tracker_clone.run_event_loop().await {
            tracing::error!("Diff tracker event loop error: {}", e);
        }
    });

    let app = BrutusApp::new(
        workspace_manager,
        agent_runner,
        diff_tracker,
    );

    // Launch cross-platform native GUI on the main thread
    run_gui(app)?;

    tracing::info!("Brutus shutdown complete");
    Ok(())
}

fn init_logging(debug: bool) {
    let filter = if debug {
        EnvFilter::new("debug")
    } else {
        EnvFilter::new("info")
    };

    let _ = fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .try_init();
}
