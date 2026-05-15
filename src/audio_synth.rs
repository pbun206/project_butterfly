use anyhow::Result;
use broken_nest::{Chain, MidiEvent, World};
use std::{
    sync::mpsc::RecvTimeoutError,
    time::{Duration, Instant},
};

use crate::config::Config;
use crate::events::{DrumMode, PbEvent, Subdivision};

pub struct AudioThreadConfig {
    tempo: f32,
    curr_subdivsion_of_bar: u32,
    drum_on_left: bool,
    cymbal_on_left: bool,
    drum_chain: Chain,
    guitar_chain: Chain,
    gui_receiver: std::sync::mpsc::Receiver<PbEvent>,
    subdivision_deadline: std::time::Instant,
    follow_deadline: Option<std::time::Instant>,
    follow_count: u8,
    drum_mode: DrumMode,
    last_attack_coords: Option<(f32, f32)>,
    last_guitar_note: Option<u8>,
    _world: World,
}

impl AudioThreadConfig {
    pub fn new(config: &Config, gui_receiver: std::sync::mpsc::Receiver<PbEvent>) -> Result<Self> {
        let world = World::with_load_all();

        let drum_config = config.drums.to_chain_config("drums");
        let drum_chain = Chain::start_with_world(&drum_config, &world)
            .map_err(|e| anyhow::anyhow!("Failed to start drum chain: {e}"))?;
        println!("Drum chain started successfully.");
        std::thread::sleep(Duration::from_millis(200));

        let guitar_config = config.guitar.to_chain_config("guitar");
        let guitar_chain = Chain::start_with_world(&guitar_config, &world)
            .map_err(|e| anyhow::anyhow!("Failed to start guitar chain: {e}"))?;
        println!("Guitar chain started successfully.");

        Ok(Self {
            tempo: config.default_tempo as f32,
            curr_subdivsion_of_bar: 0,
            drum_on_left: false,
            cymbal_on_left: false,
            drum_chain,
            guitar_chain,
            gui_receiver,
            subdivision_deadline: std::time::Instant::now(),
            follow_deadline: None,
            follow_count: 0,
            drum_mode: DrumMode::default(),
            last_attack_coords: None,
            last_guitar_note: None,
            _world: world,
        })
    }

    pub fn create_kick(&mut self) -> MidiEvent {
        self.create_kick_with_velocity(127)
    }

    pub fn create_kick_with_velocity(&mut self, velocity: u8) -> MidiEvent {
        self.drum_on_left = !self.drum_on_left;
        if self.drum_on_left {
            MidiEvent::NoteOn {
                channel: 9,
                note: 35,
                velocity: velocity,
            }
        } else {
            MidiEvent::NoteOn {
                channel: 9,
                note: 36,
                velocity: velocity,
            }
        }
    }

    pub fn create_cymbal(&mut self) -> MidiEvent {
        self.cymbal_on_left = !self.cymbal_on_left;
        let (left, right) = self.drum_mode.cymbal_mode.get_midi_note();
        if self.cymbal_on_left {
            MidiEvent::NoteOn {
                channel: 9,
                note: right,
                velocity: 127,
            }
        } else {
            MidiEvent::NoteOn {
                channel: 9,
                note: left,
                velocity: 127,
            }
        }
    }

    pub fn send_kick(&mut self) {
        let note = self.create_kick();
        if let Ok(sender) = self.drum_chain.midi_sender("drumgizmo") {
            let _ = sender.send(note);
        }
    }

    pub fn send_cymbal(&mut self) {
        let note = self.create_cymbal();
        if let Ok(sender) = self.drum_chain.midi_sender("drumgizmo") {
            let _ = sender.send(note);
        }
    }

    pub fn create_snare(&mut self) -> MidiEvent {
        MidiEvent::NoteOn {
            channel: 9,
            note: 38,
            velocity: 127,
        }
    }

    pub fn send_snare(&mut self) {
        let note = self.create_snare();
        if let Ok(sender) = self.drum_chain.midi_sender("drumgizmo") {
            let _ = sender.send(note);
        }
    }

    fn send_guitar_note_on(&mut self, col: u8, velocity: u8) {
        if let Some(prev) = self.last_guitar_note.take() {
            let off = MidiEvent::NoteOff { channel: 0, note: prev, velocity: 0 };
            if let Ok(sender) = self.guitar_chain.midi_sender("sfizz") {
                let _ = sender.send(off);
            }
        }
        let note = 38 + col;
        let on = MidiEvent::NoteOn { channel: 0, note, velocity };
        if let Ok(sender) = self.guitar_chain.midi_sender("sfizz") {
            let _ = sender.send(on);
        }
        self.last_guitar_note = Some(note);
    }

    fn send_guitar_note_off(&mut self) {
        if let Some(note) = self.last_guitar_note.take() {
            let off = MidiEvent::NoteOff { channel: 0, note, velocity: 0 };
            if let Ok(sender) = self.guitar_chain.midi_sender("sfizz") {
                let _ = sender.send(off);
            }
        }
    }

    fn send_guitar_pitch_bend(&mut self, semitones: f32) {
        const BEND_RANGE: f32 = 24.0;
        let value = (semitones / BEND_RANGE * 8192.0).clamp(-8192.0, 8191.0) as i16;
        let bend = MidiEvent::PitchBend { channel: 0, value };
        if let Ok(sender) = self.guitar_chain.midi_sender("sfizz") {
            let _ = sender.send(bend);
        }
    }

    pub fn run_loop(&mut self) {
        loop {
            let subdivision_timeout = self
                .subdivision_deadline
                .saturating_duration_since(Instant::now());
            let follow_timeout = self
                .follow_deadline
                .map(|fc| fc.saturating_duration_since(Instant::now()));
            let is_subdivision_timeout = follow_timeout.map_or(true, |ft| subdivision_timeout < ft);
            match self.gui_receiver.recv_timeout(if is_subdivision_timeout {
                subdivision_timeout
            } else {
                follow_timeout.expect("follow_deadline is None")
            }) {
                Ok(event) => match event {
                    PbEvent::TempoChange(new_bpm) => {
                        self.tempo = new_bpm;
                        self.curr_subdivsion_of_bar = 0;
                        self.subdivision_deadline = Instant::now()
                            + Duration::from_secs_f32(4.0 * 60.0 / self.tempo / 48.0);
                    }
                    PbEvent::DrumModeChange(drum_mode) => {
                        self.drum_mode = drum_mode;
                    }
                    PbEvent::Attack { col, row } => {
                        self.last_attack_coords = Some((col as f32, row as f32));
                        if self.drum_mode.follow_once != Subdivision::None {
                            self.send_kick();
                        }
                        self.send_guitar_note_on(col, 127);
                    }
                    PbEvent::AttackWithPressure { col, row, pressure } => {
                        self.last_attack_coords = Some((col as f32, row as f32));
                        if self.drum_mode.follow_once != Subdivision::None {
                            self.send_kick();
                        }
                        let velocity = (pressure * 127.0).clamp(1.0, 127.0) as u8;
                        self.send_guitar_note_on(col, velocity);
                    }
                    PbEvent::AttackHeld(semitones) | PbEvent::AttackHeldWithPressure(semitones) => {
                        self.send_guitar_pitch_bend(semitones);
                    }
                    PbEvent::AttackOff => {
                        self.send_guitar_pitch_bend(0.0);
                        self.send_guitar_note_off();
                    }
                    PbEvent::ToggleDrumUi => {
                        let _ = self.drum_chain.toggle_all_ui();
                    }
                    PbEvent::ToggleGuitarUi => {
                        let _ = self.guitar_chain.toggle_all_ui();
                    }
                    PbEvent::ToggleMute => {
                        self.drum_chain.toggle_mute();
                        self.guitar_chain.toggle_mute();
                    }
                    PbEvent::Shutdown => {
                        self.drum_chain.stop();
                        self.guitar_chain.stop();
                        broken_nest::ui_shutdown();
                        std::process::exit(0);
                    }
                },
                Err(RecvTimeoutError::Timeout) => {
                    if !is_subdivision_timeout {
                        if self
                            .drum_mode
                            .follow_count
                            .should_continue(self.follow_count)
                        {
                            self.follow_count -= 1;
                            self.send_kick();
                            self.follow_deadline = None;
                        }
                    } else {
                        if self
                            .drum_mode
                            .cymbal_subdivision
                            .is_on_curr_subdivision(self.curr_subdivsion_of_bar)
                        {
                            self.send_cymbal();
                        }
                        if self.follow_deadline.is_none()
                            && self
                                .drum_mode
                                .floor_subdivision
                                .is_on_curr_subdivision(self.curr_subdivsion_of_bar)
                        {
                            self.send_kick();
                        }
                        if self.drum_mode.snare_half_time && self.curr_subdivsion_of_bar == 24 {
                            self.send_snare();
                        }
                        self.subdivision_deadline +=
                            Duration::from_secs_f32(4.0 * 60.0 / self.tempo / 48.0);
                        self.curr_subdivsion_of_bar = (self.curr_subdivsion_of_bar + 1) % 48;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.drum_chain.stop();
                    self.guitar_chain.stop();
                    broken_nest::ui_shutdown();
                    return;
                }
            }
        }
    }
}
