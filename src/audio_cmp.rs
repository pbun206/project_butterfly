use std::sync::Arc;

use anyhow::{Context, Result};
use cpal::{
    FromSample, HostId, OutputCallbackInfo, Sample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use livi::{Features, Instance, Plugin, World, event::LV2AtomSequence};
use rtrb::Consumer;

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
    midi_consumer: Consumer<LV2AtomSequence>,
    stream: Option<cpal::Stream>,
}

impl AudioComponent {
    /// Creates a new `AudioComponent` with the given plugin chain.
    pub fn try_new(config: &Config, midi_consumer: Consumer<LV2AtomSequence>) -> Result<Self> {
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
            midi_consumer,
            stream: None,
        })
    }

    /// Runs cpal and processes audio and MIDI events.
    pub fn run(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("Cannot get default output device")?;
        let device_config = device
            .default_output_config()
            .context("Cannot get default output config")?;
        let stream_config = device_config.config();
        let sample_rate = device_config.sample_rate() as f32;
        let channels = device_config.channels() as usize;

        self.stream = Some(device.build_output_stream(
            &stream_config,
            create_output_stream(sample_rate, channels),
            |err| eprintln!("an error occurred on stream: {err}"),
            None,
        )?);
        self.stream.as_mut().unwrap().play()?;

        // let input = {
        //     let mut s = LV2AtomSequence::new(&self.lv2_features, 1024);
        //     let play_note_data = [0x90, 0x40, 0x7f];
        //     s.push_midi_event::<3>(1, self.lv2_features.midi_urid(), &play_note_data)
        //         .unwrap();
        //     s
        // };

        // // This is where the audio data will be stored.
        // let mut outputs = [
        //     vec![0.0; self.lv2_features.max_block_length()], // For mda EPiano, this is the left channel.
        //     vec![0.0; self.lv2_features.max_block_length()], // For mda EPiano, this is the right channel.
        // ];

        // // Set up the port configuration and run the plugin!
        // // The results will be stored in `outputs`.
        // let ports = livi::EmptyPortConnections::new()
        //     .with_atom_sequence_inputs(std::iter::once(&input))
        //     .with_audio_outputs(outputs.iter_mut().map(|output| output.as_mut_slice()));
        // todo!();
        // unsafe { instance.run(features.max_block_length(), ports).unwrap() };
        Ok(())
    }
}

fn write_data<T>(output: &mut [T], channels: usize, next_sample: &mut dyn FnMut() -> f32)
where
    T: Sample + FromSample<f32>,
{
    for frame in output.chunks_mut(channels) {
        let value: T = T::from_sample(next_sample());
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}

/// Uses dependency injection to create an output stream callback.
fn create_output_stream(
    sample_rate: f32,
    channels: usize,
) -> impl FnMut(&mut [f32], &OutputCallbackInfo) {
    let mut sample_clock = 0f32;
    let input = {
        let snare_on = [0x99, 0x26, 0x7f];
        atom_sequence
            .push_midi_event::<3>(
                0, // frame offset in block
                self.lv2_features.midi_urid(),
                &snare_on,
            )
            .unwrap();
        let mut s = LV2AtomSequence::new(&self.lv2_features, 1024);
        let play_note_data = [0x90, 0x40, 0x7f];
        s.push_midi_event::<3>(1, self.lv2_features.midi_urid(), &play_note_data)
            .unwrap();
        s
    };

    move |data: &mut [f32], _: &OutputCallbackInfo| {
        write_data(data, channels, &mut || {
            sample_clock = (sample_clock + 1.0) % sample_rate;
            (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin()
        });
    }
}
