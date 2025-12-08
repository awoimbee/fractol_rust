mod keyboard_input;

use std::{error::Error, sync::Arc, time::Instant};
use keyboard_input::{PressedKeys};
use vulkano::{
    buffer::{Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
        RenderPassBeginInfo,
    },
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, DescriptorSet, WriteDescriptorSet,
    },
    device::{
        physical::PhysicalDeviceType, Device, DeviceCreateInfo, DeviceExtensions, DeviceOwned,
        Queue, QueueCreateInfo, QueueFlags,
    },
    format::Format,
    image::{view::ImageView, Image, ImageCreateInfo, ImageType, ImageUsage},
    instance::{Instance, InstanceCreateFlags, InstanceCreateInfo},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{
        graphics::{
            color_blend::{ColorBlendAttachmentState, ColorBlendState},
            depth_stencil::{DepthState, DepthStencilState},
            input_assembly::InputAssemblyState,
            multisample::MultisampleState,
            rasterization::RasterizationState,
            vertex_input::{Vertex, VertexDefinition},
            viewport::{Viewport, ViewportState},
            GraphicsPipelineCreateInfo,
        },
        GraphicsPipeline, Pipeline, PipelineBindPoint, PipelineLayout,
        PipelineShaderStageCreateInfo,
    },
    render_pass::{Framebuffer, FramebufferCreateInfo, RenderPass, Subpass},
    shader::EntryPoint,
    swapchain::{
        acquire_next_image, Surface, Swapchain, SwapchainCreateInfo, SwapchainPresentInfo,
    },
    sync::{self, GpuFuture},
    Validated, VulkanError, VulkanLibrary,
};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, EventLoop},
    window::{Window, WindowId},
};

fn main() -> Result<(), impl Error> {
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new(&event_loop);

    event_loop.run_app(&mut app)
}

struct App {
    instance: Arc<Instance>,
    device: Arc<Device>,
    queue: Arc<Queue>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    vertex_buffer: Subbuffer<[MyVertex]>,
    rcx: Option<RenderContext>,
}

struct RenderContext {
    window: Arc<Window>,
    swapchain: Arc<Swapchain>,
    render_pass: Arc<RenderPass>,
    framebuffers: Vec<Arc<Framebuffer>>,
    vs: EntryPoint,
    fs: EntryPoint,
    pipeline: Arc<GraphicsPipeline>,
    uniform_buffers: Vec<Subbuffer<vs::Data>>,
    recreate_swapchain: bool,
    previous_frame_end: Option<Box<dyn GpuFuture>>,
    pressed_keys: Arc<PressedKeys>,
    uniform_data: vs::Data,
    last_frame_time: Instant,
}

#[derive(BufferContents, Vertex)]
#[repr(C)]
struct MyVertex {
    #[format(R32G32_SFLOAT)]
    position: [f32; 2],
}

const VERTICES: [MyVertex; 4] = [
    MyVertex {
        position: [-1.0, -1.0],
    },
    MyVertex {
        position: [-1.0, 1.0],
    },
    MyVertex {
        position: [1.0, -1.0],
    },
    MyVertex { position: [1., 1.] },
];

impl App {
    fn new(event_loop: &EventLoop<()>) -> Self {
        let library = unsafe { VulkanLibrary::new() }.unwrap();
        let required_extensions = Surface::required_extensions(event_loop).unwrap();
        let instance = Instance::new(
            &library,
            &InstanceCreateInfo {
                flags: InstanceCreateFlags::ENUMERATE_PORTABILITY,
                enabled_extensions: &required_extensions,
                ..Default::default()
            },
        )
        .unwrap();

        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::empty()
        };
        let (physical_device, queue_family_index) = instance
            .enumerate_physical_devices()
            .unwrap()
            .filter(|p| p.supported_extensions().contains(&device_extensions))
            .filter_map(|p| {
                p.queue_family_properties()
                    .iter()
                    .enumerate()
                    .position(|(i, q)| {
                        q.queue_flags.intersects(QueueFlags::GRAPHICS)
                            && p.presentation_support(i as u32, event_loop).unwrap()
                    })
                    .map(|i| (p, i as u32))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
                _ => 5,
            })
            .unwrap();

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );

        let (device, mut queues) = Device::new(
            &physical_device,
            &DeviceCreateInfo {
                enabled_extensions: &device_extensions,
                queue_create_infos: &[QueueCreateInfo {
                    queue_family_index,
                    ..Default::default()
                }],
                ..Default::default()
            },
        )
        .unwrap();

        let queue = queues.next().unwrap();

        let memory_allocator = Arc::new(StandardMemoryAllocator::new(&device, &Default::default()));
        let descriptor_set_allocator = Arc::new(StandardDescriptorSetAllocator::new(
            &device,
            &Default::default(),
        ));
        let command_buffer_allocator = Arc::new(StandardCommandBufferAllocator::new(
            &device,
            &Default::default(),
        ));

        let vertex_buffer = Buffer::from_iter(
            &memory_allocator,
            &BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            &AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            VERTICES,
        )
        .unwrap();

        App {
            instance,
            device,
            queue,
            memory_allocator,
            descriptor_set_allocator,
            command_buffer_allocator,
            vertex_buffer,
            rcx: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );
        let surface = Surface::from_window(&self.instance, &window).unwrap();
        let window_size = window.inner_size();

        let (swapchain, images) = {
            let surface_capabilities = self
                .device
                .physical_device()
                .surface_capabilities(&surface, &Default::default())
                .unwrap();
            let (image_format, _) = self
                .device
                .physical_device()
                .surface_formats(&surface, &Default::default())
                .unwrap()[0];

            Swapchain::new(
                &self.device,
                &surface,
                &SwapchainCreateInfo {
                    min_image_count: surface_capabilities.min_image_count.max(2),
                    image_format,
                    image_extent: window_size.into(),
                    image_usage: ImageUsage::COLOR_ATTACHMENT,
                    composite_alpha: surface_capabilities
                        .supported_composite_alpha
                        .into_iter()
                        .next()
                        .unwrap(),
                    ..Default::default()
                },
            )
            .unwrap()
        };

        let render_pass = vulkano::single_pass_renderpass!(
            &self.device,
            attachments: {
                color: {
                    format: swapchain.image_format(),
                    samples: 1,
                    load_op: Clear,
                    store_op: Store,
                },
                depth_stencil: {
                    format: Format::D16_UNORM,
                    samples: 1,
                    load_op: Clear,
                    store_op: DontCare,
                },
            },
            pass: {
                color: [color],
                depth_stencil: {depth_stencil},
            },
        )
        .unwrap();

        let vs = vs::load(&self.device).unwrap().entry_point("main").unwrap();
        let fs = fs::load(&self.device).unwrap().entry_point("main").unwrap();

        let (framebuffers, pipeline) = window_size_dependent_setup(
            window_size,
            &images,
            &render_pass,
            &self.memory_allocator,
            &vs,
            &fs,
        );

        let uniform_buffers = (0..swapchain.image_count())
            .map(|_| {
                Buffer::new_sized(
                    &self.memory_allocator,
                    &BufferCreateInfo {
                        usage: BufferUsage::UNIFORM_BUFFER,
                        ..Default::default()
                    },
                    &AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                )
                .unwrap()
            })
            .collect();

        let previous_frame_end = Some(sync::now(self.device.clone()).boxed());

        self.rcx = Some(RenderContext {
            window,
            swapchain,
            render_pass,
            framebuffers,
            vs,
            fs,
            pipeline,
            uniform_buffers,
            recreate_swapchain: false,
            previous_frame_end,
            pressed_keys: Arc::new(PressedKeys::new()),
            uniform_data:         vs::Data {
                zoom: 0.5,
                pos_x: -1.0,
                pos_y: 0.0,
            },
            last_frame_time: Instant::now(),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        let rcx = self.rcx.as_mut().unwrap();

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(_) => {
                rcx.recreate_swapchain = true;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let should_exit = keyboard_input::input_loop(event, rcx.pressed_keys.clone());
                if should_exit {
                    event_loop.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                let window_size = rcx.window.inner_size();

                if window_size.width == 0 || window_size.height == 0 {
                    return;
                }

                rcx.previous_frame_end.as_mut().unwrap().cleanup_finished();

                if rcx.recreate_swapchain {
                    let (new_swapchain, new_images) = rcx
                        .swapchain
                        .recreate(&SwapchainCreateInfo {
                            image_extent: window_size.into(),
                            ..rcx.swapchain.create_info()
                        })
                        .expect("failed to recreate swapchain");

                    rcx.swapchain = new_swapchain;
                    (rcx.framebuffers, rcx.pipeline) = window_size_dependent_setup(
                        window_size,
                        &new_images,
                        &rcx.render_pass,
                        &self.memory_allocator,
                        &rcx.vs,
                        &rcx.fs,
                    );
                    rcx.recreate_swapchain = false;
                }

                let (image_index, suboptimal, acquire_future) = match acquire_next_image(
                    rcx.swapchain.clone(),
                    None,
                )
                .map_err(Validated::unwrap)
                {
                    Ok(r) => r,
                    Err(VulkanError::OutOfDate) => {
                        rcx.recreate_swapchain = true;
                        return;
                    }
                    Err(e) => panic!("failed to acquire next image: {e}"),
                };

                // Process input for smooth movement
                let now = Instant::now();
                let duration = now.duration_since(rcx.last_frame_time);
                let duration_ms = duration.as_millis() as u32;
                rcx.last_frame_time = now;

                keyboard_input::process_input(duration_ms, &rcx.pressed_keys, &mut rcx.uniform_data);

                let uniform_buffer = {
                    let buffer = rcx.uniform_buffers[image_index as usize].clone();
                    *buffer.write().unwrap() = rcx.uniform_data.clone();

                    buffer
                };

                let layout = &rcx.pipeline.layout().set_layouts()[0];
                let descriptor_set = DescriptorSet::new(
                    self.descriptor_set_allocator.clone(),
                    layout.clone(),
                    [WriteDescriptorSet::buffer(0, uniform_buffer)],
                    [],
                )
                .unwrap();

                if suboptimal {
                    rcx.recreate_swapchain = true;
                }

                let mut builder = AutoCommandBufferBuilder::primary(
                    self.command_buffer_allocator.clone(),
                    self.queue.queue_family_index(),
                    CommandBufferUsage::OneTimeSubmit,
                )
                .unwrap();

                builder
                    .begin_render_pass(
                        RenderPassBeginInfo {
                            clear_values: vec![
                                Some([0.0, 0.0, 1.0, 1.0].into()),
                                Some(1f32.into()),
                            ],
                            ..RenderPassBeginInfo::framebuffer(
                                rcx.framebuffers[image_index as usize].clone(),
                            )
                        },
                        Default::default(),
                    )
                    .unwrap()
                    .bind_pipeline_graphics(rcx.pipeline.clone())
                    .unwrap()
                    .bind_descriptor_sets(
                        PipelineBindPoint::Graphics,
                        rcx.pipeline.layout().clone(),
                        0,
                        descriptor_set,
                    )
                    .unwrap()
                    .bind_vertex_buffers(0, self.vertex_buffer.clone())
                    .unwrap();
                unsafe { builder.draw(1, 0, 0, 0) }.unwrap();

                builder.end_render_pass(Default::default()).unwrap();

                let command_buffer = builder.build().unwrap();
                let future = rcx
                    .previous_frame_end
                    .take()
                    .unwrap()
                    .join(acquire_future)
                    .then_execute(self.queue.clone(), command_buffer)
                    .unwrap()
                    .then_swapchain_present(
                        self.queue.clone(),
                        SwapchainPresentInfo::new(rcx.swapchain.clone(), image_index),
                    )
                    .then_signal_fence_and_flush();

                match future.map_err(Validated::unwrap) {
                    Ok(future) => {
                        rcx.previous_frame_end = Some(future.boxed());
                    }
                    Err(VulkanError::OutOfDate) => {
                        rcx.recreate_swapchain = true;
                        rcx.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
                    }
                    Err(e) => {
                        println!("failed to flush future: {e}");
                        rcx.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        let rcx = self.rcx.as_mut().unwrap();
        rcx.window.request_redraw();
    }
}

/// This function is called once during initialization, then again whenever the window is resized.
fn window_size_dependent_setup(
    window_size: PhysicalSize<u32>,
    images: &[Arc<Image>],
    render_pass: &Arc<RenderPass>,
    memory_allocator: &Arc<StandardMemoryAllocator>,
    vs: &EntryPoint,
    fs: &EntryPoint,
) -> (Vec<Arc<Framebuffer>>, Arc<GraphicsPipeline>) {
    let device = memory_allocator.device();

    let depth_buffer = ImageView::new_default(
        &Image::new(
            memory_allocator,
            &ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format: Format::D16_UNORM,
                extent: images[0].extent(),
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT | ImageUsage::TRANSIENT_ATTACHMENT,
                ..Default::default()
            },
            &AllocationCreateInfo::default(),
        )
        .unwrap(),
    )
    .unwrap();

    let framebuffers = images
        .iter()
        .map(|image| {
            let view = ImageView::new_default(image).unwrap();

            Framebuffer::new(
                render_pass,
                &FramebufferCreateInfo {
                    attachments: &[&view, &depth_buffer],
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();

    let pipeline = {
        let vertex_input_state = [MyVertex::per_vertex()].definition(vs).unwrap();
        let stages = [
            PipelineShaderStageCreateInfo::new(vs),
            PipelineShaderStageCreateInfo::new(fs),
        ];
        let layout = PipelineLayout::from_stages(device, &stages).unwrap();
        let subpass = Subpass::new(render_pass, 0).unwrap();

        GraphicsPipeline::new(
            device,
            None,
            &GraphicsPipelineCreateInfo {
                stages: &stages,
                vertex_input_state: Some(&vertex_input_state),
                input_assembly_state: Some(&InputAssemblyState::default()),
                viewport_state: Some(&ViewportState {
                    viewports: &[Viewport {
                        offset: [0.0, 0.0],
                        extent: window_size.into(),
                        min_depth: 0.0,
                        max_depth: 1.0,
                    }],
                    ..Default::default()
                }),
                rasterization_state: Some(&RasterizationState::default()),
                depth_stencil_state: Some(&DepthStencilState {
                    depth: Some(DepthState::simple()),
                    ..Default::default()
                }),
                multisample_state: Some(&MultisampleState::default()),
                color_blend_state: Some(&ColorBlendState {
                    attachments: &[ColorBlendAttachmentState::default()],
                    ..Default::default()
                }),
                subpass: Some((&subpass).into()),
                ..GraphicsPipelineCreateInfo::new(&layout)
            },
        )
        .unwrap()
    };

    (framebuffers, pipeline)
}

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
#version 450

layout(location = 0) in vec2 position;
layout(binding = 0) uniform Data {
    float zoom;
    float pos_x;
    float pos_y;
} uniforms;

layout(location = 0) out vec2 pos;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    pos = position * uniforms.zoom;
    pos.x += uniforms.pos_x;
    pos.y += uniforms.pos_y;
}"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
#version 450

layout(location = 0) in vec2 pos;

layout(location = 0) out vec4 f_color;

float squared_mod(vec2 vec)
{
    return (vec.x * vec.x + vec.y * vec.y);
}

vec2 calc_d_inpc(vec2 d_inpc, vec2 z)
{
    d_inpc = d_inpc * 2;
    d_inpc = vec2(
        d_inpc.x * z.x - d_inpc.y * z.y,
        d_inpc.y * z.x + d_inpc.x * z.y
    );
    return (d_inpc);
}

vec2 c_div(vec2 c, vec2 divi)
{
    float re;

    re = c.x;
    c.x = ((c.x * divi.x) + (c.y * divi.y))
            / ((divi.x * divi.x) + (divi.y * divi.y));
    c.y = ((c.y * divi.x) - (re * divi.y))
            / ((divi.x * divi.x) + (divi.y * divi.y));
    return (c);
}

void main() {
    float dc = 0.0001;
    vec2 c = pos;
    vec2 z = c;
    vec2 d_inpc = vec2(1, 0);
    vec2 dd_inpc = vec2(dc, 0);
    float sqrmod_z;

    float i;
    for(i = 0; i < 1.; i += 0.01) {
        d_inpc = calc_d_inpc(d_inpc, z);
        dd_inpc = calc_d_inpc(dd_inpc, z) + vec2(dc, 0);
        z = vec2(
            z.x * z.x - z.y * z.y + c.x,
            z.y * z.x + z.x * z.y + c.y
        );
        if (squared_mod(d_inpc) < 0.0001)
        {
            i = 1.;
            break ;
        }
        if (squared_mod(z) > 500)
            break ;
    }

    float color;
    color = 0.;
    if (i < 0.99)
    {
        z = c_div(z, dd_inpc);
        z = c_div(z, abs(z));
        z.x = (z.x * 0.7071067811865475 + z.y * 0.7071067811865475 + 1.5) / 2.5;
        if (z.x < 0)
            z.x = 0;
        color = z.x;
        // color=1.;
    }

    f_color = vec4(vec3(color), 1.0);
}"
    }
}
