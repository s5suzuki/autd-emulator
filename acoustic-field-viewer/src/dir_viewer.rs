/*
 * File: dir_viewer.rs
 * Project: src
 * Created Date: 01/12/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 10/05/2022
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer, CpuBufferPool, TypedBufferAccess},
    command_buffer::{AutoCommandBufferBuilder, PrimaryAutoCommandBuffer},
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    device::Device,
    pipeline::{
        graphics::{
            color_blend::ColorBlendState, depth_stencil::DepthStencilState,
            input_assembly::InputAssemblyState, vertex_input::BuffersDefinition,
            viewport::ViewportState,
        },
        GraphicsPipeline, Pipeline, PipelineBindPoint,
    },
    render_pass::Subpass,
};

use crate::{
    renderer::Renderer, sound_sources::SoundSources, update_flag::UpdateFlag,
    viewer_settings::ViewerSettings, Matrix4, Vector3, Vector4,
};

#[derive(Debug, Clone, Copy)]
pub struct Axis3D {
    pub pos: Vector3,
    pub x: Vector3,
    pub y: Vector3,
    pub z: Vector3,
    pub show: bool,
}

impl Axis3D {
    pub fn new(pos: Vector3, x: Vector3, y: Vector3, z: Vector3) -> Axis3D {
        Axis3D {
            pos,
            x,
            y,
            z,
            show: true,
        }
    }
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod)]
struct Vertex {
    position: [f32; 3],
}
vulkano::impl_vertex!(Vertex, position);

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod)]
struct Data {
    view: Matrix4,
    proj: Matrix4,
}

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod)]
struct InstanceData {
    model: Matrix4,
    color: Vector4,
}
vulkano::impl_vertex!(InstanceData, model, color);

#[allow(clippy::needless_question_mark)]
mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        path: "../assets/shaders/cube.vert"
    }
}

#[allow(clippy::needless_question_mark)]
mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        path: "../assets/shaders/cube.frag"
    }
}

pub struct DirectionViewer {
    vertices: Arc<CpuAccessibleBuffer<[Vertex]>>,
    indices: Arc<CpuAccessibleBuffer<[u32]>>,
    instance_data: Option<Arc<CpuAccessibleBuffer<[InstanceData]>>>,
    device: Arc<Device>,
    pipeline: Arc<GraphicsPipeline>,
    view_projection: (Matrix4, Matrix4),
}

impl DirectionViewer {
    pub fn new(renderer: &Renderer, settings: &ViewerSettings) -> Self {
        let device = renderer.device();
        let vertices = Self::create_vertices(device.clone());
        let indices = Self::create_indices(device.clone());
        let _empty = SoundSources::new();

        let vs = vs::load(device.clone()).unwrap();
        let fs = fs::load(device.clone()).unwrap();

        let subpass = Subpass::from(renderer.render_pass(), 0).unwrap();
        let pipeline = GraphicsPipeline::start()
            .vertex_input_state(
                BuffersDefinition::new()
                    .vertex::<Vertex>()
                    .instance::<InstanceData>(),
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

        Self {
            vertices,
            indices,
            instance_data: None,
            device,
            pipeline,
            view_projection: renderer.get_view_projection(settings),
        }
    }

    pub fn update(
        &mut self,
        renderer: &mut Renderer,
        view_projection: &(Matrix4, Matrix4),
        settings: &ViewerSettings,
        axis: &[Axis3D],
        update_flag: UpdateFlag,
    ) {
        if !axis.is_empty()
            && (update_flag.contains(UpdateFlag::INIT_AXIS)
                || update_flag.contains(UpdateFlag::UPDATE_AXIS_SIZE)
                || update_flag.contains(UpdateFlag::UPDATE_AXIS_FLAG))
        {
            self.instance_data = Some(Self::create_instance_data(
                renderer.device(),
                settings,
                axis,
            ));
        }

        if update_flag.contains(UpdateFlag::UPDATE_CAMERA_POS) {
            self.view_projection = *view_projection;
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

        if let Some(instance) = &self.instance_data {
            builder
                .bind_pipeline_graphics(self.pipeline.clone())
                .bind_descriptor_sets(
                    PipelineBindPoint::Graphics,
                    self.pipeline.layout().clone(),
                    0,
                    desc_set,
                )
                .bind_vertex_buffers(0, (self.vertices.clone(), instance.clone()))
                .bind_index_buffer(self.indices.clone())
                .draw_indexed(self.indices.len() as u32, instance.len() as u32, 0, 0, 0)
                .unwrap();
        } else {
            // TODO
        }
    }

    fn create_instance_data(
        device: Arc<Device>,
        settings: &ViewerSettings,
        axis: &[Axis3D],
    ) -> Arc<CpuAccessibleBuffer<[InstanceData]>> {
        let len = axis.len();
        let mut models = Vec::with_capacity(len * 3);
        let mut colors = Vec::with_capacity(len * 3);
        for a in axis.iter() {
            let mut model = vecmath_util::mat4_t(a.pos);
            model = vecmath::col_mat4_mul(
                model,
                vecmath_util::mat4_rot(vecmath_util::quaternion_to(a.z, [0., 0., 1.])),
            );
            models.push(model);
            models.push(model);
            models.push(model);
            colors.push([1., 0., 0., if a.show { 1.0 } else { 0.0 }]);
            colors.push([0., 1., 0., if a.show { 1.0 } else { 0.0 }]);
            colors.push([0., 0., 1., if a.show { 1.0 } else { 0.0 }]);
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

        let mut data = Vec::new();
        for (model, color) in models.into_iter().zip(colors.into_iter()) {
            data.push(InstanceData { model, color });
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
                    position: [0., 0., 1.],
                },
                Vertex {
                    position: [1., 0., 1.],
                },
                Vertex {
                    position: [1., 1., 1.],
                },
                Vertex {
                    position: [0., 1., 1.],
                },
                Vertex {
                    position: [0., 1., 0.],
                },
                Vertex {
                    position: [1., 1., 0.],
                },
                Vertex {
                    position: [1., 0., 0.],
                },
                Vertex {
                    position: [0., 0., 0.],
                },
                Vertex {
                    position: [1., 0., 0.],
                },
                Vertex {
                    position: [1., 1., 0.],
                },
                Vertex {
                    position: [1., 1., 1.],
                },
                Vertex {
                    position: [1., 0., 1.],
                },
                Vertex {
                    position: [0., 0., 1.],
                },
                Vertex {
                    position: [0., 1., 1.],
                },
                Vertex {
                    position: [0., 1., 0.],
                },
                Vertex {
                    position: [0., 0., 0.],
                },
                Vertex {
                    position: [1., 1., 0.],
                },
                Vertex {
                    position: [0., 1., 0.],
                },
                Vertex {
                    position: [0., 1., 1.],
                },
                Vertex {
                    position: [1., 1., 1.],
                },
                Vertex {
                    position: [1., 0., 1.],
                },
                Vertex {
                    position: [0., 0., 1.],
                },
                Vertex {
                    position: [0., 0., 0.],
                },
                Vertex {
                    position: [1., 0., 0.],
                },
            ]
            .iter()
            .cloned(),
        )
        .unwrap()
    }

    fn create_indices(device: Arc<Device>) -> Arc<CpuAccessibleBuffer<[u32]>> {
        let indices: Vec<u32> = vec![
            0, 1, 2, 2, 3, 0, 4, 5, 6, 6, 7, 4, 8, 9, 10, 10, 11, 8, 12, 13, 14, 14, 15, 12, 16,
            17, 18, 18, 19, 16, 20, 21, 22, 22, 23, 20,
        ];
        CpuAccessibleBuffer::<[u32]>::from_iter(
            device,
            BufferUsage::index_buffer(),
            false,
            indices.into_iter(),
        )
        .unwrap()
    }
}
