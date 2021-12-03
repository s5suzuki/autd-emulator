/*
 * File: cube.vert
 * Project: shaders
 * Created Date: 19/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 01/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 * 
 */

#version 450 core

layout(location = 0) in vec3 position;

layout(location = 1) in mat4 model;
layout(location = 5) in vec4 color;

layout(location = 0) out vec4 o_color;

layout(set = 0, binding = 0) uniform Data {
    mat4 view;
    mat4 proj;
} u_view_proj;

void main() {
    o_color = color;
    mat4 worldview = u_view_proj.view * model;
    gl_Position = u_view_proj.proj * worldview * vec4(position, 1.0);
}
