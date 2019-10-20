use crate::vk_render::*;
use std::sync::Arc;
use vulkano::buffer::CpuBufferPool;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::command_buffer::DynamicState;
use vulkano::device::{Device, DeviceExtensions};
use vulkano::framebuffer::Subpass;
use vulkano::instance::{Instance, PhysicalDevice};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::swapchain::{PresentMode, SurfaceTransform, Swapchain};
use vulkano_win::VkSurfaceBuild;
use winit::WindowBuilder;

impl Graphics {
    pub fn new(events_loop: &winit::EventsLoop) -> Graphics {
        let instance = {
            let extensions = vulkano_win::required_extensions();
            Instance::new(None, &extensions, None).unwrap()
        };
        let surface = WindowBuilder::new()
            .build_vk_surface(events_loop, instance.clone())
            .unwrap();

        let _physical = PhysicalDevice::enumerate(&instance).next().unwrap();
        println!(
            "Using device: {} (type: {:?})",
            _physical.name(),
            _physical.ty()
        );

        let device_ext = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };
        let (device, mut _queues) = {
            let queue_family = _physical
                .queue_families()
                .find(|&q| q.supports_graphics() && surface.is_supported(q).unwrap_or(false))
                .unwrap();
            Device::new(
                _physical,
                _physical.supported_features(),
                &device_ext,
                [(queue_family, 0.5)].iter().cloned(),
            )
            .unwrap()
        };

        let queue = _queues.next().unwrap(); // we use only one queue, so we just retrieve the first
        let (swapchain, _images) = {
            let caps = surface.capabilities(_physical).unwrap();
            let usage = caps.supported_usage_flags;
            let alpha = caps.supported_composite_alpha.iter().next().unwrap();
            let internal_format = caps.supported_formats[0].0;
            let window = surface.window();
            let initial_dimensions: [u32; 2] = match window.get_inner_size() {
                Some(dims) => {
                    let d: (u32, u32) = dims.to_physical(window.get_hidpi_factor()).into();
                    [d.0, d.1]
                }
                None => panic!("Could not get inner window dimensions"),
            };
            Swapchain::new(
                device.clone(),
                surface.clone(),
                caps.min_image_count,
                internal_format,
                initial_dimensions,
                1,
                usage,
                &queue,
                SurfaceTransform::Identity,
                alpha,
                PresentMode::Fifo,
                true,
                None,
            )
            .unwrap()
        };

        let vertex_buffer = {
            impl_vertex!(Vertex, position);

            CpuAccessibleBuffer::from_iter(
                device.clone(),
                BufferUsage::all(),
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

        let render_pass = Arc::new(
            single_pass_renderpass!(     // describes where the output of the graphics pipeline will go
                device.clone(),
                attachments: {
                    color: {                            // custom name we give to the first and only attachment
                        load: Clear,                    // clear the content of this attachment at the start of the drawing
                        store: Store,                   // store the output of the draw in the actual image
                        format: swapchain.format(),     // set the format of the image as the same as the swapchain
                        samples: 1,
                    }
                },
                pass: { color: [color], depth_stencil: {} }
            )
            .unwrap(),
        );

        let vs = crate::vs::Shader::load(device.clone()).unwrap();
        let fs = crate::fs::Shader::load(device.clone()).unwrap();

        let pipeline = Arc::new(
            GraphicsPipeline::start()
                .vertex_input_single_buffer::<Vertex>()
                .vertex_shader(vs.main_entry_point(), ())
                .triangle_list()
                .viewports_dynamic_scissors_irrelevant(1) // Use a resizable viewport set to draw over the entire window
                .fragment_shader(fs.main_entry_point(), ())
                .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
                .build(device.clone())
                .unwrap(),
        );

        let mut dynamic_state = DynamicState {
            line_width: None,
            viewports: None,
            scissors: None,
        };

        let framebuffers =
            window_size_dependent_setup(&_images, render_pass.clone(), &mut dynamic_state);
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
