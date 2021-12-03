/*
 * File: field_compute_pipeline.rs
 * Project: src
 * Created Date: 28/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 03/12/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::{f32::consts::PI, sync::Arc};

use scarlet::{colormap::ColorMap, prelude::*};
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, PrimaryCommandBuffer},
    descriptor_set::PersistentDescriptorSet,
    device::Queue,
    format::Format,
    image::{view::ImageView, ImageDimensions, ImmutableImage, MipmapsCount},
    pipeline::{ComputePipeline, PipelineBindPoint},
    sampler::{Filter, MipmapMode, Sampler, SamplerAddressMode},
    sync::GpuFuture,
};

use crate::{
    sound_sources::{Drive, SoundSources},
    Matrix4, UpdateFlag, Vector4, ViewerSettings,
};

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
            let shader = cs::Shader::load(queue.device().clone()).unwrap();
            Arc::new(
                ComputePipeline::new(
                    queue.device().clone(),
                    &shader.main_entry_point(),
                    &(),
                    None,
                    |_| {},
                )
                .unwrap(),
            )
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
            (ImageView::new(image).unwrap(), future)
        };

        let sampler = Sampler::new(
            queue.device().clone(),
            Filter::Linear,
            Filter::Linear,
            MipmapMode::Nearest,
            SamplerAddressMode::ClampToEdge,
            SamplerAddressMode::ClampToEdge,
            SamplerAddressMode::ClampToEdge,
            0.0,
            1.0,
            0.0,
            0.0,
        )
        .unwrap();

        let layout = pipeline.layout().descriptor_set_layouts().get(4).unwrap();
        let mut set_builder = PersistentDescriptorSet::start(layout.clone());
        set_builder.add_sampled_image(texture, sampler).unwrap();
        Arc::new(set_builder.build().unwrap())
    }

    pub fn update(
        &mut self,
        sources: &SoundSources,
        update_flag: UpdateFlag,
        settings: &ViewerSettings,
    ) {
        if update_flag.contains(UpdateFlag::INIT_SOURCE) {
            self.init_source_pos(sources);
        }

        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE)
            || update_flag.contains(UpdateFlag::UPDATE_SOURCE_FLAG)
        {
            self.init_source_drive(sources);
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
        let desc_layout = pipeline_layout.descriptor_set_layouts().get(0).unwrap();
        let mut desc_set_builder = PersistentDescriptorSet::start(desc_layout.clone());
        desc_set_builder.add_buffer(image).unwrap();
        let set = desc_set_builder.build().unwrap();
        let mut builder = AutoCommandBufferBuilder::primary(
            self.queue.device().clone(),
            self.queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        let config_buffer = {
            let source_num = sources.len() as u32;
            let config = cs::ty::Config {
                source_num,
                wave_num: 2.0 * PI / settings.wave_length,
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
        let layout = self
            .pipeline
            .layout()
            .descriptor_set_layouts()
            .get(1)
            .unwrap();
        let mut set_builder = PersistentDescriptorSet::start(layout.clone());
        set_builder.add_buffer(config_buffer).unwrap();
        let set_1 = Arc::new(set_builder.build().unwrap());

        let layout = self
            .pipeline
            .layout()
            .descriptor_set_layouts()
            .get(2)
            .unwrap();
        let mut set_builder = PersistentDescriptorSet::start(layout.clone());
        set_builder
            .add_buffer(self.source_pos_buf.clone().unwrap())
            .unwrap();
        let set_2 = Arc::new(set_builder.build().unwrap());

        let layout = self
            .pipeline
            .layout()
            .descriptor_set_layouts()
            .get(3)
            .unwrap();
        let mut set_builder = PersistentDescriptorSet::start(layout.clone());
        set_builder
            .add_buffer(self.source_drive_buf.clone().unwrap())
            .unwrap();
        let set_3 = Arc::new(set_builder.build().unwrap());

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

mod cs {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "../assets/shaders/pressure.comp"
    }
}
