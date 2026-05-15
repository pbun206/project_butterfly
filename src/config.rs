use anyhow::Result;
use broken_nest::{ChainBuilder, ChainConfig, PluginBuilder};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct TomlPluginConfig {
    pub name: String,
    pub uri: String,
    pub midi_in: Option<bool>,
    #[serde(default)]
    pub generic_ui: bool,
    #[serde(default)]
    pub dual_mono: bool,
    #[serde(default)]
    pub stereo_mix: Option<Vec<[f32; 2]>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TomlChainConfig {
    pub jack_client_prefix: String,
    pub buffer_size: Option<u32>,
    #[serde(default)]
    pub plugins: Vec<TomlPluginConfig>,
}

impl TomlChainConfig {
    pub fn to_chain_config(&self, name: &str) -> ChainConfig {
        let mut builder = ChainBuilder::new(name)
            .jack_client_prefix(&self.jack_client_prefix)
            .auto_connect_output();

        if let Some(size) = self.buffer_size {
            builder = builder.buffer_size(size);
        }

        for plugin in &self.plugins {
            let mut pb = PluginBuilder::new(&plugin.uri, &plugin.name);

            if plugin.generic_ui {
                pb = pb.generic_ui();
            }
            if plugin.dual_mono {
                pb = pb.dual_mono();
            }
            if let Some(mix) = &plugin.stereo_mix {
                pb = pb.stereo_mix(mix.clone());
            }

            if let Some(midi) = plugin.midi_in {
                pb = pb.midi_in(midi);
            }

            builder = builder.plugin(pb.build());
        }
        builder.build()
    }
}

fn default_tempo() -> u32 { 60 }

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub left_handed: bool,
    #[serde(default = "default_tempo")]
    pub default_tempo: u32,
    #[serde(default)]
    pub on_the_floor: bool,
    #[serde(default)]
    pub cymbals: bool,
    pub drums: TomlChainConfig,
    pub guitar: TomlChainConfig,
}

impl Config {
    pub fn from_path(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents)?;
        Ok(config)
    }
}
