/*
 * File: command.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 05/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use vecmath::Vector3;

pub enum UICommand {
    CameraMove(Vector3<f64>),
    CameraMoveTo(Vector3<f32>),
    CameraRotate(Vector3<f32>),
    CameraSetPosture {
        right: Vector3<f32>,
        up: Vector3<f32>,
    },
    CameraPos(Vector3<f32>),
    CameraUpdate,

    SliceMove(Vector3<f32>),
    SlicePos(Vector3<f32>),
    SliceRotate(Vector3<f32>),
    SliceSetPosture {
        right: Vector3<f32>,
        up: Vector3<f32>,
    },
}
