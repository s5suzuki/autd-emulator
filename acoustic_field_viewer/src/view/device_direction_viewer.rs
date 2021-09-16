/*
 * File: device_direction_viewer.rs
 * Project: view
 * Created Date: 16/09/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 16/09/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

extern crate gfx;

use camera_controllers::model_view_projection;
use gfx::{
    format,
    handle::{Buffer, DepthStencilView, RenderTargetView},
    preset::depth,
    state::ColorMask,
    traits::*,
    BlendTarget, DepthTarget, Global, PipelineState, Slice, VertexBuffer,
};
use gfx_device_gl::{CommandBuffer, Resources};
use glutin::event::{Event, WindowEvent};
use shader_version::{glsl::GLSL, OpenGL, Shaders};

use crate::{
    axis_3d::Axis3D,
    view::{render_system, render_system::RenderSystem, UpdateFlag, ViewerSettings},
    Matrix4,
};

gfx_vertex_struct!(Vertex {
    a_pos: [i8; 4] = "a_pos",
});

impl Vertex {
    fn new(pos: [i8; 3], _: [i8; 2]) -> Vertex {
        Vertex {
            a_pos: [pos[0], pos[1], pos[2], 1],
        }
    }
}

gfx_pipeline!( pipe {
    vertex_buffer: VertexBuffer<Vertex> = (),
    u_model_view_proj: Global<[[f32; 4]; 4]> = "u_model_view_proj",
    i_color: Global<[f32; 4]> = "i_Color",
    out_color: BlendTarget<format::Srgba8> = ("o_Color", ColorMask::all(), gfx::preset::blend::ALPHA),
    out_depth: DepthTarget<format::DepthStencil> = depth::LESS_EQUAL_WRITE,
});

pub struct DeviceDirectionViewer {
    pipe_data_list: Vec<pipe::Data<Resources>>,
    pso: PipelineState<Resources, pipe::Meta>,
    slice: Slice<Resources>,
    models: Vec<Matrix4>,
    vertex_buffer: Buffer<Resources, Vertex>,
}

impl DeviceDirectionViewer {
    pub fn new(render_sys: &RenderSystem, opengl: OpenGL) -> DeviceDirectionViewer {
        let mut factory = render_sys.factory.clone();

        let vertex_data = [
            Vertex::new([0, 0, 1], [0, 0]),
            Vertex::new([1, 0, 1], [1, 0]),
            Vertex::new([1, 1, 1], [1, 1]),
            Vertex::new([0, 1, 1], [0, 1]),
            Vertex::new([0, 1, 0], [1, 0]),
            Vertex::new([1, 1, 0], [0, 0]),
            Vertex::new([1, 0, 0], [0, 1]),
            Vertex::new([0, 0, 0], [1, 1]),
            Vertex::new([1, 0, 0], [0, 0]),
            Vertex::new([1, 1, 0], [1, 0]),
            Vertex::new([1, 1, 1], [1, 1]),
            Vertex::new([1, 0, 1], [0, 1]),
            Vertex::new([0, 0, 1], [1, 0]),
            Vertex::new([0, 1, 1], [0, 0]),
            Vertex::new([0, 1, 0], [0, 1]),
            Vertex::new([0, 0, 0], [1, 1]),
            Vertex::new([1, 1, 0], [1, 0]),
            Vertex::new([0, 1, 0], [0, 0]),
            Vertex::new([0, 1, 1], [0, 1]),
            Vertex::new([1, 1, 1], [1, 1]),
            Vertex::new([1, 0, 1], [0, 0]),
            Vertex::new([0, 0, 1], [1, 0]),
            Vertex::new([0, 0, 0], [1, 1]),
            Vertex::new([1, 0, 0], [0, 1]),
        ];
        let index_data: &[u16] = &[
            0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 8, 9, 10, 10, 11, 8, 12, 13, 14, 14, 15, 12, 16,
            17, 18, 18, 19, 16, 20, 21, 22, 22, 23, 20,
        ];
        let (vertex_buffer, slice) =
            factory.create_vertex_buffer_with_slice(&vertex_data, index_data);

        let glsl = opengl.to_glsl();
        let pso = Self::initialize_shader(&mut factory, glsl);

        DeviceDirectionViewer {
            pipe_data_list: vec![],
            pso,
            slice,
            models: vec![],
            vertex_buffer,
        }
    }

    fn init_model(&mut self, settings: &ViewerSettings, axis: &[Axis3D]) {
        let len = axis.len();
        let mut models = Vec::with_capacity(len * 3);
        for a in axis.iter() {
            let mut model = vecmath_util::mat4_t(a.pos);
            model = vecmath::col_mat4_mul(
                model,
                vecmath_util::mat4_rot(vecmath_util::quaternion_to(a.z, [0., 0., 1.])),
            );
            models.push(model);
            models.push(model);
            models.push(model);
        }

        for k in 0..len {
            let s = vecmath_util::mat4_scale([
                settings.axis_length,
                settings.axis_width,
                settings.axis_width,
            ]);
            models[3 * k] = vecmath::col_mat4_mul(models[3 * k], s);
        }
        for k in 0..len {
            let s = vecmath_util::mat4_scale([
                settings.axis_width,
                settings.axis_length,
                settings.axis_width,
            ]);
            models[3 * k + 1] = vecmath::col_mat4_mul(models[3 * k + 1], s);
        }
        for k in 0..len {
            let s = vecmath_util::mat4_scale([
                settings.axis_width,
                settings.axis_width,
                settings.axis_length,
            ]);
            models[3 * k + 2] = vecmath::col_mat4_mul(models[3 * k + 2], s);
        }

        self.models = models;
    }

    pub fn update(
        &mut self,
        render_sys: &mut RenderSystem,
        view_projection: (Matrix4, Matrix4),
        settings: &ViewerSettings,
        axis: &[Axis3D],
        update_flag: UpdateFlag,
    ) {
        if update_flag.contains(UpdateFlag::INIT_AXIS) {
            self.pipe_data_list = Self::initialize_pipe_data(
                self.vertex_buffer.clone(),
                render_sys.output_color.clone(),
                render_sys.output_stencil.clone(),
                axis,
            );
            self.update_axis_visual(axis);
            self.init_model(settings, axis);
            for i in 0..self.pipe_data_list.len() {
                self.pipe_data_list[i].u_model_view_proj =
                    model_view_projection(self.models[i], view_projection.0, view_projection.1);
            }
        }

        if update_flag.contains(UpdateFlag::UPDATE_AXIS_SIZE) {
            self.init_model(settings, axis);
            for i in 0..self.pipe_data_list.len() {
                self.pipe_data_list[i].u_model_view_proj =
                    model_view_projection(self.models[i], view_projection.0, view_projection.1);
            }
        }

        if update_flag.contains(UpdateFlag::UPDATE_AXIS_FLAG) {
            self.update_axis_visual(axis);
        }

        if update_flag.contains(UpdateFlag::UPDATE_CAMERA_POS) {
            for i in 0..self.pipe_data_list.len() {
                self.pipe_data_list[i].u_model_view_proj =
                    model_view_projection(self.models[i], view_projection.0, view_projection.1);
            }
        }
    }

    pub fn handle_event(&mut self, render_sys: &RenderSystem, event: &Event<()>) {
        if let Event::WindowEvent {
            event: WindowEvent::Resized(_),
            ..
        } = event
        {
            for pipe_data in &mut self.pipe_data_list {
                pipe_data.out_color = render_sys.output_color.clone();
                pipe_data.out_depth = render_sys.output_stencil.clone();
            }
        }
    }

    pub fn renderer(
        &mut self,
        encoder: &mut gfx::Encoder<render_system::types::Resources, CommandBuffer>,
    ) {
        for i in 0..self.pipe_data_list.len() {
            encoder.draw(&self.slice, &self.pso, &self.pipe_data_list[i]);
        }
    }

    fn initialize_pipe_data(
        vertex_buffer: Buffer<Resources, Vertex>,
        out_color: RenderTargetView<Resources, (format::R8_G8_B8_A8, format::Srgb)>,
        out_depth: DepthStencilView<Resources, (format::D24_S8, format::Unorm)>,
        axis: &[Axis3D],
    ) -> Vec<pipe::Data<Resources>> {
        vec![
            pipe::Data {
                vertex_buffer,
                u_model_view_proj: [[0.; 4]; 4],
                i_color: [0., 0., 0., 1.],
                out_color,
                out_depth,
            };
            axis.len() * 3
        ]
    }

    fn update_axis_visual(&mut self, axis: &[Axis3D]) {
        for (i, a) in axis.iter().enumerate() {
            self.pipe_data_list[3 * i].i_color = [1., 0., 0., if a.show { 1.0 } else { 0.0 }];
        }
        for (i, a) in axis.iter().enumerate() {
            self.pipe_data_list[3 * i + 1].i_color = [0., 1., 0., if a.show { 1.0 } else { 0.0 }];
        }
        for (i, a) in axis.iter().enumerate() {
            self.pipe_data_list[3 * i + 2].i_color = [0., 0., 1., if a.show { 1.0 } else { 0.0 }];
        }
    }

    fn initialize_shader(
        factory: &mut gfx_device_gl::Factory,
        version: GLSL,
    ) -> PipelineState<Resources, pipe::Meta> {
        factory
            .create_pipeline_simple(
                Shaders::new()
                    .set(
                        GLSL::V1_50,
                        include_str!("../../../assets/shaders/cube.vert"),
                    )
                    .get(version)
                    .unwrap()
                    .as_bytes(),
                Shaders::new()
                    .set(
                        GLSL::V1_50,
                        include_str!("../../../assets/shaders/cube.frag"),
                    )
                    .get(version)
                    .unwrap()
                    .as_bytes(),
                pipe::new(),
            )
            .unwrap()
    }
}
