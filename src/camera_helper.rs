/*
 * File: camera_helper.rs
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

use acoustic_field_viewer::vec_utils;
use acoustic_field_viewer::vec_utils::{Vector3, Vector4};
use camera_controllers::Camera;

type Matrix4 = vecmath::Matrix4<f32>;

pub fn camera_move(camera: &mut Camera, t: Vector3) {
    camera.position = vecmath::vec3_add(camera.position, mat4_mul_vec3(camera.orthogonal(), t));
    // camera.position = vecmath::vec3_add(camera.position, t);
}

pub fn camera_move_to(camera: &mut Camera, t: Vector3) {
    camera.position = t;
}

pub fn camera_rotate(camera: &mut Camera, axis: Vector3, theta: f32) {
    let rot = quaternion::axis_angle(axis, theta);
    camera.forward = quaternion::rotate_vector(rot, camera.forward);
    camera.right = quaternion::rotate_vector(rot, camera.right);
    camera.up = quaternion::rotate_vector(rot, camera.up);
}

pub fn mat4_mul_vec3(m: Matrix4, v: Vector3) -> Vector3 {
    to_vec3(vecmath::row_mat4_transform(m, vec_utils::to_vec4(v)))
}

pub fn to_vec3(v: Vector4) -> Vector3 {
    [v[0], v[1], v[2]]
}
