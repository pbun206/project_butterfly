mod app;
mod audio_synth;
mod config;
mod egui_tools;
mod state;

use crate::{app::App, config::*};
use anyhow::{Context, Result};
use broken_nest::Chain;
use dirs::config_dir;
use std::path::PathBuf;
use winit::event_loop::EventLoop;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    config_path: Option<PathBuf>,

    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

fn main() -> Result<()> {
    // Parse config shit
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

    let chain = Chain::start(&config.chain)
        .map_err(|e| anyhow::anyhow!("Failed to start audio chain: {e}"))?;
    println!("Audio chain started successfully.");

    env_logger::init();
    let event_loop = EventLoop::new()?;
    let app = App::new(config);
    event_loop.run_app(app)?;

    println!("Program exiting");
    drop(chain);
    println!("Program exited successfully.");
    Ok(())
}
