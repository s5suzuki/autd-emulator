/*
 * File: settings.rs
 * Project: src
 * Created Date: 05/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 05/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::{fs::File, path::Path};

#[derive(Serialize, Deserialize, Debug)]
pub struct Setting {
    pub port: u16,
}

impl Setting {
    pub fn new() -> Self {
        Self { port: 50632 }
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
