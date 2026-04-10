mod app;
mod audio_synth;
mod config;
mod egui_tools;

use crate::config::*;
use std::path::{Path, PathBuf};

use clap::Parser;
use winit::event_loop::{ControlFlow, EventLoop};

/// Store arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    config_path: Option<PathBuf>,

    #[arg(short, long, action = clap::ArgAction::Count)]
    debug: u8,
}

fn main() {
    let cli = Cli::parse();
    let config: Config = Config::from_path(&cli.config_path);
}
