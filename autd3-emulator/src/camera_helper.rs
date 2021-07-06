/*
 * File: camera_helper.rs
 * Project: src
 * Created Date: 01/05/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 06/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use crate::Vector3;
use camera_controllers::Camera;

pub fn camera_move(camera: &mut Camera, t: Vector3) {
    let mut t = vecmath_util::mat4_transform_vec3(camera.orthogonal(), t);
    t[2] *= -1.0;
    camera.position = vecmath::vec3_add(camera.position, t);
}

pub fn camera_move_to(camera: &mut Camera, t: Vector3) {
    camera.position = t;
}

pub fn camera_rotate(camera: &mut Camera, axis: Vector3, theta: f32) {
    let axis = vecmath_util::mat4_transform_vec3(camera.orthogonal(), axis);
    let rot = quaternion::axis_angle(axis, theta);
    camera.forward = quaternion::rotate_vector(rot, camera.forward);
    camera.right = quaternion::rotate_vector(rot, camera.right);
    camera.up = quaternion::rotate_vector(rot, camera.up);
}
