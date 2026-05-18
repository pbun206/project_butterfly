use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

struct PlayerState {
    tracks: Vec<Vec<[f32; 2]>>,
    current_track: AtomicUsize,
    position: AtomicUsize,
    playing: AtomicBool,
}

struct PlayerHandler {
    state: Arc<PlayerState>,
    out_l: jack::Port<jack::AudioOut>,
    out_r: jack::Port<jack::AudioOut>,
}

impl jack::ProcessHandler for PlayerHandler {
    fn process(&mut self, _client: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        let buf_l = self.out_l.as_mut_slice(ps);
        let buf_r = self.out_r.as_mut_slice(ps);

        if !self.state.playing.load(Ordering::Acquire) {
            for i in 0..buf_l.len() {
                buf_l[i] = 0.0;
                buf_r[i] = 0.0;
            }
            return jack::Control::Continue;
        }

        let track_idx = self.state.current_track.load(Ordering::Acquire);
        let samples = match self.state.tracks.get(track_idx) {
            Some(s) => s,
            None => {
                for i in 0..buf_l.len() {
                    buf_l[i] = 0.0;
                    buf_r[i] = 0.0;
                }
                return jack::Control::Continue;
            }
        };

        let pos = self.state.position.load(Ordering::Relaxed);
        let remaining = samples.len().saturating_sub(pos);
        let to_copy = buf_l.len().min(remaining);

        for i in 0..to_copy {
            let [l, r] = samples[pos + i];
            buf_l[i] = l;
            buf_r[i] = r;
        }
        for i in to_copy..buf_l.len() {
            buf_l[i] = 0.0;
            buf_r[i] = 0.0;
        }
        self.state.position.store(pos + to_copy, Ordering::Relaxed);
        if to_copy == 0 {
            self.state.playing.store(false, Ordering::Release);
        }
        jack::Control::Continue
    }
}

fn decode_file(path: &str) -> Vec<[f32; 2]> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("audio_player: failed to open {path}: {e}");
            return Vec::new();
        }
    };
    let source = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if path.ends_with(".mp3") {
        hint.with_extension("mp3");
    } else if path.ends_with(".flac") {
        hint.with_extension("flac");
    } else if path.ends_with(".wav") {
        hint.with_extension("wav");
    }
    let probed = match symphonia::default::get_probe()
        .format(&hint, source, &FormatOptions::default(), &MetadataOptions::default())
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("audio_player: failed to probe {path}: {e}");
            return Vec::new();
        }
    };

    let mut reader = probed.format;
    let track = match reader.default_track() {
        Some(t) => t,
        None => return Vec::new(),
    };
    let track_id = track.id;
    let channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(2);
    let mut decoder = match symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
    {
        Ok(d) => d,
        Err(e) => {
            eprintln!("audio_player: failed to create decoder for {path}: {e}");
            return Vec::new();
        }
    };

    let mut samples = Vec::new();

    loop {
        let packet = match reader.next_packet() {
            Ok(p) => p,
            Err(_) => break,
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let spec = *decoded.spec();
        let num_frames = decoded.capacity();
        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        let interleaved = sample_buf.samples();

        if channels >= 2 {
            for chunk in interleaved.chunks(channels) {
                samples.push([chunk[0], chunk[1]]);
            }
        } else {
            for &s in interleaved {
                samples.push([s, s]);
            }
        }
    }

    samples
}

pub struct AudioPlayer {
    path_to_index: HashMap<String, usize>,
    state: Arc<PlayerState>,
    _client: Option<jack::AsyncClient<(), PlayerHandler>>,
}

impl AudioPlayer {
    pub fn new(audio_paths: &[String]) -> Self {
        let mut tracks = Vec::new();
        let mut path_to_index = HashMap::new();

        for path in audio_paths {
            if path_to_index.contains_key(path) {
                continue;
            }
            println!("Pre-decoding audio: {path}");
            let samples = decode_file(path);
            if !samples.is_empty() {
                path_to_index.insert(path.clone(), tracks.len());
                tracks.push(samples);
            }
        }

        let state = Arc::new(PlayerState {
            tracks,
            current_track: AtomicUsize::new(usize::MAX),
            position: AtomicUsize::new(0),
            playing: AtomicBool::new(false),
        });

        let (client, _status) =
            jack::Client::new("pb-audio", jack::ClientOptions::NO_START_SERVER)
                .expect("failed to create JACK client");

        let out_l = client
            .register_port("out_l", jack::AudioOut::default())
            .expect("failed to register left port");
        let out_r = client
            .register_port("out_r", jack::AudioOut::default())
            .expect("failed to register right port");

        let handler = PlayerHandler {
            state: Arc::clone(&state),
            out_l,
            out_r,
        };
        let active = client
            .activate_async((), handler)
            .expect("failed to activate JACK client");

        let jack_client = active.as_client();
        let physical: Vec<String> = jack_client.ports(
            None,
            None,
            jack::PortFlags::IS_PHYSICAL | jack::PortFlags::IS_INPUT,
        );
        if physical.len() >= 2 {
            let _ = jack_client.connect_ports_by_name("pb-audio:out_l", &physical[0]);
            let _ = jack_client.connect_ports_by_name("pb-audio:out_r", &physical[1]);
        }

        Self {
            path_to_index,
            state,
            _client: Some(active),
        }
    }

    pub fn play(&self, path: &str) {
        if let Some(&idx) = self.path_to_index.get(path) {
            self.state.playing.store(false, Ordering::Release);
            self.state.position.store(0, Ordering::Relaxed);
            self.state.current_track.store(idx, Ordering::Relaxed);
            self.state.playing.store(true, Ordering::Release);
        }
    }

    pub fn stop(&self) {
        self.state.playing.store(false, Ordering::Release);
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.state.playing.store(false, Ordering::Release);
        if let Some(client) = self._client.take() {
            client.deactivate().ok();
        }
    }
}
