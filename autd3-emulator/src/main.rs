/*
 * File: main.rs
 * Project: src
 * Created Date: 06/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 01/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

mod settings;

use std::{collections::VecDeque, f32::consts::PI, time::Instant};

use acoustic_field_viewer::{
    axis_3d::Axis3D,
    camera_helper,
    sound_source::{SoundSource, SourceFlag},
    view::{
        render_system::RenderSystem, AcousticFiledSliceViewer, DeviceDirectionViewer,
        SoundSourceViewer, System, UpdateFlag,
    },
    Matrix4, Vector3,
};
use autd3_core::hardware_defined::{
    CPUControlFlags, FPGAControlFlags, MOD_SAMPLING_FREQ_BASE, NUM_TRANS_IN_UNIT, SEQ_BASE_FREQ,
};
use autd3_emulator_server::{
    AutdData, AutdServer, DelayOffset, Gain, GainSequence, Modulation, PointSequence,
};
use gfx::Device;
use glutin::{
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    platform::run_return::EventLoopExtRunReturn,
};
use imgui::*;
use shader_version::OpenGL;

use crate::settings::Setting;

struct App {
    setting: Setting,
    sources: Vec<SoundSource>,
    axis: Vec<Axis3D>,
    dev_num: usize,
    sound_source_viewer: SoundSourceViewer,
    device_direction_viewer: DeviceDirectionViewer,
    field_slice_viewer: AcousticFiledSliceViewer,
    view_projection: (Matrix4, Matrix4),
    init: bool,
    fpga_flag: FPGAControlFlags,
    cpu_flag: CPUControlFlags,
    modulation: Option<Modulation>,
    point_sequence: Option<PointSequence>,
    gain_sequence: Option<GainSequence>,
    seq_idx: i32,
    seq_wavelength: f32,
    delay_offset: Option<DelayOffset>,
    log_buf: VecDeque<String>,
    last_frame_time: std::time::Instant,
    frame_count: usize,
    fps: f64,
}

impl App {
    pub fn new(setting: Setting, system: &System) -> Self {
        let opengl = OpenGL::V4_5;
        let sound_source_viewer = SoundSourceViewer::new(&system.render_sys, opengl);
        let field_slice_viewer =
            AcousticFiledSliceViewer::new(&system.render_sys, opengl, &setting.viewer_setting);
        let device_direction_viewer = DeviceDirectionViewer::new(&system.render_sys, opengl);
        let view_projection = system
            .render_sys
            .get_view_projection(&setting.viewer_setting);

        Self {
            setting,
            sources: Vec::new(),
            axis: Vec::new(),
            dev_num: 0,
            sound_source_viewer,
            device_direction_viewer,
            field_slice_viewer,
            view_projection,
            init: true,
            fpga_flag: FPGAControlFlags::empty(),
            cpu_flag: CPUControlFlags::empty(),
            modulation: None,
            point_sequence: None,
            gain_sequence: None,
            seq_idx: 0,
            seq_wavelength: 8.5,
            delay_offset: None,
            log_buf: VecDeque::new(),
            last_frame_time: std::time::Instant::now(),
            frame_count: 0,
            fps: 0.0,
        }
    }

    pub fn run(&mut self, system: System) {
        let System {
            mut events_loop,
            mut imgui,
            mut platform,
            mut render_sys,
            mut encoder,
            ..
        } = system;

        let mut autd_server = AutdServer::new(&format!("127.0.0.1:{}", self.setting.port)).unwrap();

        self.reset(&mut render_sys);

        let mut last_frame = Instant::now();
        let mut run = true;
        while run {
            events_loop.run_return(|event, _, control_flow| {
                self.handle_event(&mut render_sys, &event);
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

            let mut update_flag = self.handle_autd(&mut autd_server);
            update_flag |= self.update_camera(&mut render_sys, imgui.io());

            let io = imgui.io_mut();
            platform
                .prepare_frame(io, render_sys.window())
                .expect("Failed to start frame");
            let now = Instant::now();
            io.update_delta_time(now - last_frame);
            last_frame = now;
            let ui = imgui.frame();
            {
                self.frame_count += 1;
                let now = std::time::Instant::now();
                let duration = now.saturating_duration_since(self.last_frame_time);
                if duration.as_millis() > 1000 {
                    self.fps = 1000000.0 / duration.as_micros() as f64 * self.frame_count as f64;
                    self.last_frame_time = now;
                    self.frame_count = 0;
                }
            }
            update_flag |= self.update_ui(&ui, &mut render_sys);
            self.update_view(&mut render_sys, update_flag);

            encoder.clear(
                &render_sys.output_color,
                self.setting.viewer_setting.background,
            );
            encoder.clear_depth(&render_sys.output_stencil, 1.0);
            self.sound_source_viewer.renderer(&mut encoder);
            self.field_slice_viewer.renderer(&mut encoder);
            self.device_direction_viewer.renderer(&mut encoder);

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

        self.setting.merge_render_sys(&render_sys);
        self.setting.save("setting.json");
    }

    fn reset(&mut self, render_sys: &mut RenderSystem) {
        self.field_slice_viewer
            .move_to(self.setting.viewer_setting.slice_pos);
        self.field_slice_viewer
            .rotate_to(self.setting.viewer_setting.slice_angle);

        render_sys.camera.position = self.setting.viewer_setting.camera_pos;
        camera_helper::set_camera_angle(
            &mut render_sys.camera,
            self.setting.viewer_setting.camera_angle,
        );

        self.view_projection = render_sys.get_view_projection(&self.setting.viewer_setting);
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
                            for mut trans in geometry.make_autd_transducers() {
                                trans.flag.set(SourceFlag::HIDDEN, !self.setting.show[i]);
                                trans.flag.set(SourceFlag::DISABLE, !self.setting.enable[i]);
                                self.sources.push(trans);
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
                        for source in self.sources.iter_mut() {
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
                        for source in self.sources.iter_mut() {
                            source.flag.set(
                                SourceFlag::DISABLE,
                                !fpga_flag.contains(FPGAControlFlags::OUTPUT_ENABLE),
                            );
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

    fn handle_event(&mut self, render_sys: &mut RenderSystem, event: &Event<()>) {
        if self.init {
            self.update_view(render_sys, UpdateFlag::all());
            self.init = false;
        }
        self.sound_source_viewer.handle_event(render_sys, event);
        self.device_direction_viewer.handle_event(render_sys, event);
        self.field_slice_viewer.handle_event(render_sys, event);
    }

    fn update_view(&mut self, render_sys: &mut RenderSystem, update_flag: UpdateFlag) {
        self.sound_source_viewer.update(
            render_sys,
            self.view_projection,
            &self.setting.viewer_setting,
            &self.sources,
            update_flag,
        );
        self.device_direction_viewer.update(
            render_sys,
            self.view_projection,
            &self.setting.viewer_setting,
            &self.axis,
            update_flag,
        );
        self.field_slice_viewer.update(
            render_sys,
            self.view_projection,
            &self.setting.viewer_setting,
            &self.sources,
            update_flag,
        );
    }

    fn update_camera(&mut self, render_sys: &mut RenderSystem, io: &Io) -> UpdateFlag {
        let mut update_flag = UpdateFlag::empty();

        let mouse_wheel = io.mouse_wheel;
        if !io.want_capture_mouse && mouse_wheel != 0.0 {
            let trans = vecmath::vec3_scale(
                render_sys.camera.forward,
                -mouse_wheel * self.setting.camera_move_speed,
            );
            self.setting.viewer_setting.camera_pos =
                vecmath::vec3_add(self.setting.viewer_setting.camera_pos, trans);
            render_sys.camera.position = self.setting.viewer_setting.camera_pos;
            self.view_projection = render_sys.get_view_projection(&self.setting.viewer_setting);
            update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
        }
        let mouse_delta = io.mouse_delta;
        if !io.want_capture_mouse && io.mouse_down[0] && !vecmath_util::is_zero(&mouse_delta) {
            if io.key_shift {
                let mouse_delta =
                    vecmath::vec2_scale(mouse_delta, self.setting.camera_move_speed / 3000.0);
                let trans_x = vecmath::vec3_scale(render_sys.camera.right, mouse_delta[0]);
                let trans_y = vecmath::vec3_scale(render_sys.camera.up, -mouse_delta[1]);
                let to = vecmath::vec3_add(
                    vecmath::vec3_add(trans_x, trans_y),
                    render_sys.camera.forward,
                );
                let rot = vecmath_util::quaternion_to(render_sys.camera.forward, to);

                render_sys.camera.forward =
                    quaternion::rotate_vector(rot, render_sys.camera.forward);
                render_sys.camera.up = quaternion::rotate_vector(rot, render_sys.camera.up);
                render_sys.camera.right = quaternion::rotate_vector(rot, render_sys.camera.right);
                let rotm = [
                    render_sys.camera.right,
                    render_sys.camera.up,
                    render_sys.camera.forward,
                ];
                self.setting.viewer_setting.camera_angle =
                    camera_helper::rot_mat_to_euler_angles(&rotm);
            } else {
                let mouse_delta =
                    vecmath::vec2_scale(mouse_delta, self.setting.camera_move_speed / 10.0);
                let trans_x = vecmath::vec3_scale(render_sys.camera.right, -mouse_delta[0]);
                let trans_y = vecmath::vec3_scale(render_sys.camera.up, mouse_delta[1]);
                let trans = vecmath::vec3_add(trans_x, trans_y);
                self.setting.viewer_setting.camera_pos =
                    vecmath::vec3_add(self.setting.viewer_setting.camera_pos, trans);
                render_sys.camera.position = self.setting.viewer_setting.camera_pos;
            }
            self.view_projection = render_sys.get_view_projection(&self.setting.viewer_setting);
            update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
        }

        update_flag
    }

    fn update_ui(&mut self, ui: &Ui, render_sys: &mut RenderSystem) -> UpdateFlag {
        let mut update_flag = UpdateFlag::empty();
        Window::new("Controller").build(ui, || {
            TabBar::new("Settings").build(ui, || {
                TabItem::new("Slice").build(ui, || {
                    ui.text("Slice size");
                    if Slider::new("Slice width", 0, 1000)
                        .build(ui, &mut self.setting.viewer_setting.slice_width)
                    {
                        update_flag |= UpdateFlag::UPDATE_SLICE_SIZE;
                    }
                    if Slider::new("Slice heigh", 0, 1000)
                        .build(ui, &mut self.setting.viewer_setting.slice_height)
                    {
                        update_flag |= UpdateFlag::UPDATE_SLICE_SIZE;
                    }

                    ui.separator();
                    ui.text("Slice position");
                    if Drag::new("Slice X").build(ui, &mut self.setting.viewer_setting.slice_pos[0])
                    {
                        self.field_slice_viewer
                            .move_to(self.setting.viewer_setting.slice_pos);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    if Drag::new("Slice Y").build(ui, &mut self.setting.viewer_setting.slice_pos[1])
                    {
                        self.field_slice_viewer
                            .move_to(self.setting.viewer_setting.slice_pos);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    if Drag::new("Slice Z").build(ui, &mut self.setting.viewer_setting.slice_pos[2])
                    {
                        self.field_slice_viewer
                            .move_to(self.setting.viewer_setting.slice_pos);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }

                    ui.separator();
                    ui.text("Slice Rotation");
                    if AngleSlider::new("Slice RX")
                        .range_degrees(0.0, 360.0)
                        .build(ui, &mut self.setting.viewer_setting.slice_angle[0])
                    {
                        self.field_slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    if AngleSlider::new("Slice RY")
                        .range_degrees(0.0, 360.0)
                        .build(ui, &mut self.setting.viewer_setting.slice_angle[1])
                    {
                        self.field_slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    if AngleSlider::new("Slice RZ")
                        .range_degrees(0.0, 360.0)
                        .build(ui, &mut self.setting.viewer_setting.slice_angle[2])
                    {
                        self.field_slice_viewer
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
                        self.field_slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    ui.same_line();
                    if ui.small_button("yz") {
                        self.setting.viewer_setting.slice_angle = [0., -PI / 2., 0.];
                        self.field_slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                    ui.same_line();
                    if ui.small_button("zx") {
                        self.setting.viewer_setting.slice_angle = [PI / 2., 0., 0.];
                        self.field_slice_viewer
                            .rotate_to(self.setting.viewer_setting.slice_angle);
                        update_flag |= UpdateFlag::UPDATE_SLICE_POS;
                    }
                });
                TabItem::new("Camera").build(ui, || {
                    ui.text("Camera pos");
                    if Drag::new("Camera X")
                        .build(ui, &mut self.setting.viewer_setting.camera_pos[0])
                    {
                        render_sys.camera.position = self.setting.viewer_setting.camera_pos;
                        self.view_projection =
                            render_sys.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if Drag::new("Camera Y")
                        .build(ui, &mut self.setting.viewer_setting.camera_pos[1])
                    {
                        render_sys.camera.position = self.setting.viewer_setting.camera_pos;
                        self.view_projection =
                            render_sys.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if Drag::new("Camera Z")
                        .build(ui, &mut self.setting.viewer_setting.camera_pos[2])
                    {
                        render_sys.camera.position = self.setting.viewer_setting.camera_pos;
                        self.view_projection =
                            render_sys.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }

                    ui.separator();
                    ui.text("Camera rotation");
                    if AngleSlider::new("Camera RX")
                        .range_degrees(-180.0, 180.0)
                        .build(ui, &mut self.setting.viewer_setting.camera_angle[0])
                    {
                        camera_helper::set_camera_angle(
                            &mut render_sys.camera,
                            self.setting.viewer_setting.camera_angle,
                        );
                        self.view_projection =
                            render_sys.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if AngleSlider::new("Camera RY")
                        .range_degrees(-180.0, 180.0)
                        .build(ui, &mut self.setting.viewer_setting.camera_angle[1])
                    {
                        camera_helper::set_camera_angle(
                            &mut render_sys.camera,
                            self.setting.viewer_setting.camera_angle,
                        );
                        self.view_projection =
                            render_sys.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if AngleSlider::new("Camera RZ")
                        .range_degrees(-180.0, 180.0)
                        .build(ui, &mut self.setting.viewer_setting.camera_angle[2])
                    {
                        camera_helper::set_camera_angle(
                            &mut render_sys.camera,
                            self.setting.viewer_setting.camera_angle,
                        );
                        self.view_projection =
                            render_sys.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }

                    ui.separator();
                    Drag::new("camera speed")
                        .range(0.0, f32::INFINITY)
                        .speed(0.1)
                        .build(ui, &mut self.setting.camera_move_speed);

                    ui.separator();
                    ui.text("Camera perspective");
                    if AngleSlider::new("FOV")
                        .range_degrees(0.0, 180.0)
                        .build(ui, &mut self.setting.viewer_setting.fov)
                    {
                        self.view_projection =
                            render_sys.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if Drag::new("Near clip")
                        .range(0.0, f32::INFINITY)
                        .build(ui, &mut self.setting.viewer_setting.near_clip)
                    {
                        self.view_projection =
                            render_sys.get_view_projection(&self.setting.viewer_setting);
                        update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
                    }
                    if Drag::new("Far clip")
                        .range(0.0, f32::INFINITY)
                        .build(ui, &mut self.setting.viewer_setting.far_clip)
                    {
                        self.view_projection =
                            render_sys.get_view_projection(&self.setting.viewer_setting);
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
                            for j in (i * NUM_TRANS_IN_UNIT)..(i + 1) * NUM_TRANS_IN_UNIT {
                                self.sources[j]
                                    .flag
                                    .set(SourceFlag::HIDDEN, !self.setting.show[i]);
                            }
                            update_flag |= UpdateFlag::UPDATE_SOURCE_FLAG;
                        }
                        ui.same_line();
                        if ui.checkbox(&format!("enable##{}", i), &mut self.setting.enable[i]) {
                            for j in (i * NUM_TRANS_IN_UNIT)..(i + 1) * NUM_TRANS_IN_UNIT {
                                self.sources[j]
                                    .flag
                                    .set(SourceFlag::DISABLE, !self.setting.enable[i]);
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
                render_sys.camera.position = p;
                render_sys.camera.right = right;
                render_sys.camera.up = up;
                render_sys.camera.look_at(vecmath_util::to_vec3(
                    &self.setting.viewer_setting.slice_pos,
                ));
                self.setting.viewer_setting.camera_angle =
                    camera_helper::rot_mat_to_euler_angles(&[
                        render_sys.camera.right,
                        render_sys.camera.up,
                        render_sys.camera.forward,
                    ]);
                camera_helper::set_camera_angle(
                    &mut render_sys.camera,
                    self.setting.viewer_setting.camera_angle,
                );
                self.view_projection = render_sys.get_view_projection(&self.setting.viewer_setting);
                update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
            }

            ui.same_line();
            if ui.small_button("reset") {
                self.setting = Setting::load("setting.json");
                self.reset(render_sys);
                update_flag = UpdateFlag::all();
            }

            ui.same_line();
            if ui.small_button("default") {
                let default_setting = acoustic_field_viewer::view::ViewerSettings {
                    wave_length: self.setting.viewer_setting.wave_length,
                    ..Default::default()
                };
                self.setting.viewer_setting = default_setting;
                self.reset(render_sys);
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
            .zip(self.sources.iter_mut())
        {
            source.amp = (amp as f32 / 510.0 * std::f32::consts::PI).sin();
            source.phase = 2.0 * PI * (1.0 - (phase as f32 / 255.0));
        }
    }

    fn calc_focus(&mut self, duty: u8, focus: Vector3) {
        for source in self.sources.iter_mut() {
            source.amp = (duty as f32 / 510.0 * std::f32::consts::PI).sin();
            let dist = vecmath_util::dist(source.pos, focus);
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

pub fn main() {
    let setting = Setting::load("setting.json");
    let system = System::init(
        "AUTD3 emulator",
        setting.window_width as _,
        setting.window_height as _,
        setting.viewer_setting.vsync,
    );

    let mut app = App::new(setting, &system);
    app.run(system);
}
