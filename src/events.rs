pub enum PbEvent {
    TempoChange(f32),
    DrumModeChange(DrumMode),
    GuitarModeChange(GuitarMode),
    ResetBar,
    Attack { col: u8, row: u8 },
    AttackWithPressure { col: u8, row: u8, pressure: f32 },
    CursorUpdate { col: u8, row: u8 },
    AttackHeld(f32),
    AttackHeldWithPressure(f32),
    AttackOff,
    ToggleDrumUi,
    ToggleGuitarUi,
    ToggleMute,
    ToggleDrumMute,
    ToggleGuitarMute,
    ToggleCymbalUi,
    ToggleDrumBusUi,
    SpaceState(bool),
    PalmMute(bool),
    PhaseAdvance,
    PhaseRetreat,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub struct GuitarMode {
    pub subdivision: Subdivision,
}

impl Default for GuitarMode {
    fn default() -> Self {
        Self {
            subdivision: Subdivision::None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub struct DrumMode {
    pub follow_once: Subdivision,
    pub follow_count: FollowCount,
    pub floor_subdivision: Subdivision,
    pub cymbal_subdivision: Subdivision,
    pub snare_half_time: bool,
    pub random_accentness: f32,
    pub cymbal_mode: CymbalMode,
}

impl Default for DrumMode {
    fn default() -> Self {
        Self {
            follow_once: Subdivision::default(),
            follow_count: FollowCount::default(),
            floor_subdivision: Subdivision::None,
            cymbal_subdivision: Subdivision::default(),
            snare_half_time: true,
            random_accentness: 0.0,
            cymbal_mode: CymbalMode::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CymbalMode {
    HihatClosed,
    HihatSemiOpen,
    HihatOpen,
    Splash,
    Crash,
    China,
}

impl CymbalMode {
    // Returns a MIDI note number pair for this cymbal mode where the first number is the left and the second number is the right.
    pub fn get_midi_note(self) -> (u8, u8) {
        match self {
            CymbalMode::HihatClosed => (42, 42),
            CymbalMode::HihatSemiOpen => (80, 80),
            CymbalMode::HihatOpen => (46, 46),
            CymbalMode::Splash => (55, 58),
            CymbalMode::Crash => (49, 57),
            CymbalMode::China => (52, 54),
        }
    }
}

impl Default for CymbalMode {
    fn default() -> Self {
        CymbalMode::Crash
    }
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum Subdivision {
    None,
    Half,
    Quarter,
    Eighth,
    Triplet,
    Sixteenth,
    ThirtySecond,
}

impl Default for Subdivision {
    fn default() -> Self {
        Subdivision::Quarter
    }
}

impl Subdivision {
    pub fn is_on_curr_subdivision(self, curr_subdivsion_of_bar: u32) -> bool {
        match self {
            Subdivision::None => false,
            Subdivision::Half => curr_subdivsion_of_bar % 96 == 0,
            Subdivision::Quarter => curr_subdivsion_of_bar % 48 == 0,
            Subdivision::Eighth => curr_subdivsion_of_bar % 24 == 0,
            Subdivision::Triplet => curr_subdivsion_of_bar % 16 == 0,
            Subdivision::Sixteenth => curr_subdivsion_of_bar % 12 == 0,
            Subdivision::ThirtySecond => curr_subdivsion_of_bar % 6 == 0,
        }
    }

    pub fn is_at_half_subdivision(self, curr_subdivsion_of_bar: u32) -> bool {
        match self {
            Subdivision::None => false,
            Subdivision::Half => curr_subdivsion_of_bar % 96 == 48,
            Subdivision::Quarter => curr_subdivsion_of_bar % 48 == 24,
            Subdivision::Eighth => curr_subdivsion_of_bar % 24 == 12,
            Subdivision::Triplet => curr_subdivsion_of_bar % 16 == 8,
            Subdivision::Sixteenth => curr_subdivsion_of_bar % 12 == 6,
            Subdivision::ThirtySecond => curr_subdivsion_of_bar % 6 == 3,
        }
    }
}

#[derive(Clone, Debug, Copy, PartialEq)]
pub enum FollowCount {
    Count(u8),
    Infinite,
}

impl FollowCount {
    pub fn should_continue(self, count_so_far: u8) -> bool {
        match self {
            FollowCount::Count(count) => count_so_far < count,
            FollowCount::Infinite => true,
        }
    }
}

impl Default for FollowCount {
    fn default() -> Self {
        return FollowCount::Count(1);
    }
}
