/*
 * File: settings.rs
 * Project: src
 * Created Date: 05/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 09/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use acoustic_field_viewer::view::{render_system::RenderSystem, ViewerSettings};
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
};

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct Setting {
    pub port: u16,
    pub window_width: u32,
    pub window_height: u32,
    pub viewer_setting: ViewerSettings,
}

impl Setting {
    pub fn new() -> Self {
        Self {
            port: 50632,
            window_width: 960,
            window_height: 640,
            viewer_setting: ViewerSettings::new(),
        }
    }

    pub fn merge_render_sys(&mut self, render_sys: &RenderSystem) {
        let size = render_sys.window().inner_size();
        self.window_width = size.width;
        self.window_height = size.height;
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
