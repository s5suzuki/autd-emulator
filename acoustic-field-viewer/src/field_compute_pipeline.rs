/*
 * File: field_compute_pipeline.rs
 * Project: src
 * Created Date: 28/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 09/05/2022
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use scarlet::{colormap::ColorMap, prelude::*};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBuffer},
    descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet},
    device::Queue,
    format::Format,
    image::{view::ImageView, ImageDimensions, ImmutableImage, MipmapsCount},
    pipeline::{ComputePipeline, Pipeline, PipelineBindPoint},
    sampler::{Filter, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
    sync::GpuFuture,
};

use crate::{
    sound_sources::{Drive, SoundSources},
    Matrix4, UpdateFlag, Vector4, ViewerSettings,
};

#[repr(C)]
#[derive(Default, Debug, Copy, Clone, Zeroable, Pod)]
struct Config {
    source_num: u32,
    _wave_num: f32,
    color_scale: f32,
    width: u32,
    height: u32,
    pixel_size: u32,
    _dummy_0: u32,
    _dummy_1: i32,
    world: Matrix4,
}

pub struct FieldComputePipeline {
    queue: Arc<Queue>,
    pipeline: Arc<ComputePipeline>,
    source_pos_buf: Option<Arc<CpuAccessibleBuffer<[[f32; 4]]>>>,
    source_drive_buf: Option<Arc<CpuAccessibleBuffer<[Drive]>>>,
    color_map_desc_set: Arc<PersistentDescriptorSet>,
}

impl FieldComputePipeline {
    pub fn new(queue: Arc<Queue>, settings: &ViewerSettings) -> Self {
        let pipeline = {
            let shader = cs::load(queue.device().clone()).unwrap();
            ComputePipeline::new(
                queue.device().clone(),
                shader.entry_point("main").unwrap(),
                &(),
                None,
                |_| {},
            )
            .unwrap()
        };

        let color_map_desc_set =
            Self::create_color_map_desc_set(queue.clone(), pipeline.clone(), settings);
        Self {
            queue,
            pipeline,
            source_pos_buf: None,
            source_drive_buf: None,
            color_map_desc_set,
        }
    }

    fn create_color_map_desc_set(
        queue: Arc<Queue>,
        pipeline: Arc<ComputePipeline>,
        settings: &ViewerSettings,
    ) -> Arc<PersistentDescriptorSet> {
        let color_map_size = 100;
        let iter = (0..color_map_size).map(|x| x as f64 / color_map_size as f64);
        let (texture, _) = {
            let color_map: Vec<RGBColor> =
                scarlet::colormap::ListedColorMap::inferno().transform(iter);
            let dimensions = ImageDimensions::Dim1d {
                width: color_map_size,
                array_layers: 1,
            };
            let alpha = (settings.slice_alpha * 255.) as u8;
            let mut texels = Vec::with_capacity(color_map.len());
            for color in color_map {
                texels.push((color.r * 255.) as u8);
                texels.push((color.g * 255.) as u8);
                texels.push((color.b * 255.) as u8);
                texels.push(alpha);
            }
            let (image, future) = ImmutableImage::from_iter(
                texels.iter().cloned(),
                dimensions,
                MipmapsCount::One,
                Format::R8G8B8A8_UNORM,
                queue.clone(),
            )
            .unwrap();
            (ImageView::new_default(image).unwrap(), future)
        };

        let sampler = Sampler::new(
            queue.device().clone(),
            SamplerCreateInfo {
                mag_filter: Filter::Linear,
                min_filter: Filter::Linear,
                mipmap_mode: SamplerMipmapMode::Nearest,
                address_mode: [SamplerAddressMode::ClampToEdge; 3],
                mip_lod_bias: 0.0,
                ..Default::default()
            },
        )
        .unwrap();

        let layout = pipeline.layout().set_layouts().get(4).unwrap();
        PersistentDescriptorSet::new(
            layout.clone(),
            [WriteDescriptorSet::image_view_sampler(0, texture, sampler)],
        )
        .unwrap()
    }

    pub fn update(
        &mut self,
        sources: &SoundSources,
        update_flag: UpdateFlag,
        settings: &ViewerSettings,
    ) {
        if !sources.is_empty() {
            if update_flag.contains(UpdateFlag::INIT_SOURCE) {
                self.init_source_pos(sources);
            }

            if update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE)
                || update_flag.contains(UpdateFlag::UPDATE_SOURCE_FLAG)
            {
                self.init_source_drive(sources);
            }
        }

        if update_flag.contains(UpdateFlag::UPDATE_COLOR_MAP) {
            self.color_map_desc_set = Self::create_color_map_desc_set(
                self.queue.clone(),
                self.pipeline.clone(),
                settings,
            );
        }
    }

    pub fn compute(
        &mut self,
        image: Arc<CpuAccessibleBuffer<[Vector4]>>,
        slice_model: &Matrix4,
        sources: &SoundSources,
        settings: &ViewerSettings,
    ) -> Box<dyn GpuFuture> {
        let pipeline_layout = self.pipeline.layout();
        let desc_layout = pipeline_layout.set_layouts().get(0).unwrap();
        let set = PersistentDescriptorSet::new(
            desc_layout.clone(),
            [WriteDescriptorSet::buffer(0, image)],
        )
        .unwrap();
        let mut builder = AutoCommandBufferBuilder::primary(
            self.queue.device().clone(),
            self.queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let config_buffer = {
            let source_num = sources.len() as u32;
            let config = Config {
                source_num,
                _wave_num: 0.0,
                color_scale: settings.color_scale,
                width: settings.slice_width / settings.slice_pixel_size,
                height: settings.slice_height / settings.slice_pixel_size,
                pixel_size: settings.slice_pixel_size,
                _dummy_0: 0,
                _dummy_1: 0,
                world: *slice_model,
            };
            CpuAccessibleBuffer::from_data(
                self.queue.device().clone(),
                BufferUsage::all(),
                false,
                config,
            )
            .unwrap()
        };
        let layout = self.pipeline.layout().set_layouts().get(1).unwrap();
        let set_1 = PersistentDescriptorSet::new(
            layout.clone(),
            [WriteDescriptorSet::buffer(0, config_buffer)],
        )
        .unwrap();

        let layout = self.pipeline.layout().set_layouts().get(2).unwrap();
        let set_2 = PersistentDescriptorSet::new(
            layout.clone(),
            [WriteDescriptorSet::buffer(
                0,
                self.source_pos_buf.clone().unwrap(),
            )],
        )
        .unwrap();

        let layout = self.pipeline.layout().set_layouts().get(3).unwrap();
        let set_3 = PersistentDescriptorSet::new(
            layout.clone(),
            [WriteDescriptorSet::buffer(
                0,
                self.source_drive_buf.clone().unwrap(),
            )],
        )
        .unwrap();

        builder
            .bind_pipeline_compute(self.pipeline.clone())
            .bind_descriptor_sets(
                PipelineBindPoint::Compute,
                pipeline_layout.clone(),
                0,
                (set, set_1, set_2, set_3, self.color_map_desc_set.clone()),
            )
            .dispatch([
                (settings.slice_width / settings.slice_pixel_size - 1) / 32 + 1,
                (settings.slice_height / settings.slice_pixel_size - 1) / 32 + 1,
                1,
            ])
            .unwrap();
        let command_buffer = builder.build().unwrap();
        let finished = command_buffer.execute(self.queue.clone()).unwrap();
        finished.then_signal_fence_and_flush().unwrap().boxed()
    }

    fn init_source_drive(&mut self, sources: &SoundSources) {
        self.source_drive_buf = Some(
            CpuAccessibleBuffer::from_iter(
                self.queue.device().clone(),
                BufferUsage::all(),
                false,
                sources.drives().copied(),
            )
            .unwrap(),
        );
    }

    fn init_source_pos(&mut self, sources: &SoundSources) {
        self.source_pos_buf = Some(
            CpuAccessibleBuffer::from_iter(
                self.queue.device().clone(),
                BufferUsage::all(),
                false,
                sources.positions().copied(),
            )
            .unwrap(),
        );
    }
}

#[allow(clippy::needless_question_mark)]
mod cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../assets/shaders/pressure.comp"
    }
}
