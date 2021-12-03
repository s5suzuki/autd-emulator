/*
 * File: update_flag.rs
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

bitflags! {
    pub struct UpdateFlag: u32 {
        const UPDATE_SOURCE_DRIVE = 1 << 1;
        const UPDATE_COLOR_MAP = 1 << 2;
        const UPDATE_WAVENUM = 1 << 3;
        const UPDATE_CAMERA_POS = 1 << 4;
        const UPDATE_SLICE_POS = 1 << 5;
        const UPDATE_SLICE_SIZE = 1 << 6;
        const UPDATE_SOURCE_ALPHA = 1 << 7;
        const UPDATE_SOURCE_FLAG = 1 << 8;
        const INIT_SOURCE = 1 << 9;
        const INIT_AXIS = 1 << 10;
        const UPDATE_AXIS_SIZE = 1 << 11;
        const UPDATE_AXIS_FLAG = 1 << 12;
    }
}
