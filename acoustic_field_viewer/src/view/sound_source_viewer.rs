/*
 * File: sound_source_viewer.rs
 * Project: view
 * Created Date: 27/04/2020
 * Author: Shun Suzuki
 * -----
 * Last Modified: 07/07/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2020 Hapis Lab. All rights reserved.
 *
 */

extern crate gfx;

use std::f32::consts::PI;

use camera_controllers::model_view_projection;
use gfx::format;
use gfx::handle::{Buffer, DepthStencilView, RenderTargetView, ShaderResourceView};
use gfx::preset::depth;
use gfx::state::{Blend, ColorMask};
use gfx::texture::{FilterMethod, SamplerInfo, WrapMode};
use gfx::traits::*;
use gfx::{BlendTarget, DepthTarget, Global, PipelineState, Slice, TextureSampler, VertexBuffer};
use gfx_device_gl::Resources;
use piston_window::*;
use shader_version::glsl::GLSL;
use shader_version::Shaders;

use std::cell::RefCell;
use std::rc::Weak;

use crate::sound_source::SoundSource;
use crate::view::ViewerSettings;
use crate::Matrix4;

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
    pub(crate) sources: Weak<RefCell<Vec<SoundSource>>>,
    pub(crate) settings: Weak<RefCell<ViewerSettings>>,
    pipe_data_list: Vec<pipe::Data<Resources>>,
    pso_slice: Option<(PipelineState<Resources, pipe::Meta>, Slice<Resources>)>,
    models: Vec<Matrix4>,
    position_updated: bool,
    drive_updated: bool,
    vertex_buffer: Option<Buffer<Resources, Vertex>>,
    view: Option<ShaderResourceView<Resources, [f32; 4]>>,
}

impl SoundSourceViewer {
    pub fn new() -> SoundSourceViewer {
        SoundSourceViewer {
            sources: Weak::new(),
            settings: Weak::new(),
            pipe_data_list: vec![],
            pso_slice: None,
            models: vec![],
            position_updated: true,
            drive_updated: true,
            vertex_buffer: None,
            view: None,
        }
    }

    pub fn render_setting(&mut self, window: &mut PistonWindow, opengl: OpenGL) {
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
        self.initialize_shader(factory, glsl, slice);

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

        self.vertex_buffer = Some(vertex_buffer.clone());
        self.view = Some(circle.view.clone());

        self.initialize_pipe_data(
            factory,
            vertex_buffer,
            circle.view,
            window.output_color.clone(),
            window.output_stencil.clone(),
        );

        self.update_drive();
        self.update_position();
    }

    pub(crate) fn init_model(&mut self) {
        let len = self.sources.upgrade().unwrap().borrow().len();
        let s = 0.5 * self.settings.upgrade().unwrap().borrow().source_size;
        self.models = vec![vecmath_util::mat4_scale(s); len];
    }

    pub fn update_position(&mut self) {
        if self.models.len() != self.sources.upgrade().unwrap().borrow().len() {
            self.init_model();
        }

        for (i, source) in self.sources.upgrade().unwrap().borrow().iter().enumerate() {
            self.models[i][3][0] = source.pos[0];
            self.models[i][3][1] = source.pos[1];
            self.models[i][3][2] = source.pos[2];
            let rot = vecmath_util::quaternion_to([0., 0., 1.], source.dir);
            let rotm = vecmath_util::mat4_rot(rot);
            self.models[i] = vecmath::col_mat4_mul(self.models[i], rotm);
        }
        self.position_updated = true;
    }

    pub fn camera_pos_update(&mut self) {
        self.position_updated = true;
    }

    pub fn update_drive(&mut self) {
        self.drive_updated = true;
    }

    pub fn renderer(
        &mut self,
        window: &mut PistonWindow,
        event: &Event,
        view: Matrix4,
        projection: Matrix4,
    ) {
        if self.drive_updated {
            if self.pipe_data_list.len() != self.sources.upgrade().unwrap().borrow().len() {
                let factory = &mut window.factory;
                self.initialize_pipe_data(
                    factory,
                    self.vertex_buffer.clone().unwrap(),
                    self.view.clone().unwrap(),
                    window.output_color.clone(),
                    window.output_stencil.clone(),
                );
            }

            let coloring_method = self.settings.upgrade().unwrap().borrow().trans_coloring;
            for (i, source) in self.sources.upgrade().unwrap().borrow().iter().enumerate() {
                self.pipe_data_list[i].i_color =
                    coloring_method(source.phase / (2.0 * PI), source.amp);
            }
        }

        if self.position_updated {
            for i in 0..self.pipe_data_list.len() {
                self.pipe_data_list[i].u_model_view_proj =
                    model_view_projection(self.models[i], view, projection);
            }
            self.position_updated = false;
        }

        if let Some(pso_slice) = &self.pso_slice {
            for i in 0..self.pipe_data_list.len() {
                window
                    .encoder
                    .draw(&pso_slice.1, &pso_slice.0, &self.pipe_data_list[i]);
            }
        }

        if event.resize_args().is_some() {
            for pipe_data in &mut self.pipe_data_list {
                pipe_data.out_color = window.output_color.clone();
                pipe_data.out_depth = window.output_stencil.clone();
            }
        }
    }

    fn initialize_pipe_data(
        &mut self,
        factory: &mut gfx_device_gl::Factory,
        vertex_buffer: Buffer<Resources, Vertex>,
        view: ShaderResourceView<Resources, [f32; 4]>,
        out_color: RenderTargetView<Resources, (format::R8_G8_B8_A8, format::Srgb)>,
        out_depth: DepthStencilView<Resources, (format::D24_S8, format::Unorm)>,
    ) {
        let sampler_info = SamplerInfo::new(FilterMethod::Bilinear, WrapMode::Clamp);
        self.pipe_data_list = vec![
            pipe::Data {
                vertex_buffer,
                u_model_view_proj: [[0.; 4]; 4],
                t_color: (view, factory.create_sampler(sampler_info)),
                i_color: [0., 0., 0., 1.],
                out_color,
                out_depth,
            };
            self.sources.upgrade().unwrap().borrow().len()
        ]
    }

    fn initialize_shader(
        &mut self,
        factory: &mut gfx_device_gl::Factory,
        version: GLSL,
        slice: Slice<Resources>,
    ) {
        self.pso_slice = Some((
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
        ));
    }
}

impl Default for SoundSourceViewer {
    fn default() -> Self {
        Self::new()
    }
}
