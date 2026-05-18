use anyhow::Result;
use broken_nest::{Chain, MidiEvent, World};
use std::{
    sync::atomic::{AtomicU32, Ordering},
    sync::mpsc::RecvTimeoutError,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::audio_player::AudioPlayer;
use crate::config::{Config, ResolvedPhase};
use crate::events::{DrumMode, FollowCount, GuitarMode, PbEvent, Subdivision};
use crate::meter::Meter;

pub struct AudioThreadConfig {
    meter: Meter,
    drum_on_left: bool,
    cymbal_on_left: bool,
    drum_chain: Chain,
    cymbal_chain: Option<Chain>,
    drum_bus: Option<Chain>,
    guitar_chain: Chain,
    gui_receiver: std::sync::mpsc::Receiver<PbEvent>,
    follow_deadline: Option<std::time::Instant>,
    follow_count: u8,
    drum_mode: DrumMode,
    guitar_mode: GuitarMode,
    guitar_held: bool,
    last_attack_params: Option<(u8, u8, u8)>,
    cursor_cell: (u8, u8),
    repeat_cell: Arc<AtomicU32>,
    last_attack_coords: Option<(f32, f32)>,
    last_normal_notes: Vec<u8>,
    last_harmonic_notes: Vec<u8>,
    cymbal_count_since_reset: u32,
    space_held: bool,
    last_keyswitch: u8,
    palm_mute: bool,
    last_attack_channel: u8,
    phases: Vec<ResolvedPhase>,
    current_phase_index: Option<usize>,
    pending_phase_change: Option<Option<usize>>,
    beat_cell: Arc<AtomicU32>,
    phase_cell: Arc<AtomicU32>,
    audio_player: AudioPlayer,
    _world: World,
}

impl AudioThreadConfig {
    pub fn new(
        config: &Config,
        gui_receiver: std::sync::mpsc::Receiver<PbEvent>,
        repeat_cell: Arc<AtomicU32>,
        beat_cell: Arc<AtomicU32>,
        phase_cell: Arc<AtomicU32>,
        phases: Vec<ResolvedPhase>,
    ) -> Result<Self> {
        let world = World::with_load_all();

        let has_bus = config.drum_bus.is_some();

        let mut drum_config = config.drums.to_chain_config("drums");
        if has_bus {
            drum_config.auto_connect_output = false;
        }
        let drum_chain = Chain::start_with_world(&drum_config, &world)
            .map_err(|e| anyhow::anyhow!("Failed to start drum chain: {e}"))?;
        println!("Drum chain started successfully.");
        std::thread::sleep(Duration::from_millis(200));

        let cymbal_chain = if let Some(cymbal_cfg) = &config.cymbals {
            let mut cc = cymbal_cfg.to_chain_config("cymbals");
            if has_bus {
                cc.auto_connect_output = false;
            }
            let chain = Chain::start_with_world(&cc, &world)
                .map_err(|e| anyhow::anyhow!("Failed to start cymbal chain: {e}"))?;
            println!("Cymbal chain started successfully.");
            std::thread::sleep(Duration::from_millis(200));
            Some(chain)
        } else {
            None
        };

        let drum_bus = if let Some(bus_cfg) = &config.drum_bus {
            let bus_config = bus_cfg.to_chain_config("drum_bus");
            let bus = Chain::start_with_world(&bus_config, &world)
                .map_err(|e| anyhow::anyhow!("Failed to start drum bus: {e}"))?;
            println!("Drum bus started successfully.");
            std::thread::sleep(Duration::from_millis(200));
            drum_chain.connect_to(&bus)
                .map_err(|e| anyhow::anyhow!("Failed to connect drums → bus: {e}"))?;
            if let Some(ref cymbals) = cymbal_chain {
                cymbals.connect_to(&bus)
                    .map_err(|e| anyhow::anyhow!("Failed to connect cymbals → bus: {e}"))?;
            }
            Some(bus)
        } else {
            None
        };

        let guitar_config = config.guitar.to_chain_config("guitar");
        let guitar_chain = Chain::start_with_world(&guitar_config, &world)
            .map_err(|e| anyhow::anyhow!("Failed to start guitar chain: {e}"))?;
        println!("Guitar chain started successfully.");

        let audio_paths: Vec<String> = phases.iter()
            .filter_map(|p| p.audio_file.clone())
            .collect();
        let audio_player = AudioPlayer::new(&audio_paths);

        Ok(Self {
            meter: Meter::new(config.default_tempo as f32),
            drum_on_left: false,
            cymbal_on_left: false,
            drum_chain,
            cymbal_chain,
            drum_bus,
            guitar_chain,
            gui_receiver,
            follow_deadline: None,
            follow_count: 0,
            drum_mode: DrumMode {
                follow_once: Subdivision::ThirtySecond,
                follow_count: FollowCount::Count(1),
                floor_subdivision: Subdivision::None,
                cymbal_subdivision: Subdivision::None,
                snare_half_time: false,
                random_accentness: 0.0,
                cymbal_mode: Default::default(),
            },
            guitar_mode: GuitarMode::default(),
            guitar_held: false,
            last_attack_params: None,
            cursor_cell: (0, 0),
            repeat_cell,
            last_attack_coords: None,
            last_normal_notes: Vec::new(),
            last_harmonic_notes: Vec::new(),
            cymbal_count_since_reset: 0,
            space_held: false,
            last_keyswitch: 14,
            palm_mute: false,
            last_attack_channel: 0,
            phases,
            current_phase_index: None,
            pending_phase_change: None,
            beat_cell,
            phase_cell,
            audio_player,
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
        let chain = self.cymbal_chain.as_mut().unwrap_or(&mut self.drum_chain);
        if let Ok(sender) = chain.midi_sender("drumgizmo") {
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

    fn find_group(col: u8) -> Option<(u8, u8)> {
        match col {
            16..=20 => Some((20, 28)),
            12..=15 => Some((15, 52)),
            _ => None,
        }
    }

    fn keyswitch_for_col(&self, col: u8) -> Option<u8> {
        match col {
            16..=20 if self.palm_mute => Some(20), // g#0 — palm mute
            16..=20 => Some(14), // d0 — sustain
            12..=15 => Some(10), // a#-1 — picking harmonics
            _ => None,
        }
    }

    fn send_guitar_note_on(&mut self, col: u8, row: u8, velocity: u8) {
        let is_normal = matches!(col, 16..=20);
        let channel: u8 = if is_normal { 0 } else { 1 };
        if is_normal {
            self.send_all_guitar_note_off();
        } else {
            self.send_harmonic_note_off();
        }
        let (col_end, base_note) = match Self::find_group(col) {
            Some(g) => g,
            None => return,
        };
        if let Some(ks) = self.keyswitch_for_col(col) {
            if ks != self.last_keyswitch {
                if let Ok(sender) = self.guitar_chain.midi_sender("sfizz") {
                    let _ = sender.send(MidiEvent::NoteOn { channel, note: ks, velocity: 1 });
                    let _ = sender.send(MidiEvent::NoteOff { channel, note: ks, velocity: 0 });
                }
                self.last_keyswitch = ks;
            }
        }
        let pair = col_end - col;
        let note = if pair % 2 == 0 {
            base_note + pair * 12 + (11 - row)
        } else {
            base_note + pair * 12 + row
        };
        if let Ok(sender) = self.guitar_chain.midi_sender("sfizz") {
            let _ = sender.send(MidiEvent::NoteOn { channel, note, velocity });
        }
        if is_normal {
            self.last_normal_notes = vec![note];
        } else {
            self.last_harmonic_notes = vec![note];
        }
    }

    fn send_normal_note_off(&mut self) {
        if let Ok(sender) = self.guitar_chain.midi_sender("sfizz") {
            for note in self.last_normal_notes.drain(..) {
                let _ = sender.send(MidiEvent::NoteOff { channel: 0, note, velocity: 0 });
            }
        }
    }

    fn send_harmonic_note_off(&mut self) {
        if let Ok(sender) = self.guitar_chain.midi_sender("sfizz") {
            for note in self.last_harmonic_notes.drain(..) {
                let _ = sender.send(MidiEvent::NoteOff { channel: 1, note, velocity: 0 });
            }
        }
    }

    fn send_all_guitar_note_off(&mut self) {
        self.send_normal_note_off();
        self.send_harmonic_note_off();
    }

    fn send_guitar_pitch_bend(&mut self, semitones: f32) {
        const BEND_RANGE: f32 = 24.0;
        let normalized = (semitones / BEND_RANGE).clamp(-1.0, 1.0);

        let (up_cc, down_cc) = if self.last_attack_channel == 0 {
            (85u8, 86u8)
        } else {
            (87u8, 88u8)
        };

        let (up_val, down_val) = if normalized >= 0.0 {
            ((normalized * 127.0) as u8, 0u8)
        } else {
            (0u8, ((-normalized) * 127.0) as u8)
        };

        if let Ok(sender) = self.guitar_chain.midi_sender("sfizz") {
            let _ = sender.send(MidiEvent::Cc { channel: 0, controller: up_cc, value: up_val });
            let _ = sender.send(MidiEvent::Cc { channel: 0, controller: down_cc, value: down_val });
        }
    }

    fn beat_active(&self) -> bool {
        self.current_phase_index.is_some()
            || self.drum_mode.cymbal_subdivision != Subdivision::None
            || self.drum_mode.snare_half_time
    }

    fn sync_subdivision_counter(&mut self) {
        self.meter.catch_up();
        if self.beat_active() {
            self.beat_cell.store(self.meter.subdivision, Ordering::Relaxed);
        }
    }

    fn apply_phase(&mut self, phase_idx: Option<usize>) {
        let was_beat = self.beat_active();
        self.current_phase_index = phase_idx;
        let idx = phase_idx.map(|i| i as u32).unwrap_or(0xFFFF_FFFF);
        self.phase_cell.store(idx, Ordering::Relaxed);
        self.audio_player.stop();
        if let Some(i) = phase_idx {
            let phase = &self.phases[i];
            self.meter.tempo = phase.tempo;
            self.drum_mode.cymbal_subdivision = if phase.cymbal_on { Subdivision::Quarter } else { Subdivision::None };
            self.drum_mode.snare_half_time = phase.snare_on;
            if !was_beat && self.beat_active() {
                self.meter.reset();
            }
            self.beat_cell.store(self.meter.subdivision, Ordering::Relaxed);
            if let Some(ref path) = phase.audio_file {
                self.audio_player.play(path);
            }
        } else {
            self.drum_mode.cymbal_subdivision = Subdivision::None;
            self.drum_mode.snare_half_time = false;
            self.follow_deadline = None;
            self.beat_cell.store(0xFFFF_FFFF, Ordering::Relaxed);
        }
    }

    pub fn run_loop(&mut self) {
        loop {
            let subdivision_timeout = self.meter.timeout();
            let follow_timeout = self
                .follow_deadline
                .map(|fc| fc.saturating_duration_since(Instant::now()));
            let is_subdivision_timeout = follow_timeout.map_or(true, |ft| subdivision_timeout < ft);
            match self.gui_receiver.recv_timeout(if is_subdivision_timeout {
                subdivision_timeout
            } else {
                follow_timeout.expect("follow_deadline is None")
            }) {
                Ok(event) => {
                match event {
                    PbEvent::TempoChange(new_bpm) => {
                        self.meter.set_tempo(new_bpm);
                    }
                    PbEvent::DrumModeChange(drum_mode) => {
                        let was_beat = self.beat_active();
                        self.drum_mode = drum_mode;
                        if !was_beat && self.beat_active() {
                            self.meter.reset();
                        }
                        if !self.beat_active() {
                            self.beat_cell.store(0xFFFF_FFFF, Ordering::Relaxed);
                        }
                    }
                    PbEvent::GuitarModeChange(guitar_mode) => {
                        self.guitar_mode = guitar_mode;
                    }
                    PbEvent::ResetBar => {
                        self.meter.reset();
                        self.cymbal_count_since_reset = 0;
                    }
                    PbEvent::Attack { col, row } => {
                        let is_normal = matches!(col, 16..=20);
                        self.last_attack_channel = if is_normal { 0u8 } else { 1 };
                        self.send_guitar_pitch_bend(0.0);
                        self.last_attack_coords = Some((col as f32, row as f32));
                        if is_normal && self.drum_mode.follow_once != Subdivision::None {
                            self.send_kick();
                            self.follow_count = match self.drum_mode.follow_count {
                                FollowCount::Count(n) => n.saturating_sub(1),
                                FollowCount::Infinite => u8::MAX,
                            };
                            if self.follow_count > 0 {
                                let thirty_second_dur = Duration::from_secs_f32(4.0 * 60.0 / self.meter.tempo / 32.0);
                                self.follow_deadline = Some(Instant::now() + thirty_second_dur);
                            } else {
                                self.follow_deadline = None;
                            }
                        } else if !is_normal {
                            self.follow_deadline = None;
                        }
                        self.send_guitar_note_on(col, row, 127);
                        self.guitar_held = true;
                        self.last_attack_params = Some((col, row, 127));
                        self.cursor_cell = (col, row);
                    }
                    PbEvent::AttackWithPressure { col, row, pressure } => {
                        let is_normal = matches!(col, 16..=20);
                        self.last_attack_channel = if is_normal { 0u8 } else { 1 };
                        self.send_guitar_pitch_bend(0.0);
                        self.last_attack_coords = Some((col as f32, row as f32));
                        if is_normal && self.drum_mode.follow_once != Subdivision::None {
                            self.send_kick();
                            self.follow_count = match self.drum_mode.follow_count {
                                FollowCount::Count(n) => n.saturating_sub(1),
                                FollowCount::Infinite => u8::MAX,
                            };
                            if self.follow_count > 0 {
                                let thirty_second_dur = Duration::from_secs_f32(4.0 * 60.0 / self.meter.tempo / 32.0);
                                self.follow_deadline = Some(Instant::now() + thirty_second_dur);
                            } else {
                                self.follow_deadline = None;
                            }
                        } else if !is_normal {
                            self.follow_deadline = None;
                        }
                        let velocity = (pressure * 127.0).clamp(1.0, 127.0) as u8;
                        self.send_guitar_note_on(col, row, velocity);
                        self.guitar_held = true;
                        self.last_attack_params = Some((col, row, velocity));
                        self.cursor_cell = (col, row);
                    }
                    PbEvent::CursorUpdate { col, row } => {
                        self.cursor_cell = (col, row);
                        if self.guitar_held && self.guitar_mode.subdivision != Subdivision::None {
                            if let Some(params) = &mut self.last_attack_params {
                                params.0 = col;
                                params.1 = row;
                            }
                        }
                    }
                    PbEvent::AttackHeld(semitones) | PbEvent::AttackHeldWithPressure(semitones) => {
                        self.send_guitar_pitch_bend(semitones);
                    }
                    PbEvent::AttackOff => {
                        if self.space_held {
                            self.send_all_guitar_note_off();
                        }
                        self.guitar_held = false;
                        if self.drum_mode.follow_count == FollowCount::Infinite {
                            self.follow_deadline = None;
                        }
                    }
                    PbEvent::PhaseAdvance => {
                        if !self.phases.is_empty() {
                            let next = match self.current_phase_index {
                                None => Some(0),
                                Some(i) if i + 1 < self.phases.len() => Some(i + 1),
                                Some(_) => None,
                            };
                            if self.beat_active() {
                                self.pending_phase_change = Some(next);
                            } else {
                                self.apply_phase(next);
                            }
                        }
                    }
                    PbEvent::PhaseRetreat => {
                        if !self.phases.is_empty() {
                            let prev = match self.current_phase_index {
                                None => Some(self.phases.len() - 1),
                                Some(0) => None,
                                Some(i) => Some(i - 1),
                            };
                            if self.beat_active() {
                                self.pending_phase_change = Some(prev);
                            } else {
                                self.apply_phase(prev);
                            }
                        }
                    }
                    PbEvent::PalmMute(on) => {
                        self.palm_mute = on;
                        self.last_keyswitch = 0;
                        let cc_val = if on { 127 } else { 51 };
                        if let Ok(sender) = self.guitar_chain.midi_sender("sfizz") {
                            let _ = sender.send(MidiEvent::Cc { channel: 0, controller: 22, value: cc_val });
                        }
                    }
                    PbEvent::SpaceState(held) => {
                        self.space_held = held;
                        if held && !self.guitar_held
                            && (!self.last_normal_notes.is_empty() || !self.last_harmonic_notes.is_empty())
                        {
                            self.send_all_guitar_note_off();
                        }
                    }
                    PbEvent::ToggleDrumUi => {
                        let _ = self.drum_chain.toggle_all_ui();
                    }
                    PbEvent::ToggleGuitarUi => {
                        let _ = self.guitar_chain.toggle_all_ui();
                    }
                    PbEvent::ToggleMute => {
                        self.drum_chain.toggle_mute();
                        if let Some(ref c) = self.cymbal_chain {
                            c.toggle_mute();
                        }
                        self.guitar_chain.toggle_mute();
                    }
                    PbEvent::ToggleDrumMute => {
                        self.drum_chain.toggle_mute();
                        if let Some(ref c) = self.cymbal_chain {
                            c.toggle_mute();
                        }
                    }
                    PbEvent::ToggleGuitarMute => {
                        self.guitar_chain.toggle_mute();
                    }
                    PbEvent::ToggleCymbalUi => {
                        if let Some(ref mut c) = self.cymbal_chain {
                            let _ = c.toggle_all_ui();
                        }
                    }
                    PbEvent::ToggleDrumBusUi => {
                        if let Some(ref mut b) = self.drum_bus {
                            let _ = b.toggle_all_ui();
                        }
                    }
                    PbEvent::Shutdown => {
                        self.audio_player.stop();
                        broken_nest::ui_shutdown();
                        if let Some(ref mut b) = self.drum_bus {
                            b.stop();
                        }
                        if let Some(ref mut c) = self.cymbal_chain {
                            c.stop();
                        }
                        self.drum_chain.stop();
                        self.guitar_chain.stop();
                        std::process::exit(0);
                    }
                }
                self.sync_subdivision_counter();
                }
                Err(RecvTimeoutError::Timeout) => {
                    if !is_subdivision_timeout {
                        self.send_kick();
                        if self.follow_count > 1 {
                            self.follow_count -= 1;
                            let thirty_second_dur = Duration::from_secs_f32(4.0 * 60.0 / self.meter.tempo / 32.0);
                            self.follow_deadline = Some(Instant::now() + thirty_second_dur);
                        } else {
                            self.follow_deadline = None;
                        }
                    } else {
                        let sub = self.meter.subdivision;
                        if self.drum_mode.cymbal_subdivision.is_on_curr_subdivision(sub) {
                            self.send_cymbal();
                        }
                        if self.drum_mode.snare_half_time && sub == 96 {
                            self.send_snare();
                        }
                        if self.follow_deadline.is_none()
                            && self.drum_mode.floor_subdivision.is_on_curr_subdivision(sub)
                        {
                            self.send_kick();
                        }
                        if self.guitar_held
                            && self.guitar_mode.subdivision.is_on_curr_subdivision(sub)
                        {
                            if let Some((col, row, vel)) = self.last_attack_params {
                                self.send_guitar_note_on(col, row, vel);
                                self.repeat_cell.store(
                                    ((col as u32) << 8) | row as u32,
                                    Ordering::Relaxed,
                                );
                            }
                        }
                        if self.space_held
                            && self.guitar_held
                            && self.guitar_mode.subdivision.is_at_half_subdivision(sub)
                        {
                            self.send_all_guitar_note_off();
                        }
                        if self.meter.tick() {
                            if let Some(pending) = self.pending_phase_change.take() {
                                self.apply_phase(pending);
                            }
                        }
                        if self.beat_active() {
                            self.beat_cell.store(self.meter.subdivision, Ordering::Relaxed);
                        }
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    broken_nest::ui_shutdown();
                    if let Some(ref mut b) = self.drum_bus {
                        b.stop();
                    }
                    if let Some(ref mut c) = self.cymbal_chain {
                        c.stop();
                    }
                    self.drum_chain.stop();
                    self.guitar_chain.stop();
                    return;
                }
            }
        }
    }
}
