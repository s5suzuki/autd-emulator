/*
 * File: trans_viewer.rs
 * Project: src
 * Created Date: 30/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 03/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::{f32::consts::PI, io::Cursor, sync::Arc};

use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool, TypedBufferAccess},
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::PersistentDescriptorSet,
    device::{Device, Queue},
    format::Format,
    image::{
        view::ImageView, ImageDimensions, ImageViewAbstract, ImmutableImage, MipmapsCount,
        StorageImage,
    },
    pipeline::{vertex::BuffersDefinition, GraphicsPipeline, PipelineBindPoint},
    render_pass::Subpass,
    sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode},
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
#[derive(Default, Debug, Clone)]
struct Vertex {
    position: [f32; 4],
    tex_coords: [f32; 2],
}
vulkano::impl_vertex!(Vertex, position, tex_coords);

#[repr(C)]
#[derive(Default, Debug, Clone)]
struct ModelInstanceData {
    model: Matrix4,
}
vulkano::impl_vertex!(ModelInstanceData, model);

#[repr(C)]
#[derive(Default, Debug, Clone)]
struct ColorInstanceData {
    color: Vector4,
}
vulkano::impl_vertex!(ColorInstanceData, color);

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "../assets/shaders/circle.vert"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "../assets/shaders/circle.frag"
    }
}

pub struct TransViewer {
    vertices: Arc<CpuAccessibleBuffer<[Vertex]>>,
    indices: Arc<CpuAccessibleBuffer<[u32]>>,
    model_instance_data: Arc<CpuAccessibleBuffer<[ModelInstanceData]>>,
    color_instance_data: Arc<CpuAccessibleBuffer<[ColorInstanceData]>>,
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
        let empty = SoundSources::new();
        let model_instance_data =
            Self::create_model_instance_data(renderer.device(), settings, &empty);
        let coloring_method = coloring_hsv;
        let color_instance_data =
            Self::create_color_instance_data(renderer.device(), settings, &empty, coloring_method);

        let vs = vs::Shader::load(device.clone()).unwrap();
        let fs = fs::Shader::load(device.clone()).unwrap();

        let pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_input(
                    BuffersDefinition::new()
                        .vertex::<Vertex>()
                        .instance::<ModelInstanceData>()
                        .instance::<ColorInstanceData>(),
                )
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1)
                .fragment_shader(fs.main_entry_point(), ())
                .blend_alpha_blending()
                .depth_stencil_simple_depth()
                .render_pass(Subpass::from(renderer.render_pass(), 0).unwrap())
                .build(device.clone())
                .unwrap(),
        );

        let texture_desc_set = Self::create_texture_desc_set(pipeline.clone(), renderer.queue());
        Self {
            vertices,
            indices,
            model_instance_data,
            color_instance_data,
            device,
            pipeline,
            view_projection: renderer.get_view_projection(settings),
            texture_desc_set,
            coloring_method,
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
            self.model_instance_data =
                Self::create_model_instance_data(renderer.device(), settings, sources);
            self.color_instance_data = Self::create_color_instance_data(
                renderer.device(),
                settings,
                sources,
                self.coloring_method,
            );
        }

        if update_flag.contains(UpdateFlag::UPDATE_CAMERA_POS) {
            self.view_projection = *view_projection;
        }

        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE)
            || update_flag.contains(UpdateFlag::UPDATE_SOURCE_ALPHA)
            || update_flag.contains(UpdateFlag::UPDATE_SOURCE_FLAG)
        {
            self.color_instance_data = Self::create_color_instance_data(
                renderer.device(),
                settings,
                sources,
                self.coloring_method,
            );
        }
    }

    pub fn render(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        let layout = self
            .pipeline
            .layout()
            .descriptor_set_layouts()
            .get(0)
            .unwrap();
        let world_view_proj_buf =
            CpuBufferPool::<vs::ty::Data>::new(self.device.clone(), BufferUsage::all());
        let uniform_buffer_subbuffer = {
            let uniform_data = vs::ty::Data {
                view: self.view_projection.0,
                proj: self.view_projection.1,
            };
            world_view_proj_buf.next(uniform_data).unwrap()
        };
        let mut set_builder = PersistentDescriptorSet::start(layout.clone());
        set_builder
            .add_buffer(Arc::new(uniform_buffer_subbuffer))
            .unwrap();
        let desc_set = set_builder.build().unwrap();

        builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                (desc_set, self.texture_desc_set.clone()),
            )
            .bind_vertex_buffers(
                0,
                (
                    self.vertices.clone(),
                    self.model_instance_data.clone(),
                    self.color_instance_data.clone(),
                ),
            )
            .bind_index_buffer(self.indices.clone())
            .draw_indexed(
                self.indices.len() as u32,
                self.model_instance_data.len() as u32,
                0,
                0,
                0,
            )
            .unwrap();
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
        CpuAccessibleBuffer::from_iter(device, BufferUsage::all(), false, data.iter().cloned())
            .unwrap()
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
            Filter::Linear,
            Filter::Linear,
            MipmapMode::Nearest,
            SamplerAddressMode::Repeat,
            SamplerAddressMode::Repeat,
            SamplerAddressMode::Repeat,
            0.0,
            1.0,
            0.0,
            0.0,
        )
        .unwrap();
        let layout = pipeline.layout().descriptor_set_layouts().get(1).unwrap();
        let mut set_builder = PersistentDescriptorSet::start(layout.clone());
        set_builder
            .add_sampled_image(texture.clone(), sampler)
            .unwrap();
        Arc::new(set_builder.build().unwrap())
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
            (ImageView::new(image).unwrap(), future)
        };
        texture
    }
}
