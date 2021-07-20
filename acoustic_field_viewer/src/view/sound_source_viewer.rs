/*
 * File: sound_source_viewer.rs
 * Project: view
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 20/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

extern crate gfx;

use std::f32::consts::PI;

use camera_controllers::model_view_projection;
use gfx::{
    format,
    handle::{Buffer, DepthStencilView, RenderTargetView, ShaderResourceView},
    preset::depth,
    state::ColorMask,
    texture::{FilterMethod, SamplerInfo, WrapMode},
    traits::*,
    BlendTarget, DepthTarget, Global, PipelineState, Slice, TextureSampler, VertexBuffer,
};
use gfx_device_gl::{CommandBuffer, Resources};
use glutin::event::{Event, WindowEvent};
use shader_version::{glsl::GLSL, OpenGL, Shaders};

use crate::{
    common::coloring_method::{coloring_hsv, ColoringMethod},
    common::texture::create_texture_resource,
    sound_source::SoundSource,
    view::{render_system, render_system::RenderSystem, UpdateFlag, ViewerSettings},
    Matrix4,
};

gfx_vertex_struct!(Vertex {
    a_pos: [i8; 4] = "a_pos",
    a_tex_coord: [i8; 2] = "a_tex_coord",
});

impl Vertex {
    fn new(pos: [i8; 3], tc: [i8; 2]) -> Vertex {
        Vertex {
            a_pos: [pos[0], pos[1], pos[2], 1],
            a_tex_coord: tc,
        }
    }
}

gfx_pipeline!( pipe {
    vertex_buffer: VertexBuffer<Vertex> = (),
    u_model_view_proj: Global<[[f32; 4]; 4]> = "u_model_view_proj",
    t_color: TextureSampler<[f32; 4]> = "t_color",
    i_color: Global<[f32; 4]> = "i_Color",
    out_color: BlendTarget<format::Srgba8> = ("o_Color", ColorMask::all(), gfx::preset::blend::ALPHA),
    out_depth: DepthTarget<format::DepthStencil> = depth::LESS_EQUAL_WRITE,
});

pub struct SoundSourceViewer {
    pipe_data_list: Vec<pipe::Data<Resources>>,
    pso: PipelineState<Resources, pipe::Meta>,
    slice: Slice<Resources>,
    models: Vec<Matrix4>,
    vertex_buffer: Buffer<Resources, Vertex>,
    view: ShaderResourceView<Resources, [f32; 4]>,
    coloring_method: ColoringMethod,
}

impl SoundSourceViewer {
    pub fn new(render_sys: &RenderSystem, opengl: OpenGL) -> SoundSourceViewer {
        let mut factory = render_sys.factory.clone();

        let vertex_data = vec![
            Vertex::new([-1, -1, 0], [0, 0]),
            Vertex::new([1, -1, 0], [1, 0]),
            Vertex::new([1, 1, 0], [1, 1]),
            Vertex::new([-1, 1, 0], [0, 1]),
        ];
        let index_data: &[u16] = &[0, 1, 2, 2, 3, 0];
        let (vertex_buffer, slice) =
            factory.create_vertex_buffer_with_slice(&vertex_data, index_data);

        let glsl = opengl.to_glsl();
        let pso = Self::initialize_shader(&mut factory, glsl);

        let assets = find_folder::Search::ParentsThenKids(3, 3)
            .for_folder("assets")
            .unwrap();
        let view =
            create_texture_resource(assets.join("textures/circle.png"), &mut factory).unwrap();

        SoundSourceViewer {
            pipe_data_list: vec![],
            pso,
            slice,
            models: vec![],
            vertex_buffer,
            view,
            coloring_method: coloring_hsv,
        }
    }

    fn init_model(&mut self, settings: &ViewerSettings, sources: &[SoundSource]) {
        let len = sources.len();
        let s = 0.5 * settings.source_size;
        self.models = vec![vecmath_util::mat4_scale(s); len];
    }

    pub fn update(
        &mut self,
        render_sys: &mut RenderSystem,
        view_projection: (Matrix4, Matrix4),
        settings: &ViewerSettings,
        sources: &[SoundSource],
        update_flag: UpdateFlag,
    ) {
        if update_flag.contains(UpdateFlag::INIT_SOURCE) {
            let factory = &mut render_sys.factory;
            self.pipe_data_list = Self::initialize_pipe_data(
                factory,
                self.vertex_buffer.clone(),
                self.view.clone(),
                render_sys.output_color.clone(),
                render_sys.output_stencil.clone(),
                sources,
            );
            self.init_model(settings, sources);
            for (i, source) in sources.iter().enumerate() {
                self.models[i][3][0] = source.pos[0];
                self.models[i][3][1] = source.pos[1];
                self.models[i][3][2] = source.pos[2];
                let rot = vecmath_util::quaternion_to(source.dir, [0., 0., 1.]);
                let rotm = vecmath_util::mat4_rot(rot);
                self.models[i] = vecmath::col_mat4_mul(self.models[i], rotm);
            }
            for i in 0..self.pipe_data_list.len() {
                self.pipe_data_list[i].u_model_view_proj =
                    model_view_projection(self.models[i], view_projection.0, view_projection.1);
            }
        }

        if update_flag.contains(UpdateFlag::UPDATE_CAMERA_POS) {
            for i in 0..self.pipe_data_list.len() {
                self.pipe_data_list[i].u_model_view_proj =
                    model_view_projection(self.models[i], view_projection.0, view_projection.1);
            }
        }

        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE) {
            for (i, source) in sources.iter().enumerate() {
                self.pipe_data_list[i].i_color = (self.coloring_method)(
                    source.phase / (2.0 * PI),
                    source.amp,
                    settings.source_alpha,
                );
            }
        }

        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_ALPHA) {
            for pipe_data in self.pipe_data_list.iter_mut() {
                pipe_data.i_color[3] = settings.source_alpha;
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
        factory: &mut gfx_device_gl::Factory,
        vertex_buffer: Buffer<Resources, Vertex>,
        view: ShaderResourceView<Resources, [f32; 4]>,
        out_color: RenderTargetView<Resources, (format::R8_G8_B8_A8, format::Srgb)>,
        out_depth: DepthStencilView<Resources, (format::D24_S8, format::Unorm)>,
        sources: &[SoundSource],
    ) -> Vec<pipe::Data<Resources>> {
        let sampler_info = SamplerInfo::new(FilterMethod::Bilinear, WrapMode::Clamp);
        vec![
            pipe::Data {
                vertex_buffer,
                u_model_view_proj: [[0.; 4]; 4],
                t_color: (view, factory.create_sampler(sampler_info)),
                i_color: [0., 0., 0., 1.],
                out_color,
                out_depth,
            };
            sources.len()
        ]
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
                        include_str!("../../../assets/shaders/circle.vert"),
                    )
                    .get(version)
                    .unwrap()
                    .as_bytes(),
                Shaders::new()
                    .set(
                        GLSL::V1_50,
                        include_str!("../../../assets/shaders/circle.frag"),
                    )
                    .get(version)
                    .unwrap()
                    .as_bytes(),
                pipe::new(),
            )
            .unwrap()
    }
}
