use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

/// Configuration for the project butterfly.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub left_handed: bool,
    pub percussion_lv2_uri: String,
    // music_tracks_dir: PathBuf,
}

impl Config {
    /// From a path to a config file, load the configuration.
    pub fn from_path(path: &Path) -> Result<Self> {
        // Parse contents of the config file
        let contents = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents)?;

        // // Check if lv2 is valid
        // if config
        //     .path_to_percussion_lv2
        //     .extension()
        //     .and_then(|e| e.to_str())
        //     != Some("lv2")
        // {
        //     return Err(anyhow::anyhow!(
        //         "Percussion LV2 must have .lv2 extension: {}",
        //         config.path_to_percussion_lv2.display()
        //     ));
        // } else if !config.path_to_percussion_lv2.is_dir() {
        //     return Err(anyhow::anyhow!(
        //         "Percussion LV2 not found: {}",
        //         config.path_to_percussion_lv2.display()
        //     ));
        //     // Check if the lv2 is valid. TODO
        // } else if true {
        // }
        // Now, check if music directory are valid
        // if !config.music_tracks_dir.is_dir() {
        //     return Err(anyhow::anyhow!(
        //         "Music tracks directory not found: {}",
        //         config.music_tracks_dir.display()
        //     ));
        // }
        Ok(config)
    }
}
