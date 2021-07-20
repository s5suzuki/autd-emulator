/*
* File: offscreen_render_sys.rs
* Project: src
* Created Date: 12/07/2021
* Author: Shun Suzuki
* -----
* Last Modified: 12/07/2021
* Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
* -----
* Copyright (c) 2021 Hapis Lab. All rights reserved.
*
*/

use acoustic_field_viewer::view::UpdateFlag;
use imgui::ImString;

use crate::settings::Setting;
pub use std::path::Path;

pub struct OffscreenRenderSystem {
    pub(crate) offscreen_renderer: offscreen_renderer::OffscreenRenderer,
    pub(crate) save_path: ImString,
    pub(crate) record_path: ImString,
    pub(crate) recording: bool,
    pub(crate) update_flag_for_save: UpdateFlag,
}

impl OffscreenRenderSystem {
    pub fn new(setting: &Setting) -> Self {
        Self {
            offscreen_renderer: offscreen_renderer::OffscreenRenderer::new(),
            save_path: ImString::new(&setting.save_file_path),
            record_path: ImString::new(&setting.record_path),
            recording: false,
            update_flag_for_save: UpdateFlag::all(),
        }
    }
}
