use std::time::{Duration, Instant};

pub struct Meter {
    pub subdivision: u32,
    deadline: Instant,
    pub tempo: f32,
}

impl Meter {
    pub fn new(tempo: f32) -> Self {
        Self {
            subdivision: 0,
            deadline: Instant::now(),
            tempo,
        }
    }

    pub fn sub_duration(&self) -> Duration {
        Duration::from_secs_f32(4.0 * 60.0 / self.tempo / 192.0)
    }

    pub fn reset(&mut self) {
        self.subdivision = 0;
        self.deadline = Instant::now() + self.sub_duration();
    }

    pub fn set_tempo(&mut self, tempo: f32) {
        self.tempo = tempo;
        self.reset();
    }

    pub fn timeout(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub fn tick(&mut self) -> bool {
        self.deadline += self.sub_duration();
        self.subdivision = (self.subdivision + 1) % 192;
        self.subdivision == 0
    }

    pub fn catch_up(&mut self) {
        let sub_dur = self.sub_duration();
        while self.deadline <= Instant::now() {
            self.deadline += sub_dur;
            self.subdivision = (self.subdivision + 1) % 192;
        }
    }
}
