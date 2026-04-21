use anyhow::Result;
use livi::{Features, Instance, Plugin, World};

pub struct AudioComponent {
    lv2_features: Features,
    plugin_instances: Vec<Instance>,
    lv2_world: World,
    // Should be pre-allocated
    buffer_1: Vec<f32>,
    // Should be pre-allocated too
    buffer_2: Vec<f32>,
}

impl AudioComponent {
    /// Creates a new `AudioComponent` with the given plugin chain.
    pub fn try_new(lv2_world: World, plugins: Vec<Plugin>) -> Result<Self> {
        // const SAMPLE_RATE: f64 = 44100.0;
        // let lv2_features = lv2_world.build_features(livi::FeaturesBuilder::default());
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
        todo!();
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
