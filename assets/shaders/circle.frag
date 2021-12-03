/*
 * File: circle.frag
 * Project: shaders
 * Created Date: 26/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 01/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 * 
 */

#version 450 core

layout(location = 0) in vec2 v_tex_coords;
layout(location = 1) in vec4 i_color;

layout(location = 0) out vec4 o_color;

layout(set = 1, binding = 0) uniform sampler2D t_color;

void main() {
    vec4 tex = texture(t_color, v_tex_coords);
    o_color = i_color * tex;
}
