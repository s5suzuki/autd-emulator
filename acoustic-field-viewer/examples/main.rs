/*
 * File: main.rs
 * Project: examples
 * Created Date: 11/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 03/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::{f32::consts::PI, time::Instant};

use acoustic_field_viewer::{
    camera_helper,
    dir_viewer::{Axis3D, DirectionViewer},
    field_compute_pipeline::FieldComputePipeline,
    renderer::Renderer,
    slice_viewer::SliceViewer,
    sound_sources::{Drive, SoundSources},
    trans_viewer::TransViewer,
    Matrix4, UpdateFlag, Vector3, ViewerSettings,
};
use autd3_core::hardware_defined::{
    is_missing_transducer, NUM_TRANS_X, NUM_TRANS_Y, TRANS_SPACING_MM,
};
use imgui::{
    AngleSlider, Context, Drag, FontConfig, FontGlyphRanges, FontSource, Slider, TabBar, TabItem,
    Ui,
};
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use vulkano::{
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, SubpassContents},
    image::view::ImageView,
    sync::GpuFuture,
};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
};

const TRANS_SIZE: f32 = TRANS_SPACING_MM as _;
const WINDOW_WIDTH: f64 = 960.;
const WINDOW_HEIGHT: f64 = 640.;
const FOCAL_POS: Vector3 = [TRANS_SIZE * 8.5, TRANS_SIZE * 6.5, 150.];

struct App {
    is_running: bool,
    sources: SoundSources,
    axes: Vec<Axis3D>,
    focal_pos: Vector3,
    last_frame: Instant,
    field_compute_pipeline: FieldComputePipeline,
    slice_viewer: SliceViewer,
    trans_viewer: TransViewer,
    dir_viewer: DirectionViewer,
    viewer_settings: ViewerSettings,
    view_projection: (Matrix4, Matrix4),
    frame_count: usize,
    fps: f64,
}

impl App {
    pub fn new(renderer: &Renderer) -> Self {
        let viewer_settings = ViewerSettings::new();

        let mut sources = SoundSources::new();
        let zdir = [0., 0., 1.];
        for y in 0..NUM_TRANS_Y {
            for x in 0..NUM_TRANS_X {
                if is_missing_transducer(x, y) {
                    continue;
                }
                let pos = [TRANS_SIZE * x as f32, TRANS_SIZE * y as f32, 0.];
                sources.add(pos, zdir, Drive::new(1.0, 0.0, 1.0, 1.0));
            }
        }

        let axes = vec![Axis3D::new(
            [0., 0., 0.],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
        )];

        Self {
            is_running: true,
            sources,
            axes,
            focal_pos: FOCAL_POS,
            last_frame: Instant::now(),
            field_compute_pipeline: FieldComputePipeline::new(renderer.queue(), &viewer_settings),
            slice_viewer: SliceViewer::new(renderer, &viewer_settings),
            trans_viewer: TransViewer::new(renderer, &viewer_settings),
            dir_viewer: DirectionViewer::new(renderer, &viewer_settings),
            viewer_settings,
            view_projection: renderer.get_view_projection(&viewer_settings),
            frame_count: 0,
            fps: 0.0,
        }
    }

    fn reset(&mut self, render: &mut Renderer) {
        self.slice_viewer.move_to(self.viewer_settings.slice_pos);
        self.slice_viewer
            .rotate_to(self.viewer_settings.slice_angle);

        render.camera.position = self.viewer_settings.camera_pos;
        camera_helper::set_camera_angle(&mut render.camera, self.viewer_settings.camera_angle);

        self.focal_pos = FOCAL_POS;
        Self::calc_focus_phase(self.focal_pos, &mut self.sources, &self.viewer_settings);

        self.field_compute_pipeline
            .update(&self.sources, UpdateFlag::all(), &self.viewer_settings);

        let view_projection = render.get_view_projection(&self.viewer_settings);
        self.slice_viewer.update(
            render,
            &view_projection,
            &self.viewer_settings,
            UpdateFlag::all(),
        );
        self.trans_viewer.update(
            render,
            &view_projection,
            &self.viewer_settings,
            &self.sources,
            UpdateFlag::all(),
        );
        self.dir_viewer.update(
            render,
            &view_projection,
            &self.viewer_settings,
            &self.axes,
            UpdateFlag::all(),
        );

        self.view_projection = view_projection;
    }

    fn update_view(&mut self, renderer: &mut Renderer, update_flag: UpdateFlag) {
        self.trans_viewer.update(
            renderer,
            &self.view_projection,
            &self.viewer_settings,
            &self.sources,
            update_flag,
        );
        self.dir_viewer.update(
            renderer,
            &self.view_projection,
            &self.viewer_settings,
            &self.axes,
            update_flag,
        );
        self.slice_viewer.update(
            renderer,
            &self.view_projection,
            &self.viewer_settings,
            update_flag,
        );
        self.field_compute_pipeline
            .update(&self.sources, update_flag, &self.viewer_settings);
    }

    fn update_ui(&mut self, ui: &Ui, renderer: &mut Renderer) -> UpdateFlag {
        let mut update_flag = UpdateFlag::empty();
        ui.text(format!("fps: {:.1}", self.fps));
        TabBar::new("Settings").build(ui, || {
            TabItem::new("Focus").build(ui, || {
                ui.text("Focus position");
                if Drag::new("Pos X").build(ui, &mut self.focal_pos[0]) {
                    Self::calc_focus_phase(
                        self.focal_pos,
                        &mut self.sources,
                        &self.viewer_settings,
                    );
                    update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                }
                if Drag::new("Pos Y").build(ui, &mut self.focal_pos[1]) {
                    Self::calc_focus_phase(
                        self.focal_pos,
                        &mut self.sources,
                        &self.viewer_settings,
                    );
                    update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                }
                if Drag::new("Pos Z").build(ui, &mut self.focal_pos[2]) {
                    Self::calc_focus_phase(
                        self.focal_pos,
                        &mut self.sources,
                        &self.viewer_settings,
                    );

                    update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                }
                if Drag::new("Wavelength")
                    .range(0.0, f32::INFINITY)
                    .build(ui, &mut self.viewer_settings.wave_length)
                {
                    Self::calc_focus_phase(
                        self.focal_pos,
                        &mut self.sources,
                        &self.viewer_settings,
                    );
                    update_flag |= UpdateFlag::UPDATE_WAVENUM;
                }

                ui.separator();
                if Slider::new("Transducer alpha", 0.0, 1.0)
                    .build(ui, &mut self.viewer_settings.source_alpha)
                {
                    update_flag |= UpdateFlag::UPDATE_SOURCE_ALPHA;
                }
            });
            TabItem::new("Slice").build(ui, || {
                ui.text("Slice position");
                if Drag::new("Slice X").build(ui, &mut self.viewer_settings.slice_pos[0]) {
                    self.slice_viewer.move_to(self.viewer_settings.slice_pos);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                if Drag::new("Slice Y").build(ui, &mut self.viewer_settings.slice_pos[1]) {
                    self.slice_viewer.move_to(self.viewer_settings.slice_pos);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                if Drag::new("Slice Z").build(ui, &mut self.viewer_settings.slice_pos[2]) {
                    self.slice_viewer.move_to(self.viewer_settings.slice_pos);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                ui.separator();
                ui.text("Slice Rotation");
                if AngleSlider::new("Slice RX")
                    .range_degrees(0.0, 360.0)
                    .build(ui, &mut self.viewer_settings.slice_angle[0])
                {
                    self.slice_viewer
                        .rotate_to(self.viewer_settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                if AngleSlider::new("Slice RY")
                    .range_degrees(0.0, 360.0)
                    .build(ui, &mut self.viewer_settings.slice_angle[1])
                {
                    self.slice_viewer
                        .rotate_to(self.viewer_settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                if AngleSlider::new("Slice RZ")
                    .range_degrees(0.0, 360.0)
                    .build(ui, &mut self.viewer_settings.slice_angle[2])
                {
                    self.slice_viewer
                        .rotate_to(self.viewer_settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }

                ui.separator();
                ui.text("Slice color setting");
                if Slider::new("Color scale", 0.0, 10.0)
                    .build(ui, &mut self.viewer_settings.color_scale)
                {
                    update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
                }
                if Slider::new("Slice alpha", 0.0, 1.0)
                    .build(ui, &mut self.viewer_settings.slice_alpha)
                {
                    update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
                }

                ui.separator();
                if ui.small_button("xy") {
                    self.viewer_settings.slice_angle = [0., 0., 0.];
                    self.slice_viewer
                        .rotate_to(self.viewer_settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                ui.same_line();
                if ui.small_button("yz") {
                    self.viewer_settings.slice_angle = [0., -PI / 2., 0.];
                    self.slice_viewer
                        .rotate_to(self.viewer_settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                ui.same_line();
                if ui.small_button("zx") {
                    self.viewer_settings.slice_angle = [PI / 2., 0., 0.];
                    self.slice_viewer
                        .rotate_to(self.viewer_settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
            });
            TabItem::new("Camera").build(ui, || {
                ui.text("Camera pos");
                if Drag::new("Camera X").build(ui, &mut self.viewer_settings.camera_pos[0]) {
                    renderer.camera.position = self.viewer_settings.camera_pos;
                    self.view_projection = renderer.get_view_projection(&self.viewer_settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if Drag::new("Camera Y").build(ui, &mut self.viewer_settings.camera_pos[1]) {
                    renderer.camera.position = self.viewer_settings.camera_pos;
                    self.view_projection = renderer.get_view_projection(&self.viewer_settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if Drag::new("Camera Z").build(ui, &mut self.viewer_settings.camera_pos[2]) {
                    renderer.camera.position = self.viewer_settings.camera_pos;
                    self.view_projection = renderer.get_view_projection(&self.viewer_settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                ui.separator();
                ui.text("Camera rotation");
                if AngleSlider::new("Camera RX")
                    .range_degrees(-180.0, 180.0)
                    .build(ui, &mut self.viewer_settings.camera_angle[0])
                {
                    camera_helper::set_camera_angle(
                        &mut renderer.camera,
                        self.viewer_settings.camera_angle,
                    );
                    self.view_projection = renderer.get_view_projection(&self.viewer_settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if AngleSlider::new("Camera RY")
                    .range_degrees(-180.0, 180.0)
                    .build(ui, &mut self.viewer_settings.camera_angle[1])
                {
                    camera_helper::set_camera_angle(
                        &mut renderer.camera,
                        self.viewer_settings.camera_angle,
                    );
                    self.view_projection = renderer.get_view_projection(&self.viewer_settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if AngleSlider::new("Camera RZ")
                    .range_degrees(-180.0, 180.0)
                    .build(ui, &mut self.viewer_settings.camera_angle[2])
                {
                    camera_helper::set_camera_angle(
                        &mut renderer.camera,
                        self.viewer_settings.camera_angle,
                    );
                    self.view_projection = renderer.get_view_projection(&self.viewer_settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                ui.separator();
                ui.text("Camera perspective");
                if AngleSlider::new("FOV")
                    .range_degrees(0.0, 180.0)
                    .build(ui, &mut self.viewer_settings.fov)
                {
                    self.view_projection = renderer.get_view_projection(&self.viewer_settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if Drag::new("Near clip")
                    .range(0.0, f32::INFINITY)
                    .build(ui, &mut self.viewer_settings.near_clip)
                {
                    self.view_projection = renderer.get_view_projection(&self.viewer_settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if Drag::new("Far clip")
                    .range(0.0, f32::INFINITY)
                    .build(ui, &mut self.viewer_settings.far_clip)
                {
                    self.view_projection = renderer.get_view_projection(&self.viewer_settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
            });
        });

        if ui.small_button("toggle source visible") {
            for drive in self.sources.drives_mut() {
                drive.visible = if drive.visible == 0.0 { 1.0 } else { 0.0 };
            }
            update_flag |= UpdateFlag::UPDATE_SOURCE_FLAG;
        }
        ui.same_line();
        if ui.small_button("toggle source enable") {
            for drive in self.sources.drives_mut() {
                drive.enable = if drive.enable == 0.0 { 1.0 } else { 0.0 };
            }
            update_flag |= UpdateFlag::UPDATE_SOURCE_FLAG;
        }

        ui.separator();
        if ui.small_button("auto") {
            let rot = quaternion::euler_angles(
                self.viewer_settings.slice_angle[0],
                self.viewer_settings.slice_angle[1],
                self.viewer_settings.slice_angle[2],
            );
            let model = vecmath_util::mat4_rot(rot);

            let right = vecmath_util::to_vec3(&model[0]);
            let up = vecmath_util::to_vec3(&model[1]);
            let forward = vecmath::vec3_cross(right, up);

            let d = vecmath::vec3_scale(forward, 500.);
            let p = vecmath::vec3_add(vecmath_util::to_vec3(&self.viewer_settings.slice_pos), d);

            self.viewer_settings.camera_pos = p;
            renderer.camera.position = p;
            renderer.camera.right = right;
            renderer.camera.up = up;
            renderer
                .camera
                .look_at(vecmath_util::to_vec3(&self.viewer_settings.slice_pos));
            self.viewer_settings.camera_angle = camera_helper::rot_mat_to_euler_angles(&[
                renderer.camera.right,
                renderer.camera.up,
                renderer.camera.forward,
            ]);
            camera_helper::set_camera_angle(
                &mut renderer.camera,
                self.viewer_settings.camera_angle,
            );
            self.view_projection = renderer.get_view_projection(&self.viewer_settings);

            update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
        }

        ui.same_line();
        if ui.small_button("reset") {
            self.viewer_settings = ViewerSettings::default();
            self.reset(renderer);
            update_flag = UpdateFlag::all();
        }

        update_flag
    }

    pub fn render<F>(
        &mut self,
        renderer: &mut Renderer,
        imgui: &mut Context,
        platform: &mut WinitPlatform,
        imgui_renderer: &mut imgui_vulkano_renderer::Renderer,
        before_future: F,
    ) -> Box<dyn GpuFuture>
    where
        F: GpuFuture + 'static,
    {
        let framebuffer = renderer.frame_buffer();

        let mut builder = AutoCommandBufferBuilder::primary(
            renderer.device(),
            renderer.queue().family(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let clear_values = vec![[0.3, 0.3, 0.3, 1.0].into(), 1f32.into()];
        builder
            .begin_render_pass(framebuffer, SubpassContents::Inline, clear_values)
            .unwrap()
            .set_viewport(0, [renderer.viewport()]);

        self.dir_viewer.render(&mut builder);
        self.slice_viewer.render(&mut builder);
        self.trans_viewer.render(&mut builder);
        builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        let filed_image = self.slice_viewer.field_image_view();
        let after_compute = self
            .field_compute_pipeline
            .compute(
                filed_image,
                self.slice_viewer.model(),
                &self.sources,
                &self.viewer_settings,
            )
            .join(before_future);
        let slice_future = after_compute
            .then_execute(renderer.queue(), command_buffer)
            .unwrap();

        let io = imgui.io_mut();
        platform
            .prepare_frame(io, renderer.window())
            .expect("Failed to start frame");
        let ui = imgui.frame();
        {
            self.frame_count += 1;
            let now = std::time::Instant::now();
            let duration = now.saturating_duration_since(self.last_frame);
            if duration.as_millis() > 1000 {
                self.fps = 1000000.0 / duration.as_micros() as f64 * self.frame_count as f64;
                self.last_frame = now;
                self.frame_count = 0;
            }
        }
        let update_flag = self.update_ui(&ui, renderer);
        self.update_view(renderer, update_flag);

        let mut cmd_buf_builder = AutoCommandBufferBuilder::primary(
            renderer.device(),
            renderer.queue().family(),
            vulkano::command_buffer::CommandBufferUsage::OneTimeSubmit,
        )
        .expect("Failed to create command buffer");

        platform.prepare_render(&ui, renderer.window());
        let draw_data = ui.render();
        imgui_renderer
            .draw_commands(
                &mut cmd_buf_builder,
                renderer.queue(),
                ImageView::new(renderer.image()).unwrap(),
                draw_data,
            )
            .expect("Rendering failed");

        let cmd_buf = cmd_buf_builder
            .build()
            .expect("Failed to build command buffer");

        let ui_future = slice_future
            .then_execute(renderer.queue(), cmd_buf)
            .unwrap();

        ui_future.boxed()
    }

    fn is_running(&self) -> bool {
        self.is_running
    }

    fn calc_focus_phase(focal_pos: Vector3, sources: &mut SoundSources, settings: &ViewerSettings) {
        for (pos, drive) in sources.positions_drives_mut() {
            let d = vecmath_util::dist(vecmath_util::to_vec3(pos), focal_pos);
            let p = (d % settings.wave_length) / settings.wave_length;
            drive.phase = 2.0 * PI * (1.0 - p);
        }
    }
}

fn init_imgui(renderer: &Renderer) -> (Context, WinitPlatform, imgui_vulkano_renderer::Renderer) {
    let mut imgui = Context::create();

    let mut platform = WinitPlatform::init(&mut imgui);
    platform.attach_window(imgui.io_mut(), renderer.window(), HiDpiMode::Default);

    let hidpi_factor = platform.hidpi_factor();
    let font_size = (16.0 * hidpi_factor) as f32;
    imgui.fonts().add_font(&[FontSource::TtfData {
        data: include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"),
        size_pixels: font_size,
        config: Some(FontConfig {
            rasterizer_multiply: 1.,
            glyph_ranges: FontGlyphRanges::default(),
            ..FontConfig::default()
        }),
    }]);

    imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

    let renderer = imgui_vulkano_renderer::Renderer::init(
        &mut imgui,
        renderer.device(),
        renderer.queue(),
        vulkano::format::Format::B8G8R8A8_UNORM,
    )
    .expect("Failed to initialize renderer");
    (imgui, platform, renderer)
}

fn main() {
    let mut event_loop = EventLoop::new();
    let mut renderer = Renderer::new(&event_loop, "AFCv2", WINDOW_WIDTH, WINDOW_HEIGHT, true);

    let mut app = App::new(&renderer);
    app.reset(&mut renderer);

    let (mut imgui, mut platform, mut imgui_renderer) = init_imgui(&renderer);

    let mut is_running = true;
    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match &event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                is_running = false;
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(..) | WindowEvent::ScaleFactorChanged { .. },
                ..
            } => {
                renderer.resize();
                platform.handle_event(imgui.io_mut(), renderer.window(), &event);
            }
            Event::MainEventsCleared => {
                renderer.window().request_redraw();
                platform
                    .prepare_frame(imgui.io_mut(), renderer.window())
                    .expect("Failed to prepare frame");
            }
            Event::RedrawRequested(_) => {
                let before_pipeline_future = match renderer.start_frame() {
                    Err(e) => {
                        eprintln!("{}", e.to_string());
                        return;
                    }
                    Ok(future) => future,
                };
                let after_future = app.render(
                    &mut renderer,
                    &mut imgui,
                    &mut platform,
                    &mut imgui_renderer,
                    before_pipeline_future,
                );
                renderer.finish_frame(after_future);
            }
            event => {
                platform.handle_event(imgui.io_mut(), renderer.window(), event);
            }
        }
        if !app.is_running() {
            *control_flow = ControlFlow::Exit;
        }
    });
}
