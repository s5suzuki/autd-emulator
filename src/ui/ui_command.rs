/*
 * File: command.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 02/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use vecmath::Vector3;

pub enum UICommand {
    CameraMove(Vector3<f64>),
    CameraRotate(Vector3<f64>),
    CameraSetPosture(Vector3<f64>, Vector3<f64>),
}
