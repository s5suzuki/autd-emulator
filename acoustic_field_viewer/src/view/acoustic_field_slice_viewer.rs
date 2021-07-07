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

use std::{cell::RefCell, rc::Weak, vec};

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

use crate::{sound_source::SoundSource, view::ViewerSettings, Matrix4, Vector3};

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
    pub(crate) settings: Weak<RefCell<ViewerSettings>>,
    pub(crate) sources: Weak<RefCell<Vec<SoundSource>>>,
    pipe_data: Option<pipe::Data<Resources>>,
    model: Matrix4,
    pso_slice: Option<(PipelineState<Resources, pipe::Meta>, Slice<Resources>)>,
    position_updated: bool,
    drive_updated: bool,
    colomap_updated: bool,
}

impl AcousticFiledSliceViewer {
    pub fn new(model: Matrix4) -> AcousticFiledSliceViewer {
        AcousticFiledSliceViewer {
            settings: Weak::new(),
            pipe_data: None,
            sources: Weak::new(),
            model,
            pso_slice: None,
            position_updated: false,
            drive_updated: false,
            colomap_updated: false,
        }
    }

    pub fn render_setting(&mut self, window: &PistonWindow, opengl: OpenGL) {
        let factory = &mut window.factory.clone();

        let (width, height) = self.settings.upgrade().unwrap().borrow().size;

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
        self.initialize_shader(factory, glsl, slice);

        let len = self.sources.upgrade().unwrap().borrow().len();
        let drive_view = AcousticFiledSliceViewer::generate_empty_view(factory, len);

        self.initialize_pipe_data(
            factory,
            vertex_buffer,
            drive_view,
            window.output_color.clone(),
            window.output_stencil.clone(),
        );

        self.update_source_pos();
        self.update_source_drive();
        self.update_color_map();
    }

    pub fn update_source_pos(&mut self) {
        self.position_updated = true;
    }

    pub fn update_source_drive(&mut self) {
        self.drive_updated = true;
    }

    pub fn update_color_map(&mut self) {
        self.colomap_updated = true;
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

    pub fn rotate(&mut self, axis: Vector3, rot: f32) {
        let rot = quaternion::axis_angle(axis, rot);
        let rotm = vecmath_util::mat4_rot(rot);
        self.model = vecmath::col_mat4_mul(self.model, rotm);
    }

    pub fn renderer(
        &mut self,
        window: &mut PistonWindow,
        event: &Event,
        view: Matrix4,
        projection: Matrix4,
    ) {
        window.draw_3d(event, |window| {
            if let Some(data) = &mut self.pipe_data {
                if self.drive_updated {
                    AcousticFiledSliceViewer::update_drive_texture(
                        data,
                        &mut window.factory,
                        &self.sources.upgrade().unwrap().borrow(),
                    );
                    self.drive_updated = false;
                }

                if self.position_updated {
                    let source_num = self.sources.upgrade().unwrap().borrow().len();
                    data.u_trans_num = source_num as f32;
                    AcousticFiledSliceViewer::update_position_texture(
                        data,
                        &mut window.factory,
                        &self.sources.upgrade().unwrap().borrow(),
                    );
                    self.position_updated = false;
                }

                if self.colomap_updated {
                    let iter = (0..100).map(|x| x as f64 / 100.0);
                    let colors = self
                        .settings
                        .upgrade()
                        .unwrap()
                        .borrow()
                        .field_color_map
                        .transform(iter);
                    let alpha = self.settings.upgrade().unwrap().borrow().slice_alpha;
                    AcousticFiledSliceViewer::update_color_map_texture(
                        data,
                        &mut window.factory,
                        &colors,
                        alpha,
                    );
                    data.u_color_scale = self.settings.upgrade().unwrap().borrow().color_scale;
                    self.colomap_updated = false;
                }

                data.u_model = self.model;
                data.u_model_view_proj = model_view_projection(self.model, view, projection);
                if let Some(pso_slice) = &self.pso_slice {
                    window.encoder.draw(&pso_slice.1, &pso_slice.0, data);
                }

                if event.resize_args().is_some() {
                    data.out_color = window.output_color.clone();
                    data.out_depth = window.output_stencil.clone();
                }
            }
        });
    }

    fn update_drive_texture(
        data: &mut pipe::Data<gfx_device_gl::Resources>,
        factory: &mut gfx_device_gl::Factory,
        sources: &[SoundSource],
    ) {
        use std::f32::consts::PI;
        let sampler_info = SamplerInfo::new(FilterMethod::Scale, WrapMode::Tile);
        let mut texels = Vec::with_capacity(sources.len());
        for source in sources {
            texels.push([
                (source.phase / (2.0 * PI) * 255.) as u8,
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
                // vecmath_util::vec4_map(pos, |p| ((p / source_size).round() as u16 % 256) as u8)
                vecmath_util::vec4_map(pos, |p| unsafe { *(&p as *const _ as *const u32) })
            })
            .collect();
        let (_, texture_view) = factory
            .create_texture_immutable::<Rgba32F>(kind, mipmap, &[&texels])
            .unwrap();
        data.u_trans_pos = (texture_view, factory.create_sampler(sampler_info));
    }

    fn initialize_pipe_data(
        &mut self,
        factory: &mut gfx_device_gl::Factory,
        vertex_buffer: Buffer<Resources, Vertex>,
        drive_view: ShaderResourceView<Resources, [f32; 4]>,
        out_color: RenderTargetView<Resources, (format::R8_G8_B8_A8, format::Srgb)>,
        out_depth: DepthStencilView<Resources, (format::D24_S8, format::Unorm)>,
    ) {
        let sampler_info = SamplerInfo::new(FilterMethod::Scale, WrapMode::Tile);
        let len = self.sources.upgrade().unwrap().borrow().len();
        let wave_length = self.settings.upgrade().unwrap().borrow().wave_length;
        self.pipe_data = Some(pipe::Data {
            vertex_buffer,
            u_model_view_proj: [[0.; 4]; 4],
            u_model: vecmath_util::mat4_scale(1.0),
            u_color_scale: 1.0,
            u_wavenum: 2.0 * std::f32::consts::PI / wave_length,
            u_trans_num: len as f32,
            u_color_map: (
                AcousticFiledSliceViewer::generate_empty_view(factory, len),
                factory.create_sampler(SamplerInfo::new(FilterMethod::Bilinear, WrapMode::Clamp)),
            ),
            u_trans_pos: (
                AcousticFiledSliceViewer::generate_empty_view(factory, len),
                factory.create_sampler(sampler_info),
            ),
            u_trans_drive: (drive_view, factory.create_sampler(sampler_info)),
            out_color,
            out_depth,
        });
    }

    fn generate_empty_view(
        factory: &mut gfx_device_gl::Factory,
        size: usize,
    ) -> ShaderResourceView<Resources, [f32; 4]> {
        let texels = vec![[0, 0, 0, 0]; size];
        let (_, view) = factory
            .create_texture_immutable::<format::Rgba8>(
                Kind::D1(size as u16),
                Mipmap::Provided,
                &[&texels],
            )
            .unwrap();
        view
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
        ));
    }
}
