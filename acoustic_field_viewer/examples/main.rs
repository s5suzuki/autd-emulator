/*
 * File: main.rs
 * Project: examples
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 22/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use std::{f32::consts::PI, time::Instant};

use acoustic_field_viewer::{
    camera_helper,
    sound_source::SoundSource,
    view::{
        render_system::RenderSystem, AcousticFiledSliceViewer, SoundSourceViewer, System,
        UpdateFlag, ViewerSettings,
    },
    Matrix4, Vector3,
};
use autd3_core::hardware_defined::{
    is_missing_transducer, NUM_TRANS_X, NUM_TRANS_Y, TRANS_SPACING_MM,
};
use gfx::Device;
use glutin::{
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    platform::run_return::EventLoopExtRunReturn,
};
use imgui::*;
use shader_version::OpenGL;

const TRANS_SIZE: f32 = TRANS_SPACING_MM as _;
const WINDOW_WIDTH: f64 = 960.;
const WINDOW_HEIGHT: f64 = 640.;
const FOCAL_POS: Vector3 = [TRANS_SIZE * 8.5, TRANS_SIZE * 6.5, 150.];

struct App {
    settings: ViewerSettings,
    sources: Vec<SoundSource>,
    sound_source_viewer: SoundSourceViewer,
    field_slice_viewer: AcousticFiledSliceViewer,
    view_projection: (Matrix4, Matrix4),
    focal_pos: Vector3,
    init: bool,
}

impl App {
    pub fn new(system: &System) -> Self {
        let settings = ViewerSettings::default();

        let mut sources = Vec::new();
        let zdir = [0., 0., 1.];
        for y in 0..NUM_TRANS_Y {
            for x in 0..NUM_TRANS_X {
                if is_missing_transducer(x, y) {
                    continue;
                }
                let pos = [TRANS_SIZE * x as f32, TRANS_SIZE * y as f32, 0.];
                sources.push(SoundSource::new(pos, zdir, 1.0, 0.0));
            }
        }

        let opengl = OpenGL::V4_5;
        let sound_source_viewer = SoundSourceViewer::new(&system.render_sys, opengl);
        let field_slice_viewer =
            AcousticFiledSliceViewer::new(&system.render_sys, opengl, &settings);

        Self {
            settings,
            sources,
            sound_source_viewer,
            field_slice_viewer,
            view_projection: system.render_sys.get_view_projection(&settings),
            focal_pos: FOCAL_POS,
            init: true,
        }
    }

    fn reset(&mut self, render_sys: &mut RenderSystem) {
        self.field_slice_viewer.move_to(self.settings.slice_pos);
        self.field_slice_viewer.rotate_to(self.settings.slice_angle);

        render_sys.camera.position = self.settings.camera_pos;
        camera_helper::set_camera_angle(&mut render_sys.camera, self.settings.camera_angle);

        self.focal_pos = FOCAL_POS;
        Self::calc_focus_phase(self.focal_pos, &mut self.sources, &self.settings);

        self.view_projection = render_sys.get_view_projection(&self.settings);
    }

    fn update_view(&mut self, render_sys: &mut RenderSystem, update_flag: UpdateFlag) {
        self.sound_source_viewer.update(
            render_sys,
            self.view_projection,
            &self.settings,
            &self.sources,
            update_flag,
        );
        self.field_slice_viewer.update(
            render_sys,
            self.view_projection,
            &self.settings,
            &self.sources,
            update_flag,
        );
    }

    fn handle_event(&mut self, render_sys: &mut RenderSystem, event: &Event<()>) {
        if self.init {
            self.update_view(render_sys, UpdateFlag::all());
            self.init = false;
        }
        self.sound_source_viewer.handle_event(&render_sys, event);
        self.field_slice_viewer.handle_event(&render_sys, event);
    }

    fn update_ui(&mut self, ui: &Ui, render_sys: &mut RenderSystem) -> UpdateFlag {
        let mut update_flag = UpdateFlag::empty();
        TabBar::new(im_str!("Settings")).build(&ui, || {
            TabItem::new(im_str!("Focus")).build(&ui, || {
                ui.text(im_str!("Focus position"));
                if Drag::new(im_str!("Pos X")).build(&ui, &mut self.focal_pos[0]) {
                    Self::calc_focus_phase(self.focal_pos, &mut self.sources, &self.settings);
                    update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                }
                if Drag::new(im_str!("Pos Y")).build(&ui, &mut self.focal_pos[1]) {
                    Self::calc_focus_phase(self.focal_pos, &mut self.sources, &self.settings);
                    update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                }
                if Drag::new(im_str!("Pos Z")).build(&ui, &mut self.focal_pos[2]) {
                    Self::calc_focus_phase(self.focal_pos, &mut self.sources, &self.settings);

                    update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                }
                if Drag::new(im_str!("Wavelength"))
                    .range(0.0..=f32::INFINITY)
                    .build(&ui, &mut self.settings.wave_length)
                {
                    Self::calc_focus_phase(self.focal_pos, &mut self.sources, &self.settings);
                    update_flag |= UpdateFlag::UPDATE_WAVENUM;
                }

                ui.separator();
                if Slider::new(im_str!("Transducer alpha"))
                    .range(0.0..=1.0)
                    .build(&ui, &mut self.settings.source_alpha)
                {
                    update_flag |= UpdateFlag::UPDATE_SOURCE_ALPHA;
                }
            });
            TabItem::new(im_str!("Slice")).build(&ui, || {
                ui.text(im_str!("Slice position"));
                if Drag::new(im_str!("Slice X")).build(&ui, &mut self.settings.slice_pos[0]) {
                    self.field_slice_viewer.move_to(self.settings.slice_pos);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                if Drag::new(im_str!("Slice Y")).build(&ui, &mut self.settings.slice_pos[1]) {
                    self.field_slice_viewer.move_to(self.settings.slice_pos);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                if Drag::new(im_str!("Slice Z")).build(&ui, &mut self.settings.slice_pos[2]) {
                    self.field_slice_viewer.move_to(self.settings.slice_pos);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                ui.separator();
                ui.text(im_str!("Slice Rotation"));
                if AngleSlider::new(im_str!("Slice RX"))
                    .range_degrees(0.0..=360.0)
                    .build(&ui, &mut self.settings.slice_angle[0])
                {
                    self.field_slice_viewer.rotate_to(self.settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                if AngleSlider::new(im_str!("Slice RY"))
                    .range_degrees(0.0..=360.0)
                    .build(&ui, &mut self.settings.slice_angle[1])
                {
                    self.field_slice_viewer.rotate_to(self.settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                if AngleSlider::new(im_str!("Slice RZ"))
                    .range_degrees(0.0..=360.0)
                    .build(&ui, &mut self.settings.slice_angle[2])
                {
                    self.field_slice_viewer.rotate_to(self.settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }

                ui.separator();
                ui.text(im_str!("Slice color setting"));
                if Slider::new(im_str!("Color scale"))
                    .range(0.0..=10.0)
                    .build(&ui, &mut self.settings.color_scale)
                {
                    update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
                }
                if Slider::new(im_str!("Slice alpha"))
                    .range(0.0..=1.0)
                    .build(&ui, &mut self.settings.slice_alpha)
                {
                    update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
                }

                ui.separator();
                if ui.small_button(im_str!("xy")) {
                    self.settings.slice_angle = [0., 0., 0.];
                    self.field_slice_viewer.rotate_to(self.settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                ui.same_line(0.);
                if ui.small_button(im_str!("yz")) {
                    self.settings.slice_angle = [0., -PI / 2., 0.];
                    self.field_slice_viewer.rotate_to(self.settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
                ui.same_line(0.);
                if ui.small_button(im_str!("zx")) {
                    self.settings.slice_angle = [PI / 2., 0., 0.];
                    self.field_slice_viewer.rotate_to(self.settings.slice_angle);
                    update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                }
            });
            TabItem::new(im_str!("Camera")).build(&ui, || {
                ui.text(im_str!("Camera pos"));
                if Drag::new(im_str!("Camera X")).build(&ui, &mut self.settings.camera_pos[0]) {
                    render_sys.camera.position = self.settings.camera_pos;
                    self.view_projection = render_sys.get_view_projection(&self.settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if Drag::new(im_str!("Camera Y")).build(&ui, &mut self.settings.camera_pos[1]) {
                    render_sys.camera.position = self.settings.camera_pos;
                    self.view_projection = render_sys.get_view_projection(&self.settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if Drag::new(im_str!("Camera Z")).build(&ui, &mut self.settings.camera_pos[2]) {
                    render_sys.camera.position = self.settings.camera_pos;
                    self.view_projection = render_sys.get_view_projection(&self.settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                ui.separator();
                ui.text(im_str!("Camera rotation"));
                if AngleSlider::new(im_str!("Camera RX"))
                    .range_degrees(-180.0..=180.0)
                    .build(&ui, &mut self.settings.camera_angle[0])
                {
                    camera_helper::set_camera_angle(
                        &mut render_sys.camera,
                        self.settings.camera_angle,
                    );
                    self.view_projection = render_sys.get_view_projection(&self.settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if AngleSlider::new(im_str!("Camera RY"))
                    .range_degrees(-180.0..=180.0)
                    .build(&ui, &mut self.settings.camera_angle[1])
                {
                    camera_helper::set_camera_angle(
                        &mut render_sys.camera,
                        self.settings.camera_angle,
                    );
                    self.view_projection = render_sys.get_view_projection(&self.settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if AngleSlider::new(im_str!("Camera RZ"))
                    .range_degrees(-180.0..=180.0)
                    .build(&ui, &mut self.settings.camera_angle[2])
                {
                    camera_helper::set_camera_angle(
                        &mut render_sys.camera,
                        self.settings.camera_angle,
                    );
                    self.view_projection = render_sys.get_view_projection(&self.settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                ui.separator();
                ui.text(im_str!("Camera perspective"));
                if AngleSlider::new(im_str!("FOV"))
                    .range_degrees(0.0..=180.0)
                    .build(&ui, &mut self.settings.fov)
                {
                    self.view_projection = render_sys.get_view_projection(&self.settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if Drag::new(im_str!("Near clip"))
                    .range(0.0..=f32::INFINITY)
                    .build(&ui, &mut self.settings.near_clip)
                {
                    self.view_projection = render_sys.get_view_projection(&self.settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
                if Drag::new(im_str!("Far clip"))
                    .range(0.0..=f32::INFINITY)
                    .build(&ui, &mut self.settings.far_clip)
                {
                    self.view_projection = render_sys.get_view_projection(&self.settings);
                    update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                }
            });
        });

        ui.separator();
        if ui.small_button(im_str!("auto")) {
            let rot = quaternion::euler_angles(
                self.settings.slice_angle[0],
                self.settings.slice_angle[1],
                self.settings.slice_angle[2],
            );
            let model = vecmath_util::mat4_rot(rot);

            let right = vecmath_util::to_vec3(&model[0]);
            let up = vecmath_util::to_vec3(&model[1]);
            let forward = vecmath::vec3_cross(right, up);

            let d = vecmath::vec3_scale(forward, 500.);
            let p = vecmath::vec3_add(vecmath_util::to_vec3(&self.settings.slice_pos), d);

            self.settings.camera_pos = p;
            render_sys.camera.position = p;
            render_sys.camera.right = right;
            render_sys.camera.up = up;
            render_sys
                .camera
                .look_at(vecmath_util::to_vec3(&self.settings.slice_pos));
            self.settings.camera_angle = camera_helper::rot_mat_to_euler_angles(&[
                render_sys.camera.right,
                render_sys.camera.up,
                render_sys.camera.forward,
            ]);
            camera_helper::set_camera_angle(&mut render_sys.camera, self.settings.camera_angle);
            self.view_projection = render_sys.get_view_projection(&self.settings);

            update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
        }

        ui.same_line(0.);
        if ui.small_button(im_str!("reset")) {
            self.settings = ViewerSettings::default();
            self.reset(render_sys);
            update_flag = UpdateFlag::all();
        }

        update_flag
    }

    pub fn main_loop(&mut self, system: System) {
        let System {
            mut events_loop,
            mut imgui,
            mut platform,
            mut render_sys,
            mut encoder,
            ..
        } = system;

        self.reset(&mut render_sys);

        let mut last_frame = Instant::now();
        let mut run = true;
        while run {
            events_loop.run_return(|event, _, control_flow| {
                self.handle_event(&mut render_sys, &event);
                render_sys.update_views();
                platform.handle_event(imgui.io_mut(), render_sys.window(), &event);
                if let Event::WindowEvent { event, .. } = event {
                    match event {
                        WindowEvent::Resized(_) => render_sys.update_views(),
                        WindowEvent::CloseRequested => {
                            run = false;
                        }
                        _ => (),
                    }
                }
                *control_flow = ControlFlow::Exit;
            });
            if !run {
                break;
            }

            let io = imgui.io_mut();
            platform
                .prepare_frame(io, render_sys.window())
                .expect("Failed to start frame");
            let now = Instant::now();
            io.update_delta_time(now - last_frame);
            last_frame = now;
            let ui = imgui.frame();

            let update_flag = self.update_ui(&ui, &mut render_sys);
            self.update_view(&mut render_sys, update_flag);

            encoder.clear(&render_sys.output_color, [0.3, 0.3, 0.3, 1.0]);
            encoder.clear_depth(&render_sys.output_stencil, 1.0);
            self.sound_source_viewer.renderer(&mut encoder);
            self.field_slice_viewer.renderer(&mut encoder);

            platform.prepare_render(&ui, render_sys.window());
            let draw_data = ui.render();
            render_sys
                .renderer
                .render(
                    &mut render_sys.factory,
                    &mut encoder,
                    &mut render_sys.output_color,
                    draw_data,
                )
                .expect("Rendering failed");
            encoder.flush(&mut render_sys.device);
            render_sys.swap_buffers();
            render_sys.device.cleanup();
        }
    }

    fn calc_focus_phase(
        focal_pos: Vector3,
        sources: &mut [SoundSource],
        settings: &ViewerSettings,
    ) {
        for source in sources.iter_mut() {
            let pos = source.pos;
            let d = vecmath_util::dist(pos, focal_pos);
            let phase = (d % settings.wave_length) / settings.wave_length;
            source.phase = 2.0 * PI * phase;
        }
    }
}

pub fn main() {
    let system = System::init("example", WINDOW_WIDTH, WINDOW_HEIGHT, true);
    let mut app = App::new(&system);
    app.main_loop(system);
}
