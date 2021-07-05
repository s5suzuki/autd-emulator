/*
 * File: main.rs
 * Project: examples
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 01/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

extern crate acoustic_field_viewer;

use std::f32::consts::PI;

use acoustic_field_viewer::coloring_method::coloring_hsv;
use acoustic_field_viewer::sound_source::SoundSource;
use acoustic_field_viewer::vec_utils;
use acoustic_field_viewer::view::event::*;
use acoustic_field_viewer::view::{
    AcousticFiledSliceViewer, SoundSourceViewer, UpdateHandler, ViewWindow, ViewerSettings,
};

pub fn main() {
    const NUM_TRANS_X: usize = 18;
    const NUM_TRANS_Y: usize = 14;
    const TRANS_SIZE: f32 = 10.18;
    const WAVE_LENGTH: f32 = 8.5;

    let mut focal_pos = [TRANS_SIZE * 8.5, TRANS_SIZE * 6.5, 150.];

    let mut transducers = Vec::new();
    let zdir = [0., 0., 1.];
    for y in 0..NUM_TRANS_Y {
        for x in 0..NUM_TRANS_X {
            let pos = [TRANS_SIZE * x as f32, TRANS_SIZE * y as f32, 0.];
            let d = vec_utils::dist(pos, focal_pos);
            let phase = (d % WAVE_LENGTH) / WAVE_LENGTH;
            let phase = 2.0 * PI * phase;
            transducers.push(SoundSource::new(pos, zdir, phase));
        }
    }

    let mut settings = ViewerSettings::new(
        40e3,
        TRANS_SIZE,
        coloring_hsv,
        scarlet::colormap::ListedColorMap::inferno(),
    );
    settings.color_scale = 0.6;
    settings.slice_alpha = 0.95;

    let source_viewer = SoundSourceViewer::new();
    let mut acoustic_field_viewer = AcousticFiledSliceViewer::new();
    acoustic_field_viewer.translate(focal_pos);

    let update = |update_handler: &mut UpdateHandler, button: Option<Button>| {
        let travel = 5.0;
        match button {
            Some(Button::Keyboard(Key::Up)) => {
                update_handler.camera.position =
                    vecmath::vec3_add(update_handler.camera.position, [0., travel, 0.]);
                update_handler.update_position();
            }
            Some(Button::Keyboard(Key::Down)) => {
                update_handler.camera.position =
                    vecmath::vec3_add(update_handler.camera.position, [0., travel, 0.]);
                update_handler.update_position();
            }
            Some(Button::Keyboard(Key::Left)) => {
                update_handler
                    .field_slice_viewer
                    .translate([-travel, 0., 0.]);
            }
            Some(Button::Keyboard(Key::Right)) => {
                update_handler
                    .field_slice_viewer
                    .translate([travel, 0., 0.]);
            }
            Some(Button::Keyboard(Key::Z)) => {
                update_handler.field_slice_viewer.rotate([0., 0., 1.], 0.05);
            }
            Some(Button::Keyboard(Key::X)) => {
                update_handler
                    .field_slice_viewer
                    .rotate([0., 0., 1.], -0.05);
            }
            Some(Button::Keyboard(Key::C)) => {
                update_handler.settings.borrow_mut().color_scale += 0.1;
                update_handler.field_slice_viewer.update_color_map();
            }
            Some(Button::Keyboard(Key::V)) => {
                update_handler.settings.borrow_mut().color_scale -= 0.1;
                update_handler.field_slice_viewer.update_color_map();
            }
            Some(Button::Keyboard(Key::G)) => {
                focal_pos = vecmath::vec3_add(focal_pos, [travel, 0., 0.]);
                let dist = |l: vecmath::Vector3<f32>, r: vecmath::Vector3<f32>| {
                    let d = vecmath::vec3_sub(l, r);
                    vecmath::vec3_dot(d, d).sqrt()
                };
                for source in update_handler.sources.borrow_mut().iter_mut() {
                    let pos = source.pos;
                    let d = dist(pos, focal_pos);
                    let phase = (d % WAVE_LENGTH) / WAVE_LENGTH;
                    let phase = 2.0 * PI * phase;

                    source.phase = phase;
                }
                update_handler.update_phase();
            }
            Some(Button::Keyboard(Key::F)) => {
                focal_pos = vecmath::vec3_add(focal_pos, [-travel, 0., 0.]);
                let dist = |l: vecmath::Vector3<f32>, r: vecmath::Vector3<f32>| {
                    let d = vecmath::vec3_sub(l, r);
                    vecmath::vec3_dot(d, d).sqrt()
                };
                for source in update_handler.sources.borrow_mut().iter_mut() {
                    let pos = source.pos;
                    let d = dist(pos, focal_pos);
                    let phase = (d % WAVE_LENGTH) / WAVE_LENGTH;
                    let phase = 2.0 * PI * phase;

                    source.phase = phase;
                }
                update_handler.update_phase();
            }
            _ => (),
        }
    };

    let mut window = ViewWindow::new(transducers, source_viewer, acoustic_field_viewer, settings);
    window.update = Some(update);
    window.start();
}
