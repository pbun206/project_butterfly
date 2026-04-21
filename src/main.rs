mod app;
mod audio_cmp;
mod audio_synth;
mod config;
mod egui_tools;
mod state;

use crate::{app::App, config::*, state::State};
use anyhow::{Context, Result};
use dirs::config_dir;
use std::path::PathBuf;
use winit::event_loop::EventLoop;

use clap::Parser;

/// Store arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    config_path: Option<PathBuf>,

    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config_path = cli.config_path.unwrap_or(
        config_dir()
            .with_context(|| "Config directory not found. Likely due to unsupported OS")?
            .join("project_butterfly/project_butterfly.toml"),
    );
    let config: Config = Config::from_path(&config_path)?;
    if cli.debug > 0 {
        dbg!(&config);
    }

    env_logger::init();
    // TODO make sure removing run_user_event wasn't a bad idea
    let event_loop = EventLoop::new()?;
    let app = App::new(config);
    event_loop.run_app(app)?;
    Ok(())
}
