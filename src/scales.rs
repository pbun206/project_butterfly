use palette::{FromColor, Oklab, Srgb};

const MINOR: [u8; 7] = [0, 2, 3, 5, 7, 8, 10];
const HIJO: [u8; 5] = [0, 1, 5, 7, 8];
const OCT1: [u8; 8] = [0, 1, 3, 4, 6, 7, 9, 10];
const OCT2: [u8; 8] = [0, 2, 3, 5, 6, 8, 9, 11];
const OCT3: [u8; 8] = [1, 2, 4, 5, 7, 8, 10, 11];

const ALL_SCALES: [&[u8]; 5] = [&MINOR, &HIJO, &OCT1, &OCT2, &OCT3];

pub fn next_note_probabilities(history: &[u8]) -> [f32; 12] {
    let mut scale_fit = [0.0f32; 5];
    for (i, &note) in history.iter().rev().take(8).enumerate() {
        let pc = note.wrapping_sub(28) % 12;
        let weight = 1.0 / (i as f32 + 1.0);
        for (s, scale) in ALL_SCALES.iter().enumerate() {
            if scale.contains(&pc) {
                scale_fit[s] += weight;
            }
        }
    }

    let mut scores = [0.0f32; 12];
    for pc in 0..12u8 {
        for (s, scale) in ALL_SCALES.iter().enumerate() {
            if scale.contains(&pc) {
                scores[pc as usize] += scale_fit[s];
            }
        }
    }

    let max = scores.iter().cloned().fold(0.0f32, f32::max);
    if max > 0.0 {
        for s in &mut scores {
            *s /= max;
        }
    }

    scores
}

pub fn oklab_to_rgb(l: f32, a: f32, b: f32) -> [u8; 3] {
    let rgb: Srgb<u8> = Srgb::<f32>::from_color(Oklab::new(l, a, b)).into_format();
    [rgb.red, rgb.green, rgb.blue]
}
