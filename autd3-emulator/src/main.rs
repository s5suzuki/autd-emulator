/*
 * File: main.rs
 * Project: src
 * Created Date: 06/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 03/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

mod settings;

use std::{collections::VecDeque, f32::consts::PI, path::Path, time::Instant};

use acoustic_field_viewer::{
    camera_helper,
    dir_viewer::{Axis3D, DirectionViewer},
    field_compute_pipeline::FieldComputePipeline,
    renderer::Renderer,
    slice_viewer::SliceViewer,
    sound_sources::{Drive, SoundSources},
    trans_viewer::TransViewer,
    Matrix4, UpdateFlag, Vector3,
};
use autd3_core::hardware_defined::{
    CPUControlFlags, FPGAControlFlags, MOD_SAMPLING_FREQ_BASE, NUM_TRANS_IN_UNIT, SEQ_BASE_FREQ,
};
use autd3_emulator_server::{
    AutdData, AutdServer, DelayOffset, Gain, GainSequence, Modulation, PointSequence,
};

use imgui::*;
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

use crate::settings::Setting;

struct App {
    setting: Setting,
    sources: SoundSources,
    axis: Vec<Axis3D>,
    dev_num: usize,
    trans_viewer: TransViewer,
    dir_viewer: DirectionViewer,
    slice_viewer: SliceViewer,
    field_compute_pipeline: FieldComputePipeline,
    view_projection: (Matrix4, Matrix4),
    fpga_flag: FPGAControlFlags,
    cpu_flag: CPUControlFlags,
    modulation: Option<Modulation>,
    point_sequence: Option<PointSequence>,
    gain_sequence: Option<GainSequence>,
    seq_idx: i32,
    seq_wavelength: f32,
    delay_offset: Option<DelayOffset>,
    log_buf: VecDeque<String>,
    last_frame: Instant,
    last_frame_fps: Instant,
    frame_count: usize,
    fps: f64,
    save_image: bool,
    recording: bool,
}

impl App {
    pub fn new(setting: Setting, renderer: &Renderer) -> Self {
        let trans_viewer = TransViewer::new(renderer, &setting.viewer_setting);
        let slice_viewer = SliceViewer::new(renderer, &setting.viewer_setting);
        let dir_viewer = DirectionViewer::new(renderer, &setting.viewer_setting);
        let field_compute_pipeline =
            FieldComputePipeline::new(renderer.queue(), &setting.viewer_setting);
        let view_projection = renderer.get_view_projection(&setting.viewer_setting);

        Self {
            setting,
            sources: SoundSources::new(),
            axis: Vec::new(),
            dev_num: 0,
            trans_viewer,
            dir_viewer,
            slice_viewer,
            field_compute_pipeline,
            view_projection,
            fpga_flag: FPGAControlFlags::empty(),
            cpu_flag: CPUControlFlags::empty(),
            modulation: None,
            point_sequence: None,
            gain_sequence: None,
            seq_idx: 0,
            seq_wavelength: 8.5,
            delay_offset: None,
            log_buf: VecDeque::new(),
            last_frame: std::time::Instant::now(),
            last_frame_fps: std::time::Instant::now(),
            frame_count: 0,
            fps: 0.0,
            save_image: false,
            recording: false,
        }
    }

    pub fn render<F>(
        &mut self,
        renderer: &mut Renderer,
        imgui: &mut Context,
        platform: &mut WinitPlatform,
        imgui_renderer: &mut imgui_vulkano_renderer::Renderer,
        autd_server: &mut AutdServer,
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

        let clear_values = vec![self.setting.viewer_setting.background.into(), 1f32.into()];
        builder
            .begin_render_pass(framebuffer, SubpassContents::Inline, clear_values)
            .unwrap()
            .set_viewport(0, [renderer.viewport()]);

        self.trans_viewer.render(&mut builder);
        self.slice_viewer.render(&mut builder);
        self.dir_viewer.render(&mut builder);
        builder.end_render_pass().unwrap();
        let command_buffer = builder.build().unwrap();

        let mut update_flag = self.handle_autd(autd_server);
        update_flag |= self.update_camera(renderer, imgui.io());

        let io = imgui.io_mut();
        platform
            .prepare_frame(io, renderer.window())
            .expect("Failed to start frame");
        let now = Instant::now();
        io.update_delta_time(now - self.last_frame);
        self.last_frame = now;
        {
            self.frame_count += 1;
            let duration = now.saturating_duration_since(self.last_frame_fps);
            if duration.as_millis() > 1000 {
                self.fps = 1000000.0 / duration.as_micros() as f64 * self.frame_count as f64;
                self.last_frame_fps = now;
                self.frame_count = 0;
            }
        }

        let ui = imgui.frame();
        update_flag |= self.update_ui(&ui, renderer);
        self.update_view(renderer, update_flag);

        let update_field = update_flag.contains(UpdateFlag::INIT_SOURCE)
            || update_flag.contains(UpdateFlag::UPDATE_COLOR_MAP)
            || update_flag.contains(UpdateFlag::UPDATE_SLICE_POS)
            || update_flag.contains(UpdateFlag::UPDATE_SLICE_SIZE)
            || update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE)
            || update_flag.contains(UpdateFlag::UPDATE_SOURCE_FLAG)
            || update_flag.contains(UpdateFlag::UPDATE_WAVENUM);

        let filed_image = self.slice_viewer.field_image_view();
        let slice_future = if update_field {
            let after_compute = self
                .field_compute_pipeline
                .compute(
                    filed_image,
                    self.slice_viewer.model(),
                    &self.sources,
                    &self.setting.viewer_setting,
                )
                .join(before_future);
            after_compute
                .then_execute(renderer.queue(), command_buffer)
                .unwrap()
                .boxed()
        } else {
            before_future
                .then_execute(renderer.queue(), command_buffer)
                .unwrap()
                .boxed()
        };
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

    fn reset(&mut self, render: &mut Renderer) {
        self.slice_viewer
            .move_to(self.setting.viewer_setting.slice_pos);
        self.slice_viewer
            .rotate_to(self.setting.viewer_setting.slice_angle);

        render.camera.position = self.setting.viewer_setting.camera_pos;
        camera_helper::set_camera_angle(
            &mut render.camera,
            self.setting.viewer_setting.camera_angle,
        );

        self.field_compute_pipeline.update(
            &self.sources,
            UpdateFlag::all(),
            &self.setting.viewer_setting,
        );
        let view_projection = render.get_view_projection(&self.setting.viewer_setting);
        self.slice_viewer.update(
            render,
            &view_projection,
            &self.setting.viewer_setting,
            UpdateFlag::all(),
        );
        self.trans_viewer.update(
            render,
            &view_projection,
            &self.setting.viewer_setting,
            &self.sources,
            UpdateFlag::all(),
        );
        self.dir_viewer.update(
            render,
            &view_projection,
            &self.setting.viewer_setting,
            &self.axis,
            UpdateFlag::all(),
        );

        self.view_projection = view_projection;
    }

    fn handle_autd(&mut self, autd_server: &mut AutdServer) -> UpdateFlag {
        let mut update_flag = UpdateFlag::empty();
        autd_server.update(|data| {
            for d in data {
                match d {
                    AutdData::Geometries(geometries) => {
                        self.sources.clear();
                        self.axis.clear();
                        self.dev_num = geometries.len();
                        if self.setting.show.len() < self.dev_num {
                            self.setting.show.resize(self.dev_num, true);
                        }
                        if self.setting.enable.len() < self.dev_num {
                            self.setting.enable.resize(self.dev_num, true);
                        }
                        if self.setting.show_axis.len() < self.dev_num {
                            self.setting.show_axis.resize(self.dev_num, false);
                        }
                        for (i, geometry) in geometries.iter().enumerate() {
                            for (pos, dir) in geometry.make_autd_transducers() {
                                let enable = if self.setting.enable[i] { 1.0 } else { 0.0 };
                                let visible = if self.setting.show[i] { 1.0 } else { 0.0 };
                                self.sources
                                    .add(pos, dir, Drive::new(0.0, 0.0, enable, visible));
                            }
                            let mut axis = Axis3D::new(
                                geometry.origin,
                                geometry.right,
                                geometry.up,
                                vecmath::vec3_cross(geometry.right, geometry.up),
                            );
                            axis.show = self.setting.show_axis[i];
                            self.axis.push(axis);
                        }
                        self.log("geometry");
                        update_flag |= UpdateFlag::INIT_SOURCE;
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                        update_flag |= UpdateFlag::INIT_AXIS;
                    }
                    AutdData::Gain(gain) => {
                        self.set_gain(&gain);
                        self.log("gain");
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AutdData::Clear => {
                        for source in self.sources.drives_mut() {
                            source.amp = 0.;
                            source.phase = 0.;
                        }
                        self.modulation = None;
                        self.point_sequence = None;
                        self.gain_sequence = None;
                        self.delay_offset = None;
                        self.log("clear");
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AutdData::Modulation(m) => {
                        self.modulation = Some(m);
                        self.log("receive modulation");
                    }
                    AutdData::CtrlFlag(fpga_flag, cpu_flag) => {
                        self.fpga_flag = fpga_flag;
                        self.cpu_flag = cpu_flag;
                        for source in self.sources.drives_mut() {
                            source.enable = if fpga_flag.contains(FPGAControlFlags::OUTPUT_ENABLE) {
                                1.0
                            } else {
                                0.0
                            };
                        }
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AutdData::RequestFpgaVerMsb => {
                        self.log("req fpga ver msb");
                    }
                    AutdData::RequestFpgaVerLsb => {
                        self.log("req fpga ver lsb");
                    }
                    AutdData::RequestCpuVerMsb => {
                        self.log("req cpu ver lsb");
                    }
                    AutdData::RequestCpuVerLsb => {
                        self.log("req cpu ver lsb");
                    }
                    AutdData::PointSequence(seq) => {
                        let (focus, duty) = seq.seq_data[0];
                        self.seq_wavelength = seq.wavelength as f32 / 1000.0;
                        self.point_sequence = Some(seq);
                        self.gain_sequence = None;
                        self.seq_idx = 0;
                        self.calc_focus(duty, focus);
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AutdData::GainSequence(seq) => {
                        self.point_sequence = None;
                        self.set_gain(&seq.seq_data[0]);
                        self.gain_sequence = Some(seq);
                        self.seq_idx = 0;
                        self.log("receive gain sequence");
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AutdData::DelayOffset(d) => {
                        self.delay_offset = Some(d);
                        self.log("receive delay offset");
                    }
                }
            }
        });
        update_flag
    }

    fn update_view(&mut self, renderer: &mut Renderer, update_flag: UpdateFlag) {
        self.trans_viewer.update(
            renderer,
            &self.view_projection,
            &self.setting.viewer_setting,
            &self.sources,
            update_flag,
        );
        self.dir_viewer.update(
            renderer,
            &self.view_projection,
            &self.setting.viewer_setting,
            &self.axis,
            update_flag,
        );
        self.slice_viewer.update(
            renderer,
            &self.view_projection,
            &self.setting.viewer_setting,
            update_flag,
        );
        self.field_compute_pipeline.update(
            &self.sources,
            update_flag,
            &self.setting.viewer_setting,
        );
    }

    fn update_camera(&mut self, renderer: &mut Renderer, io: &Io) -> UpdateFlag {
        let mut update_flag = UpdateFlag::empty();

        let mouse_wheel = io.mouse_wheel;
        if !io.want_capture_mouse && mouse_wheel != 0.0 {
            let trans = vecmath::vec3_scale(
                renderer.camera.forward,
                -mouse_wheel * self.setting.viewer_setting.camera_move_speed,
            );
            self.setting.viewer_setting.camera_pos =
                vecmath::vec3_add(self.setting.viewer_setting.camera_pos, trans);
            renderer.camera.position = self.setting.viewer_setting.camera_pos;
            self.view_projection = renderer.get_view_projection(&self.setting.viewer_setting);
            update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
        }
        let mouse_delta = io.mouse_delta;
        if !io.want_capture_mouse && io.mouse_down[0] && !vecmath_util::is_zero(&mouse_delta) {
            if io.key_shift {
                let mouse_delta = vecmath::vec2_scale(
                    mouse_delta,
                    self.setting.viewer_setting.camera_move_speed / 3000.0,
                );
                let trans_x = vecmath::vec3_scale(renderer.camera.right, mouse_delta[0]);
                let trans_y = vecmath::vec3_scale(renderer.camera.up, -mouse_delta[1]);
                let to =
                    vecmath::vec3_add(vecmath::vec3_add(trans_x, trans_y), renderer.camera.forward);
                let rot = vecmath_util::quaternion_to(renderer.camera.forward, to);

                renderer.camera.forward = quaternion::rotate_vector(rot, renderer.camera.forward);
                renderer.camera.up = quaternion::rotate_vector(rot, renderer.camera.up);
                renderer.camera.right = quaternion::rotate_vector(rot, renderer.camera.right);
                let rotm = [
                    renderer.camera.right,
                    renderer.camera.up,
                    renderer.camera.forward,
                ];
                self.setting.viewer_setting.camera_angle =
                    camera_helper::rot_mat_to_euler_angles(&rotm);
            } else {
                let mouse_delta = vecmath::vec2_scale(
                    mouse_delta,
                    self.setting.viewer_setting.camera_move_speed / 10.0,
                );
                let trans_x = vecmath::vec3_scale(renderer.camera.right, -mouse_delta[0]);
                let trans_y = vecmath::vec3_scale(renderer.camera.up, mouse_delta[1]);
                let trans = vecmath::vec3_add(trans_x, trans_y);
                self.setting.viewer_setting.camera_pos =
                    vecmath::vec3_add(self.setting.viewer_setting.camera_pos, trans);
                renderer.camera.position = self.setting.viewer_setting.camera_pos;
            }
            self.view_projection = renderer.get_view_projection(&self.setting.viewer_setting);
            update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
        }

        update_flag
    }

    fn update_ui(&mut self, ui: &Ui, renderer: &mut Renderer) -> UpdateFlag {
        let mut update_flag = UpdateFlag::empty();
        self.save_image = false;
        Window::new("Controller").build(ui, || {
            TabBar::new("Settings").build(ui, || {
                TabItem::new("Slice").build(ui, || {
                    ui.text("Slice size");
                    if Slider::new("Slice width", 1, 1000)
                        .build(ui, &mut self.setting.viewer_setting.slice_width)
                    {
                        update_flag |= UpdateFlag::UPDATE_SLICE_SIZE;
                    }
                    if Slider::new("Slice heigh", 1, 1000)
                        .build(ui, &mut self.setting.viewer_setting.slice_height)
                    {
                        update_flag |= UpdateFlag::UPDATE_SLICE_SIZE;
                    }
                    if Slider::new("Pixel size", 1, 8)
                        .build(ui, &mut self.setting.viewer_setting.slice_pixel_size)
                    {
                        update_flag |= UpdateFlag::UPDATE_SLICE_SIZE;
                    }

                    ui.separator();
                    ui.text("Slice position");
                    if Drag::new("Slice X").build(ui, &mut self.setting.viewer_setting.slice_pos[0])
                    {
                        self.slice_viewer
                            .move_to(self.setting.viewer_setting.slice_pos);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    if Drag::new("Slice Y").build(ui, &mut self.setting.viewer_setting.slice_pos[1])
                    {
                        self.slice_viewer
                            .move_to(self.setting.viewer_setting.slice_pos);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    if Drag::new("Slice Z").build(ui, &mut self.setting.viewer_setting.slice_pos[2])
                    {
                        self.slice_viewer
                            .move_to(self.setting.viewer_setting.slice_pos);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }

                    ui.separator();
                    ui.text("Slice Rotation");
                    if AngleSlider::new("Slice RX")
                        .range_degrees(0.0, 360.0)
                        .build(ui, &mut self.setting.viewer_setting.slice_angle[0])
                    {
                        self.slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    if AngleSlider::new("Slice RY")
                        .range_degrees(0.0, 360.0)
                        .build(ui, &mut self.setting.viewer_setting.slice_angle[1])
                    {
                        self.slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    if AngleSlider::new("Slice RZ")
                        .range_degrees(0.0, 360.0)
                        .build(ui, &mut self.setting.viewer_setting.slice_angle[2])
                    {
                        self.slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }

                    ui.separator();
                    ui.text("Slice color setting");
                    if Drag::new("Color scale")
                        .speed(0.1)
                        .range(0.0, f32::INFINITY)
                        .build(ui, &mut self.setting.viewer_setting.color_scale)
                    {
                        update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
                    }
                    if Slider::new("Slice alpha", 0.0, 1.0)
                        .build(ui, &mut self.setting.viewer_setting.slice_alpha)
                    {
                        update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
                    }

                    ui.separator();
                    if ui.small_button("xy") {
                        self.setting.viewer_setting.slice_angle = [0., 0., 0.];
                        self.slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    ui.same_line();
                    if ui.small_button("yz") {
                        self.setting.viewer_setting.slice_angle = [0., -PI / 2., 0.];
                        self.slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    ui.same_line();
                    if ui.small_button("zx") {
                        self.setting.viewer_setting.slice_angle = [PI / 2., 0., 0.];
                        self.slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                });
                TabItem::new("Camera").build(ui, || {
                    ui.text("Camera pos");
                    if Drag::new("Camera X")
                        .build(ui, &mut self.setting.viewer_setting.camera_pos[0])
                    {
                        renderer.camera.position = self.setting.viewer_setting.camera_pos;
                        self.view_projection =
                            renderer.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if Drag::new("Camera Y")
                        .build(ui, &mut self.setting.viewer_setting.camera_pos[1])
                    {
                        renderer.camera.position = self.setting.viewer_setting.camera_pos;
                        self.view_projection =
                            renderer.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if Drag::new("Camera Z")
                        .build(ui, &mut self.setting.viewer_setting.camera_pos[2])
                    {
                        renderer.camera.position = self.setting.viewer_setting.camera_pos;
                        self.view_projection =
                            renderer.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }

                    ui.separator();
                    ui.text("Camera rotation");
                    if AngleSlider::new("Camera RX")
                        .range_degrees(-180.0, 180.0)
                        .build(ui, &mut self.setting.viewer_setting.camera_angle[0])
                    {
                        camera_helper::set_camera_angle(
                            &mut renderer.camera,
                            self.setting.viewer_setting.camera_angle,
                        );
                        self.view_projection =
                            renderer.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if AngleSlider::new("Camera RY")
                        .range_degrees(-180.0, 180.0)
                        .build(ui, &mut self.setting.viewer_setting.camera_angle[1])
                    {
                        camera_helper::set_camera_angle(
                            &mut renderer.camera,
                            self.setting.viewer_setting.camera_angle,
                        );
                        self.view_projection =
                            renderer.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if AngleSlider::new("Camera RZ")
                        .range_degrees(-180.0, 180.0)
                        .build(ui, &mut self.setting.viewer_setting.camera_angle[2])
                    {
                        camera_helper::set_camera_angle(
                            &mut renderer.camera,
                            self.setting.viewer_setting.camera_angle,
                        );
                        self.view_projection =
                            renderer.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }

                    ui.separator();
                    Drag::new("camera speed")
                        .range(0.0, f32::INFINITY)
                        .speed(0.1)
                        .build(ui, &mut self.setting.viewer_setting.camera_move_speed);

                    ui.separator();
                    ui.text("Camera perspective");
                    if AngleSlider::new("FOV")
                        .range_degrees(0.0, 180.0)
                        .build(ui, &mut self.setting.viewer_setting.fov)
                    {
                        self.view_projection =
                            renderer.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if Drag::new("Near clip")
                        .range(0.0, f32::INFINITY)
                        .build(ui, &mut self.setting.viewer_setting.near_clip)
                    {
                        self.view_projection =
                            renderer.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if Drag::new("Far clip")
                        .range(0.0, f32::INFINITY)
                        .build(ui, &mut self.setting.viewer_setting.far_clip)
                    {
                        self.view_projection =
                            renderer.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                });
                TabItem::new("Config").build(ui, || {
                    if Drag::new("Wavelength")
                        .speed(0.1)
                        .range(0.0, f32::INFINITY)
                        .build(ui, &mut self.setting.viewer_setting.wave_length)
                    {
                        update_flag |= UpdateFlag::UPDATE_WAVENUM;
                    }
                    ui.separator();
                    if Slider::new("Transducer alpha", 0.0, 1.0)
                        .build(ui, &mut self.setting.viewer_setting.source_alpha)
                    {
                        update_flag |= UpdateFlag::UPDATE_SOURCE_ALPHA;
                    }
                    ui.separator();
                    ui.text("Device index/show/enable/axis");
                    for i in 0..self.dev_num {
                        ui.text(format!("Device {}", i));
                        ui.same_line();
                        if ui.checkbox(&format!("show##{}", i), &mut self.setting.show[i]) {
                            for trans in self
                                .sources
                                .drives_mut()
                                .skip(i * NUM_TRANS_IN_UNIT)
                                .take(NUM_TRANS_IN_UNIT)
                            {
                                trans.visible = if self.setting.show[i] { 1.0 } else { 0.0 };
                            }
                            update_flag |= UpdateFlag::UPDATE_SOURCE_FLAG;
                        }
                        ui.same_line();
                        if ui.checkbox(&format!("enable##{}", i), &mut self.setting.enable[i]) {
                            for trans in self
                                .sources
                                .drives_mut()
                                .skip(i * NUM_TRANS_IN_UNIT)
                                .take(NUM_TRANS_IN_UNIT)
                            {
                                trans.enable = if self.setting.enable[i] { 1.0 } else { 0.0 };
                            }
                            update_flag |= UpdateFlag::UPDATE_SOURCE_FLAG;
                        }
                        ui.same_line();
                        if ui.checkbox(&format!("axis##{}", i), &mut self.setting.show_axis[i]) {
                            self.axis[i].show = self.setting.show_axis[i];
                            update_flag |= UpdateFlag::UPDATE_AXIS_FLAG;
                        }
                    }
                    if Drag::new("Axis length")
                        .speed(1.0)
                        .range(0.0, f32::INFINITY)
                        .build(ui, &mut self.setting.viewer_setting.axis_length)
                    {
                        update_flag |= UpdateFlag::UPDATE_AXIS_SIZE;
                    }
                    if Drag::new("Axis width")
                        .speed(0.1)
                        .range(0.0, f32::INFINITY)
                        .build(ui, &mut self.setting.viewer_setting.axis_width)
                    {
                        update_flag |= UpdateFlag::UPDATE_AXIS_SIZE;
                    }

                    ui.separator();
                    ColorPicker::new("Background", &mut self.setting.viewer_setting.background)
                        .alpha(true)
                        .build(ui);
                });
                TabItem::new("Info").build(ui, || {
                    ui.text(format!("fps: {:.1}", self.fps));

                    if let Some(m) = &self.modulation {
                        ui.separator();
                        ui.text("Modulation");
                        ui.text(format!("Modulation size: {}", m.mod_data.len()));
                        ui.text(format!("Modulation division: {}", m.mod_div));
                        let smpl_period =
                            (1000000.0 / MOD_SAMPLING_FREQ_BASE) as usize * m.mod_div as usize;
                        ui.text(format!("Modulation sampling period: {} [us]", smpl_period));
                        ui.text(format!(
                            "Modulation period: {} [us]",
                            smpl_period * m.mod_data.len()
                        ));
                        if !m.mod_data.is_empty() {
                            ui.text(format!("mod[0]: {}", m.mod_data[0]));
                        }
                        if m.mod_data.len() == 2 || m.mod_data.len() == 3 {
                            ui.text(format!("mod[1]: {}", m.mod_data[1]));
                        } else if m.mod_data.len() > 3 {
                            ui.text("...");
                        }
                        if m.mod_data.len() >= 3 {
                            let idx = m.mod_data.len() - 1;
                            ui.text(format!("mod[{}]: {}", idx, m.mod_data[idx]));
                        }

                        if ui.radio_button_bool("show mod plot", self.setting.show_mod_plot) {
                            self.setting.show_mod_plot = !self.setting.show_mod_plot;
                        }

                        if self.setting.show_mod_plot {
                            let mod_v = self.mod_values(|&v| ((v as f32) / 512.0 * PI).sin());
                            PlotLines::new(ui, "mod plot", &mod_v)
                                .graph_size(self.setting.mod_plot_size)
                                .build();
                            if ui.radio_button_bool(
                                "show mod plot (raw)",
                                self.setting.show_mod_plot_raw,
                            ) {
                                self.setting.show_mod_plot_raw = !self.setting.show_mod_plot_raw;
                            }
                            if self.setting.show_mod_plot_raw {
                                ui.separator();
                                let mod_v = self.mod_values(|&v| v as f32);
                                PlotLines::new(ui, "mod plot (raw)", &mod_v)
                                    .graph_size(self.setting.mod_plot_size)
                                    .build();
                            }

                            Drag::new("plot size")
                                .range(0.0, f32::INFINITY)
                                .build_array(ui, &mut self.setting.mod_plot_size);
                        }
                    }

                    if self.fpga_flag.contains(FPGAControlFlags::OP_MODE) {
                        ui.separator();
                        if let Some(seq) = &self.point_sequence {
                            ui.text("PointSequence mode");
                            ui.text(format!("Sequence size: {}", seq.seq_data.len()));
                            ui.text(format!("Sequence division: {}", seq.seq_div));
                            let smpl_period = (1000000 / SEQ_BASE_FREQ) * seq.seq_div as usize;
                            ui.text(format!("Sequence sampling period: {} [us]", smpl_period));
                            ui.text(format!(
                                "Sequence period: {} [us]",
                                smpl_period * seq.seq_data.len()
                            ));
                            if ui.input_int("Sequence idx", &mut self.seq_idx).build() {
                                if self.seq_idx >= seq.seq_data.len() as _ {
                                    self.seq_idx = 0;
                                }
                                if self.seq_idx < 0 {
                                    self.seq_idx = seq.seq_data.len() as i32 - 1;
                                }
                                let (focus, duty) = seq.seq_data[self.seq_idx as usize];
                                self.calc_focus(duty, focus);
                                update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                            }
                            ui.text(format!(
                                "time: {} [us]",
                                smpl_period * self.seq_idx as usize
                            ));
                        }
                        if let Some(seq) = self.gain_sequence.take() {
                            ui.text("GainSequence mode");
                            ui.text(format!(
                                "Gain mode: {}",
                                match seq.gain_mode {
                                    autd3_core::hardware_defined::GainMode::DutyPhaseFull =>
                                        "DutyPhaseFull",
                                    autd3_core::hardware_defined::GainMode::PhaseFull =>
                                        "PhaseFull",
                                    autd3_core::hardware_defined::GainMode::PhaseHalf =>
                                        "PhaseHalf",
                                }
                            ));
                            ui.text(format!("Sequence size: {}", seq.seq_data.len()));
                            ui.text(format!("Sequence division: {}", seq.seq_div));
                            let smpl_period = (1000000 / SEQ_BASE_FREQ) * seq.seq_div as usize;
                            ui.text(format!("Sequence sampling period: {} [us]", smpl_period));
                            ui.text(format!(
                                "Sequence period: {} [us]",
                                smpl_period * seq.seq_data.len()
                            ));
                            if ui.input_int("Sequence idx", &mut self.seq_idx).build() {
                                if self.seq_idx >= seq.seq_data.len() as _ {
                                    self.seq_idx = 0;
                                }
                                if self.seq_idx < 0 {
                                    self.seq_idx = seq.seq_data.len() as i32 - 1;
                                }
                                let idx = self.seq_idx as usize;
                                self.set_gain(&seq.seq_data[idx as usize]);
                                update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                            }
                            ui.text(format!(
                                "time: {} [us]",
                                smpl_period * self.seq_idx as usize
                            ));
                            self.gain_sequence = Some(seq);
                        }
                    }

                    if let Some(d) = &self.delay_offset {
                        ui.separator();
                        ui.text("Duty offset and Delay");
                        ui.text(format!(
                            "offset[0]: {}, delay[0]: {}",
                            d.delay_offset[0].1, d.delay_offset[0].0
                        ));
                        ui.text("...");
                        let idx = d.delay_offset.len() - 1;
                        ui.text(format!(
                            "offset[{0}]: {1}, delay[{0}]: {2}",
                            idx, d.delay_offset[idx].1, d.delay_offset[idx].0
                        ));
                    }

                    ui.separator();
                    ui.text("FPGA flag");
                    let mut flag = self.fpga_flag;
                    ui.checkbox_flags("OUTPUT ENABLE", &mut flag, FPGAControlFlags::OUTPUT_ENABLE);
                    ui.checkbox_flags(
                        "OUTPUT BALANCE",
                        &mut flag,
                        FPGAControlFlags::OUTPUT_BALANCE,
                    );
                    ui.checkbox_flags("SILENT", &mut flag, FPGAControlFlags::SILENT);
                    ui.checkbox_flags("FORCE FAN", &mut flag, FPGAControlFlags::FORCE_FAN);
                    ui.checkbox_flags("OP MODE", &mut flag, FPGAControlFlags::OP_MODE);
                    ui.checkbox_flags("SEQ MODE", &mut flag, FPGAControlFlags::SEQ_MODE);

                    ui.separator();
                    ui.text("CPU flag");
                    let mut flag = self.cpu_flag;
                    ui.checkbox_flags("MOD BEGIN", &mut flag, CPUControlFlags::MOD_BEGIN);
                    ui.checkbox_flags("MOD END", &mut flag, CPUControlFlags::MOD_END);
                    ui.checkbox_flags("SEQ BEGIN", &mut flag, CPUControlFlags::SEQ_BEGIN);
                    ui.checkbox_flags("SEQ END", &mut flag, CPUControlFlags::SEQ_END);
                    ui.checkbox_flags(
                        "READS FPGA INFO",
                        &mut flag,
                        CPUControlFlags::READS_FPGA_INFO,
                    );
                    ui.checkbox_flags("WRITE BODY", &mut flag, CPUControlFlags::WRITE_BODY);
                });
                TabItem::new("Log").build(ui, || {
                    if ui.radio_button_bool("enable", self.setting.log_enable) {
                        self.setting.log_enable = !self.setting.log_enable;
                    }
                    if self.setting.log_enable {
                        Slider::new("Max", 0, 1000).build(ui, &mut self.setting.log_max);

                        ui.text(self.get_log_txt());
                    }
                });
            });

            ui.separator();
            ui.text("Save as file");
            InputText::new(ui, "path to image", &mut self.setting.save_file_path).build();
            if ui.small_button("save") {
                self.save_image = true;
            }

            ui.separator();
            InputText::new(ui, "path to recorded images", &mut self.setting.record_path).build();
            if ui.small_button(if self.recording {
                "stop recording"
            } else {
                "record"
            }) {
                self.recording = !self.recording;
            }

            ui.separator();

            if ui.small_button("auto") {
                let rot = quaternion::euler_angles(
                    self.setting.viewer_setting.slice_angle[0],
                    self.setting.viewer_setting.slice_angle[1],
                    self.setting.viewer_setting.slice_angle[2],
                );
                let model = vecmath_util::mat4_rot(rot);

                let right = vecmath_util::to_vec3(&model[0]);
                let up = vecmath_util::to_vec3(&model[1]);
                let forward = vecmath::vec3_cross(right, up);

                let d = vecmath::vec3_scale(forward, 500.);
                let p = vecmath::vec3_add(
                    vecmath_util::to_vec3(&self.setting.viewer_setting.slice_pos),
                    d,
                );

                self.setting.viewer_setting.camera_pos = p;
                renderer.camera.position = p;
                renderer.camera.right = right;
                renderer.camera.up = up;
                renderer.camera.look_at(vecmath_util::to_vec3(
                    &self.setting.viewer_setting.slice_pos,
                ));
                self.setting.viewer_setting.camera_angle =
                    camera_helper::rot_mat_to_euler_angles(&[
                        renderer.camera.right,
                        renderer.camera.up,
                        renderer.camera.forward,
                    ]);
                camera_helper::set_camera_angle(
                    &mut renderer.camera,
                    self.setting.viewer_setting.camera_angle,
                );
                self.view_projection = renderer.get_view_projection(&self.setting.viewer_setting);
                update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
            }

            ui.same_line();
            if ui.small_button("reset") {
                let show = self.setting.show.to_owned();
                let enable = self.setting.enable.to_owned();
                let show_axis = self.setting.show_axis.to_owned();
                self.setting = Setting {
                    show,
                    enable,
                    show_axis,
                    ..Setting::load("setting.json")
                };
                self.reset(renderer);
                update_flag = UpdateFlag::all();
            }

            ui.same_line();
            if ui.small_button("default") {
                let viewer_setting = acoustic_field_viewer::ViewerSettings {
                    wave_length: self.setting.viewer_setting.wave_length,
                    vsync: self.setting.viewer_setting.vsync,
                    ..Default::default()
                };
                let show = self.setting.show.to_owned();
                let enable = self.setting.enable.to_owned();
                let show_axis = self.setting.show_axis.to_owned();
                let port = self.setting.port;
                let window_width = self.setting.window_width;
                let window_height = self.setting.window_height;
                self.setting = Setting {
                    port,
                    window_width,
                    window_height,
                    viewer_setting,
                    show,
                    enable,
                    show_axis,
                    ..Setting::new()
                };
                self.reset(renderer);
                update_flag = UpdateFlag::all();
            }
        });

        update_flag
    }

    fn mod_values<F>(&self, f: F) -> Vec<f32>
    where
        F: Fn(&u8) -> f32,
    {
        if let Some(m) = &self.modulation {
            m.mod_data.iter().map(f).collect()
        } else {
            vec![]
        }
    }

    fn set_gain(&mut self, gain: &Gain) {
        for ((&phase, &amp), source) in gain
            .phases
            .iter()
            .zip(gain.amps.iter())
            .zip(self.sources.drives_mut())
        {
            source.amp = (amp as f32 / 510.0 * std::f32::consts::PI).sin();
            source.phase = 2.0 * PI * (1.0 - (phase as f32 / 255.0));
        }
    }

    fn calc_focus(&mut self, duty: u8, focus: Vector3) {
        for (pos, source) in self.sources.positions_drives_mut() {
            source.amp = (duty as f32 / 510.0 * std::f32::consts::PI).sin();
            let dist = vecmath_util::dist(vecmath_util::to_vec3(pos), focus);
            let phase = (dist / self.seq_wavelength) % 1.0;
            source.phase = 2.0 * PI * (1.0 - phase);
        }
    }

    // TODO: This log system is not so efficient
    fn log(&mut self, msg: &str) {
        if self.setting.log_enable {
            let date = chrono::Local::now();
            self.log_buf
                .push_back(format!("{}: {}", date.format("%Y-%m-%d %H:%M:%S.%3f"), msg));
            while self.log_buf.len() > self.setting.log_max as usize {
                self.log_buf.pop_front();
            }
        }
    }

    fn get_log_txt(&self) -> String {
        let mut log = String::new();
        for line in &self.log_buf {
            log.push_str(line);
            log.push('\n');
        }
        log
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
pub fn main() {
    let setting = Setting::load("setting.json");

    let mut event_loop = EventLoop::new();
    let mut renderer = Renderer::new(
        &event_loop,
        "AUTD3 emulator",
        setting.window_width as _,
        setting.window_height as _,
        setting.viewer_setting.vsync,
    );

    let mut app = App::new(setting, &renderer);
    app.reset(&mut renderer);

    let (mut imgui, mut platform, mut imgui_renderer) = init_imgui(&renderer);

    let mut autd_server = AutdServer::new(&format!("127.0.0.1:{}", app.setting.port)).unwrap();

    let mut is_running = true;
    while is_running {
        event_loop.run_return(|event, _, control_flow| {
            *control_flow = ControlFlow::Exit;
            platform.handle_event(imgui.io_mut(), renderer.window(), &event);
            match &event {
                Event::WindowEvent {
                    event: WindowEvent::CloseRequested,
                    ..
                } => {
                    is_running = false;
                }
                Event::WindowEvent {
                    event: WindowEvent::Resized(..) | WindowEvent::ScaleFactorChanged { .. },
                    ..
                } => {
                    renderer.resize();
                }
                _ => (),
            }

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
                &mut autd_server,
                before_pipeline_future,
            );
            renderer.finish_frame(after_future);

            if app.save_image || app.recording {
                let image = app.slice_viewer.field_image_view();
                let result = image.read().unwrap();

                use image::png::PngEncoder;
                use image::ColorType;
                use std::fs::File;

                let width = app.setting.viewer_setting.slice_width
                    / app.setting.viewer_setting.slice_pixel_size;
                let height = app.setting.viewer_setting.slice_height
                    / app.setting.viewer_setting.slice_pixel_size;
                let pixels: Vec<_> = (&result[0..(width as usize * height as usize)])
                    .chunks_exact(width as _)
                    .rev()
                    .flatten()
                    .map(|&c| vecmath_util::vec4_map(c, |v| (v * 255.0) as u8))
                    .flatten()
                    .collect();

                if app.save_image {
                    let output = File::create(&app.setting.save_file_path).unwrap();
                    let encoder = PngEncoder::new(output);
                    encoder
                        .encode(&pixels, width, height, ColorType::Rgba8)
                        .unwrap();
                }

                if app.recording {
                    std::fs::create_dir_all(&app.setting.record_path).unwrap();
                    let date = chrono::Local::now();
                    let path = Path::new(&app.setting.record_path)
                        .join(format!("{}", date.format("%Y-%m-%d_%H-%M-%S_%3f.png")));
                    let output = File::create(path).unwrap();
                    let encoder = PngEncoder::new(output);
                    encoder
                        .encode(&pixels, width, height, ColorType::Rgba8)
                        .unwrap();
                }
            }
        });
    }

    app.setting.merge_render_sys(&renderer);
    app.setting.save("setting.json");
}
