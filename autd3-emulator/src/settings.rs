/*
 * File: settings.rs
 * Project: src
 * Created Date: 05/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 06/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::{fs::File, path::Path};

use crate::Matrix4;

#[derive(Serialize, Deserialize, Debug)]
pub struct Setting {
    pub port: u16,
    pub slice_model: Matrix4,
    pub wave_length: f32,
    pub slice_width: i32,
    pub slice_height: i32,
    pub window_width: u32,
    pub window_height: u32,
}

impl Setting {
    pub fn new() -> Self {
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

        Self {
            port: 50632,
            slice_model,
            wave_length: 8.5,
            slice_width: 200,
            slice_height: 200,
            window_width: 640,
            window_height: 480,
        }
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Self {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(_) => return Self::new(),
        };
        let setting: Self = match serde_json::from_reader(file) {
            Ok(setting) => setting,
            Err(_) => Self::new(),
        };
        setting
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) {
        let json = serde_json::to_string_pretty(self).unwrap();
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(path)
            .unwrap();
        writeln!(&mut file, "{}", json).unwrap();
    }
}
