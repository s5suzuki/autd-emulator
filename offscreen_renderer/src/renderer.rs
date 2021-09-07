/*
 * File: renderer.rs
 * Project: src
 * Created Date: 10/07/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 07/09/2021
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::{f32::consts::PI, path::Path, sync::Arc};

use acoustic_field_viewer::{
    sound_source::SoundSource,
    view::{AcousticFiledSliceViewer, UpdateFlag, ViewerSettings},
    Vector4,
};
use scarlet::prelude::RGBColor;
use vulkano::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage},
    descriptor_set::PersistentDescriptorSet,
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceExtensions, Features, Queue,
    },
    instance::{Instance, InstanceExtensions},
    pipeline::{ComputePipeline, ComputePipelineAbstract},
    sync::{self, GpuFuture},
    Version,
};

mod cs_pressure {
    vulkano_shaders::shader! {
        ty: "compute",
        path: "./../assets/shaders/pressure.comp"
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Config {
    source_num: u32,
    num_x: u32,
    num_y: u32,
    num_z: u32,
    wave_num: f32,
    color_scale: f32,
    _dummy_0: f32,
    _dummy_1: f32,
}

pub struct OffscreenRenderer {
    device: Arc<Device>,
    queue: Arc<Queue>,
    pipeline: Option<Arc<ComputePipeline>>,
    results_buf: Option<Arc<CpuAccessibleBuffer<[f32]>>>,
    points_buf: Option<Arc<CpuAccessibleBuffer<[[f32; 4]]>>>,
    source_pos_buf: Option<Arc<CpuAccessibleBuffer<[[f32; 4]]>>>,
    source_amp_buf: Option<Arc<CpuAccessibleBuffer<[f32]>>>,
    source_phase_buf: Option<Arc<CpuAccessibleBuffer<[f32]>>>,
}

impl OffscreenRenderer {
    pub fn new() -> Self {
        let (device, queue) = Self::init_gpu();
        Self {
            device,
            queue,
            pipeline: None,
            results_buf: None,
            points_buf: None,
            source_pos_buf: None,
            source_amp_buf: None,
            source_phase_buf: None,
        }
    }

    fn align_to_four_multiple(x: usize) -> usize {
        (x + 3) & !0x3
    }

    fn get_renderer_point(
        field_slice_view: &AcousticFiledSliceViewer,
        setting: &ViewerSettings,
    ) -> Vec<Vector4> {
        let mut vec = vec![];

        let model = field_slice_view.model();
        let slice_width = setting.slice_width;
        let slice_height = setting.slice_height;

        for h in 0..slice_height {
            for w in 0..slice_width {
                let p = [
                    (w - slice_width / 2) as f32,
                    (h - slice_height / 2) as f32,
                    0.0,
                    1.0,
                ];
                let p = vecmath::col_mat4_transform(model, p);
                vec.push(p);
            }
        }

        vec
    }

    fn init_pipeline(&mut self) {
        let pipeline = std::sync::Arc::new({
            let shader = cs_pressure::Shader::load(self.device.clone()).unwrap();
            vulkano::pipeline::ComputePipeline::new(
                self.device.clone(),
                &shader.main_entry_point(),
                &(),
                None,
            )
            .unwrap()
        });
        self.pipeline = Some(pipeline);
    }

    fn init_renderer_points(
        &mut self,
        field_slice_view: &AcousticFiledSliceViewer,
        setting: &ViewerSettings,
    ) {
        let points = Self::get_renderer_point(field_slice_view, setting);
        let len = points.len();
        let res_buffer = {
            let data_iter = (0..len).map(|_| f32::default());
            CpuAccessibleBuffer::from_iter(
                self.device.clone(),
                BufferUsage::all(),
                false,
                data_iter,
            )
            .unwrap()
        };
        let points_buffer = {
            let pos = (0..Self::align_to_four_multiple(len)).map(|n| {
                if n < points.len() {
                    points[n]
                } else {
                    Default::default()
                }
            });
            CpuAccessibleBuffer::from_iter(self.device.clone(), BufferUsage::all(), false, pos)
                .unwrap()
        };
        self.results_buf = Some(res_buffer);
        self.points_buf = Some(points_buffer);
    }

    fn init_source_drive(&mut self, sources: &[SoundSource]) {
        let source_phase_buffer = {
            let pos = (0..Self::align_to_four_multiple(sources.len())).map(|n| {
                if n < sources.len() {
                    sources[n].phase
                } else {
                    Default::default()
                }
            });
            CpuAccessibleBuffer::from_iter(self.device.clone(), BufferUsage::all(), false, pos)
                .unwrap()
        };
        let source_amp_buffer = {
            let pos = (0..Self::align_to_four_multiple(sources.len())).map(|n| {
                if n < sources.len() {
                    sources[n].amp
                } else {
                    Default::default()
                }
            });
            CpuAccessibleBuffer::from_iter(self.device.clone(), BufferUsage::all(), false, pos)
                .unwrap()
        };
        self.source_amp_buf = Some(source_amp_buffer);
        self.source_phase_buf = Some(source_phase_buffer);
    }

    fn init_source_pos(&mut self, sources: &[SoundSource]) {
        let source_pos_buffer = {
            let pos = (0..Self::align_to_four_multiple(sources.len())).map(|n| {
                if n < sources.len() {
                    vecmath_util::to_vec4(sources[n].pos)
                } else {
                    Default::default()
                }
            });
            CpuAccessibleBuffer::from_iter(self.device.clone(), BufferUsage::all(), false, pos)
                .unwrap()
        };
        self.source_pos_buf = Some(source_pos_buffer);
    }

    fn init_cache(
        &mut self,
        sources: &[SoundSource],
        field_slice_view: &AcousticFiledSliceViewer,
        setting: &ViewerSettings,
    ) {
        self.init_pipeline();
        self.init_renderer_points(field_slice_view, setting);
        self.init_source_pos(sources);
        self.init_source_drive(sources);
    }

    pub fn update(
        &mut self,
        sources: &[SoundSource],
        field_slice_view: &AcousticFiledSliceViewer,
        setting: &ViewerSettings,
        update_flag: UpdateFlag,
    ) {
        if self.pipeline.is_none() {
            self.init_cache(sources, field_slice_view, setting);
            return;
        }

        if update_flag.contains(UpdateFlag::UPDATE_SLICE_POS)
            || update_flag.contains(UpdateFlag::UPDATE_SLICE_SIZE)
        {
            self.init_renderer_points(field_slice_view, setting);
        }

        if update_flag.contains(UpdateFlag::INIT_SOURCE) {
            self.init_source_pos(sources);
        }

        if update_flag.contains(UpdateFlag::UPDATE_SOURCE_DRIVE) {
            self.init_source_drive(sources);
        }
    }

    pub fn calculate_field(&mut self, sources: &[SoundSource], setting: &ViewerSettings) {
        let (num_x, num_y) = (setting.slice_width as _, setting.slice_height as _);
        let config_buffer = {
            let source_num = sources.len() as u32;
            let config = Config {
                source_num,
                num_x,
                num_y,
                num_z: 1,
                wave_num: 2.0 * PI / setting.wave_length,
                color_scale: setting.color_scale,
                _dummy_0: 0.0,
                _dummy_1: 0.0,
            };
            CpuAccessibleBuffer::from_data(self.device.clone(), BufferUsage::all(), false, config)
                .unwrap()
        };

        let pipeline = self.pipeline.clone().unwrap();

        let res_buffer = self.results_buf.clone().unwrap();
        let points_buffer = self.points_buf.clone().unwrap();
        let source_pos_buffer = self.source_pos_buf.clone().unwrap();
        let source_amp_buffer = self.source_amp_buf.clone().unwrap();
        let source_phase_buffer = self.source_phase_buf.clone().unwrap();

        let set_0 = Arc::new(
            PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layouts()[0].clone())
                .add_buffer(res_buffer)
                .unwrap()
                .build()
                .unwrap(),
        );

        let set_1 = Arc::new(
            PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layouts()[1].clone())
                .add_buffer(config_buffer)
                .unwrap()
                .build()
                .unwrap(),
        );

        let set_2 = Arc::new(
            PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layouts()[2].clone())
                .add_buffer(source_pos_buffer)
                .unwrap()
                .build()
                .unwrap(),
        );

        let set_3 = Arc::new(
            PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layouts()[3].clone())
                .add_buffer(source_phase_buffer)
                .unwrap()
                .add_buffer(source_amp_buffer)
                .unwrap()
                .build()
                .unwrap(),
        );

        let set_4 = Arc::new(
            PersistentDescriptorSet::start(pipeline.layout().descriptor_set_layouts()[4].clone())
                .add_buffer(points_buffer)
                .unwrap()
                .build()
                .unwrap(),
        );

        let mut builder = AutoCommandBufferBuilder::primary(
            self.device.clone(),
            self.queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();
        builder
            .dispatch(
                [num_x, num_y, 1],
                pipeline,
                (set_0, set_1, set_2, set_3, set_4),
                (),
            )
            .unwrap();
        let command_buffer = builder.build().unwrap();

        let future = sync::now(self.device.clone())
            .then_execute(self.queue.clone(), command_buffer)
            .unwrap()
            .then_signal_fence_and_flush()
            .unwrap();
        future.wait(None).unwrap();
    }

    pub fn save<P>(&self, path: P, bb: (usize, usize), colormap: &[RGBColor])
    where
        P: AsRef<Path>,
    {
        let result = self.results_buf.clone().unwrap();
        let res_buffer = result.read().unwrap();

        use image::png::PngEncoder;
        use image::ColorType;
        use std::fs::File;

        let output = File::create(path).unwrap();
        let len = bb.0 * bb.1;
        let pixels: Vec<_> = (&res_buffer[0..len])
            .chunks_exact(bb.0)
            .rev()
            .flatten()
            .map(|&v| colormap[(v.clamp(0., 1.) * (colormap.len() - 1) as f32) as usize])
            .flat_map(|c| vecmath_util::vec3_map([c.r, c.g, c.b], |v| (v * 255.0) as u8))
            .collect();

        let encoder = PngEncoder::new(output);
        encoder
            .encode(&pixels, bb.0 as u32, bb.1 as u32, ColorType::Rgb8)
            .unwrap();
    }

    fn init_gpu() -> (Arc<Device>, Arc<Queue>) {
        let instance =
            Instance::new(None, Version::V1_1, &InstanceExtensions::none(), None).unwrap();

        let device_extensions = DeviceExtensions {
            khr_storage_buffer_storage_class: true,
            ..DeviceExtensions::none()
        };

        let (physical_device, queue_family) = PhysicalDevice::enumerate(&instance)
            .filter_map(|p| {
                p.queue_families()
                    .find(|&q| q.supports_compute())
                    .map(|q| (p, q))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
            })
            .unwrap();

        let (device, mut queues) = Device::new(
            physical_device,
            &Features::none(),
            &device_extensions,
            [(queue_family, 0.5)].iter().cloned(),
        )
        .unwrap();
        let queue = queues.next().unwrap();

        (device, queue)
    }
}

impl Default for OffscreenRenderer {
    fn default() -> Self {
        Self::new()
    }
}
