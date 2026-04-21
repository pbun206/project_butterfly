use std::sync::Arc;

use anyhow::{Context, Result};
use livi::{Features, Instance, Plugin, World};

use crate::config::Config;

pub struct AudioComponent {
    lv2_features: Arc<Features>,
    guitar_plugin_instances: Vec<Instance>,
    drum_instance: Instance,
    lv2_world: World,
    // Should be pre-allocated
    buffer_1: Vec<f32>,
    // Should be pre-allocated too
    buffer_2: Vec<f32>,
}

impl AudioComponent {
    /// Creates a new `AudioComponent` with the given plugin chain.
    pub fn try_new(config: &Config) -> Result<Self> {
        let lv2_world = livi::World::new();
        const SAMPLE_RATE: f64 = 44100.0;
        let lv2_features = lv2_world.build_features(livi::FeaturesBuilder::default());
        let buffer_1 = vec![0.0; lv2_features.max_block_length()];
        let buffer_2 = vec![0.0; lv2_features.max_block_length()];
        let drum_plugin = lv2_world
            .plugin_by_uri(&config.percussion_lv2_uri)
            .context("Cannot load drum plugin")?;
        let drum_instance = unsafe {
            drum_plugin
                .instantiate(lv2_features.clone(), SAMPLE_RATE)
                .expect("Could not instantiate drum plugin.")
        };

        // let plugin_instances: Vec<Instance> = plugins
        //     .into_iter()
        //     .map(|plugin| unsafe {
        //         plugin
        //             .instantiate(features.clone(), SAMPLE_RATE)
        //             .expect("Could not instantiate plugin.")
        //     })
        //     .collect();
        // Ok(Self {
        //     plugin_instances,
        //     lv2_features,
        //     lv2_world,
        // })
        Ok(Self {
            lv2_features,
            guitar_plugin_instances: vec![],
            drum_instance,
            lv2_world,
            buffer_1,
            buffer_2,
        })
    }

    /// TODO
    pub fn run(&mut self) {
        // Where midi events will be read from.
        let input = {
            let mut s = livi::event::LV2AtomSequence::new(&self.lv2_features, 1024);
            let play_note_data = [0x90, 0x40, 0x7f];
            s.push_midi_event::<3>(1, self.lv2_features.midi_urid(), &play_note_data)
                .unwrap();
            s
        };

        // This is where the audio data will be stored.
        let mut outputs = [
            vec![0.0; self.lv2_features.max_block_length()], // For mda EPiano, this is the left channel.
            vec![0.0; self.lv2_features.max_block_length()], // For mda EPiano, this is the right channel.
        ];

        // Set up the port configuration and run the plugin!
        // The results will be stored in `outputs`.
        let ports = livi::EmptyPortConnections::new()
            .with_atom_sequence_inputs(std::iter::once(&input))
            .with_audio_outputs(outputs.iter_mut().map(|output| output.as_mut_slice()));
        todo!();
        // unsafe { instance.run(features.max_block_length(), ports).unwrap() };
    }
}
