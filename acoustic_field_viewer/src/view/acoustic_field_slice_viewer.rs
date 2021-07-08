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

use camera_controllers::model_view_projection;
use gfx::{
    format::{self, Rgba32F},
    handle::{Buffer, DepthStencilView, RenderTargetView, ShaderResourceView},
    preset::depth,
    state::{Blend, ColorMask},
    texture::{FilterMethod, Kind, Mipmap, SamplerInfo, WrapMode},
    traits::*,
    BlendTarget, DepthTarget, Global, PipelineState, Slice, TextureSampler, VertexBuffer,
};
use gfx_device_gl::Resources;
use piston_window::*;
use scarlet::{color::RGBColor, colormap::ColorMap};
use shader_version::{glsl::GLSL, Shaders};

use crate::{
    sound_source::SoundSource,
    view::{UpdateFlag, ViewerSettings},
    Matrix4, Vector3,
};

gfx_vertex_struct!(Vertex {
    a_pos: [i16; 4] = "a_pos",
});

impl Vertex {
    fn new(pos: [i16; 3]) -> Vertex {
        Vertex {
            a_pos: [pos[0], pos[1], pos[2], 1],
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
    u_model: Global<[[f32; 4]; 4]> = "u_model",
    u_color_scale : Global<f32> = "u_color_scale",
    u_wavenum : Global<f32> = "u_wavenum",
    u_color_map: TextureSampler<[f32; 4]> = "u_color_map",
    u_trans_num : Global<f32> = "u_trans_num",
    u_trans_pos: TextureSampler<[f32; 4]> = "u_trans_pos",
    u_trans_drive: TextureSampler<[f32; 4]> = "u_trans_drive",
    out_color: BlendTarget<format::Srgba8> = ("o_Color", ColorMask::all(), alpha_blender()),
    out_depth: DepthTarget<format::DepthStencil> = depth::LESS_EQUAL_WRITE,
});

pub struct AcousticFiledSliceViewer {
    pipe_data: pipe::Data<Resources>,
    model: Matrix4,
    pso_slice: (PipelineState<Resources, pipe::Meta>, Slice<Resources>),
}

impl AcousticFiledSliceViewer {
    pub fn new(
        model: Matrix4,
        window: &PistonWindow,
        opengl: OpenGL,
        settings: &ViewerSettings,
    ) -> AcousticFiledSliceViewer {
        let factory = &mut window.factory.clone();

        let (width, height) = settings.size;

        let wl = (-width / 2).clamp(-32768, 0) as i16;
        let wr = ((width + 1) / 2).clamp(0, 32767) as i16;
        let hb = (-height / 2).clamp(-32768, 0) as i16;
        let ht = ((height + 1) / 2).clamp(0, 32767) as i16;
        let vertex_data = vec![
            Vertex::new([wl, hb, 0]),
            Vertex::new([wr, hb, 0]),
            Vertex::new([wr, ht, 0]),
            Vertex::new([wl, ht, 0]),
        ];
        let index_data: &[u16] = &[0, 1, 2, 2, 3, 0];
        let (vertex_buffer, slice) =
            factory.create_vertex_buffer_with_slice(&vertex_data, index_data);

        let glsl = opengl.to_glsl();

        let drive_view = AcousticFiledSliceViewer::generate_empty_view(factory);

        AcousticFiledSliceViewer {
            pipe_data: Self::initialize_pipe_data(
                factory,
                vertex_buffer,
                drive_view,
                window.output_color.clone(),
                window.output_stencil.clone(),
            ),
            model,
            pso_slice: Self::initialize_shader(factory, glsl, slice),
        }
    }

    pub fn translate(&mut self, travel: Vector3) {
        self.model[3][0] += travel[0];
        self.model[3][1] += travel[1];
        self.model[3][2] += travel[2];
    }

    pub fn set_posture(&mut self, right: Vector3, up: Vector3) {
        let forward = vecmath::vec3_cross(right, up);
        self.model[0] = vecmath_util::to_vec4(right);
        self.model[1] = vecmath_util::to_vec4(up);
        self.model[2] = vecmath_util::to_vec4(forward);
    }

    pub fn rotate(&mut self, axis: Vector3, rot: f32) {
        let rot = quaternion::axis_angle(axis, rot);
        let rotm = vecmath_util::mat4_rot(rot);
        self.model = vecmath::col_mat4_mul(self.model, rotm);
    }

    pub fn model(&self) -> Matrix4 {
        self.model
    }

    pub fn position(&self) -> Vector3 {
        vecmath_util::to_vec3(&self.model[3])
    }

    pub fn right(&self) -> Vector3 {
        vecmath::vec3_normalized(vecmath_util::to_vec3(&self.model[0]))
    }

    pub fn up(&self) -> Vector3 {
        vecmath::vec3_normalized(vecmath_util::to_vec3(&self.model[1]))
    }

    pub fn forward(&self) -> Vector3 {
        vecmath::vec3_normalized(vecmath_util::to_vec3(&self.model[2]))
    }

    pub fn renderer(
        &mut self,
        window: &mut PistonWindow,
        event: &Event,
        view_projection: (Matrix4, Matrix4),
        settings: &ViewerSettings,
        sources: &[SoundSource],
        update_flag: UpdateFlag,
    ) {
        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE) {
            AcousticFiledSliceViewer::update_drive_texture(
                &mut self.pipe_data,
                &mut window.factory,
                sources,
            );
        }

        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_POS) {
            self.pipe_data.u_trans_num = sources.len() as f32;
            AcousticFiledSliceViewer::update_position_texture(
                &mut self.pipe_data,
                &mut window.factory,
                sources,
            );
        }

        if update_flag.contains(UpdateFlag::UPDATE_COLOR_MAP) {
            let iter = (0..100).map(|x| x as f64 / 100.0);
            let colors = settings.field_color_map.transform(iter);
            let alpha = settings.slice_alpha;
            AcousticFiledSliceViewer::update_color_map_texture(
                &mut self.pipe_data,
                &mut window.factory,
                &colors,
                alpha,
            );
            self.pipe_data.u_color_scale = settings.color_scale;
        }

        if update_flag.contains(UpdateFlag::UPDATE_WAVENUM) {
            self.pipe_data.u_wavenum = 2.0 * std::f32::consts::PI / settings.wave_length;
        }

        if update_flag.contains(UpdateFlag::UPDATE_CAMERA_POS)
            || update_flag.contains(UpdateFlag::UPDATE_SLICE_POS)
        {
            self.pipe_data.u_model = self.model;
            self.pipe_data.u_model_view_proj =
                model_view_projection(self.model, view_projection.0, view_projection.1);
        }

        window.draw_3d(event, |window| {
            window
                .encoder
                .draw(&self.pso_slice.1, &self.pso_slice.0, &self.pipe_data);

            if event.resize_args().is_some() {
                self.pipe_data.out_color = window.output_color.clone();
                self.pipe_data.out_depth = window.output_stencil.clone();
            }
        });
    }

    fn update_drive_texture(
        data: &mut pipe::Data<gfx_device_gl::Resources>,
        factory: &mut gfx_device_gl::Factory,
        sources: &[SoundSource],
    ) {
        let sampler_info = SamplerInfo::new(FilterMethod::Scale, WrapMode::Tile);
        let mut texels = Vec::with_capacity(sources.len());
        for source in sources {
            texels.push([
                (source.phase / (2.0 * std::f32::consts::PI) * 255.) as u8,
                (source.amp * 255.0) as u8,
                0x00,
                0x00,
            ]);
        }
        let (_, texture_view) = factory
            .create_texture_immutable::<format::Rgba8>(
                Kind::D1(sources.len() as u16),
                Mipmap::Provided,
                &[&texels],
            )
            .unwrap();
        data.u_trans_drive = (texture_view, factory.create_sampler(sampler_info));
    }

    fn update_position_texture(
        data: &mut pipe::Data<gfx_device_gl::Resources>,
        factory: &mut gfx_device_gl::Factory,
        sources: &[SoundSource],
    ) {
        let sampler_info = SamplerInfo::new(FilterMethod::Scale, WrapMode::Tile);
        let size = sources.len();
        let kind = Kind::D1(size as u16);
        let mipmap = Mipmap::Provided;

        let texels: Vec<[u32; 4]> = sources
            .iter()
            .map(|source| {
                let pos = vecmath_util::to_vec4(source.pos);
                vecmath_util::vec4_map(pos, |p| unsafe { *(&p as *const _ as *const u32) })
            })
            .collect();
        let (_, texture_view) = factory
            .create_texture_immutable::<Rgba32F>(kind, mipmap, &[&texels])
            .unwrap();
        data.u_trans_pos = (texture_view, factory.create_sampler(sampler_info));
    }

    fn update_color_map_texture(
        data: &mut pipe::Data<gfx_device_gl::Resources>,
        factory: &mut gfx_device_gl::Factory,
        colors: &[RGBColor],
        alpha: f32,
    ) {
        let sampler_info = SamplerInfo::new(FilterMethod::Scale, WrapMode::Tile);
        let mut texels = Vec::with_capacity(colors.len());
        for color in colors {
            texels.push([
                (color.r * 255.) as u8,
                (color.g * 255.) as u8,
                (color.b * 255.) as u8,
                (alpha * 255.) as u8,
            ]);
        }
        let (_, texture_view) = factory
            .create_texture_immutable::<format::Rgba8>(
                Kind::D1(colors.len() as u16),
                Mipmap::Provided,
                &[&texels],
            )
            .unwrap();
        data.u_color_map = (texture_view, factory.create_sampler(sampler_info));
    }

    fn initialize_pipe_data(
        factory: &mut gfx_device_gl::Factory,
        vertex_buffer: Buffer<Resources, Vertex>,
        drive_view: ShaderResourceView<Resources, [f32; 4]>,
        out_color: RenderTargetView<Resources, (format::R8_G8_B8_A8, format::Srgb)>,
        out_depth: DepthStencilView<Resources, (format::D24_S8, format::Unorm)>,
    ) -> pipe::Data<Resources> {
        let sampler_info = SamplerInfo::new(FilterMethod::Scale, WrapMode::Tile);
        pipe::Data {
            vertex_buffer,
            u_model_view_proj: [[0.; 4]; 4],
            u_model: vecmath_util::mat4_scale(1.0),
            u_color_scale: 1.0,
            u_wavenum: 0.0,
            u_trans_num: 0.0,
            u_color_map: (
                AcousticFiledSliceViewer::generate_empty_view(factory),
                factory.create_sampler(SamplerInfo::new(FilterMethod::Bilinear, WrapMode::Clamp)),
            ),
            u_trans_pos: (
                AcousticFiledSliceViewer::generate_empty_view(factory),
                factory.create_sampler(sampler_info),
            ),
            u_trans_drive: (drive_view, factory.create_sampler(sampler_info)),
            out_color,
            out_depth,
        }
    }

    fn generate_empty_view(
        factory: &mut gfx_device_gl::Factory,
    ) -> ShaderResourceView<Resources, [f32; 4]> {
        let texels = vec![[0, 0, 0, 0]; 1];
        let (_, view) = factory
            .create_texture_immutable::<format::Rgba8>(Kind::D1(1), Mipmap::Provided, &[&texels])
            .unwrap();
        view
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
                            include_str!("../../../assets/shaders/slice.vert"),
                        )
                        .get(version)
                        .unwrap()
                        .as_bytes(),
                    Shaders::new()
                        .set(
                            GLSL::V4_50,
                            include_str!("../../../assets/shaders/slice.frag"),
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
