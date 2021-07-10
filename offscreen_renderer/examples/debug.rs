/*
 * File: debug.rs
 * Project: example
 * Created Date: 10/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 10/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::f32::consts::PI;

use acoustic_field_viewer::{
    sound_source::SoundSource,
    view::{AcousticFiledSliceViewer, System, UpdateFlag, ViewerSettings},
    Vector3,
};
use autd3_core::hardware_defined::{
    is_missing_transducer, NUM_TRANS_X, NUM_TRANS_Y, TRANS_SPACING_MM,
};
use offscreen_renderer::OffscreenRenderer;

fn main() {
    const FOCAL_POS: Vector3 = [
        TRANS_SPACING_MM as f32 * 8.5,
        TRANS_SPACING_MM as f32 * 6.5,
        150.,
    ];

    let mut g = OffscreenRenderer::new();

    let opengl = shader_version::OpenGL::V4_5;
    let settings = ViewerSettings::default();
    let system = System::init("debug", 960., 640.);
    let mut field_slice_viewer =
        AcousticFiledSliceViewer::new(&system.render_sys, opengl, &settings);
    field_slice_viewer.move_to(settings.slice_pos);
    field_slice_viewer.rotate_to(settings.slice_angle);

    let mut sources = Vec::new();
    let zdir = [0., 0., 1.];
    for y in 0..NUM_TRANS_Y {
        for x in 0..NUM_TRANS_X {
            if is_missing_transducer(x, y) {
                continue;
            }
            let pos = [
                TRANS_SPACING_MM as f32 * x as f32,
                TRANS_SPACING_MM as f32 * y as f32,
                0.,
            ];
            sources.push(SoundSource::new(pos, zdir, 1.0, 0.0));
        }
    }

    for source in sources.iter_mut() {
        let pos = source.pos;
        let d = vecmath_util::dist(pos, FOCAL_POS);
        let phase = (d % settings.wave_length) / settings.wave_length;
        source.phase = 2.0 * PI * phase;
    }

    g.update(&sources, &field_slice_viewer, &settings, UpdateFlag::all());
    g.calculate_field(&sources, &settings);
    let bb = (
        settings.slice_width as usize,
        settings.slice_height as usize,
    );
    g.save("debug.png", bb, field_slice_viewer.color_map());
}
