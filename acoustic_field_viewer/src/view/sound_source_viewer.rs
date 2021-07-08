/*
 * File: sound_source_viewer.rs
 * Project: view
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 08/07/2021
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
    state::{Blend, ColorMask},
    texture::{FilterMethod, SamplerInfo, WrapMode},
    traits::*,
    BlendTarget, DepthTarget, Global, PipelineState, Slice, TextureSampler, VertexBuffer,
};
use gfx_device_gl::Resources;
use piston_window::*;
use shader_version::{glsl::GLSL, Shaders};

use crate::{
    sound_source::SoundSource,
    view::{UpdateFlag, ViewerSettings},
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

fn alpha_blender() -> Blend {
    use gfx::state::{BlendValue, Equation, Factor};
    Blend::new(
        Equation::Add,
        Factor::ZeroPlus(BlendValue::SourceAlpha),
        Factor::OneMinus(BlendValue::SourceAlpha),
    )
}

gfx_pipeline!( pipe {
    vertex_buffer: VertexBuffer<Vertex> = (),
    u_model_view_proj: Global<[[f32; 4]; 4]> = "u_model_view_proj",
    t_color: TextureSampler<[f32; 4]> = "t_color",
    i_color: Global<[f32; 4]> = "i_Color",
    out_color: BlendTarget<format::Srgba8> = ("o_Color", ColorMask::all(), alpha_blender()),
    out_depth: DepthTarget<format::DepthStencil> = depth::LESS_EQUAL_WRITE,
});

pub struct SoundSourceViewer {
    pipe_data_list: Vec<pipe::Data<Resources>>,
    pso_slice: (PipelineState<Resources, pipe::Meta>, Slice<Resources>),
    models: Vec<Matrix4>,
    vertex_buffer: Buffer<Resources, Vertex>,
    view: ShaderResourceView<Resources, [f32; 4]>,
}

impl SoundSourceViewer {
    pub fn new(window: &mut PistonWindow, opengl: OpenGL) -> SoundSourceViewer {
        let factory = &mut window.factory.clone();

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
        let pso_slice = Self::initialize_shader(factory, glsl, slice);

        let assets = find_folder::Search::ParentsThenKids(3, 3)
            .for_folder("assets")
            .unwrap();
        let circle: G2dTexture = Texture::from_path(
            &mut window.create_texture_context(),
            assets.join("textures/circle.png"),
            Flip::None,
            &TextureSettings::new(),
        )
        .unwrap();

        let vertex_buffer = vertex_buffer;
        let view = circle.view;

        SoundSourceViewer {
            pipe_data_list: vec![],
            pso_slice,
            models: vec![],
            vertex_buffer,
            view,
        }
    }

    fn init_model(&mut self, settings: &ViewerSettings, sources: &[SoundSource]) {
        let len = sources.len();
        let s = 0.5 * settings.source_size;
        self.models = vec![vecmath_util::mat4_scale(s); len];
    }

    pub fn update(
        &mut self,
        window: &mut PistonWindow,
        event: &Event,
        view_projection: (Matrix4, Matrix4),
        settings: &ViewerSettings,
        sources: &[SoundSource],
        update_flag: UpdateFlag,
    ) {
        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE) {
            if self.pipe_data_list.len() != sources.len() {
                let factory = &mut window.factory;
                self.pipe_data_list = Self::initialize_pipe_data(
                    factory,
                    self.vertex_buffer.clone(),
                    self.view.clone(),
                    window.output_color.clone(),
                    window.output_stencil.clone(),
                    sources,
                );
            }

            let coloring_method = settings.trans_coloring;
            for (i, source) in sources.iter().enumerate() {
                self.pipe_data_list[i].i_color =
                    coloring_method(source.phase / (2.0 * PI), source.amp);
            }
        }

        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_POS) {
            if self.models.len() != sources.len() {
                self.init_model(settings, sources);
            }

            for (i, source) in sources.iter().enumerate() {
                self.models[i][3][0] = source.pos[0];
                self.models[i][3][1] = source.pos[1];
                self.models[i][3][2] = source.pos[2];
                let rot = vecmath_util::quaternion_to([0., 0., 1.], source.dir);
                let rotm = vecmath_util::mat4_rot(rot);
                self.models[i] = vecmath::col_mat4_mul(self.models[i], rotm);
            }
            for i in 0..self.pipe_data_list.len() {
                self.pipe_data_list[i].u_model_view_proj =
                    model_view_projection(self.models[i], view_projection.0, view_projection.1);
            }
        } else if update_flag.contains(UpdateFlag::UPDATE_CAMERA_POS) {
            for i in 0..self.pipe_data_list.len() {
                self.pipe_data_list[i].u_model_view_proj =
                    model_view_projection(self.models[i], view_projection.0, view_projection.1);
            }
        }

        if event.resize_args().is_some() {
            for pipe_data in &mut self.pipe_data_list {
                pipe_data.out_color = window.output_color.clone();
                pipe_data.out_depth = window.output_stencil.clone();
            }
        }
    }

    pub fn renderer(&mut self, window: &mut PistonWindow) {
        for i in 0..self.pipe_data_list.len() {
            window.encoder.draw(
                &self.pso_slice.1,
                &self.pso_slice.0,
                &self.pipe_data_list[i],
            );
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
        slice: Slice<Resources>,
    ) -> (PipelineState<Resources, pipe::Meta>, Slice<Resources>) {
        (
            factory
                .create_pipeline_simple(
                    Shaders::new()
                        .set(
                            GLSL::V4_50,
                            include_str!("../../../assets/shaders/circle.vert"),
                        )
                        .get(version)
                        .unwrap()
                        .as_bytes(),
                    Shaders::new()
                        .set(
                            GLSL::V4_50,
                            include_str!("../../../assets/shaders/circle.frag"),
                        )
                        .get(version)
                        .unwrap()
                        .as_bytes(),
                    pipe::new(),
                )
                .unwrap(),
            slice,
        )
    }
}
