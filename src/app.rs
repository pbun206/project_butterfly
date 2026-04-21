use crate::config::Config;
use crate::state::State;
use egui::{Color32, Frame, LayerId};
use egui_wgpu::wgpu::SurfaceTexture;
use egui_wgpu::{ScreenDescriptor, wgpu};
use std::sync::Arc;
use wgpu::CurrentSurfaceTexture;
use winit::application::ApplicationHandler;
use winit::cursor::CursorIcon;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::monitor::Fullscreen;
use winit::window::{Window, WindowAttributes};

/// This structure operates input handling, windowing, and uses State to handle rendering.
pub struct App {
    state: Option<State>,
    config: Config,
    window: Option<Arc<dyn Window>>,
}

impl App {
    /// Creates a new instance of the app with the given configuration.
    pub fn new(config: Config) -> Self {
        Self {
            state: None,
            config,
            window: None,
        }
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
            let color = Color32::WHITE;
            let radius = 2.5;

            for i in 0..21 {
                for j in 0..14 {
                    painter.circle_filled(
                        egui::pos2(
                            x_offset + width * (i as f32 / 20.0),
                            y_offset + height * (j as f32 / 13.0),
                        ),
                        radius,
                        color,
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

        // &self.window.request_redraw();
        // TODO
    }
}

impl ApplicationHandler for App {
    // TODO should be able to make the same
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        #[allow(unused_mut)]
        let mut window_attributes = WindowAttributes::default()
            .with_fullscreen(Some(Fullscreen::Borderless(None)))
            .with_cursor(CursorIcon::Crosshair);

        let window = Arc::from((event_loop.create_window(window_attributes)).unwrap());

        pollster::block_on(self.set_window(window));
    }

    // TODO make these events work
    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let state = match &mut self.state {
            Some(canvas) => canvas,
            None => return,
        };
        println!("{:#?}", event);
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
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
                (KeyCode::Escape, true) => event_loop.exit(),
                _ => {}
            },
            _ => {}
        }
    }
}
