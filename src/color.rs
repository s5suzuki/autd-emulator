/*
 * File: color.rs
 * Project: src
 * Created Date: 02/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 02/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use conrod_core::color;
use conrod_core::color::Color;

pub const BASE_COLOR: Color = color::Color::Rgba(0.267, 0.224, 1., 1.);
pub const BLACK: Color = color::Color::Rgba(0.004, 0., 0.043, 1.);
pub const WHITE: Color = color::Color::Rgba(1., 1., 1., 1.);

// .rgba-primary-0 { color: rgba(  4, 34, 67,1) }
// .rgba-primary-1 { color: rgba(  4, 22, 42,1) }
// .rgba-primary-2 { color: rgba(  3, 26, 52,1) }
// .rgba-primary-3 { color: rgba(  6, 51,101,1) }
// .rgba-primary-4 { color: rgba(  8, 69,138,1) }
