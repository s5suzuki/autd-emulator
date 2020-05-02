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

use acoustic_field_viewer::vec_utils::Vector3;

pub enum UICommand {
    CameraMove(Vector3),
    CameraRotate(Vector3),
    CameraSetPosture(Vector3, Vector3),
}
