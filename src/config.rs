use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Configuration for the project butterfly.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    left_handed: bool,
    path_to_precussion_lv2: PathBuf,
    music_tracks: Vec<PathBuf>,
}

impl Config {
    pub fn from_path(path: &Path) -> Result<Self> {
        // Parse contents of the config file
        let contents = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents)?;
        // Check if lv2 is valid
        if config
            .path_to_precussion_lv2
            .extension()
            .and_then(|e| e.to_str())
            != Some("lv2")
        {
            return Err(anyhow::anyhow!(
                "Precussion LV2 must have .lv2 extension: {}",
                config.path_to_precussion_lv2.display()
            ));
        } else if !config.path_to_precussion_lv2.is_dir() {
            return Err(anyhow::anyhow!(
                "Precussion LV2 not found: {}",
                config.path_to_precussion_lv2.display()
            ));
            // Check if the lv2 is valid.
        } else if todo!() {
        }
        // Now, check if music tracks are valid
        for track in &config.music_tracks {
            if !track.is_file() {
                return Err(anyhow::anyhow!(
                    "Music track not foun   d: {}",
                    track.display()
                ));
            }
        }
        Ok(config)
    }
}
