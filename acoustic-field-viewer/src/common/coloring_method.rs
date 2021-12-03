/*
 * File: coloring.rs
 * Project: Color
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 01/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use super::color::Color;
use super::color::Hsv;

pub type ColoringMethod = fn(f32, f32, f32) -> [f32; 4];

pub fn coloring_hsv(h: f32, v: f32, a: f32) -> [f32; 4] {
    let hsv = Hsv { h, s: 1., v, a };
    hsv.rgba()
}
