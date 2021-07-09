/*
 * File: system.rs
 * Project: view
 * Created Date: 08/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 09/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use gfx_device_gl::CommandBuffer;
use glutin::{dpi::LogicalSize, event_loop::EventLoop, window::WindowBuilder};
use imgui::{Context, FontConfig, FontGlyphRanges, FontSource};
use imgui_winit_support::{HiDpiMode, WinitPlatform};

use crate::view::render_system::RenderSystem;

use super::render_system::types::Resources;

type EventsLoop = EventLoop<()>;

pub struct System {
    pub events_loop: EventsLoop,
    pub imgui: Context,
    pub platform: WinitPlatform,
    pub render_sys: RenderSystem,
    pub font_size: f32,
    pub encoder: gfx::Encoder<Resources, CommandBuffer>,
}

impl System {
    pub fn init(title: &str, width: f64, heigh: f64) -> Self {
        let events_loop = EventsLoop::new();
        let builder = WindowBuilder::new()
            .with_title(title.to_owned())
            .with_inner_size(LogicalSize::new(width, heigh));

        let mut imgui = Context::create();
        // imgui.set_ini_filename(Some("imgui.ini"));

        let mut platform = WinitPlatform::init(&mut imgui);

        let hidpi_factor = platform.hidpi_factor();
        let font_size = (16.0 * hidpi_factor) as f32;
        imgui.fonts().add_font(&[FontSource::TtfData {
            data: include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf"),
            size_pixels: font_size,
            config: Some(FontConfig {
                rasterizer_multiply: 1.,
                glyph_ranges: FontGlyphRanges::japanese(),
                ..FontConfig::default()
            }),
        }]);

        imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        let mut render_sys = RenderSystem::init(&mut imgui, builder, &events_loop);
        platform.attach_window(imgui.io_mut(), render_sys.window(), HiDpiMode::Default);
        let encoder: gfx::Encoder<_, _> = render_sys.factory.create_command_buffer().into();
        System {
            events_loop,
            imgui,
            platform,
            render_sys,
            font_size,
            encoder,
        }
    }
}
