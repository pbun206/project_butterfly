use anyhow::Result;
use broken_nest::{ChainBuilder, ChainConfig, PluginBuilder};
use indexmap::IndexMap;
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DrumInstrumentMode {
    On,
    Off,
    #[default]
    Auto,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PhaseConfig {
    pub tempo: Option<f32>,
    #[serde(default)]
    pub cymbal: DrumInstrumentMode,
    #[serde(default)]
    pub snare: DrumInstrumentMode,
    pub audio_file: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedPhase {
    pub name: String,
    pub tempo: f32,
    pub cymbal_on: bool,
    pub snare_on: bool,
    pub audio_file: Option<String>,
}

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
    pub cymbals: Option<TomlChainConfig>,
    pub drum_bus: Option<TomlChainConfig>,
    pub drums: TomlChainConfig,
    pub guitar: TomlChainConfig,
    #[serde(default)]
    pub phase: IndexMap<String, PhaseConfig>,
}

impl Config {
    pub fn from_path(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&contents)?;
        Ok(config)
    }

    pub fn resolve_phases(&self) -> Vec<ResolvedPhase> {
        let mut resolved = Vec::new();
        let mut prev_tempo = self.default_tempo as f32;
        let mut prev_cymbal = false;
        let mut prev_snare = false;

        for (name, phase) in &self.phase {
            let tempo = phase.tempo.unwrap_or(prev_tempo);
            let cymbal_on = match phase.cymbal {
                DrumInstrumentMode::On => true,
                DrumInstrumentMode::Off => false,
                DrumInstrumentMode::Auto => prev_cymbal,
            };
            let snare_on = match phase.snare {
                DrumInstrumentMode::On => true,
                DrumInstrumentMode::Off => false,
                DrumInstrumentMode::Auto => prev_snare,
            };

            prev_tempo = tempo;
            prev_cymbal = cymbal_on;
            prev_snare = snare_on;

            resolved.push(ResolvedPhase {
                name: name.clone(),
                tempo,
                cymbal_on,
                snare_on,
                audio_file: phase.audio_file.clone(),
            });
        }
        resolved
    }
}
