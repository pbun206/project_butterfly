mod app;
mod audio_synth;
mod config;
mod egui_tools;
mod events;
mod state;

use crate::{app::App, audio_synth::AudioThreadConfig, config::*, events::PbEvent};
use anyhow::{Context, Result};
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

    let (audio_thread_sender, audio_thread_receiver) = std::sync::mpsc::channel::<PbEvent>();
    let (startup_tx, startup_rx) = std::sync::mpsc::sync_channel::<Result<()>>(1);

    let config_for_audio = config.clone();
    std::thread::spawn(move || {
        match AudioThreadConfig::new(&config_for_audio, audio_thread_receiver) {
            Ok(mut audio) => {
                let _ = startup_tx.send(Ok(()));
                audio.run_loop();
            }
            Err(e) => {
                let _ = startup_tx.send(Err(e));
            }
        }
    });

    startup_rx
        .recv()
        .context("Audio thread died before reporting")?
        .context("Audio thread failed to start")?;

    let event_loop = EventLoop::new()?;
    let app = App::new(audio_thread_sender);
    event_loop.run_app(app)?;

    println!("Program exiting");
    Ok(())
}
