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

use std::f32::consts::PI;

use acoustic_field_viewer::{
    coloring_method::coloring_hsv,
    sound_source::SoundSource,
    view::{UpdateFlag, ViewWindow, ViewerSettings},
};
use piston_window::{Button, Key, PressEvent};

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

    let (mut window_view, mut window) = ViewWindow::new(
        vecmath_util::mat4_scale(1.0),
        &settings,
        [WINDOW_WIDTH, WINDOW_HEIGHT],
    );
    window_view.field_slice_viewer.translate(focal_pos);
    window_view
        .field_slice_viewer
        .set_posture([1., 0., 0.], [0., 0., 1.]);

    if let Some(e) = window.next() {
        window_view.renderer(&mut window, e, &settings, &sources, UpdateFlag::all());
    }

    while let Some(e) = window.next() {
        let travel = 5.0;
        let mut update_flag = UpdateFlag::empty();
        match e.press_args() {
            Some(Button::Keyboard(Key::Up)) => {
                window_view.field_slice_viewer.translate([0., 0., travel]);
                update_flag |= UpdateFlag::UPDATE_SLICE_POS;
            }
            Some(Button::Keyboard(Key::Down)) => {
                window_view.field_slice_viewer.translate([0., 0., -travel]);
                update_flag |= UpdateFlag::UPDATE_SLICE_POS;
            }
            Some(Button::Keyboard(Key::Left)) => {
                window_view.field_slice_viewer.translate([-travel, 0., 0.]);
                update_flag |= UpdateFlag::UPDATE_SLICE_POS;
            }
            Some(Button::Keyboard(Key::Right)) => {
                window_view.field_slice_viewer.translate([travel, 0., 0.]);
                update_flag |= UpdateFlag::UPDATE_SLICE_POS;
            }
            Some(Button::Keyboard(Key::Z)) => {
                window_view.field_slice_viewer.rotate([0., 0., 1.], 0.05);
                update_flag |= UpdateFlag::UPDATE_SLICE_POS;
            }
            Some(Button::Keyboard(Key::X)) => {
                window_view.field_slice_viewer.rotate([0., 0., 1.], -0.05);
                update_flag |= UpdateFlag::UPDATE_SLICE_POS;
            }
            Some(Button::Keyboard(Key::C)) => {
                settings.color_scale += 0.1;
                update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
            }
            Some(Button::Keyboard(Key::V)) => {
                settings.color_scale -= 0.1;
                update_flag |= UpdateFlag::UPDATE_COLOR_MAP;
            }
            Some(Button::Keyboard(Key::G)) => {
                focal_pos = vecmath::vec3_add(focal_pos, [travel, 0., 0.]);
                let dist = |l: vecmath::Vector3<f32>, r: vecmath::Vector3<f32>| {
                    let d = vecmath::vec3_sub(l, r);
                    vecmath::vec3_dot(d, d).sqrt()
                };
                for source in sources.iter_mut() {
                    let pos = source.pos;
                    let d = dist(pos, focal_pos);
                    let phase = (d % WAVE_LENGTH) / WAVE_LENGTH;
                    let phase = 2.0 * PI * phase;

                    source.phase = phase;
                }
                update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
            }
            Some(Button::Keyboard(Key::F)) => {
                focal_pos = vecmath::vec3_add(focal_pos, [-travel, 0., 0.]);
                let dist = |l: vecmath::Vector3<f32>, r: vecmath::Vector3<f32>| {
                    let d = vecmath::vec3_sub(l, r);
                    vecmath::vec3_dot(d, d).sqrt()
                };
                for source in sources.iter_mut() {
                    let pos = source.pos;
                    let d = dist(pos, focal_pos);
                    let phase = (d % WAVE_LENGTH) / WAVE_LENGTH;
                    let phase = 2.0 * PI * phase;

                    source.phase = phase;
                }
                update_flag |= UpdateFlag::UPDATE_SOURCE_DRIVE;
            }
            _ => (),
        }
        window_view.renderer(&mut window, e, &settings, &sources, update_flag);
    }
}
