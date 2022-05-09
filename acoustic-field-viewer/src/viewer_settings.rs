/*
 * File: viewer_settings.rs
 * Project: src
 * Created Date: 26/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 09/05/2022
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::f32::consts::PI;

use crate::{Vector3, Vector4};
use autd3_core::TRANS_SPACING_MM;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct ViewerSettings {
    pub source_size: f32,
    pub frequency: f32,
    pub sound_speed: f32,
    pub axis_length: f32,
    pub axis_width: f32,
    pub color_scale: f32,
    pub slice_alpha: f32,
    pub source_alpha: f32,
    pub slice_width: u32,
    pub slice_height: u32,
    pub slice_pixel_size: u32,
    pub slice_pos: Vector4,
    pub slice_angle: Vector3,
    pub camera_pos: Vector3,
    pub camera_angle: Vector3,
    pub camera_move_speed: f32,
    pub fov: f32,
    pub near_clip: f32,
    pub far_clip: f32,
    pub background: Vector4,
    pub vsync: bool,
}

impl ViewerSettings {
    pub fn new() -> ViewerSettings {
        Self::default()
    }
}

impl Default for ViewerSettings {
    fn default() -> Self {
        ViewerSettings {
            source_size: autd3_core::TRANS_SPACING_MM as _,
            color_scale: 2.0,
            slice_alpha: 0.95,
            axis_length: 50.0,
            axis_width: 2.0,
            frequency: 40e3,
            sound_speed: 340e3,
            slice_width: 400,
            slice_height: 300,
            slice_pixel_size: 1,
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
            camera_move_speed: 10.0,
            fov: 60. * PI / 180.0,
            near_clip: 0.1,
            far_clip: 1000.,
            background: [0.3, 0.3, 0.3, 0.0],
            vsync: false,
        }
    }
}
