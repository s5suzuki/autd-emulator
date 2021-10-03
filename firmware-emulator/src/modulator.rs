/*
 * File: modulator.rs
 * Project: src
 * Created Date: 19/09/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 19/09/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

pub struct Modulator {
    mod_idx: usize,
    mod_cnt: u16,
    mod_idx_div: u16,
    modulation: Vec<u8>,
}

impl Modulator {
    pub fn new() -> Self {
        Self {
            mod_idx: 0,
            mod_cnt: 0,
            mod_idx_div: 0,
            modulation: vec![0xFF],
        }
    }

    pub fn set(&mut self, modulation: &[u8], mod_idx_div: u16) {
        self.modulation = modulation.to_vec();
        self.mod_idx_div = mod_idx_div;
        self.mod_idx = 0;
    }

    pub fn modulate(&self, duty: u8) -> u8 {
        let m = self.modulation[self.mod_idx] as u16 + 1;
        let d = duty as u16 * m;
        (d >> 8) as _
    }

    pub fn update(&mut self) {
        self.mod_cnt += 1;
        if self.mod_cnt < self.mod_idx_div {
            return;
        }
        self.mod_cnt = 0;
        self.mod_idx += 1;
        if self.mod_idx >= self.modulation.len() {
            self.mod_idx = 0;
        }
    }
}
