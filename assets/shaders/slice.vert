/*
 * File: slice.vert
 * Project: shaders
 * Created Date: 26/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 30/11/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 * 
 */

#version 450

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 tex_coords;

layout(location = 0) out vec2 o_tex_coords;

layout(set = 0, binding = 0) uniform Data {
    mat4 world;
    mat4 view;
    mat4 proj;
} u_model_view_proj;

void main() {
    mat4 worldview = u_model_view_proj.view * u_model_view_proj.world;
    gl_Position = u_model_view_proj.proj * worldview * vec4(position, 1.0);
    o_tex_coords = tex_coords;
}
