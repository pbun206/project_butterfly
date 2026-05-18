use crate::config::ResolvedPhase;
use crate::events::{DrumMode, FollowCount, GuitarMode, PbEvent, Subdivision};
use crate::scales;
use crate::state::State;
use egui::{Color32, LayerId};
use egui_wgpu::wgpu::SurfaceTexture;
use egui_wgpu::{ScreenDescriptor, wgpu};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;

use wgpu::CurrentSurfaceTexture;
use winit::application::ApplicationHandler;
use winit::cursor::CustomCursorSource;
use winit::event::{ButtonSource, KeyEvent, TabletToolButton, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::monitor::Fullscreen;
use winit::window::{Window, WindowAttributes};

fn snake_pitch(col: u8, row: u8) -> Option<u8> {
    if row > 11 {
        return None;
    }
    let (col_end, base_note) = match col {
        16..=20 => (20, 28),
        12..=15 => (15, 52),
        _ => return None,
    };
    let pair = col_end - col;
    Some(if pair % 2 == 0 {
        base_note + pair * 12 + (11 - row)
    } else {
        base_note + pair * 12 + row
    })
}

pub struct App {
    audio_thread_sender: Sender<PbEvent>,
    state: Option<State>,
    window: Option<Arc<dyn Window>>,
    attack_origin: Option<(f64, f64, u8)>,
    drum_mode: DrumMode,
    guitar_mode: GuitarMode,
    space_held: bool,
    shift_held: bool,
    contact_pressed: bool,
    pending_release: bool,
    last_attack_col: u8,
    last_attack_row: u8,
    cursor_pos: (f64, f64),
    repeat_cell: Arc<AtomicU32>,
    note_history: Vec<u8>,
    show_score_labels: bool,
    show_score_colors: bool,
    selected_blue_decay: f32,
    selected_cell: Option<(u8, u8)>,
    beat_cell: Arc<AtomicU32>,
    phase_cell: Arc<AtomicU32>,
    phases: Vec<ResolvedPhase>,
}

impl App {
    pub fn new(
        audio_thread_sender: Sender<PbEvent>,
        repeat_cell: Arc<AtomicU32>,
        beat_cell: Arc<AtomicU32>,
        phase_cell: Arc<AtomicU32>,
        phases: Vec<ResolvedPhase>,
    ) -> Self {
        let drum_mode = DrumMode {
            follow_once: Subdivision::ThirtySecond,
            follow_count: FollowCount::Count(1),
            floor_subdivision: Subdivision::None,
            cymbal_subdivision: Subdivision::None,
            snare_half_time: false,
            random_accentness: 0.0,
            cymbal_mode: Default::default(),
        };
        Self {
            state: None,
            window: None,
            audio_thread_sender,
            attack_origin: None,
            drum_mode,
            guitar_mode: GuitarMode::default(),
            space_held: false,
            shift_held: false,
            contact_pressed: false,
            pending_release: false,
            last_attack_col: 0,
            last_attack_row: 0,
            cursor_pos: (0.0, 0.0),
            repeat_cell,
            note_history: Vec::new(),
            show_score_labels: false,
            show_score_colors: false,
            selected_blue_decay: 0.0,
            selected_cell: None,
            beat_cell,
            phase_cell,
            phases,
        }
    }

    fn position_to_grid(&self, x: f64, y: f64) -> (u8, u8) {
        let state = self.state.as_ref().unwrap();
        let scale = self.window.as_ref().unwrap().scale_factor();
        let w = state.surface_config.width as f64 / scale;
        let h = state.surface_config.height as f64 / scale;
        let col = (x / w * 21.0).floor().clamp(0.0, 20.0) as u8;
        let row = (y / h * 13.0 - 1.0).floor().clamp(0.0, 11.0) as u8;
        (col, row)
    }

    fn cell_center(&self, col: u8, row: u8) -> (f64, f64) {
        let state = self.state.as_ref().unwrap();
        let scale = self.window.as_ref().unwrap().scale_factor();
        let w = state.surface_config.width as f64 / scale;
        let h = state.surface_config.height as f64 / scale;
        ((col as f64 + 0.5) * w / 21.0, (row as f64 + 1.5) * h / 13.0)
    }

    fn record_note(&mut self, col: u8, row: u8) {
        if let Some(pitch) = snake_pitch(col, row) {
            self.note_history.push(pitch);
            if self.note_history.len() > 8 {
                self.note_history.remove(0);
            }
            self.selected_cell = Some((col, row));
            self.selected_blue_decay = 1.0;
        }
    }

    fn shutdown(&mut self) {
        let _ = self.audio_thread_sender.send(PbEvent::Shutdown);
    }

    /// Initializes the window and sets up the state.
    async fn set_window(&mut self, window: Arc<dyn Window>) {
        // hopefully display handle isn't important TODO
        let instance =
            egui_wgpu::wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let initial_width = 900;
        let initial_height = 900;

        // pray this ain't important TODO
        // let _ = (*window).request_inner_size(PhysicalSize::new(initial_width, initial_height));

        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface!");

        let state = State::new(&instance, surface, initial_width, initial_height, &*window).await;

        self.state.get_or_insert(state);
        self.window.get_or_insert(window);
    }

    /// Handles a window resize event by calling `resize_surface` on the state.
    fn handle_resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.state.as_mut().unwrap().resize_surface(width, height);
        }
    }

    fn handle_redraw(&mut self) {
        // Checks if it's minimalized
        if let Some(window) = &self.window {
            if let Some(min) = window.is_minimized() {
                if min {
                    println!("Window is minimized");
                    return;
                }
            }
        }

        if self.attack_origin.is_some() {
            let packed = self.repeat_cell.swap(0xFFFF_FFFF, Ordering::Relaxed);
            if packed != 0xFFFF_FFFF {
                self.last_attack_col = (packed >> 8) as u8;
                self.last_attack_row = (packed & 0xFF) as u8;
                self.record_note(self.last_attack_col, self.last_attack_row);
                self.window.as_ref().unwrap().request_redraw();
            }
        }

        if self.selected_blue_decay > 0.01 {
            self.selected_blue_decay *= 0.95;
            self.window.as_ref().unwrap().request_redraw();
        } else {
            self.selected_blue_decay = 0.0;
            self.selected_cell = None;
        }

        let scores = scales::next_note_probabilities(&self.note_history);

        // Creates alias for self.state basically
        let state = self.state.as_mut().unwrap();

        // Basically like screen config
        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [state.surface_config.width, state.surface_config.height],
            pixels_per_point: self.window.as_ref().unwrap().scale_factor() as f32
                * state.scale_factor,
        };

        // Get surface texture
        let surface_texture_result = state.surface.get_current_texture();

        // TODO remove all the todos
        let surface_texture: SurfaceTexture = match surface_texture_result {
            CurrentSurfaceTexture::Success(surface_texture) => surface_texture,
            CurrentSurfaceTexture::Suboptimal(surface_texture) => surface_texture,
            CurrentSurfaceTexture::Timeout => todo!(),
            CurrentSurfaceTexture::Occluded => todo!(),
            CurrentSurfaceTexture::Outdated => todo!(),
            CurrentSurfaceTexture::Lost => todo!(),
            CurrentSurfaceTexture::Validation => todo!(),
        };

        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = state
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        // this is ugly
        let window = &**self.window.as_ref().unwrap();

        // TODO drawing shit
        {
            state.egui_renderer.begin_frame(window);

            let ui = state.egui_renderer.context();
            let available_rect = ui.content_rect();
            let width = available_rect.width();
            let x_offset = available_rect.left();
            let height = available_rect.height();
            let y_offset = available_rect.top();
            let painter = ui.layer_painter(LayerId::background());
            let radius = 2.5;

            let cell_w = width / 21.0;
            let cell_h = height / 13.0;

            for i in 0..22 {
                for j in 1..14 {
                    let pos = egui::pos2(
                        x_offset + width * (i as f32 / 21.0),
                        y_offset + height * (j as f32 / 13.0),
                    );

                    let cell_center = pos + egui::vec2(cell_w * 0.5, cell_h * 0.5);
                    let col_u8 = i as u8;
                    let row_u8 = (j - 1) as u8;

                    if self.show_score_colors {
                        if let Some(pitch) = snake_pitch(col_u8, row_u8) {
                            let pc = pitch.wrapping_sub(28) % 12;
                            let bg = if self.selected_cell == Some((col_u8, row_u8)) {
                                let d = self.selected_blue_decay;
                                let [r, g, b] = scales::oklab_to_rgb(0.25 * d, -0.03 * d, -0.13 * d);
                                Color32::from_rgb(r, g, b)
                            } else {
                                let l = scores[pc as usize] * 0.25;
                                let [r, g, b] = scales::oklab_to_rgb(l, 0.0, 0.0);
                                Color32::from_rgb(r, g, b)
                            };
                            let rect = egui::Rect::from_min_size(pos, egui::vec2(cell_w, cell_h));
                            painter.rect_filled(rect, 0.0, bg);
                        }
                    }

                    painter.circle_filled(pos, radius, Color32::WHITE);

                    if self.show_score_labels {
                        if let Some(pitch) = snake_pitch(col_u8, row_u8) {
                            let pc = pitch.wrapping_sub(28) % 12;
                            if self.selected_cell != Some((col_u8, row_u8)) {
                                let s = scores[pc as usize];
                                let label = format!("{:.2}", s);
                                painter.text(
                                    cell_center,
                                    egui::Align2::CENTER_CENTER,
                                    label,
                                    egui::FontId::proportional(cell_h * 0.22),
                                    Color32::WHITE,
                                );
                            }
                        }
                    }
                }
            }

            const GROUP_LABELS: &[(u8, u8, &str)] = &[
                (16, 19, "normal"),
                (12, 15, "harmonics"),
            ];
            for &(start, end, label) in GROUP_LABELS {
                let center_col = (start as f32 + end as f32 + 1.0) / 2.0;
                painter.text(
                    egui::pos2(
                        x_offset + width * (center_col / 21.0),
                        y_offset + height * (0.5 / 13.0),
                    ),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(height / 13.0 * 0.5),
                    Color32::WHITE,
                );
            }

            let separator_color = Color32::from_gray(153);
            for divider_col in [12.0, 16.0] {
                let divider_x = x_offset + width * (divider_col / 21.0);
                for j in 1..13 {
                    let mid_y = y_offset + height * ((j as f32 + 0.5) / 13.0);
                    painter.circle_filled(egui::pos2(divider_x, mid_y), radius, separator_color);
                }
            }
            let header_y = y_offset + height * (1.0 / 13.0);
            for i in 0..21 {
                let mid_x = x_offset + width * ((i as f32 + 0.5) / 21.0);
                painter.circle_filled(egui::pos2(mid_x, header_y), radius, separator_color);
            }

            let header_center_y = y_offset + height * (0.5 / 13.0);
            let font = egui::FontId::proportional(cell_h * 0.5);
            let phase_idx = self.phase_cell.load(Ordering::Relaxed);
            if phase_idx != 0xFFFF_FFFF {
                if let Some(phase) = self.phases.get(phase_idx as usize) {
                    painter.text(
                        egui::pos2(x_offset + width * (1.5 / 21.0), header_center_y),
                        egui::Align2::CENTER_CENTER,
                        &phase.name,
                        font.clone(),
                        Color32::WHITE,
                    );
                }
            }
            let subdivision = self.beat_cell.load(Ordering::Relaxed);
            if subdivision != 0xFFFF_FFFF {
                let beat_number = subdivision / 48 + 1;
                painter.text(
                    egui::pos2(x_offset + width * (4.0 / 21.0), header_center_y),
                    egui::Align2::CENTER_CENTER,
                    format!("{}/4", beat_number),
                    font,
                    Color32::WHITE,
                );
            }

            if let Some((origin_x, origin_y, _)) = self.attack_origin {
                let (cx, cy) = self.cursor_pos;
                painter.line_segment(
                    [egui::pos2(origin_x as f32, origin_y as f32), egui::pos2(cx as f32, cy as f32)],
                    egui::Stroke::new(3.0, Color32::from_rgb(0x57, 0xdb, 0x00)),
                );
                if let Some(pitch) = snake_pitch(self.last_attack_col, self.last_attack_row) {
                let col = self.last_attack_col as f32;
                let row = self.last_attack_row as f32;
                let top_left = egui::pos2(
                    x_offset + width * (col / 21.0),
                    y_offset + height * ((row + 1.0) / 13.0),
                );
                let bottom_right = egui::pos2(
                    x_offset + width * ((col + 1.0) / 21.0),
                    y_offset + height * ((row + 2.0) / 13.0),
                );
                let cell_rect = egui::Rect::from_two_pos(top_left, bottom_right);
                let side = cell_rect.width().min(cell_rect.height());
                let square = egui::Rect::from_center_size(cell_rect.center(), egui::vec2(side, side));
                painter.rect_stroke(
                    square,
                    0.0,
                    egui::Stroke::new(radius * 2.0, Color32::from_rgb(0x14, 0x00, 0xb0)),
                    egui::StrokeKind::Middle,
                );
                const NOTE_NAMES: [&str; 12] = [
                    "E", "F", "F#", "G", "G#", "A", "A#", "B", "C", "C#", "D", "D#",
                ];
                let name = NOTE_NAMES[(pitch - 28) as usize % 12];
                painter.text(
                    square.center(),
                    egui::Align2::CENTER_CENTER,
                    name,
                    egui::FontId::proportional(side * 0.6),
                    Color32::WHITE,
                );
                }
            }

            state.egui_renderer.end_frame_and_draw(
                &state.device,
                &state.queue,
                &mut encoder,
                window,
                &surface_view,
                screen_descriptor,
            );
        }

        state.queue.submit(Some(encoder.finish()));
        surface_texture.present();

        if self.attack_origin.is_some() && self.guitar_mode.subdivision != Subdivision::None {
            self.window.as_ref().unwrap().request_redraw();
        }
        if self.phase_cell.load(Ordering::Relaxed) != 0xFFFF_FFFF {
            self.window.as_ref().unwrap().request_redraw();
        }
    }
}

impl ApplicationHandler for App {
    // TODO should be able to make the same
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = WindowAttributes::default()
            .with_fullscreen(Some(Fullscreen::Borderless(None)));

        let window: Arc<dyn Window> = Arc::from(event_loop.create_window(window_attributes).unwrap());

        let size: u16 = 31;
        let mut rgba = vec![0u8; (size as usize * size as usize) * 4];
        for y in 0..size {
            for x in 0..size {
                let on_h = y == size / 2 && (x as i16 - (size as i16 / 2)).unsigned_abs() <= 12;
                let on_v = x == size / 2 && (y as i16 - (size as i16 / 2)).unsigned_abs() <= 12;
                if on_h || on_v {
                    let i = (y as usize * size as usize + x as usize) * 4;
                    rgba[i] = 0x57;
                    rgba[i + 1] = 0xdb;
                    rgba[i + 2] = 0x00;
                    rgba[i + 3] = 255;
                }
            }
        }
        let hotspot = size / 2;
        let source = CustomCursorSource::from_rgba(rgba, size, size, hotspot, hotspot).unwrap();
        let custom_cursor = event_loop.create_custom_cursor(source).unwrap();
        window.set_cursor(custom_cursor.into());

        pollster::block_on(self.set_window(window));
    }

    // TODO make these events work
    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let _state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };
        if self.pending_release {
            self.pending_release = false;
            let _ = self.audio_thread_sender.send(PbEvent::AttackOff);
        }
        match event {
            WindowEvent::CloseRequested => {
                self.shutdown();
            }
            WindowEvent::SurfaceResized(size) => self.handle_resize(size.width, size.height),
            WindowEvent::RedrawRequested => {
                self.handle_redraw();
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(code),
                        state: key_state,
                        ..
                    },
                ..
            } => match (code, key_state.is_pressed()) {
                (KeyCode::Escape, true) => {
                    self.shutdown();
                }
                (KeyCode::Slash, true) => {
                    let _ = self.audio_thread_sender.send(PbEvent::ToggleDrumUi);
                }
                (KeyCode::Quote, true) => {
                    let _ = self.audio_thread_sender.send(PbEvent::ToggleGuitarUi);
                }
                (KeyCode::KeyM, true) => {
                    let _ = self.audio_thread_sender.send(PbEvent::ToggleMute);
                }
                (KeyCode::KeyD, true) => {
                    self.drum_mode.follow_count = FollowCount::Count(1);
                    let _ = self.audio_thread_sender.send(PbEvent::DrumModeChange(self.drum_mode));
                }
                (KeyCode::KeyE, true) => {
                    self.drum_mode.follow_count = FollowCount::Count(2);
                    let _ = self.audio_thread_sender.send(PbEvent::DrumModeChange(self.drum_mode));
                }
                (KeyCode::KeyR, true) => {
                    self.drum_mode.follow_count = FollowCount::Infinite;
                    let _ = self.audio_thread_sender.send(PbEvent::DrumModeChange(self.drum_mode));
                }
                (KeyCode::KeyS, true) => {
                    self.drum_mode.snare_half_time = !self.drum_mode.snare_half_time;
                    let _ = self.audio_thread_sender.send(PbEvent::DrumModeChange(self.drum_mode));
                }
                (KeyCode::KeyW, true) => {
                    if self.drum_mode.cymbal_subdivision == Subdivision::Quarter {
                        self.drum_mode.cymbal_subdivision = Subdivision::None;
                    } else {
                        self.drum_mode.cymbal_subdivision = Subdivision::Quarter;
                    }
                    let _ = self.audio_thread_sender.send(PbEvent::DrumModeChange(self.drum_mode));
                    let _ = self.audio_thread_sender.send(PbEvent::ResetBar);
                }
                (KeyCode::KeyF, true) => {
                    self.guitar_mode.subdivision = Subdivision::None;
                    let _ = self.audio_thread_sender.send(PbEvent::GuitarModeChange(self.guitar_mode));
                }
                (KeyCode::KeyT, true) => {
                    self.guitar_mode.subdivision = Subdivision::Eighth;
                    let _ = self.audio_thread_sender.send(PbEvent::GuitarModeChange(self.guitar_mode));
                }
                (KeyCode::KeyG, true) => {
                    self.guitar_mode.subdivision = Subdivision::Triplet;
                    let _ = self.audio_thread_sender.send(PbEvent::GuitarModeChange(self.guitar_mode));
                }
                (KeyCode::KeyV, true) => {
                    self.guitar_mode.subdivision = Subdivision::Sixteenth;
                    let _ = self.audio_thread_sender.send(PbEvent::GuitarModeChange(self.guitar_mode));
                }
                (KeyCode::KeyB, true) => {
                    self.guitar_mode.subdivision = Subdivision::ThirtySecond;
                    let _ = self.audio_thread_sender.send(PbEvent::GuitarModeChange(self.guitar_mode));
                }
                (KeyCode::KeyL, true) => {
                    let _ = self.audio_thread_sender.send(PbEvent::ToggleGuitarMute);
                }
                (KeyCode::KeyK, true) => {
                    let _ = self.audio_thread_sender.send(PbEvent::ToggleDrumMute);
                }
                (KeyCode::Period, true) => {
                    let _ = self.audio_thread_sender.send(PbEvent::ToggleCymbalUi);
                }
                (KeyCode::Comma, true) => {
                    let _ = self.audio_thread_sender.send(PbEvent::ToggleDrumBusUi);
                }
                (KeyCode::Space, pressed) => {
                    self.space_held = pressed;
                    let _ = self.audio_thread_sender.send(PbEvent::SpaceState(pressed));
                }
                (KeyCode::ShiftLeft | KeyCode::ShiftRight, pressed) => {
                    self.shift_held = pressed;
                }
                (KeyCode::KeyP, true) => {
                    self.show_score_labels = !self.show_score_labels;
                    self.window.as_ref().unwrap().request_redraw();
                }
                (KeyCode::KeyO, true) => {
                    self.show_score_colors = !self.show_score_colors;
                    self.window.as_ref().unwrap().request_redraw();
                }
                (KeyCode::KeyA, true) => {
                    let _ = self.audio_thread_sender.send(PbEvent::PhaseAdvance);
                    self.window.as_ref().unwrap().request_redraw();
                }
                (KeyCode::KeyQ, true) => {
                    let _ = self.audio_thread_sender.send(PbEvent::PhaseRetreat);
                    self.window.as_ref().unwrap().request_redraw();
                }
                _ => {}
            },
            WindowEvent::PointerMoved { position, .. } => {
                self.cursor_pos = (position.x, position.y);
                if let Some((origin_x, origin_y, origin_col)) = self.attack_origin {
                    self.window.as_ref().unwrap().request_redraw();
                    let (current_col, current_row) = self.position_to_grid(position.x, position.y);
                    let _ = self.audio_thread_sender.send(PbEvent::CursorUpdate {
                        col: current_col,
                        row: current_row,
                    });
                    let scale = self.window.as_ref().unwrap().scale_factor();
                    let w = self.state.as_ref().unwrap().surface_config.width as f64 / scale;
                    let h = self.state.as_ref().unwrap().surface_config.height as f64 / scale;
                    let col_width = w / 21.0;
                    let row_height = h / 13.0;
                    if self.guitar_mode.subdivision != Subdivision::None {
                        if current_col != self.last_attack_col || current_row != self.last_attack_row {
                            self.last_attack_col = current_col;
                            self.last_attack_row = current_row;
                            self.attack_origin = Some((position.x, position.y, current_col));
                            let _ = self.audio_thread_sender.send(PbEvent::AttackHeld(0.0));
                            self.window.as_ref().unwrap().request_redraw();
                        } else if let Some(col_end) = match origin_col { 16..=20 => Some(20u8), 12..=15 => Some(15), _ => None } {
                            let (bend_x, bend_y) = if self.shift_held { self.cell_center(origin_col, self.last_attack_row) } else { (origin_x, origin_y) };
                            let h_semitones = -(position.x - bend_x) / col_width;
                            let rows_moved = (position.y - bend_y) / row_height;
                            let pair = col_end - origin_col;
                            let v_semitones = if pair % 2 == 0 { -rows_moved } else { rows_moved };
                            let semitones = (h_semitones + v_semitones) as f32;
                            let _ = self.audio_thread_sender.send(PbEvent::AttackHeld(semitones));
                        }
                    } else if let Some(col_end) = match origin_col { 16..=20 => Some(20u8), 12..=15 => Some(15), _ => None } {
                        let (bend_x, bend_y) = if self.shift_held { self.cell_center(origin_col, self.last_attack_row) } else { (origin_x, origin_y) };
                        let h_semitones = -(position.x - bend_x) / col_width;
                        let rows_moved = (position.y - bend_y) / row_height;
                        let pair = col_end - origin_col;
                        let v_semitones = if pair % 2 == 0 { -rows_moved } else { rows_moved };
                        let semitones = (h_semitones + v_semitones) as f32;
                        let _ = self.audio_thread_sender.send(PbEvent::AttackHeld(semitones));
                    }
                }
            }
            WindowEvent::PointerButton {
                state: button_state,
                position,
                button,
                ..
            } => {
                match &button {
                    ButtonSource::TabletTool { button: tab_btn, .. } => {
                        match (tab_btn, button_state.is_pressed()) {
                            (TabletToolButton::Contact, true) => {
                                if !self.contact_pressed {
                                    self.contact_pressed = true;
                                    let (col, row) = self.position_to_grid(position.x, position.y);
                                    self.last_attack_col = col;
                                    self.last_attack_row = row;
                                    self.attack_origin = Some((position.x, position.y, col));
                                    self.record_note(col, row);
                                    let _ = self.audio_thread_sender.send(PbEvent::Attack { col, row });
                                    self.window.as_ref().unwrap().request_redraw();
                                }
                            }
                            (TabletToolButton::Barrel, pressed) => {
                                let _ = self.audio_thread_sender.send(PbEvent::PalmMute(pressed));
                            }
                            (TabletToolButton::Contact, false) => {
                                if self.contact_pressed {
                                    self.contact_pressed = false;
                                    self.attack_origin = None;
                                    self.pending_release = true;
                                    self.window.as_ref().unwrap().request_redraw();
                                }
                            }
                            _ => {}
                        }
                    }
                    ButtonSource::Mouse(_) | _ => {
                        if button_state.is_pressed() {
                            let (col, row) = self.position_to_grid(position.x, position.y);
                            self.last_attack_col = col;
                            self.last_attack_row = row;
                            self.attack_origin = Some((position.x, position.y, col));
                            self.record_note(col, row);
                            let _ = self.audio_thread_sender.send(PbEvent::Attack { col, row });
                            self.window.as_ref().unwrap().request_redraw();
                        } else {
                            self.attack_origin = None;
                            let _ = self.audio_thread_sender.send(PbEvent::AttackOff);
                            self.window.as_ref().unwrap().request_redraw();
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
