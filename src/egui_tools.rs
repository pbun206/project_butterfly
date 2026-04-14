use egui::{Context, Pos2, Rect};
use egui_wgpu::wgpu::{CommandEncoder, Device, Queue, StoreOp, TextureFormat, TextureView};
use egui_wgpu::{Renderer, RendererOptions, ScreenDescriptor, wgpu};
use winit::window::Window;

pub struct EguiRenderer {
    egui_ctx: Context,
    raw_input: egui::RawInput,
    start_time: std::time::Instant,
    renderer: Renderer,
    frame_started: bool,
}

impl EguiRenderer {
    pub fn context(&self) -> &Context {
        &self.egui_ctx
    }

    pub fn new(
        device: &Device,
        output_color_format: TextureFormat,
        window: &dyn Window,
    ) -> EguiRenderer {
        let egui_ctx = Context::default();

        let egui_renderer = Renderer::new(device, output_color_format, RendererOptions::default());

        EguiRenderer {
            raw_input: egui::RawInput {
                focused: false, // winit will tell us when we have focus
                ..Default::default()
            },
            start_time: std::time::Instant::now(),
            egui_ctx,
            renderer: egui_renderer,
            frame_started: false,
        }
    }

    /// Sets the pixels per point (PPP) scale factor for egui.
    pub fn ppp(&mut self, v: f32) {
        self.context().set_pixels_per_point(v);
    }

    /// Calculate the `pixels_per_point` for a given window, given the current egui zoom factor
    pub fn pixels_per_point(&self, window: &dyn Window) -> f32 {
        let native_pixels_per_point = window.scale_factor() as f32;
        let egui_zoom_factor = self.egui_ctx.zoom_factor();
        egui_zoom_factor * native_pixels_per_point
    }

    pub fn begin_frame(&mut self, window: &dyn Window) {
        self.raw_input.time = Some(self.start_time.elapsed().as_secs_f64());
        let size = window.surface_size();
        let screen_size_in_pixels = egui::vec2(size.width as f32, size.height as f32);
        let screen_size_in_points = screen_size_in_pixels / self.pixels_per_point(window);

        self.raw_input.screen_rect = (screen_size_in_points.x > 0.0
            && screen_size_in_points.y > 0.0)
            .then(|| Rect::from_min_size(Pos2::ZERO, screen_size_in_points));

        let frame_input = self.raw_input.take();
        self.egui_ctx.begin_pass(frame_input);
        self.frame_started = true;
    }

    pub fn end_frame_and_draw(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        window: &dyn Window,
        window_surface_view: &TextureView,
        screen_descriptor: ScreenDescriptor,
    ) {
        if !self.frame_started {
            panic!("begin_frame must be called before end_frame_and_draw can be called!");
        }

        self.ppp(screen_descriptor.pixels_per_point);

        let full_output = self.egui_ctx.end_pass();

        // Hopefully not important
        // self.state
        //     .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .egui_ctx
            .tessellate(full_output.shapes, self.egui_ctx.pixels_per_point());
        for (id, image_delta) in &full_output.textures_delta.set {
            self.renderer
                .update_texture(device, queue, *id, image_delta);
        }
        self.renderer
            .update_buffers(device, queue, encoder, &tris, &screen_descriptor);
        let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: window_surface_view,
                depth_slice: None,
                resolve_target: None,
                ops: egui_wgpu::wgpu::Operations {
                    load: egui_wgpu::wgpu::LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            label: Some("egui main render pass"),
            occlusion_query_set: None,
        });

        self.renderer
            .render(&mut rpass.forget_lifetime(), &tris, &screen_descriptor);
        for x in &full_output.textures_delta.free {
            self.renderer.free_texture(x)
        }

        self.frame_started = false;
    }
}
