/*
 * File: trans_viewer.rs
 * Project: src
 * Created Date: 30/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 09/05/2022
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::{f32::consts::PI, io::Cursor, sync::Arc};

use bytemuck::{Pod, Zeroable};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool, TypedBufferAccess},
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    device::{Device, Queue},
    format::Format,
    image::{
        view::ImageView, ImageDimensions, ImageViewAbstract, ImmutableImage, MipmapsCount,
        StorageImage,
    },
    pipeline::{
        graphics::{
            color_blend::ColorBlendState, depth_stencil::DepthStencilState,
            input_assembly::InputAssemblyState, vertex_input::BuffersDefinition,
            viewport::ViewportState,
        },
        GraphicsPipeline, Pipeline, PipelineBindPoint,
    },
    render_pass::Subpass,
    sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
};

use crate::{
    common::coloring_method::{coloring_hsv, ColoringMethod},
    renderer::Renderer,
    sound_sources::SoundSources,
    update_flag::UpdateFlag,
    viewer_settings::ViewerSettings,
    Matrix4, Vector4,
};

pub type FieldImageView = Arc<ImageView<Arc<StorageImage>>>;

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod)]
struct Vertex {
    position: [f32; 4],
    tex_coords: [f32; 2],
}
vulkano::impl_vertex!(Vertex, position, tex_coords);

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod)]
struct Data {
    view: Matrix4,
    proj: Matrix4,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod)]
struct ModelInstanceData {
    model: Matrix4,
}
vulkano::impl_vertex!(ModelInstanceData, model);

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod)]
struct ColorInstanceData {
    color: Vector4,
}
vulkano::impl_vertex!(ColorInstanceData, color);

#[allow(clippy::needless_question_mark)]
mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "../assets/shaders/circle.vert"
    }
}

#[allow(clippy::needless_question_mark)]
mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "../assets/shaders/circle.frag"
    }
}

pub struct TransViewer {
    vertices: Arc<CpuAccessibleBuffer<[Vertex]>>,
    indices: Arc<CpuAccessibleBuffer<[u32]>>,
    model_instance_data: Option<Arc<CpuAccessibleBuffer<[ModelInstanceData]>>>,
    color_instance_data: Option<Arc<CpuAccessibleBuffer<[ColorInstanceData]>>>,
    device: Arc<Device>,
    pipeline: Arc<GraphicsPipeline>,
    view_projection: (Matrix4, Matrix4),
    texture_desc_set: Arc<PersistentDescriptorSet>,
    coloring_method: ColoringMethod,
}

impl TransViewer {
    pub fn new(renderer: &Renderer, settings: &ViewerSettings) -> Self {
        let device = renderer.device();
        let vertices = Self::create_vertices(device.clone());
        let indices = Self::create_indices(device.clone());

        let vs = vs::load(device.clone()).unwrap();
        let fs = fs::load(device.clone()).unwrap();

        let subpass = Subpass::from(renderer.render_pass(), 0).unwrap();
        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(
                BuffersDefinition::new()
                    .vertex::<Vertex>()
                    .instance::<ModelInstanceData>()
                    .instance::<ColorInstanceData>(),
            )
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .input_assembly_state(InputAssemblyState::new())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .color_blend_state(ColorBlendState::new(subpass.num_color_attachments()).blend_alpha())
            .depth_stencil_state(DepthStencilState::simple_depth_test())
            .render_pass(subpass)
            .build(device.clone())
            .unwrap();

        let texture_desc_set = Self::create_texture_desc_set(pipeline.clone(), renderer.queue());
        Self {
            vertices,
            indices,
            model_instance_data: None,
            color_instance_data: None,
            device,
            pipeline,
            view_projection: renderer.get_view_projection(settings),
            texture_desc_set,
            coloring_method: coloring_hsv,
        }
    }

    pub fn update(
        &mut self,
        renderer: &mut Renderer,
        view_projection: &(Matrix4, Matrix4),
        settings: &ViewerSettings,
        sources: &SoundSources,
        update_flag: UpdateFlag,
    ) {
        if update_flag.contains(UpdateFlag::INIT_SOURCE) {
            self.model_instance_data = Some(Self::create_model_instance_data(
                renderer.device(),
                settings,
                sources,
            ));
            self.color_instance_data = Some(Self::create_color_instance_data(
                renderer.device(),
                settings,
                sources,
                self.coloring_method,
            ));
        }

        if update_flag.contains(UpdateFlag::UPDATE_CAMERA_POS) {
            self.view_projection = *view_projection;
        }

        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE)
            || update_flag.contains(UpdateFlag::UPDATE_SOURCE_ALPHA)
            || update_flag.contains(UpdateFlag::UPDATE_SOURCE_FLAG)
        {
            self.color_instance_data = Some(Self::create_color_instance_data(
                renderer.device(),
                settings,
                sources,
                self.coloring_method,
            ));
        }
    }

    pub fn render(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        let layout = self.pipeline.layout().set_layouts().get(0).unwrap();
        let world_view_proj_buf =
            CpuBufferPool::<Data>::new(self.device.clone(), BufferUsage::all());
        let uniform_buffer_subbuffer = {
            let uniform_data = Data {
                view: self.view_projection.0,
                proj: self.view_projection.1,
            };
            world_view_proj_buf.next(uniform_data).unwrap()
        };
        let desc_set = PersistentDescriptorSet::new(
            layout.clone(),
            [WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer)],
        )
        .unwrap();

        if let (Some(model), Some(color)) = (&self.color_instance_data, &self.model_instance_data) {
            builder
                .bind_pipeline_graphics(self.pipeline.clone())
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    self.pipeline.layout().clone(),
                    0,
                    (desc_set, self.texture_desc_set.clone()),
                )
                .bind_vertex_buffers(0, (self.vertices.clone(), model.clone(), color.clone()))
                .bind_index_buffer(self.indices.clone())
                .draw_indexed(self.indices.len() as u32, model.len() as u32, 0, 0, 0)
                .unwrap();
        } else {
            // TODO
        }
    }

    fn create_model_instance_data(
        device: Arc<Device>,
        settings: &ViewerSettings,
        sources: &SoundSources,
    ) -> Arc<CpuAccessibleBuffer<[ModelInstanceData]>> {
        let mut data = Vec::new();
        for (pos, dir) in sources.position_dirs() {
            let s = 0.5 * settings.source_size;
            let mut m = vecmath_util::mat4_scale([s, s, s]);
            m[3][0] = pos[0];
            m[3][1] = pos[1];
            m[3][2] = pos[2];
            let rot = vecmath_util::quaternion_to(*dir, [0., 0., 1.]);
            let rotm = vecmath_util::mat4_rot(rot);
            data.push(ModelInstanceData {
                model: vecmath::col_mat4_mul(m, rotm),
            });
        }
        let buf = CpuAccessibleBuffer::from_iter(device, BufferUsage::all(), false, data)
            .expect("failed to create buffer");
        buf
    }

    fn create_color_instance_data(
        device: Arc<Device>,
        settings: &ViewerSettings,
        sources: &SoundSources,
        coloring_method: ColoringMethod,
    ) -> Arc<CpuAccessibleBuffer<[ColorInstanceData]>> {
        let mut data = Vec::new();
        for drive in sources.drives() {
            let color = coloring_method(
                drive.phase / (2.0 * PI),
                drive.amp,
                drive.visible * settings.source_alpha,
            );
            data.push(ColorInstanceData { color });
        }
        CpuAccessibleBuffer::from_iter(device, BufferUsage::all(), false, data.iter().cloned())
            .unwrap()
    }

    fn create_vertices(device: Arc<Device>) -> Arc<CpuAccessibleBuffer<[Vertex]>> {
        CpuAccessibleBuffer::from_iter(
            device,
            BufferUsage::all(),
            false,
            [
                Vertex {
                    position: [-1.0, -1.0, 0.0, 1.0],
                    tex_coords: [0.0, 1.0],
                },
                Vertex {
                    position: [1.0, -1.0, 0.0, 1.0],
                    tex_coords: [1.0, 1.0],
                },
                Vertex {
                    position: [1.0, 1.0, 0.0, 1.0],
                    tex_coords: [1.0, 0.0],
                },
                Vertex {
                    position: [-1.0, 1.0, 0.0, 1.0],
                    tex_coords: [0.0, 0.0],
                },
            ]
            .iter()
            .cloned(),
        )
        .unwrap()
    }

    fn create_indices(device: Arc<Device>) -> Arc<CpuAccessibleBuffer<[u32]>> {
        let indices: Vec<u32> = vec![0, 1, 2, 2, 3, 0];
        CpuAccessibleBuffer::<[u32]>::from_iter(
            device,
            BufferUsage::index_buffer(),
            false,
            indices.into_iter(),
        )
        .unwrap()
    }

    fn create_texture_desc_set(
        pipeline: Arc<GraphicsPipeline>,
        queue: Arc<Queue>,
    ) -> Arc<PersistentDescriptorSet> {
        let texture = Self::load_image(queue.clone());
        let sampler = Sampler::new(
            queue.device().clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                mipmap_mode: SamplerMipmapMode::Nearest,
                address_mode: [SamplerAddressMode::Repeat; 3],
                mip_lod_bias: 0.0,
                ..Default::default()
            },
        )
        .unwrap();
        let layout = pipeline.layout().set_layouts().get(1).unwrap();

        PersistentDescriptorSet::new(
            layout.clone(),
            [WriteDescriptorSet::image_view_sampler(
                0,
                texture.clone(),
                sampler,
            )],
        )
        .unwrap()
    }

    fn load_image(queue: Arc<Queue>) -> Arc<dyn ImageViewAbstract> {
        let (texture, _tex_future) = {
            let png_bytes = include_bytes!("../../assets/textures/circle.png").to_vec();
            let cursor = Cursor::new(png_bytes);
            let decoder = png::Decoder::new(cursor);
            let mut reader = decoder.read_info().unwrap();
            let info = reader.info();
            let dimensions = ImageDimensions::Dim2d {
                width: info.width,
                height: info.height,
                array_layers: 1,
            };
            let mut image_data = Vec::new();
            image_data.resize((info.width * info.height * 4) as usize, 0);
            reader.next_frame(&mut image_data).unwrap();

            let (image, future) = ImmutableImage::from_iter(
                image_data.iter().cloned(),
                dimensions,
                MipmapsCount::One,
                Format::R8G8B8A8_SRGB,
                queue,
            )
            .unwrap();
            (ImageView::new_default(image).unwrap(), future)
        };
        texture
    }
}
