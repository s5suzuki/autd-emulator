/*
 * File: direction.rs
 * Project: src
 * Created Date: 16/09/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 16/09/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use crate::Vector3;

#[derive(Debug, Clone, Copy)]
pub struct Axis3D {
    pub pos: Vector3,
    pub x: Vector3,
    pub y: Vector3,
    pub z: Vector3,
    pub show: bool,
}

impl Axis3D {
    pub fn new(pos: Vector3, x: Vector3, y: Vector3, z: Vector3) -> Axis3D {
        Axis3D {
            pos,
            x,
            y,
            z,
            show: false,
        }
    }
}
