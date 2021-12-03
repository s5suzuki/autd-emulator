/*
 * File: slice.frag
 * Project: shaders
 * Created Date: 26/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 03/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 * 
 */

#version 450

layout(location = 0) in vec2 v_tex_coords;

layout(location = 0) out vec4 f_color;

layout(set = 0, binding = 1) buffer Config {
    uint width;
    uint height;
    uint dummy_0;
    uint dummy_1;
} config;
layout(set = 0, binding = 2) buffer Data {
    vec4 data[];
} data;

void main() {
  uint w = uint(floor(v_tex_coords.x * config.width));
  uint h = uint(floor(v_tex_coords.y * config.height));
  uint idx = w + config.width * h;
  f_color = data.data[idx];
}
