/*
 * File: main.rs
 * Project: src
 * Created Date: 06/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 08/08/2022
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod interface;
mod server;
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
    Matrix4, UpdateFlag,
};

use autd3_core::{CPUControlFlags, Duty, Phase, FPGA_CLK_FREQ, NUM_TRANS_IN_UNIT};
use imgui::*;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use server::{AUTDEvent, AUTDServer};
use vulkano::{
    command_buffer::{
        AutoCommandBufferBuilder, CommandBufferUsage, RenderPassBeginInfo, SubpassContents,
    },
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
    stm_idx: i32,
    log_buf: VecDeque<String>,
    last_frame: Instant,
    last_frame_fps: Instant,
    frame_count: usize,
    fps: f64,
    save_image: bool,
    recording: bool,
    modulation: (Vec<u8>, u32),
    drives: Vec<Vec<([Duty; NUM_TRANS_IN_UNIT], [Phase; NUM_TRANS_IN_UNIT])>>,
    cycles: Vec<u16>,
    is_legacy_mode: bool,
    is_stm_mode: bool,
    is_gain_stm_mode: bool,
    is_force_fan: bool,
    stm_freq_div: u32,
    point_stm_sound_speed: u32,
    silencer_cycle: u16,
    silencer_step: u16,
    static_mod: f32,
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
            stm_idx: 0,
            log_buf: VecDeque::new(),
            last_frame: std::time::Instant::now(),
            last_frame_fps: std::time::Instant::now(),
            frame_count: 0,
            fps: 0.0,
            save_image: false,
            recording: false,
            modulation: (vec![], 0),
            drives: vec![],
            cycles: vec![],
            is_legacy_mode: true,
            is_stm_mode: false,
            is_gain_stm_mode: true,
            is_force_fan: false,
            stm_freq_div: 0,
            point_stm_sound_speed: 0,
            silencer_cycle: 0,
            silencer_step: 0,
            static_mod: 0.0,
        }
    }

    pub fn render<F>(
        &mut self,
        renderer: &mut Renderer,
        imgui: &mut Context,
        platform: &mut WinitPlatform,
        imgui_renderer: &mut imgui_vulkano_renderer::Renderer,
        autd_server: &mut AUTDServer,
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

        let clear_values = vec![
            Some(self.setting.viewer_setting.background.into()),
            Some(1f32.into()),
        ];
        builder
            .begin_render_pass(
                RenderPassBeginInfo {
                    clear_values,
                    ..RenderPassBeginInfo::framebuffer(framebuffer)
                },
                SubpassContents::Inline,
            )
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
            || update_flag.contains(UpdateFlag::UPDATE_SOURCE_FLAG);

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
                ImageView::new_default(renderer.image()).unwrap(),
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

    fn handle_autd(&mut self, autd_server: &mut AUTDServer) -> UpdateFlag {
        let mut update_flag = UpdateFlag::empty();
        autd_server.update(|event, emulator| match event {
            AUTDEvent::Geometries(geometries) => {
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
                    let frequencies = emulator
                        .fpga(i)
                        .cycles()
                        .map(|c| (FPGA_CLK_FREQ as f64 / c as f64) as f32);
                    for (j, (pos, dir)) in geometry.make_autd_transducers().iter().enumerate() {
                        let enable = if self.setting.enable[i] { 1.0 } else { 0.0 };
                        let visible = if self.setting.show[i] { 1.0 } else { 0.0 };
                        self.sources.add(
                            *pos,
                            *dir,
                            Drive::new(
                                0.0,
                                0.0,
                                enable,
                                frequencies[j],
                                self.setting.viewer_setting.sound_speed,
                            ),
                            visible,
                        );
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
                self.log("init geometry");
                update_flag |= UpdateFlag::INIT_SOURCE;
                update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                update_flag |= UpdateFlag::INIT_AXIS;
            }
            AUTDEvent::Clear => {
                self.sources
                    .drives_mut()
                    .zip(
                        emulator
                            .cpus()
                            .iter()
                            .flat_map(|cpu| cpu.fpga().drives()[0].0),
                    )
                    .zip(
                        emulator
                            .cpus()
                            .iter()
                            .flat_map(|cpu| cpu.fpga().drives()[0].1),
                    )
                    .zip(emulator.cpus().iter().flat_map(|cpu| cpu.fpga().cycles()))
                    .for_each(|(((drive, duty), phase), cycle)| {
                        drive.amp = duty.duty as f32 / cycle as f32;
                        drive.phase = 2.0 * PI * phase.phase as f32 / cycle as f32;
                        drive.set_wave_number(
                            FPGA_CLK_FREQ as f32 / cycle as f32,
                            self.setting.viewer_setting.sound_speed,
                        );
                    });
                self.modulation = emulator.fpga(0).modulation();
                self.log("clear");
                update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
            }
            AUTDEvent::RequestCpuVersion => {
                self.log("req cpu ver");
            }
            AUTDEvent::RequestFpgaVersion => {
                self.log("req fpga ver");
            }
            AUTDEvent::RequestFpgaFunctions => {
                self.log("req fpga functions");
            }
            AUTDEvent::Normal(flag) => {
                if emulator.cpus().is_empty() {
                    return;
                }

                if flag.contains(CPUControlFlags::MOD_END) {
                    self.modulation = emulator.fpga(0).modulation();
                    if !self.modulation.0.is_empty() {
                        let v = self.modulation.0[0];
                        if self.modulation.0.iter().all(|&m| m == v) {
                            self.static_mod = v as f32 / 255.0;
                        } else {
                            self.static_mod = 1.0;
                        }
                    } else {
                        self.static_mod = 1.0;
                    }
                }

                self.is_legacy_mode = emulator.cpu(0).fpga().is_legacy_mode();
                self.is_stm_mode = emulator.cpu(0).fpga().is_stm_mode();
                self.is_gain_stm_mode = emulator.cpu(0).fpga().is_stm_gain_mode();
                self.is_force_fan = emulator.cpu(0).fpga().is_force_fan();

                self.stm_freq_div = emulator.cpu(0).fpga().stm_frequency_division();
                self.silencer_cycle = emulator.cpu(0).fpga().silencer_cycle();
                self.silencer_step = emulator.cpu(0).fpga().silencer_step();
                self.point_stm_sound_speed = emulator.cpu(0).fpga().sound_speed();

                if !flag.contains(CPUControlFlags::CONFIG_EN_N)
                    && flag.contains(CPUControlFlags::CONFIG_SYNC)
                {
                    self.cycles = emulator
                        .cpus()
                        .iter()
                        .flat_map(|cpu| cpu.fpga().cycles())
                        .collect();
                }

                if self.is_stm_mode {
                    if flag.contains(CPUControlFlags::STM_END) {
                        self.drives = emulator
                            .cpus()
                            .iter()
                            .map(|cpu| cpu.fpga().drives())
                            .collect();
                    }
                    self.update_drive(0);
                } else {
                    self.drives = emulator
                        .cpus()
                        .iter()
                        .map(|cpu| cpu.fpga().drives())
                        .collect();
                    self.update_drive(0);
                }

                self.log("update drive");
                update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
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
                    if Drag::new("Sound speed")
                        .speed(0.1)
                        .range(0.0, f32::INFINITY)
                        .build(ui, &mut self.setting.viewer_setting.sound_speed)
                    {
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
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
                            for v in self
                                .sources
                                .visibilities_mut()
                                .skip(i * NUM_TRANS_IN_UNIT)
                                .take(NUM_TRANS_IN_UNIT)
                            {
                                *v = if self.setting.show[i] { 1.0 } else { 0.0 };
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

                    ui.separator();
                    ui.text("Silencer");
                    ui.text(format!("Cycle: {}", self.silencer_cycle));
                    let freq = FPGA_CLK_FREQ as f64 / self.silencer_cycle as f64;
                    ui.text(format!("Sampling Frequency: {} [Hz]", freq));
                    ui.text(format!("Step: {}", self.silencer_step));

                    let m = &self.modulation;
                    ui.separator();
                    ui.text("Modulation");
                    ui.text(format!("Size: {}", m.0.len()));
                    ui.text(format!("Frequency division: {}", m.1));
                    let sampling_freq = FPGA_CLK_FREQ as f64 / m.1 as f64;
                    ui.text(format!("Sampling frequency: {} [Hz]", sampling_freq));
                    let sampling_period = (1000000.0 * m.1 as f64 / FPGA_CLK_FREQ as f64) as usize;
                    ui.text(format!(
                        "Modulation sampling period: {} [us]",
                        sampling_period
                    ));
                    ui.text(format!(
                        "Modulation period: {} [us]",
                        sampling_period * m.0.len()
                    ));
                    if !m.0.is_empty() {
                        ui.text(format!("mod[0]: {}", m.0[0]));
                    }
                    if m.0.len() == 2 || m.0.len() == 3 {
                        ui.text(format!("mod[1]: {}", m.0[1]));
                    } else if m.0.len() > 3 {
                        ui.text("...");
                    }
                    if m.0.len() >= 3 {
                        let idx = m.0.len() - 1;
                        ui.text(format!("mod[{}]: {}", idx, m.0[idx]));
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

                    if self.is_stm_mode {
                        ui.separator();
                        if self.is_gain_stm_mode {
                            ui.text("GainSTM mode");
                        } else {
                            ui.text("PointSTM mode");
                            ui.text(format!(
                                "Sound speed: {} [mm/s]",
                                (self.point_stm_sound_speed * 1000) as f32 / 1024.0
                            ));
                        }
                        ui.text(format!("Size: {}", self.drives[0].len()));
                        ui.text(format!("Frequency division: {}", self.stm_freq_div));
                        let sampling_freq = FPGA_CLK_FREQ as f64 / self.stm_freq_div as f64;
                        ui.text(format!("Sampling frequency: {} [Hz]", sampling_freq));
                        let sampling_period = (1000000_usize * self.stm_freq_div as usize) as f64
                            / FPGA_CLK_FREQ as f64;
                        ui.text(format!("Sampling period: {} [us]", sampling_period));
                        ui.text(format!(
                            "Period: {} [us]",
                            sampling_period * self.drives[0].len() as f64
                        ));
                        if ui.input_int("Index", &mut self.stm_idx).build() {
                            if self.stm_idx >= self.drives[0].len() as _ {
                                self.stm_idx = 0;
                            }
                            if self.stm_idx < 0 {
                                self.stm_idx = self.drives[0].len() as i32 - 1;
                            }
                            self.update_drive(self.stm_idx as usize);

                            update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                        }
                        ui.text(format!(
                            "time: {} [us]",
                            sampling_period * self.stm_idx as f64
                        ));
                    }

                    ui.separator();
                    ui.text("FPGA flag");
                    let mut value = self.is_legacy_mode;
                    ui.checkbox("LEGACY MODE", &mut value);
                    let mut value = self.is_force_fan;
                    ui.checkbox("FORCE FAN", &mut value);
                    let mut value = self.is_stm_mode;
                    ui.checkbox("STM MODE", &mut value);
                    let mut value = self.is_gain_stm_mode;
                    ui.checkbox("STM GAIN MODE", &mut value);
                });
                TabItem::new("Log").build(ui, || {
                    if ui.radio_button_bool("enable", self.setting.log_enable) {
                        self.setting.log_enable = !self.setting.log_enable;
                    }
                    if ui.small_button("clear") {
                        self.log_clear();
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
        self.modulation.0.iter().map(f).collect()
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

    fn log_clear(&mut self) {
        self.log_buf.clear();
    }

    fn get_log_txt(&self) -> String {
        let mut log = String::new();
        for line in &self.log_buf {
            log.push_str(line);
            log.push('\n');
        }
        log
    }

    fn update_drive(&mut self, idx: usize) {
        self.sources
            .drives_mut()
            .zip(self.drives.iter().flat_map(|d| d[idx].0))
            .zip(self.drives.iter().flat_map(|d| d[idx].1))
            .zip(self.cycles.iter())
            .for_each(|(((drive, duty), phase), cycle)| {
                drive.amp = (PI * self.static_mod * duty.duty as f32 / *cycle as f32).sin();
                drive.phase = 2.0 * PI * (*cycle - phase.phase) as f32 / *cycle as f32;
                drive.set_wave_number(
                    FPGA_CLK_FREQ as f32 / *cycle as f32,
                    self.setting.viewer_setting.sound_speed,
                );
            });
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

    let mut autd_server = AUTDServer::new(&format!("127.0.0.1:{}", app.setting.port)).unwrap();

    let mut is_running = true;
    let mut last_frame = Instant::now();
    while is_running {
        event_loop.run_return(|event, _, control_flow| match &event {
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
                platform
                    .prepare_frame(imgui.io_mut(), renderer.window())
                    .expect("Failed to prepare frame");
                renderer.window().request_redraw();
            }
            Event::NewEvents(_) => {
                let now = Instant::now();
                imgui.io_mut().update_delta_time(now - last_frame);
                last_frame = now;
                renderer.window().request_redraw();
            }
            Event::RedrawRequested(_) => {
                let before_pipeline_future = match renderer.start_frame() {
                    Err(e) => {
                        eprintln!("{}", e);
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

                    use image::codecs::png::PngEncoder;
                    use image::ColorType;
                    use image::ImageEncoder;
                    use std::fs::File;

                    let width = app.setting.viewer_setting.slice_width
                        / app.setting.viewer_setting.slice_pixel_size;
                    let height = app.setting.viewer_setting.slice_height
                        / app.setting.viewer_setting.slice_pixel_size;
                    let pixels: Vec<_> = (&result[0..(width as usize * height as usize)])
                        .chunks_exact(width as _)
                        .rev()
                        .flatten()
                        .flat_map(|&c| vecmath_util::vec4_map(c, |v| (v * 255.0) as u8))
                        .collect();

                    if app.save_image {
                        let output = File::create(&app.setting.save_file_path).unwrap();
                        let encoder = PngEncoder::new(output);
                        encoder
                            .write_image(&pixels, width, height, ColorType::Rgba8)
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
                            .write_image(&pixels, width, height, ColorType::Rgba8)
                            .unwrap();
                    }
                }
            }
            event => {
                platform.handle_event(imgui.io_mut(), renderer.window(), event);
            }
        });
    }

    autd_server.close();

    app.setting.merge_render_sys(&renderer);
    app.setting.save("setting.json");
}
