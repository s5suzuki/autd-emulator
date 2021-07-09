/*
 * File: update_flag.rs
 * Project: view
 * Created Date: 07/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 09/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

bitflags! {
    pub struct UpdateFlag: u32 {
        const UPDATE_SOURCE_POS = 1;
        const UPDATE_SOURCE_DRIVE = 1 << 1;
        const UPDATE_COLOR_MAP = 1 << 2;
        const UPDATE_WAVENUM = 1 << 3;
        const UPDATE_CAMERA_POS = 1 << 4;
        const UPDATE_SLICE_POS = 1 << 5;
        const UPDATE_SLICE_SIZE = 1 << 6;
        const UPDATE_SOURCE_ALPHA = 1 << 7;
    }
}
