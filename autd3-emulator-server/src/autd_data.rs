/*
 * File: autd_data.rs
 * Project: src
 * Created Date: 07/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 08/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use crate::Vector3;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Modulation {
    pub(crate) mod_data: Vec<u8>,
}

#[derive(Debug)]
pub struct Gain {
    pub amps: Vec<u8>,
    pub phases: Vec<u8>,
}

#[derive(Debug)]
pub struct Geometry {
    pub origin: Vector3,
    pub right: Vector3,
    pub up: Vector3,
}

#[derive(Debug)]
pub enum AUTDData {
    Modulation(Modulation),
    Gain(Gain),
    Geometries(Vec<Geometry>),
    Clear,
    Pause,
    Resume,
}
