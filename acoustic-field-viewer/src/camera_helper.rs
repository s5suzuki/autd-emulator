/*
 * File: camera_helper.rs
 * Project: src
 * Created Date: 26/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 26/11/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use camera_controllers::Camera;

use crate::{Matrix3, Vector3};

pub fn set_camera_angle(camera: &mut Camera<f32>, angle: Vector3) {
    let rot = quaternion::euler_angles(angle[0], angle[1], angle[2]);
    let model = vecmath_util::mat4_rot(rot);
    camera.right = vecmath_util::to_vec3(&model[0]);
    camera.up = vecmath_util::to_vec3(&model[1]);
    camera.forward = vecmath_util::to_vec3(&model[2]);
}

pub fn rot_mat_to_euler_angles(mat: &Matrix3) -> Vector3 {
    let sy = (mat[0][0] * mat[0][0] + mat[1][0] * mat[1][0]).sqrt();
    if sy < 1e-3 {
        let x = (mat[1][1]).atan2(mat[1][2]);
        let y = (mat[2][0]).atan2(sy);
        let z = 0.;
        [x, y, z]
    } else {
        let x = (-mat[2][1]).atan2(mat[2][2]);
        let y = (-mat[2][0]).atan2(sy);
        let z = (mat[1][0]).atan2(mat[0][0]);
        [x, y, z]
    }
}
