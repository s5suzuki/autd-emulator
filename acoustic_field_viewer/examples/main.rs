/*
 * File: main.rs
 * Project: examples
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 09/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use std::{f32::consts::PI, time::Instant};

use acoustic_field_viewer::{
    sound_source::SoundSource,
    view::{AcousticFiledSliceViewer, SoundSourceViewer, System, UpdateFlag, ViewerSettings},
    Matrix3, Vector3,
};
use autd3_core::hardware_defined::is_missing_transducer;
use camera_controllers::Camera;
use gfx::Device;
use glutin::{
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    platform::run_return::EventLoopExtRunReturn,
};
use imgui::*;
use shader_version::OpenGL;

const NUM_TRANS_X: usize = 18;
const NUM_TRANS_Y: usize = 14;
const TRANS_SIZE: f32 = 10.16;

fn calc_focus_phase(focal_pos: Vector3, sources: &mut [SoundSource], settings: &ViewerSettings) {
    for source in sources.iter_mut() {
        let pos = source.pos;
        let d = vecmath_util::dist(pos, focal_pos);
        let phase = (d % settings.wave_length) / settings.wave_length;
        source.phase = 2.0 * PI * phase;
    }
}

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

pub fn main() {
    const WAVE_LENGTH: f32 = 8.5;
    const FOV: f32 = 60. * PI / 180.0;
    const NEAR_CLIP: f32 = 0.1;
    const FAR_CLIP: f32 = 1000.0;
    const COLOR_SCALE: f32 = 0.6;
    const SLICE_ALPHA: f32 = 0.95;

    const VIEW_SLICE_WIDTH: i32 = 400;
    const VIEW_SLICE_HEIGHT: i32 = 300;
    const WINDOW_WIDTH: f64 = 960.;
    const WINDOW_HEIGHT: f64 = 640.;

    const SLICE_ANGLE: Vector3 = [PI / 2., 0., 0.];
    const CAMERA_ANGLE: Vector3 = [PI / 2., 0., 0.];
    const FOCAL_POS: Vector3 = [TRANS_SIZE * 8.5, TRANS_SIZE * 6.5, 150.];
    const CAMERA_POS: Vector3 = [0., -500.0, 120.0];

    let mut settings = ViewerSettings::new();
    settings.frequency = 40e3;
    settings.source_size = TRANS_SIZE;
    settings.wave_length = WAVE_LENGTH;
    settings.color_scale = COLOR_SCALE;
    settings.slice_alpha = SLICE_ALPHA;
    settings.slice_width = VIEW_SLICE_WIDTH;
    settings.slice_height = VIEW_SLICE_HEIGHT;
    settings.slice_pos = [FOCAL_POS[0], FOCAL_POS[1], FOCAL_POS[2], 1.0];
    settings.slice_angle = SLICE_ANGLE;
    settings.camera_pos = CAMERA_POS;
    settings.camera_angle = CAMERA_ANGLE;
    settings.fov = FOV;
    settings.near_clip = NEAR_CLIP;
    settings.far_clip = FAR_CLIP;

    let mut focal_pos = FOCAL_POS;
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
    calc_focus_phase(focal_pos, &mut sources, &settings);

    let system = System::init("example", WINDOW_WIDTH, WINDOW_HEIGHT);
    let System {
        mut events_loop,
        mut imgui,
        mut platform,
        mut render_sys,
        mut encoder,
        ..
    } = system;

    let opengl = OpenGL::V4_5;
    let mut sound_source_viewer = SoundSourceViewer::new(&render_sys, opengl);
    let mut field_slice_viewer = AcousticFiledSliceViewer::new(&render_sys, opengl, &settings);
    field_slice_viewer.move_to(settings.slice_pos);
    field_slice_viewer.rotate_to(settings.slice_angle);

    render_sys.camera.position = settings.camera_pos;
    set_camera_angle(&mut render_sys.camera, settings.camera_angle);

    let mut view_projection = render_sys.get_view_projection(&settings);
    let mut last_frame = Instant::now();
    let mut run = true;
    let mut init = true;
    while run {
        events_loop.run_return(|event, _, control_flow| {
            if init {
                sound_source_viewer.update(
                    &mut render_sys,
                    view_projection,
                    &settings,
                    &sources,
                    UpdateFlag::all(),
                );
                field_slice_viewer.update(
                    &mut render_sys,
                    view_projection,
                    &settings,
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

        let io = imgui.io_mut();
        platform
            .prepare_frame(io, render_sys.window())
            .expect("Failed to start frame");
        let now = Instant::now();
        io.update_delta_time(now - last_frame);
        last_frame = now;
        let ui = imgui.frame();

        let mut focus_changed = false;
        let mut slice_changed = false;
        let mut rot_changed = false;
        let mut color_changed = false;
        let mut camera_update = false;
        TabBar::new(im_str!("Settings")).build(&ui, || {
            TabItem::new(im_str!("Focus")).build(&ui, || {
                ui.text(im_str!("Focus position"));
                focus_changed = Drag::new(im_str!("Pos X")).build(&ui, &mut focal_pos[0]);
                focus_changed |= Drag::new(im_str!("Pos Y")).build(&ui, &mut focal_pos[1]);
                focus_changed |= Drag::new(im_str!("Pos Z")).build(&ui, &mut focal_pos[2]);
                focus_changed |= Drag::new(im_str!("Wavelength"))
                    .range(0.0..=f32::INFINITY)
                    .build(&ui, &mut settings.wave_length);
            });
            TabItem::new(im_str!("Slice")).build(&ui, || {
                ui.text(im_str!("Slice position"));
                slice_changed =
                    Drag::new(im_str!("Slice X")).build(&ui, &mut settings.slice_pos[0]);
                slice_changed |=
                    Drag::new(im_str!("Slice Y")).build(&ui, &mut settings.slice_pos[1]);
                slice_changed |=
                    Drag::new(im_str!("Slice Z")).build(&ui, &mut settings.slice_pos[2]);

                ui.separator();
                ui.text(im_str!("Slice Rotation"));
                rot_changed = AngleSlider::new(im_str!("Slice RX"))
                    .range_degrees(0.0..=360.0)
                    .build(&ui, &mut settings.slice_angle[0]);
                rot_changed |= AngleSlider::new(im_str!("Slice RY"))
                    .range_degrees(0.0..=360.0)
                    .build(&ui, &mut settings.slice_angle[1]);
                rot_changed |= AngleSlider::new(im_str!("Slice RZ"))
                    .range_degrees(0.0..=360.0)
                    .build(&ui, &mut settings.slice_angle[2]);

                ui.separator();
                ui.text(im_str!("Slice color setting"));
                color_changed = Slider::new(im_str!("Color scale"))
                    .range(0.0..=10.0)
                    .build(&ui, &mut settings.color_scale);
                color_changed |= Slider::new(im_str!("Slice alpha"))
                    .range(0.0..=1.0)
                    .build(&ui, &mut settings.slice_alpha);

                ui.separator();
                if ui.small_button(im_str!("xy")) {
                    settings.slice_angle = [0., 0., 0.];
                    rot_changed = true;
                }
                ui.same_line(0.);
                if ui.small_button(im_str!("yz")) {
                    settings.slice_angle = [0., -PI / 2., 0.];
                    rot_changed = true;
                }
                ui.same_line(0.);
                if ui.small_button(im_str!("zx")) {
                    settings.slice_angle = [PI / 2., 0., 0.];
                    rot_changed = true;
                }
            });
            TabItem::new(im_str!("Camera")).build(&ui, || {
                ui.text(im_str!("Camera pos"));
                camera_update =
                    Drag::new(im_str!("Camera X")).build(&ui, &mut render_sys.camera.position[0]);
                camera_update |=
                    Drag::new(im_str!("Camera Y")).build(&ui, &mut render_sys.camera.position[1]);
                camera_update |=
                    Drag::new(im_str!("Camera Z")).build(&ui, &mut render_sys.camera.position[2]);
                ui.separator();
                ui.text(im_str!("Camera rotation"));
                camera_update |= AngleSlider::new(im_str!("Camera RX"))
                    .range_degrees(-180.0..=180.0)
                    .build(&ui, &mut settings.camera_angle[0]);
                camera_update |= AngleSlider::new(im_str!("Camera RY"))
                    .range_degrees(-180.0..=180.0)
                    .build(&ui, &mut settings.camera_angle[1]);
                camera_update |= AngleSlider::new(im_str!("Camera RZ"))
                    .range_degrees(-180.0..=180.0)
                    .build(&ui, &mut settings.camera_angle[2]);
                ui.separator();
                ui.text(im_str!("Camera perspective"));
                camera_update |= AngleSlider::new(im_str!("FOV"))
                    .range_degrees(0.0..=180.0)
                    .build(&ui, &mut settings.fov);
                camera_update |= Drag::new(im_str!("Near clip"))
                    .range(0.0..=f32::INFINITY)
                    .build(&ui, &mut settings.near_clip);
                camera_update |= Drag::new(im_str!("Far clip"))
                    .range(0.0..=f32::INFINITY)
                    .build(&ui, &mut settings.far_clip);
            });
        });

        ui.separator();

        if ui.small_button(im_str!("auto")) {
            let rot = quaternion::euler_angles(
                settings.slice_angle[0],
                settings.slice_angle[1],
                settings.slice_angle[2],
            );
            let model = vecmath_util::mat4_rot(rot);

            let right = vecmath_util::to_vec3(&model[0]);
            let up = vecmath_util::to_vec3(&model[1]);
            let forward = vecmath::vec3_cross(right, up);

            let d = vecmath::vec3_scale(forward, 500.);
            let p = vecmath::vec3_add(vecmath_util::to_vec3(&settings.slice_pos), d);

            settings.camera_pos = p;
            render_sys.camera.position = p;
            render_sys.camera.right = right;
            render_sys.camera.up = up;
            render_sys
                .camera
                .look_at(vecmath_util::to_vec3(&settings.slice_pos));
            settings.camera_angle = rot_mat_to_euler_angles(&[
                render_sys.camera.right,
                render_sys.camera.up,
                render_sys.camera.forward,
            ]);
            camera_update = true;
        }

        ui.same_line(0.);
        if ui.small_button(im_str!("reset")) {
            focal_pos = FOCAL_POS;

            settings.frequency = 40e3;
            settings.source_size = TRANS_SIZE;
            settings.wave_length = WAVE_LENGTH;
            settings.color_scale = COLOR_SCALE;
            settings.slice_alpha = SLICE_ALPHA;
            settings.slice_width = VIEW_SLICE_WIDTH;
            settings.slice_height = VIEW_SLICE_HEIGHT;
            settings.slice_pos = [FOCAL_POS[0], FOCAL_POS[1], FOCAL_POS[2], 1.0];
            settings.slice_angle = SLICE_ANGLE;
            settings.camera_pos = CAMERA_POS;
            settings.camera_angle = CAMERA_ANGLE;
            settings.fov = FOV;
            settings.near_clip = NEAR_CLIP;
            settings.far_clip = FAR_CLIP;

            render_sys.camera.position = settings.camera_pos;
            set_camera_angle(&mut render_sys.camera, settings.camera_angle);

            focus_changed = true;
            slice_changed = true;
            rot_changed = true;
            color_changed = true;
            camera_update = true;
        }

        if focus_changed {
            calc_focus_phase(focal_pos, &mut sources, &settings);

            update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
            update_flag |= UpdateFlag::UPDATE_WAVENUM;
        }

        if slice_changed {
            field_slice_viewer.move_to(settings.slice_pos);
            update_flag |= UpdateFlag::UPDATE_SLICE_POS;
        }

        if rot_changed {
            field_slice_viewer.rotate_to(settings.slice_angle);
            update_flag |= UpdateFlag::UPDATE_SLICE_POS;
        }

        if color_changed {
            update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
        }

        if camera_update {
            render_sys.camera.position = settings.camera_pos;
            set_camera_angle(&mut render_sys.camera, settings.camera_angle);

            view_projection = render_sys.get_view_projection(&settings);
            update_flag |= UpdateFlag::UPDATE_CAMERA_POS;
        }

        /////

        sound_source_viewer.update(
            &mut render_sys,
            view_projection,
            &settings,
            &sources,
            update_flag,
        );
        field_slice_viewer.update(
            &mut render_sys,
            view_projection,
            &settings,
            &sources,
            update_flag,
        );

        encoder.clear(&mut render_sys.output_color, [0.3, 0.3, 0.3, 1.0]);
        encoder.clear_depth(&mut render_sys.output_stencil, 1.0);
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
}
