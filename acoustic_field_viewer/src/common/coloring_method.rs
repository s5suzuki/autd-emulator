/*
 * File: coloring.rs
 * Project: Color
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 27/04/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use super::color::Color;
use super::color::HSV;

pub type ColoringMethod = fn(f32) -> [f32; 4];

pub fn coloring_hsv(v: f32) -> [f32; 4] {
    let hsv = HSV {
        h: v,
        s: 1.,
        v: 1.,
        a: 1.,
    };
    hsv.rgba()
}
