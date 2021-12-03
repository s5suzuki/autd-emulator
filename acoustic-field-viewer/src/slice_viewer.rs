/*
 * File: slice_viewer.rs
 * Project: src
 * Created Date: 11/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 03/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::sync::Arc;

use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool, TypedBufferAccess},
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::PersistentDescriptorSet,
    device::Device,
    image::{view::ImageView, StorageImage},
    pipeline::{GraphicsPipeline, PipelineBindPoint},
    render_pass::Subpass,
};

use crate::{
    renderer::Renderer, update_flag::UpdateFlag, viewer_settings::ViewerSettings, Matrix4, Vector3,
    Vector4,
};

pub type FieldImageView = Arc<ImageView<Arc<StorageImage>>>;

#[derive(Default, Debug, Clone)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}
vulkano::impl_vertex!(Vertex, position, tex_coords);

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "../assets/shaders/slice.vert"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "../assets/shaders/slice.frag"
    }
}

pub struct SliceViewer {
    vertices: Arc<CpuAccessibleBuffer<[Vertex]>>,
    indices: Arc<CpuAccessibleBuffer<[u32]>>,
    device: Arc<Device>,
    pipeline: Arc<GraphicsPipeline>,
    view_projection: (Matrix4, Matrix4),
    model: Matrix4,
    field_image_view: Arc<CpuAccessibleBuffer<[Vector4]>>,
    slice_size: [u32; 2],
}

impl SliceViewer {
    pub fn new(renderer: &Renderer, settings: &ViewerSettings) -> Self {
        let device = renderer.device();
        let vertices = Self::create_vertices(device.clone(), settings);
        let indices = Self::create_indices(device.clone());

        let vs = vs::Shader::load(device.clone()).unwrap();
        let fs = fs::Shader::load(device.clone()).unwrap();

        let pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_input_single_buffer::<Vertex>()
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

        let width = settings.slice_width / settings.slice_pixel_size;
        let height = settings.slice_height / settings.slice_pixel_size;
        let field_image_view = Self::create_field_image_view(renderer, [width, height]);

        Self {
            vertices,
            indices,
            device,
            pipeline,
            view_projection: renderer.get_view_projection(settings),
            model: vecmath_util::mat4_identity(),
            field_image_view,
            slice_size: [settings.slice_width, settings.slice_height],
        }
    }

    pub fn move_to(&mut self, pos: Vector4) {
        self.model[3] = pos;
    }

    pub fn rotate_to(&mut self, euler_angle: Vector3) {
        let rot = quaternion::euler_angles(euler_angle[0], euler_angle[1], euler_angle[2]);
        let mut model = vecmath_util::mat4_rot(rot);
        model[3] = self.model[3];
        self.model = model;
    }

    pub fn model(&self) -> &Matrix4 {
        &self.model
    }

    pub fn field_image_view(&self) -> Arc<CpuAccessibleBuffer<[Vector4]>> {
        self.field_image_view.clone()
    }

    pub fn update(
        &mut self,
        renderer: &mut Renderer,
        view_projection: &(Matrix4, Matrix4),
        settings: &ViewerSettings,
        update_flag: UpdateFlag,
    ) {
        if update_flag.contains(UpdateFlag::UPDATE_SLICE_SIZE) {
            self.vertices = Self::create_vertices(renderer.device(), settings);
            self.indices = Self::create_indices(renderer.device());
            self.slice_size = [
                settings.slice_width / settings.slice_pixel_size,
                settings.slice_height / settings.slice_pixel_size,
            ];
            self.field_image_view = Self::create_field_image_view(renderer, self.slice_size);
        }

        if update_flag.contains(UpdateFlag::UPDATE_CAMERA_POS) {
            self.view_projection = *view_projection;
        }
    }

    pub fn render(&mut self, builder: &mut AutoCommandBufferBuilder<PrimaryAutoCommandBuffer>) {
        let desc_set = self.create_descriptor_set(self.field_image_view.clone());
        builder
            .bind_pipeline_graphics(self.pipeline.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Graphics,
                self.pipeline.layout().clone(),
                0,
                desc_set,
            )
            .bind_vertex_buffers(0, self.vertices.clone())
            .bind_index_buffer(self.indices.clone())
            .draw_indexed(self.indices.len() as u32, 1, 0, 0, 0)
            .unwrap();
    }

    fn create_descriptor_set(
        &mut self,
        image: Arc<CpuAccessibleBuffer<[Vector4]>>,
    ) -> PersistentDescriptorSet {
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
                world: self.model,
                view: self.view_projection.0,
                proj: self.view_projection.1,
            };
            world_view_proj_buf.next(uniform_data).unwrap()
        };
        let mut set_builder = PersistentDescriptorSet::start(layout.clone());
        set_builder
            .add_buffer(Arc::new(uniform_buffer_subbuffer))
            .unwrap();

        let config_buf =
            CpuBufferPool::<fs::ty::Config>::new(self.device.clone(), BufferUsage::all());
        let uniform_buffer_subbuffer = {
            let uniform_data = fs::ty::Config {
                width: self.slice_size[0],
                height: self.slice_size[1],
                dummy_0: 0,
                dummy_1: 0,
            };
            config_buf.next(uniform_data).unwrap()
        };
        set_builder
            .add_buffer(Arc::new(uniform_buffer_subbuffer))
            .unwrap();

        set_builder.add_buffer(image).unwrap();
        set_builder.build().unwrap()
    }

    fn create_field_image_view(
        renderer: &Renderer,
        view_size: [u32; 2],
    ) -> Arc<CpuAccessibleBuffer<[Vector4]>> {
        let data_iter = vec![[0., 0., 0., 1.]; view_size[0] as usize * view_size[1] as usize];
        CpuAccessibleBuffer::from_iter(
            renderer.device(),
            BufferUsage {
                storage_buffer: true,
                ..BufferUsage::none()
            },
            false,
            data_iter,
        )
        .unwrap()
    }

    fn create_vertices(
        device: Arc<Device>,
        settings: &ViewerSettings,
    ) -> Arc<CpuAccessibleBuffer<[Vertex]>> {
        let width = settings.slice_width as f32;
        let height = settings.slice_height as f32;
        CpuAccessibleBuffer::from_iter(
            device,
            BufferUsage::all(),
            false,
            [
                Vertex {
                    position: [-width / 2.0, -height / 2.0, 0.0],
                    tex_coords: [0.0, 0.0],
                },
                Vertex {
                    position: [width / 2.0, -height / 2.0, 0.0],
                    tex_coords: [1.0, 0.0],
                },
                Vertex {
                    position: [width / 2.0, height / 2.0, 0.0],
                    tex_coords: [1.0, 1.0],
                },
                Vertex {
                    position: [-width / 2.0, height / 2.0, 0.0],
                    tex_coords: [0.0, 1.0],
                },
            ]
            .iter()
            .cloned(),
        )
        .unwrap()
    }

    fn create_indices(device: Arc<Device>) -> Arc<CpuAccessibleBuffer<[u32]>> {
        let indices: Vec<u32> = vec![0, 2, 1, 0, 3, 2];
        CpuAccessibleBuffer::<[u32]>::from_iter(
            device,
            BufferUsage::index_buffer(),
            false,
            indices.into_iter(),
        )
        .unwrap()
    }
}
