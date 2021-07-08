/*
 * File: main.rs
 * Project: examples
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 08/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use std::{f32::consts::PI, time::Instant};

use acoustic_field_viewer::{
    coloring_method::coloring_hsv,
    sound_source::SoundSource,
    view::{AcousticFiledSliceViewer, SoundSourceViewer, System, UpdateFlag, ViewerSettings},
};
use camera_controllers::{CameraPerspective, FirstPerson, FirstPersonSettings};
use gfx::Device;
use glutin::{
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    platform::run_return::EventLoopExtRunReturn,
    window::Window,
};
use imgui::im_str;
use shader_version::OpenGL;

type Matrix4 = vecmath::Matrix4<f32>;

fn get_projection(w: &Window) -> Matrix4 {
    let draw_size = w.inner_size();
    CameraPerspective {
        fov: 60.0,
        near_clip: 0.1,
        far_clip: 1000.0,
        aspect_ratio: (draw_size.width as f32) / (draw_size.height as f32),
    }
    .projection()
}

pub fn main() {
    const NUM_TRANS_X: usize = 18;
    const NUM_TRANS_Y: usize = 14;
    const TRANS_SIZE: f32 = 10.16;
    const WAVE_LENGTH: f32 = 8.5;

    const VIEW_SLICE_WIDTH: i32 = 400;
    const VIEW_SLICE_HEIGHT: i32 = 300;
    const WINDOW_WIDTH: u32 = 640;
    const WINDOW_HEIGHT: u32 = 480;

    let mut focal_pos = [TRANS_SIZE * 8.5, TRANS_SIZE * 6.5, 150.];

    let mut sources = Vec::new();
    let zdir = [0., 0., 1.];
    for y in 0..NUM_TRANS_Y {
        for x in 0..NUM_TRANS_X {
            let pos = [TRANS_SIZE * x as f32, TRANS_SIZE * y as f32, 0.];
            let d = vecmath_util::dist(pos, focal_pos);
            let phase = (d % WAVE_LENGTH) / WAVE_LENGTH;
            let phase = 2.0 * PI * phase;
            sources.push(SoundSource::new(pos, zdir, 1.0, phase));
        }
    }

    let mut settings = ViewerSettings::new(
        40e3,
        WAVE_LENGTH,
        TRANS_SIZE,
        coloring_hsv,
        scarlet::colormap::ListedColorMap::inferno(),
        (VIEW_SLICE_WIDTH, VIEW_SLICE_HEIGHT),
    );
    settings.color_scale = 0.6;
    settings.slice_alpha = 0.95;

    let system = System::init("example");
    let System {
        mut events_loop,
        mut imgui,
        mut platform,
        mut render_sys,
        ..
    } = system;
    let mut encoder: gfx::Encoder<_, _> = render_sys.factory.create_command_buffer().into();

    let projection = get_projection(&render_sys.window());
    let first_person = FirstPerson::new([90., -250.0, 120.0], FirstPersonSettings::keyboard_wasd());
    let mut camera = first_person.camera(0.);
    camera.set_yaw_pitch(0., -std::f32::consts::PI / 2.0);

    let mut slice_model = vecmath_util::mat4_t([
        autd3_core::hardware_defined::TRANS_SPACING_MM as f32 * 8.5,
        autd3_core::hardware_defined::TRANS_SPACING_MM as f32 * 6.5,
        150.,
    ]);
    let right = [1., 0., 0.];
    let up = [0., 0., 1.];
    let forward = vecmath::vec3_cross(right, up);
    slice_model[0] = vecmath_util::to_vec4(right);
    slice_model[1] = vecmath_util::to_vec4(up);
    slice_model[2] = vecmath_util::to_vec4(forward);
    let opengl = OpenGL::V4_5;
    let mut sound_source_viewer = SoundSourceViewer::new(&render_sys, opengl);
    let mut field_slice_viewer =
        AcousticFiledSliceViewer::new(slice_model, &render_sys, opengl, &settings);
    let cam_orth = camera.orthogonal();
    let view_projection = (cam_orth, projection);

    let mut last_frame = Instant::now();
    let mut run = true;
    let mut init = true;
    while run {
        events_loop.run_return(|event, _, control_flow| {
            if init {
                sound_source_viewer.update(
                    &mut render_sys,
                    &event,
                    view_projection,
                    &settings,
                    &sources,
                    UpdateFlag::all(),
                );
                field_slice_viewer.update(
                    &mut render_sys,
                    &event,
                    view_projection,
                    &settings,
                    &sources,
                    UpdateFlag::all(),
                );
                init = false;
            }

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
        let mut ui = imgui.frame();
        /////
        ui.text(im_str!("Hello world!"));
        ui.separator();
        let mouse_pos = ui.io().mouse_pos;
        ui.text(format!(
            "Mouse Position: ({:.1},{:.1})",
            mouse_pos[0], mouse_pos[1]
        ));
        /////

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

    // if let Some(e) = window.next() {
    //     window_view.renderer(&mut window, e, &settings, &sources, UpdateFlag::all());
    // }

    // while let Some(e) = window.next() {
    //     let travel = 5.0;
    //     let mut update_flag = UpdateFlag::empty();
    //     match e.press_args() {
    //         Some(Button::Keyboard(Key::Up)) => {
    //             window_view.field_slice_viewer.translate([0., 0., travel]);
    //             update_flag |= UpdateFlag::UPDATE_SLICE_POS;
    //         }
    //         Some(Button::Keyboard(Key::Down)) => {
    //             window_view.field_slice_viewer.translate([0., 0., -travel]);
    //             update_flag |= UpdateFlag::UPDATE_SLICE_POS;
    //         }
    //         Some(Button::Keyboard(Key::Left)) => {
    //             window_view.field_slice_viewer.translate([-travel, 0., 0.]);
    //             update_flag |= UpdateFlag::UPDATE_SLICE_POS;
    //         }
    //         Some(Button::Keyboard(Key::Right)) => {
    //             window_view.field_slice_viewer.translate([travel, 0., 0.]);
    //             update_flag |= UpdateFlag::UPDATE_SLICE_POS;
    //         }
    //         Some(Button::Keyboard(Key::Z)) => {
    //             window_view.field_slice_viewer.rotate([0., 0., 1.], 0.05);
    //             update_flag |= UpdateFlag::UPDATE_SLICE_POS;
    //         }
    //         Some(Button::Keyboard(Key::X)) => {
    //             window_view.field_slice_viewer.rotate([0., 0., 1.], -0.05);
    //             update_flag |= UpdateFlag::UPDATE_SLICE_POS;
    //         }
    //         Some(Button::Keyboard(Key::C)) => {
    //             settings.color_scale += 0.1;
    //             update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
    //         }
    //         Some(Button::Keyboard(Key::V)) => {
    //             settings.color_scale -= 0.1;
    //             update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
    //         }
    //         Some(Button::Keyboard(Key::G)) => {
    //             focal_pos = vecmath::vec3_add(focal_pos, [travel, 0., 0.]);
    //             let dist = |l: vecmath::Vector3<f32>, r: vecmath::Vector3<f32>| {
    //                 let d = vecmath::vec3_sub(l, r);
    //                 vecmath::vec3_dot(d, d).sqrt()
    //             };
    //             for source in sources.iter_mut() {
    //                 let pos = source.pos;
    //                 let d = dist(pos, focal_pos);
    //                 let phase = (d % WAVE_LENGTH) / WAVE_LENGTH;
    //                 let phase = 2.0 * PI * phase;

    //                 source.phase = phase;
    //             }
    //             update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
    //         }
    //         Some(Button::Keyboard(Key::F)) => {
    //             focal_pos = vecmath::vec3_add(focal_pos, [-travel, 0., 0.]);
    //             let dist = |l: vecmath::Vector3<f32>, r: vecmath::Vector3<f32>| {
    //                 let d = vecmath::vec3_sub(l, r);
    //                 vecmath::vec3_dot(d, d).sqrt()
    //             };
    //             for source in sources.iter_mut() {
    //                 let pos = source.pos;
    //                 let d = dist(pos, focal_pos);
    //                 let phase = (d % WAVE_LENGTH) / WAVE_LENGTH;
    //                 let phase = 2.0 * PI * phase;

    //                 source.phase = phase;
    //             }
    //             update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
    //         }
    //         _ => (),
    //     }
    //     window_view.renderer(&mut window, e, &settings, &sources, update_flag);
    // }
}
