/*
 * File: setting.rs
 * Project: sound_source
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 11/05/2020
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

use crate::common::coloring_method::ColoringMethod;
use scarlet::colormap::ListedColorMap;

#[derive(Debug, Clone)]
pub struct ViewerSettings {
    pub frequency: f32,
    pub source_size: f32,
    pub wave_length: f32,
    pub trans_coloring: ColoringMethod,
    pub field_color_map: ListedColorMap,
    pub color_scale: f32,
    pub slice_alpha: f32,
}

impl ViewerSettings {
    pub fn new(
        frequency: f32,
        source_size: f32,
        trans_coloring: ColoringMethod,
        field_color_map: ListedColorMap,
    ) -> ViewerSettings {
        ViewerSettings {
            frequency,
            source_size,
            wave_length: 340e3 / frequency,
            trans_coloring,
            field_color_map,
            color_scale: 1.0,
            slice_alpha: 1.0,
        }
    }
}
