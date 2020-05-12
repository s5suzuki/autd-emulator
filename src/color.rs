/*
 * File: color.rs
 * Project: src
 * Created Date: 02/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 12/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

#![allow(dead_code)]

use conrod_core::color;
use conrod_core::color::Color;

macro_rules! from_hex {
    ($r: expr,$g: expr, $b: expr) => {
        color::Color::Rgba($r as f32 / 255.0, $g as f32 / 255.0, $b as f32 / 255.0, 1.0)
    };
}

pub const DARK: Color = from_hex!(45, 45, 45);
pub const GRAY: Color = from_hex!(171, 171, 171);
pub const BLACK: Color = from_hex!(0, 0, 0);
pub const WHITE: Color = from_hex!(255, 255, 255);

pub const ALPHA: Color = color::Color::Rgba(0., 0., 0., 0.);

pub const PRIMARY_0: Color = from_hex!(4, 34, 67);
pub const PRIMARY_1: Color = from_hex!(4, 22, 42);
pub const PRIMARY_2: Color = from_hex!(3, 26, 52);
pub const PRIMARY_3: Color = from_hex!(6, 51, 101);
pub const PRIMARY_4: Color = from_hex!(8, 69, 138);
