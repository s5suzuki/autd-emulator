/*
 * File: coloring.rs
 * Project: Color
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 06/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use super::color::Color;
use super::color::HSV;

pub type ColoringMethod = fn(f32, f32) -> [f32; 4];

pub fn coloring_hsv(h: f32, v: f32) -> [f32; 4] {
    let hsv = HSV {
        h: h,
        s: 1.,
        v: v,
        a: 1.,
    };
    hsv.rgba()
}
