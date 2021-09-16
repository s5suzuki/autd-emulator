/*
 * File: settings.rs
 * Project: src
 * Created Date: 05/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 16/09/2021
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Setting {
    pub port: u16,
    pub window_width: u32,
    pub window_height: u32,
    pub camera_move_speed: f32,
    pub viewer_setting: ViewerSettings,
    pub log_enable: bool,
    pub log_max: u32,
    pub show_mod_plot: bool,
    pub show_mod_plot_raw: bool,
    pub mod_plot_size: [f32; 2],
    pub save_file_path: String,
    pub record_path: String,
    pub show: Vec<bool>,
    pub enable: Vec<bool>,
    pub show_axis: Vec<bool>,
}

impl Setting {
    pub fn new() -> Self {
        Self {
            port: 50632,
            window_width: 960,
            window_height: 640,
            camera_move_speed: 10.0,
            viewer_setting: ViewerSettings::new(),
            log_enable: true,
            log_max: 100,
            show_mod_plot: true,
            show_mod_plot_raw: false,
            mod_plot_size: [200.0, 50.],
            save_file_path: std::env::current_dir()
                .unwrap_or_default()
                .join("image.png")
                .to_str()
                .unwrap_or("")
                .to_owned(),
            record_path: std::env::current_dir()
                .unwrap_or_default()
                .join("record")
                .to_str()
                .unwrap_or("")
                .to_owned(),
            show: vec![],
            enable: vec![],
            show_axis: vec![],
        }
    }

    pub fn merge_render_sys(&mut self, render_sys: &RenderSystem) {
        let scale_factor = render_sys.window().scale_factor();
        let size = render_sys.window().inner_size().to_logical(scale_factor);
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
