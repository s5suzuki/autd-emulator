/*
 * File: renderer.rs
 * Project: src
 * Created Date: 11/11/2021
 * Author: Shun Suzuki
 * -----
 * Last Modified: 17/12/2021
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
        Device, DeviceExtensions, Features, Queue,
    },
    format::Format,
    image::{view::ImageView, AttachmentImage, ImageAccess, ImageUsage, SwapchainImage},
    instance::{
        debug::{DebugCallback, MessageSeverity, MessageType},
        Instance, InstanceExtensions,
    },
    pipeline::graphics::viewport::Viewport,
    render_pass::{Framebuffer, RenderPass},
    swapchain::{
        self, AcquireError, ColorSpace, FullscreenExclusive, PresentMode, Surface,
        SurfaceTransform, Swapchain, SwapchainCreationError,
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
    _instance: Arc<Instance>,
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

        let _instance = if cfg!(debug_assertions) {
            let layers = vec!["VK_LAYER_KHRONOS_validation"];
            let instance = Instance::new(None, Version::V1_2, &instance_extensions, layers)
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
            Instance::new(None, Version::V1_2, &instance_extensions, None)
                .expect("Failed to create instance")
        };

        let physical_device = PhysicalDevice::enumerate(&_instance)
            .min_by_key(|p| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
            })
            .unwrap();

        let surface = WindowBuilder::new()
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .with_title(title)
            .build_vk_surface(event_loop, _instance.clone())
            .unwrap();

        let (device, queue) = Self::create_device(physical_device, surface.clone());

        let mut viewport = Viewport {
            origin: [0.0, 0.0],
            dimensions: [0.0, 0.0],
            depth_range: 0.0..1.0,
        };
        let (swap_chain, images) = Self::create_swap_chain(
            surface.clone(),
            physical_device,
            device.clone(),
            queue.clone(),
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
                    format: swap_chain.format(),
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
            _instance,
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
        physical: PhysicalDevice,
        surface: Arc<Surface<Window>>,
    ) -> (Arc<Device>, Arc<Queue>) {
        let queue_family = physical
            .queue_families()
            .find(|&q| q.supports_graphics() && surface.is_supported(q).unwrap_or(false))
            .unwrap();

        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };

        let features = Features::none();
        let (device, mut queues) = {
            Device::new(
                physical,
                &features,
                &physical.required_extensions().union(&device_extensions),
                [(queue_family, 0.5)].iter().cloned(),
            )
            .unwrap()
        };
        (device, queues.next().unwrap())
    }

    fn create_swap_chain(
        surface: Arc<Surface<Window>>,
        physical: PhysicalDevice,
        device: Arc<Device>,
        queue: Arc<Queue>,
        present_mode: PresentMode,
    ) -> (Arc<Swapchain<Window>>, Vec<Arc<SwapchainImage<Window>>>) {
        let caps = surface.capabilities(physical).unwrap();
        let alpha = caps.supported_composite_alpha.iter().next().unwrap();
        let format = caps.supported_formats[0].0;
        let dimensions: [u32; 2] = surface.window().inner_size().into();
        Swapchain::start(device, surface)
            .num_images(caps.min_image_count)
            .format(format)
            .dimensions(dimensions)
            .usage(ImageUsage::color_attachment())
            .sharing_mode(&queue)
            .composite_alpha(alpha)
            .transform(SurfaceTransform::Identity)
            .present_mode(present_mode)
            .fullscreen_exclusive(FullscreenExclusive::Default)
            .clipped(true)
            .color_space(ColorSpace::SrgbNonLinear)
            .layers(1)
            .build()
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
        self.swap_chain.format()
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
        let (new_swapchain, new_images) =
            match self.swap_chain.recreate().dimensions(dimensions).build() {
                Ok(r) => r,
                Err(SwapchainCreationError::UnsupportedDimensions) => {
                    println!(
                        "{}",
                        SwapchainCreationError::UnsupportedDimensions.to_string()
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

        let depth_buffer = ImageView::new(
            AttachmentImage::transient(device, dimensions, Format::D16_UNORM).unwrap(),
        )
        .unwrap();

        viewport.dimensions = [dimensions[0] as f32, dimensions[1] as f32];
        images
            .iter()
            .map(|image| {
                let view = ImageView::new(image.clone()).unwrap();
                Framebuffer::start(render_pass.clone())
                    .add(view)
                    .unwrap()
                    .add(depth_buffer.clone())
                    .unwrap()
                    .build()
                    .unwrap()
            })
            .collect::<Vec<_>>()
    }
}
