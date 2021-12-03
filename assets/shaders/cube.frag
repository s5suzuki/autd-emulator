/*
 * File: cube.frag
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

layout(location = 0) in vec4 i_color;

layout(location = 0) out vec4 o_color;

void main() {
    o_color = i_color;
}
