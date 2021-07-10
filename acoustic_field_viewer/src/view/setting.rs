/*
 * File: setting.rs
 * Project: sound_source
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 10/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use std::f32::consts::PI;

use crate::{Vector3, Vector4};
use autd3_core::hardware_defined::TRANS_SPACING_MM;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct ViewerSettings {
    pub frequency: f32,
    pub source_size: f32,
    pub wave_length: f32,
    pub color_scale: f32,
    pub slice_alpha: f32,
    pub source_alpha: f32,
    pub slice_width: i32,
    pub slice_height: i32,
    pub slice_pos: Vector4,
    pub slice_angle: Vector3,
    pub camera_pos: Vector3,
    pub camera_angle: Vector3,
    pub fov: f32,
    pub near_clip: f32,
    pub far_clip: f32,
}

impl ViewerSettings {
    pub fn new() -> ViewerSettings {
        Self::default()
    }
}

impl Default for ViewerSettings {
    fn default() -> Self {
        ViewerSettings {
            frequency: autd3_core::hardware_defined::ULTRASOUND_FREQUENCY as _,
            source_size: autd3_core::hardware_defined::TRANS_SPACING_MM as _,
            color_scale: 2.0,
            slice_alpha: 0.95,
            wave_length: 8.5,
            slice_width: 400,
            slice_height: 300,
            source_alpha: 1.0,
            slice_pos: [
                TRANS_SPACING_MM as f32 * 8.5,
                TRANS_SPACING_MM as f32 * 6.5,
                150.,
                1.,
            ],
            slice_angle: [PI / 2., 0., 0.],
            camera_pos: [0., -500.0, 200.0],
            camera_angle: [PI / 2., 0., 0.],
            fov: 60. * PI / 180.0,
            near_clip: 0.1,
            far_clip: 1000.,
        }
    }
}
