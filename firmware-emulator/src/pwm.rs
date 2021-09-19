/*
* File: pwm.rs
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

use crate::consts::CYCLE;

pub struct PWM {}

impl PWM {
    pub fn output(time: u16, duty: u8, phase: u8, duty_offset: u8) -> f64 {
        let d = duty as u16 + duty_offset as u16;
        let p = (0xFF - phase as u16) << 1;
        let dl = d >> 1;
        let dr = (d + 1) >> 1;

        let pwm1 = p <= time + dl;
        let pwm1o = CYCLE + p <= time + dl;
        let pwm2 = time < p + dr;
        let pwm2o = time + CYCLE < p + dr;

        if ((p < dl) & (pwm1o | pwm2)) | ((p + dr > CYCLE) & (pwm1 | pwm2o)) | (pwm1 & pwm2) {
            12.0
        } else {
            -12.0
        }
    }
}
