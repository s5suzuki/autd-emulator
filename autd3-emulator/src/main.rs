/*
 * File: main.rs
 * Project: src
 * Created Date: 06/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 10/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

mod settings;

use std::{collections::VecDeque, f32::consts::PI, time::Instant};

use acoustic_field_viewer::{
    view::{AcousticFiledSliceViewer, SoundSourceViewer, System, UpdateFlag},
    Matrix3, Vector3,
};
use autd3_core::hardware_defined::{
    RxGlobalControlFlags, MOD_SAMPLING_FREQ_BASE, POINT_SEQ_BASE_FREQ,
};
use autd3_emulator_server::{AUTDData, AUTDServer, DelayOffset, Modulation, Sequence};
use camera_controllers::Camera;
use gfx::Device;
use glutin::{
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    platform::run_return::EventLoopExtRunReturn,
};
use imgui::*;
use shader_version::OpenGL;

use crate::settings::Setting;

fn set_camera_angle(camera: &mut Camera<f32>, angle: Vector3) {
    let rot = quaternion::euler_angles(angle[0], angle[1], angle[2]);
    let model = vecmath_util::mat4_rot(rot);
    camera.right = vecmath_util::to_vec3(&model[0]);
    camera.up = vecmath_util::to_vec3(&model[1]);
    camera.forward = vecmath_util::to_vec3(&model[2]);
}

fn rot_mat_to_euler_angles(mat: &Matrix3) -> Vector3 {
    let sy = (mat[0][0] * mat[0][0] + mat[1][0] * mat[1][0]).sqrt();
    if sy < 1e-3 {
        let x = (mat[1][1]).atan2(mat[1][2]);
        let y = (mat[2][0]).atan2(sy);
        let z = 0.;
        [x, y, z]
    } else {
        let x = (-mat[2][1]).atan2(mat[2][2]);
        let y = (-mat[2][0]).atan2(sy);
        let z = (mat[1][0]).atan2(mat[0][0]);
        [x, y, z]
    }
}

// TODO: This log system is not so efficient
fn log(log_buf: &mut VecDeque<String>, msg: &str, setting: &Setting) {
    if setting.log_enable {
        let date = chrono::Local::now();
        log_buf.push_back(format!("{}: {}", date.format("%Y-%m-%d %H:%M:%S.%3f"), msg));
        while log_buf.len() > setting.log_max as usize {
            log_buf.pop_front();
        }
    }
}
fn get_log_txt(log_buf: &VecDeque<String>) -> String {
    let mut log = String::new();
    for line in log_buf {
        log.push_str(line);
        log.push('\n');
    }
    log
}

pub fn main() {
    let mut setting = Setting::load("setting.json");
    let init_setting = setting;

    let mut autd_server = AUTDServer::new(&format!("127.0.0.1:{}", setting.port)).unwrap();

    let mut sources = Vec::new();
    let mut last_amp = Vec::new();

    let mut log_buf = VecDeque::new();

    let System {
        mut events_loop,
        mut imgui,
        mut platform,
        mut render_sys,
        mut encoder,
        ..
    } = System::init(
        "AUTD3 emulator",
        setting.window_width as _,
        setting.window_height as _,
    );

    let opengl = OpenGL::V4_5;
    let mut sound_source_viewer = SoundSourceViewer::new(&render_sys, opengl);
    let mut field_slice_viewer =
        AcousticFiledSliceViewer::new(&render_sys, opengl, &setting.viewer_setting);
    field_slice_viewer.move_to(setting.viewer_setting.slice_pos);
    field_slice_viewer.rotate_to(setting.viewer_setting.slice_angle);

    render_sys.camera.position = setting.viewer_setting.camera_pos;
    set_camera_angle(&mut render_sys.camera, setting.viewer_setting.camera_angle);

    let mut view_projection = render_sys.get_view_projection(&setting.viewer_setting);
    let mut last_frame = Instant::now();
    let mut run = true;
    let mut init = true;
    let mut ctrl_flag = RxGlobalControlFlags::empty();
    let mut modulation: Option<Modulation> = None;
    let mut sequence: Option<Sequence> = None;
    let mut delay_offset: Option<DelayOffset> = None;
    while run {
        events_loop.run_return(|event, _, control_flow| {
            if init {
                sound_source_viewer.update(
                    &mut render_sys,
                    view_projection,
                    &setting.viewer_setting,
                    &sources,
                    UpdateFlag::all(),
                );
                field_slice_viewer.update(
                    &mut render_sys,
                    view_projection,
                    &setting.viewer_setting,
                    &sources,
                    UpdateFlag::all(),
                );
                render_sys.update_views();
                init = false;
            }

            sound_source_viewer.handle_event(&render_sys, &event);
            field_slice_viewer.handle_event(&render_sys, &event);
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

        let mut update_flag = UpdateFlag::empty();

        autd_server.update(|data| {
            for d in data {
                match d {
                    AUTDData::Geometries(geometries) => {
                        sources.clear();
                        for geometry in geometries {
                            for trans in geometry.make_autd_transducers() {
                                sources.push(trans);
                            }
                        }
                        log(&mut log_buf, "geometry", &setting);
                        update_flag |= UpdateFlag::UPDATE_SOURCE_POS;
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AUTDData::Gain(gain) => {
                        for ((&phase, &amp), source) in gain
                            .phases
                            .iter()
                            .zip(gain.amps.iter())
                            .zip(sources.iter_mut())
                        {
                            source.amp = (amp as f32 / 510.0 * std::f32::consts::PI).sin();
                            source.phase = 2.0 * PI * (1.0 - (phase as f32 / 255.0));
                        }
                        log(&mut log_buf, "gain", &setting);
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AUTDData::Clear => {
                        for source in sources.iter_mut() {
                            source.amp = 0.;
                            source.phase = 0.;
                        }
                        modulation = None;
                        sequence = None;
                        delay_offset = None;
                        log(&mut log_buf, "clear", &setting);
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AUTDData::Pause => {
                        last_amp.clear();
                        for source in sources.iter_mut() {
                            last_amp.push(source.amp);
                            source.amp = 0.;
                        }
                        log(&mut log_buf, "pause", &setting);
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AUTDData::Resume => {
                        for (source, &amp) in sources.iter_mut().zip(last_amp.iter()) {
                            source.amp = amp;
                        }
                        last_amp.clear();
                        log(&mut log_buf, "resume", &setting);
                        update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
                    }
                    AUTDData::Modulation(m) => {
                        modulation = Some(m);
                        log(&mut log_buf, "receive modulation", &setting);
                    }
                    AUTDData::CtrlFlag(flag) => {
                        ctrl_flag = flag;
                    }
                    AUTDData::RequestFPGAVerMSB => {
                        log(&mut log_buf, "req fpga ver msb", &setting);
                    }
                    AUTDData::RequestFPGAVerLSB => {
                        log(&mut log_buf, "req fpga ver lsb", &setting);
                    }
                    AUTDData::RequestCPUVerMSB => {
                        log(&mut log_buf, "req cpu ver lsb", &setting);
                    }
                    AUTDData::RequestCPUVerLSB => {
                        log(&mut log_buf, "req cpu ver lsb", &setting);
                    }
                    AUTDData::Sequence(seq) => {
                        sequence = Some(seq);
                        log(&mut log_buf, "receive sequence", &setting);
                    }
                    AUTDData::DelayOffset(d) => {
                        delay_offset = Some(d);
                        log(&mut log_buf, "receive delay offset", &setting);
                    }
                }
            }
        });

        let io = imgui.io_mut();
        platform
            .prepare_frame(io, render_sys.window())
            .expect("Failed to start frame");
        let now = Instant::now();
        io.update_delta_time(now - last_frame);
        last_frame = now;
        let ui = imgui.frame();

        let mut slice_size_changed = false;
        let mut slice_geo_changed = false;
        let mut rot_changed = false;
        let mut color_changed = false;
        let mut camera_update = false;
        TabBar::new(im_str!("Settings")).build(&ui, || {
            TabItem::new(im_str!("Slice")).build(&ui, || {
                ui.text(im_str!("Slice size"));
                slice_size_changed = Slider::new(im_str!("Slice width"))
                    .range(0..=1000)
                    .build(&ui, &mut setting.viewer_setting.slice_width);
                slice_size_changed |= Slider::new(im_str!("Slice heigh"))
                    .range(0..=1000)
                    .build(&ui, &mut setting.viewer_setting.slice_height);

                ui.separator();
                ui.text(im_str!("Slice position"));
                slice_geo_changed = Drag::new(im_str!("Slice X"))
                    .build(&ui, &mut setting.viewer_setting.slice_pos[0]);
                slice_geo_changed |= Drag::new(im_str!("Slice Y"))
                    .build(&ui, &mut setting.viewer_setting.slice_pos[1]);
                slice_geo_changed |= Drag::new(im_str!("Slice Z"))
                    .build(&ui, &mut setting.viewer_setting.slice_pos[2]);

                ui.separator();
                ui.text(im_str!("Slice Rotation"));
                rot_changed = AngleSlider::new(im_str!("Slice RX"))
                    .range_degrees(0.0..=360.0)
                    .build(&ui, &mut setting.viewer_setting.slice_angle[0]);
                rot_changed |= AngleSlider::new(im_str!("Slice RY"))
                    .range_degrees(0.0..=360.0)
                    .build(&ui, &mut setting.viewer_setting.slice_angle[1]);
                rot_changed |= AngleSlider::new(im_str!("Slice RZ"))
                    .range_degrees(0.0..=360.0)
                    .build(&ui, &mut setting.viewer_setting.slice_angle[2]);

                ui.separator();
                ui.text(im_str!("Slice color setting"));
                color_changed = Drag::new(im_str!("Color scale"))
                    .speed(0.1)
                    .build(&ui, &mut setting.viewer_setting.color_scale);
                color_changed |= Slider::new(im_str!("Slice alpha"))
                    .range(0.0..=1.0)
                    .build(&ui, &mut setting.viewer_setting.slice_alpha);

                ui.separator();
                if ui.small_button(im_str!("xy")) {
                    setting.viewer_setting.slice_angle = [0., 0., 0.];
                    rot_changed = true;
                }
                ui.same_line(0.);
                if ui.small_button(im_str!("yz")) {
                    setting.viewer_setting.slice_angle = [0., -PI / 2., 0.];
                    rot_changed = true;
                }
                ui.same_line(0.);
                if ui.small_button(im_str!("zx")) {
                    setting.viewer_setting.slice_angle = [PI / 2., 0., 0.];
                    rot_changed = true;
                }
            });
            TabItem::new(im_str!("Camera")).build(&ui, || {
                ui.text(im_str!("Camera pos"));
                camera_update = Drag::new(im_str!("Camera X"))
                    .build(&ui, &mut setting.viewer_setting.camera_pos[0]);
                camera_update |= Drag::new(im_str!("Camera Y"))
                    .build(&ui, &mut setting.viewer_setting.camera_pos[1]);
                camera_update |= Drag::new(im_str!("Camera Z"))
                    .build(&ui, &mut setting.viewer_setting.camera_pos[2]);
                ui.separator();
                ui.text(im_str!("Camera rotation"));
                camera_update |= AngleSlider::new(im_str!("Camera RX"))
                    .range_degrees(-180.0..=180.0)
                    .build(&ui, &mut setting.viewer_setting.camera_angle[0]);
                camera_update |= AngleSlider::new(im_str!("Camera RY"))
                    .range_degrees(-180.0..=180.0)
                    .build(&ui, &mut setting.viewer_setting.camera_angle[1]);
                camera_update |= AngleSlider::new(im_str!("Camera RZ"))
                    .range_degrees(-180.0..=180.0)
                    .build(&ui, &mut setting.viewer_setting.camera_angle[2]);
                ui.separator();
                ui.text(im_str!("Camera perspective"));
                camera_update |= AngleSlider::new(im_str!("FOV"))
                    .range_degrees(0.0..=180.0)
                    .build(&ui, &mut setting.viewer_setting.fov);
                camera_update |= Drag::new(im_str!("Near clip"))
                    .range(0.0..=f32::INFINITY)
                    .build(&ui, &mut setting.viewer_setting.near_clip);
                camera_update |= Drag::new(im_str!("Far clip"))
                    .range(0.0..=f32::INFINITY)
                    .build(&ui, &mut setting.viewer_setting.far_clip);
            });
            TabItem::new(im_str!("Config")).build(&ui, || {
                if Drag::new(im_str!("Wavelength"))
                    .speed(0.1)
                    .range(0.0..=f32::INFINITY)
                    .build(&ui, &mut setting.viewer_setting.wave_length)
                {
                    update_flag |= UpdateFlag::UPDATE_WAVENUM;
                }
                ui.separator();
                if Slider::new(im_str!("Transducer alpha"))
                    .range(0.0..=1.0)
                    .build(&ui, &mut setting.viewer_setting.source_alpha)
                {
                    update_flag |= UpdateFlag::UPDATE_SOURCE_ALPHA;
                }
                ui.separator();
                if ColorPicker::new(im_str!("Background"), &mut setting.background)
                    .alpha(true)
                    .build(&ui)
                {}
            });
            TabItem::new(im_str!("Info")).build(&ui, || {
                ui.text("Control flag");
                let mut flag = ctrl_flag;
                ui.checkbox_flags(
                    im_str!("MOD BEGIN"),
                    &mut flag,
                    RxGlobalControlFlags::MOD_BEGIN,
                );
                ui.checkbox_flags(im_str!("MOD END"), &mut flag, RxGlobalControlFlags::MOD_END);
                ui.checkbox_flags(
                    im_str!("MOD END"),
                    &mut flag,
                    RxGlobalControlFlags::READ_FPGA_INFO,
                );
                ui.checkbox_flags(im_str!("SILENT"), &mut flag, RxGlobalControlFlags::SILENT);
                ui.checkbox_flags(
                    im_str!("FORCE FAN"),
                    &mut flag,
                    RxGlobalControlFlags::FORCE_FAN,
                );
                ui.checkbox_flags(
                    im_str!("SEQ MODE"),
                    &mut flag,
                    RxGlobalControlFlags::SEQ_MODE,
                );
                ui.checkbox_flags(
                    im_str!("SEQ BEGIN"),
                    &mut flag,
                    RxGlobalControlFlags::SEQ_BEGIN,
                );
                ui.checkbox_flags(im_str!("SEQ END"), &mut flag, RxGlobalControlFlags::SEQ_END);

                if let Some(m) = &modulation {
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
                }

                if ctrl_flag.contains(RxGlobalControlFlags::SEQ_MODE) {
                    ui.separator();
                    ui.text("Sequence mode");
                    if let Some(seq) = &sequence {
                        ui.text(format!("Sequence size: {}", seq.seq_data.len()));
                        ui.text(format!("Sequence division: {}", seq.seq_div));
                        let smpl_period = (1000000 / POINT_SEQ_BASE_FREQ) * seq.seq_div as usize;
                        ui.text(format!("Sequence sampling period: {} [us]", smpl_period));
                        ui.text(format!(
                            "Sequence period: {} [us]",
                            smpl_period * seq.seq_data.len()
                        ));
                        if !seq.seq_data.is_empty() {
                            ui.text(format!(
                                "seq[0]: {:?} / {}",
                                seq.seq_data[0].0, seq.seq_data[0].1
                            ));
                        }
                        if seq.seq_data.len() == 2 || seq.seq_data.len() == 3 {
                            ui.text(format!(
                                "seq[1]: {:?} / {}",
                                seq.seq_data[1].0, seq.seq_data[1].1
                            ));
                        } else if seq.seq_data.len() > 3 {
                            ui.text("...");
                        }
                        if seq.seq_data.len() >= 3 {
                            let idx = seq.seq_data.len() - 1;
                            ui.text(format!(
                                "seq[{}]: {:?} / {}",
                                idx, seq.seq_data[idx].0, seq.seq_data[idx].1
                            ));
                        }
                    }
                }

                if let Some(d) = &delay_offset {
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
            });
            TabItem::new(im_str!("Log")).build(&ui, || {
                if ui.radio_button_bool(im_str!("enable"), setting.log_enable) {
                    setting.log_enable = !setting.log_enable;
                }
                Slider::new(im_str!("Max"))
                    .range(0..=1000)
                    .build(&ui, &mut setting.log_max);
                ui.text(get_log_txt(&log_buf));
            });
        });

        ui.separator();

        if ui.small_button(im_str!("auto")) {
            let rot = quaternion::euler_angles(
                setting.viewer_setting.slice_angle[0],
                setting.viewer_setting.slice_angle[1],
                setting.viewer_setting.slice_angle[2],
            );
            let model = vecmath_util::mat4_rot(rot);

            let right = vecmath_util::to_vec3(&model[0]);
            let up = vecmath_util::to_vec3(&model[1]);
            let forward = vecmath::vec3_cross(right, up);

            let d = vecmath::vec3_scale(forward, 500.);
            let p = vecmath::vec3_add(vecmath_util::to_vec3(&setting.viewer_setting.slice_pos), d);

            setting.viewer_setting.camera_pos = p;
            render_sys.camera.position = p;
            render_sys.camera.right = right;
            render_sys.camera.up = up;
            render_sys
                .camera
                .look_at(vecmath_util::to_vec3(&setting.viewer_setting.slice_pos));
            setting.viewer_setting.camera_angle = rot_mat_to_euler_angles(&[
                render_sys.camera.right,
                render_sys.camera.up,
                render_sys.camera.forward,
            ]);
            camera_update = true;
        }

        let mut reset = false;
        ui.same_line(0.);
        if ui.small_button(im_str!("reset")) {
            setting = init_setting;
            reset = true;
        }

        ui.same_line(0.);
        if ui.small_button(im_str!("default")) {
            let default_setting = acoustic_field_viewer::view::ViewerSettings {
                wave_length: setting.viewer_setting.wave_length,
                ..Default::default()
            };
            setting.viewer_setting = default_setting;
            reset = true;
        }

        if reset {
            render_sys.camera.position = setting.viewer_setting.camera_pos;
            slice_size_changed = true;
            slice_geo_changed = true;
            rot_changed = true;
            color_changed = true;
            camera_update = true;
        }

        if slice_size_changed {
            update_flag |= UpdateFlag::UPDATE_SLICE_SIZE;
        }

        if slice_geo_changed {
            field_slice_viewer.move_to(setting.viewer_setting.slice_pos);
            update_flag |= UpdateFlag::UPDATE_SLICE_POS;
        }

        if rot_changed {
            field_slice_viewer.rotate_to(setting.viewer_setting.slice_angle);
            update_flag |= UpdateFlag::UPDATE_SLICE_POS;
        }

        if color_changed {
            update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
        }

        if camera_update {
            render_sys.camera.position = setting.viewer_setting.camera_pos;
            set_camera_angle(&mut render_sys.camera, setting.viewer_setting.camera_angle);

            view_projection = render_sys.get_view_projection(&setting.viewer_setting);
            update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
        }

        sound_source_viewer.update(
            &mut render_sys,
            view_projection,
            &setting.viewer_setting,
            &sources,
            update_flag,
        );
        field_slice_viewer.update(
            &mut render_sys,
            view_projection,
            &setting.viewer_setting,
            &sources,
            update_flag,
        );

        encoder.clear(&render_sys.output_color, setting.background);
        encoder.clear_depth(&render_sys.output_stencil, 1.0);
        sound_source_viewer.renderer(&mut encoder);
        field_slice_viewer.renderer(&mut encoder);

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

    setting.merge_render_sys(&render_sys);

    setting.save("setting.json");
}
