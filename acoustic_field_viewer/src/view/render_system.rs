/*
 * File: render_system.rs
 * Project: view
 * Created Date: 08/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 08/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use glutin::{event_loop::EventLoop, window::WindowBuilder};
use imgui::Context;
use imgui_gfx_renderer::{Renderer, Shaders};
use old_school_gfx_glutin_ext::*;

type ColorFormat = gfx::format::Srgba8;
type DepthFormat = gfx::format::DepthStencil;
type EventsLoop = EventLoop<()>;

pub mod types {
    pub type Device = gfx_device_gl::Device;
    pub type Factory = gfx_device_gl::Factory;
    pub type Resources = gfx_device_gl::Resources;
}

pub struct RenderSystem {
    pub renderer: Renderer<ColorFormat, types::Resources>,
    pub windowed_context: glutin::WindowedContext<glutin::PossiblyCurrent>,
    pub device: types::Device,
    pub factory: types::Factory,
    pub output_color: gfx::handle::RenderTargetView<types::Resources, ColorFormat>,
    pub output_stencil: gfx::handle::DepthStencilView<types::Resources, DepthFormat>,
}

impl RenderSystem {
    pub fn init(imgui: &mut Context, builder: WindowBuilder, events_loop: &EventsLoop) -> Self {
        {
            fn imgui_gamma_to_linear(col: [f32; 4]) -> [f32; 4] {
                let x = col[0].powf(2.2);
                let y = col[1].powf(2.2);
                let z = col[2].powf(2.2);
                let w = 1.0 - (1.0 - col[3]).powf(2.2);
                [x, y, z, w]
            }

            let style = imgui.style_mut();
            for col in 0..style.colors.len() {
                style.colors[col] = imgui_gamma_to_linear(style.colors[col]);
            }
        }

        let (windowed_context, device, mut factory, output_color, output_stencil) =
            glutin::ContextBuilder::new()
                .with_vsync(true)
                .with_gfx_color_depth::<ColorFormat, DepthFormat>()
                .build_windowed(builder, events_loop)
                .expect("Failed to initialize graphics")
                .init_gfx::<ColorFormat, DepthFormat>();

        let shaders = Shaders::GlSl400;
        let renderer =
            Renderer::init(imgui, &mut factory, shaders).expect("Failed to initialize renderer");
        Self {
            renderer,
            windowed_context,
            device,
            factory,
            output_color,
            output_stencil,
        }
    }
    pub fn window(&self) -> &glutin::window::Window {
        self.windowed_context.window()
    }
    pub fn update_views(&mut self) {
        self.windowed_context
            .update_gfx(&mut self.output_color, &mut self.output_stencil);
    }
    pub fn swap_buffers(&mut self) {
        self.windowed_context.swap_buffers().unwrap();
    }
}
