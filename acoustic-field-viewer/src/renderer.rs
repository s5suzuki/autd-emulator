/*
 * File: renderer.rs
 * Project: src
 * Created Date: 11/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 09/05/2022
 * Modified By: Shun Suzuki (suzuki@hapis.k.u-tokyo.ac.jp)
 * -----
 * Copyright (c) 2021 Hapis Lab. All rights reserved.
 *
 */

use std::{f32::consts::PI, sync::Arc};

use camera_controllers::{Camera, CameraPerspective, FirstPerson, FirstPersonSettings};
use vulkano::{
    device::{
        physical::{PhysicalDevice, PhysicalDeviceType},
        Device, DeviceCreateInfo, DeviceExtensions, Features, Queue, QueueCreateInfo,
    },
    format::Format,
    image::{view::ImageView, AttachmentImage, ImageAccess, ImageUsage, SwapchainImage},
    instance::{
        debug::{DebugCallback, MessageSeverity, MessageType},
        Instance, InstanceCreateInfo, InstanceExtensions,
    },
    pipeline::graphics::viewport::Viewport,
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass},
    swapchain::{
        self, AcquireError, ColorSpace, FullScreenExclusive, PresentMode, Surface,
        SurfaceTransform, Swapchain, SwapchainCreateInfo, SwapchainCreationError,
    },
    sync::{self, FlushError, GpuFuture},
    Version,
};
use vulkano_win::VkSurfaceBuild;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};

use crate::{viewer_settings::ViewerSettings, Matrix4};

pub struct Renderer {
    device: Arc<Device>,
    surface: Arc<Surface<Window>>,
    queue: Arc<Queue>,
    swap_chain: Arc<Swapchain<Window>>,
    image_index: usize,
    images: Vec<Arc<SwapchainImage<Window>>>,
    recreate_swapchain: bool,
    previous_frame_end: Option<Box<dyn GpuFuture>>,
    frame_buffers: Vec<Arc<Framebuffer>>,
    render_pass: Arc<RenderPass>,
    viewport: Viewport,
    pub camera: Camera<f32>,
}

impl Renderer {
    pub fn new(
        event_loop: &EventLoop<()>,
        title: &str,
        width: f64,
        height: f64,
        v_sync: bool,
    ) -> Self {
        let instance_extensions = if cfg!(debug_assertions) {
            InstanceExtensions {
                ext_debug_utils: true,
                ..vulkano_win::required_extensions()
            }
        } else {
            InstanceExtensions {
                ..vulkano_win::required_extensions()
            }
        };

        let instance = if cfg!(debug_assertions) {
            let instance = Instance::new(InstanceCreateInfo {
                engine_version: Version::V1_6,
                enabled_extensions: instance_extensions,
                enabled_layers: vec!["VK_LAYER_KHRONOS_validation".to_owned()],
                ..Default::default()
            })
            .expect("Failed to create instance");

            let severity = MessageSeverity {
                error: true,
                warning: true,
                information: true,
                verbose: true,
            };

            let ty = MessageType::all();

            let _debug_callback = DebugCallback::new(&instance, severity, ty, |msg| {
                let severity = if msg.severity.error {
                    "error"
                } else if msg.severity.warning {
                    "warning"
                } else if msg.severity.information {
                    "information"
                } else if msg.severity.verbose {
                    "verbose"
                } else {
                    panic!("no-impl");
                };

                let ty = if msg.ty.general {
                    "general"
                } else if msg.ty.validation {
                    "validation"
                } else if msg.ty.performance {
                    "performance"
                } else {
                    panic!("no-impl");
                };

                println!(
                    "{} {} {}: {}",
                    msg.layer_prefix.unwrap_or("unknown"),
                    ty,
                    severity,
                    msg.description
                );
            })
            .ok();
            instance
        } else {
            Instance::new(InstanceCreateInfo {
                engine_version: Version::V1_6,
                enabled_extensions: instance_extensions,
                ..Default::default()
            })
            .expect("Failed to create instance")
        };

        let surface = WindowBuilder::new()
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .with_title(title)
            .build_vk_surface(event_loop, instance.clone())
            .unwrap();

        let (device, queue) = Self::create_device(instance.clone(), surface.clone());

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };
        let (swap_chain, images) = Self::create_swap_chain(
            surface.clone(),
            device.physical_device(),
            device.clone(),
            if v_sync {
                PresentMode::Fifo
            } else {
                PresentMode::Immediate
            },
        );
        let render_pass = vulkano::single_pass_renderpass!(
            device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swap_chain.image_format(),
                    samples: 1,
                },
                depth: {
                    load: Clear,
                    store: DontCare,
                    format: Format::D16_UNORM,
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {depth}
            }
        )
        .unwrap();
        let frame_buffers = Self::window_size_dependent_setup(
            device.clone(),
            &images,
            render_pass.clone(),
            &mut viewport,
        );
        let previous_frame_end = Some(sync::now(device.clone()).boxed());

        let mut camera =
            FirstPerson::new([0., -500.0, 120.0], FirstPersonSettings::keyboard_wasd()).camera(0.);
        camera.set_yaw_pitch(0., -std::f32::consts::PI / 2.0);

        Renderer {
            device,
            surface,
            queue,
            swap_chain,
            image_index: 0,
            images,
            previous_frame_end,
            recreate_swapchain: false,
            frame_buffers,
            render_pass,
            viewport,
            camera,
        }
    }

    pub fn get_projection(&self, settings: &ViewerSettings) -> Matrix4 {
        let draw_size = self.surface.window().inner_size();
        CameraPerspective {
            fov: settings.fov / PI * 180.0,
            near_clip: settings.near_clip,
            far_clip: settings.far_clip,
            aspect_ratio: (draw_size.width as f32) / (draw_size.height as f32),
        }
        .projection()
    }

    pub fn get_view(&self) -> Matrix4 {
        self.camera.orthogonal()
    }

    pub fn get_view_projection(&self, settings: &ViewerSettings) -> (Matrix4, Matrix4) {
        let mut projection = self.get_projection(settings);
        let view = self.get_view();
        projection[0][1] = -projection[0][1];
        projection[1][1] = -projection[1][1];
        projection[2][1] = -projection[2][1];
        (view, projection)
    }

    fn create_device(
        instance: Arc<Instance>,
        surface: Arc<Surface<Window>>,
    ) -> (Arc<Device>, Arc<Queue>) {
        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };

        let (physical_device, queue_family) = PhysicalDevice::enumerate(&instance)
            .filter(|&p| p.supported_extensions().is_superset_of(&device_extensions))
            .filter_map(|p| {
                p.queue_families()
                    .find(|&q| {
                        q.supports_graphics() && q.supports_surface(&surface).unwrap_or(false)
                    })
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

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );

        let features = Features::none();
        let (device, mut queues) = {
            Device::new(
                physical_device,
                DeviceCreateInfo {
                    enabled_extensions: physical_device
                        .required_extensions()
                        .union(&device_extensions),
                    enabled_features: features,
                    queue_create_infos: vec![QueueCreateInfo {
                        queues: vec![0.5],
                        ..QueueCreateInfo::family(queue_family)
                    }],
                    ..Default::default()
                },
            )
            .unwrap()
        };
        (device, queues.next().unwrap())
    }

    fn create_swap_chain(
        surface: Arc<Surface<Window>>,
        physical: PhysicalDevice,
        device: Arc<Device>,
        present_mode: PresentMode,
    ) -> (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>) {
        let caps = physical
            .surface_capabilities(&surface, Default::default())
            .unwrap();
        let alpha = caps.supported_composite_alpha.iter().next().unwrap();
        let format = physical
            .surface_formats(&surface, Default::default())
            .unwrap()[0]
            .0;
        let dimensions: [u32; 2] = surface.window().inner_size().into();
        Swapchain::new(
            device,
            surface,
            SwapchainCreateInfo {
                min_image_count: caps.min_image_count,
                image_format: Some(format),
                image_color_space: ColorSpace::SrgbNonLinear,
                image_extent: dimensions,
                image_array_layers: 1,
                image_usage: ImageUsage::color_attachment(),
                pre_transform: SurfaceTransform::Identity,
                composite_alpha: alpha,
                present_mode,
                clipped: true,
                full_screen_exclusive: FullScreenExclusive::Default,
                ..Default::default()
            },
        )
        .unwrap()
    }

    pub fn device(&self) -> Arc<Device> {
        self.device.clone()
    }

    pub fn swap_chain(&self) -> Arc<Swapchain<Window>> {
        self.swap_chain.clone()
    }

    pub fn window(&self) -> &Window {
        self.surface.window()
    }

    pub fn window_size(&self) -> [u32; 2] {
        let size = self.window().inner_size();
        [size.width, size.height]
    }

    pub fn queue(&self) -> Arc<Queue> {
        self.queue.clone()
    }

    pub fn frame_buffer(&self) -> Arc<Framebuffer> {
        self.frame_buffers[self.image_index].clone()
    }

    pub fn image(&self) -> Arc<SwapchainImage<Window>> {
        self.images[self.image_index].clone()
    }

    pub fn render_pass(&self) -> Arc<RenderPass> {
        self.render_pass.clone()
    }

    pub fn viewport(&self) -> Viewport {
        self.viewport.clone()
    }

    pub fn format(&self) -> Format {
        self.swap_chain.image_format()
    }

    pub fn resize(&mut self) {
        self.recreate_swapchain = true
    }

    pub fn start_frame(&mut self) -> Result<Box<dyn GpuFuture>, AcquireError> {
        if self.recreate_swapchain {
            self.recreate_swapchain_and_views();
        }

        let (image_num, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.swap_chain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return Err(AcquireError::OutOfDate);
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };
        if suboptimal {
            self.recreate_swapchain = true;
        }
        self.image_index = image_num;

        let future = self.previous_frame_end.take().unwrap().join(acquire_future);
        Ok(future.boxed())
    }

    pub fn finish_frame(&mut self, after_future: Box<dyn GpuFuture>) {
        let future = after_future
            .then_swapchain_present(
                self.queue.clone(),
                self.swap_chain.clone(),
                self.image_index,
            )
            .then_signal_fence_and_flush();
        match future {
            Ok(future) => {
                match future.wait(None) {
                    Ok(x) => x,
                    Err(err) => println!("{:?}", err),
                }
                self.previous_frame_end = Some(future.boxed());
            }
            Err(FlushError::OutOfDate) => {
                self.recreate_swapchain = true;
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
            Err(e) => {
                println!("Failed to flush future: {:?}", e);
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
        }
    }

    fn recreate_swapchain_and_views(&mut self) {
        let dimensions: [u32; 2] = self.window().inner_size().into();
        let (new_swapchain, new_images) = match self.swap_chain.recreate(SwapchainCreateInfo {
            image_extent: dimensions,
            ..self.swap_chain.create_info()
        }) {
            Ok(r) => r,
            Err(SwapchainCreationError::ImageExtentNotSupported {
                provided,
                min_supported,
                max_supported,
            }) => {
                println!(
                    "provided {:?}, min_supported = {:?}, max_supported = {:?}",
                    provided, min_supported, max_supported
                );
                return;
            }
            Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
        };

        self.swap_chain = new_swapchain;
        self.frame_buffers = Self::window_size_dependent_setup(
            self.device(),
            &new_images,
            self.render_pass.clone(),
            &mut self.viewport,
        );
        self.images = new_images;
        self.recreate_swapchain = false;
    }

    fn window_size_dependent_setup(
        device: Arc<Device>,
        images: &[Arc<SwapchainImage<Window>>],
        render_pass: Arc<RenderPass>,
        viewport: &mut Viewport,
    ) -> Vec<Arc<Framebuffer>> {
        let dimensions = images[0].dimensions().width_height();

        let depth_buffer = ImageView::new_default(
            AttachmentImage::transient(device, dimensions, Format::D16_UNORM).unwrap(),
        )
        .unwrap();

        viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];
        images
            .iter()
            .map(|image| {
                let view = ImageView::new_default(image.clone()).unwrap();
                Framebuffer::new(
                    render_pass.clone(),
                    FramebufferCreateInfo {
                        attachments: vec![view, depth_buffer.clone()],
                        ..Default::default()
                    },
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
    }
}
