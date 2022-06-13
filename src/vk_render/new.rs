use crate::vk_render::*;
use std::convert::TryFrom;
use std::sync::Arc;
use vulkano::buffer::CpuBufferPool;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::device::physical::PhysicalDevice;
use vulkano::device::{Device, DeviceCreateInfo, DeviceExtensions, QueueCreateInfo};
use vulkano::instance::{Instance, InstanceCreateInfo};
use vulkano::pipeline::{DynamicState, GraphicsPipeline};
use vulkano::render_pass::Subpass;
use vulkano::swapchain::{PresentMode, SurfaceTransform, Swapchain, SwapchainCreateInfo};
use vulkano::{impl_vertex, single_pass_renderpass};
use vulkano_win::VkSurfaceBuild;
use vulkano::image::ImageUsage;
use winit::window::WindowBuilder;

impl Graphics {
    pub fn new<T>(events_loop: &winit::event_loop::EventLoop<T>) -> Graphics {
        let instance = {
            let extensions = vulkano_win::required_extensions();
            let mut create_info = InstanceCreateInfo::application_from_cargo_toml();
            create_info.enabled_extensions = extensions;

            Instance::new(create_info).unwrap()
        };
        let surface = WindowBuilder::new()
            .build_vk_surface(events_loop, instance.clone())
            .unwrap();

        let _physical = PhysicalDevice::enumerate(&instance).next().unwrap();
        println!(
            "Using device: {} (type: {:?})",
            _physical.properties().device_name,
            _physical.properties().device_type
        );

        let device_ext = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };
        let (device, mut _queues) = {
            let queue_family = _physical
                .queue_families()
                .find(|&q| q.supports_graphics() && q.supports_surface(&surface).unwrap_or(false))
                .unwrap();

            Device::new(
                _physical,
                DeviceCreateInfo {
                    enabled_extensions: device_ext,
                    enabled_features: _physical.supported_features().clone(),
                    queue_create_infos: vec![QueueCreateInfo {
                        queues: vec![0.5],
                        ..QueueCreateInfo::family(queue_family)
                    }],
                    ..Default::default()
                },
            )
            .unwrap()
        };

        let queue = _queues.next().unwrap(); // we use only one queue, so we just retrieve the first
        let (swapchain, _images) = {
            let caps = _physical
                .surface_capabilities(&surface, Default::default())
                .unwrap();
            // let usage = caps.supported_usage_flags;
            let usage = ImageUsage::color_attachment();
            let alpha = caps.supported_composite_alpha.iter().next().unwrap();
            let internal_format = _physical
                .surface_formats(&surface, Default::default())
                .unwrap()[0]
                .0;
            let window = surface.window();
            let initial_dimensions = window.inner_size().into();
            Swapchain::new(
                device.clone(),
                surface.clone(),
                SwapchainCreateInfo {
                    min_image_count: caps.min_image_count,
                    image_format: Some(internal_format),
                    image_extent: initial_dimensions,
                    image_usage: usage,
                    pre_transform: SurfaceTransform::Identity,
                    present_mode: PresentMode::Fifo,
                    image_array_layers: 1,
                    // image_sharing: queue,
                    composite_alpha: alpha,
                    clipped: true,
                    ..Default::default()
                },
            )
            .unwrap()
        };

        let vertex_buffer = {
            impl_vertex!(Vertex, position);

            CpuAccessibleBuffer::from_iter(
                device.clone(),
                BufferUsage::all(),
                false,
                [
                    Vertex {
                        position: [-1., -1.],
                    },
                    Vertex {
                        position: [-1., 1.],
                    },
                    Vertex {
                        position: [1., -1.],
                    },
                    Vertex {
                        position: [1., -1.],
                    },
                    Vertex { position: [1., 1.] },
                    Vertex {
                        position: [-1., 1.],
                    },
                ]
                .iter()
                .cloned(),
            )
            .unwrap()
        };

        let uniform_buffer = CpuBufferPool::uniform_buffer(device.clone());

        let render_pass = single_pass_renderpass!(     // describes where the output of the graphics pipeline will go
                device.clone(),
                attachments: {
                    color: {                              // custom name we give to the first and only attachment
                        load: Clear,                      // clear the content of this attachment at the start of the drawing
                        store: Store,                     // store the output of the draw in the actual image
                        format: swapchain.image_format(), // set the format of the image as the same as the swapchain
                        samples: 1,
                    }
                },
                pass: { color: [color], depth_stencil: {} }

        )
        .unwrap();

        let vs = crate::vs::load(device.clone()).unwrap();
        let fs = crate::fs::load(device.clone()).unwrap();

        println!("vs {:?} fs {:?}", vs, fs);

        let pipeline = GraphicsPipeline::start()
            .vertex_input_single_buffer::<Vertex>()
            .vertex_shader(vs.entry_point("vertex").unwrap(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1) // Use a resizable viewport set to draw over the entire window
            .fragment_shader(fs.entry_point("fragment").unwrap(), ())
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap();

        let dynamic_state = DynamicState::Viewport; // default();

        let framebuffers = window_size_dependent_setup(_images.as_ref(), render_pass.clone());
        let recreate_swapchain = Arc::new(AtomicBool::new(false));
        let exit = Arc::new(AtomicBool::new(false));

        Graphics {
            surface,

            instance,
            device_ext,
            device,
            queue,
            swapchain,

            vertex_buffer,
            uniform_buffer,

            render_pass,
            pipeline,

            dynamic_state,
            framebuffers,

            recreate_swapchain,
            exit,
        }
    }
}
