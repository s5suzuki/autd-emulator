/*
 * File: camera_helper.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 12/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use vecmath_utils::{mat4, vec3};

use crate::Vector3;
use camera_controllers::Camera;

pub fn camera_move(camera: &mut Camera, t: Vector3) {
    camera.position = vec3::add(camera.position, mat4::mul_vec3(camera.orthogonal(), t));
}

pub fn camera_move_to(camera: &mut Camera, t: Vector3) {
    camera.position = t;
}

pub fn camera_rotate(camera: &mut Camera, axis: Vector3, theta: f32) {
    let axis = mat4::mul_vec3(camera.orthogonal(), axis);
    let rot = quaternion::axis_angle(axis, theta);
    camera.forward = quaternion::rotate_vector(rot, camera.forward);
    camera.right = quaternion::rotate_vector(rot, camera.right);
    camera.up = quaternion::rotate_vector(rot, camera.up);
}
