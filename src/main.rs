#[macro_use]
extern crate vulkano;
extern crate vulkano_shaders;
extern crate winit;
extern crate vulkano_win;

use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer};
use vulkano::command_buffer::{AutoCommandBufferBuilder, DynamicState};
use vulkano::buffer::CpuBufferPool;
use vulkano::device::{Device, DeviceExtensions};
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, Subpass, RenderPassAbstract};
use vulkano::image::SwapchainImage;
use vulkano::instance::{Instance, PhysicalDevice};
use vulkano::pipeline::GraphicsPipeline;
use vulkano::pipeline::viewport::Viewport;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
use vulkano::swapchain::{AcquireError, PresentMode, SurfaceTransform, Swapchain, SwapchainCreationError};
use vulkano::swapchain;
use vulkano::sync::{GpuFuture, FlushError};
use vulkano::sync;

use vulkano_win::VkSurfaceBuild;

use winit::{EventsLoop, Window, WindowBuilder, Event, WindowEvent};

use std::sync::Arc;

fn main() {

    println!("Use W/S to zoom/unzoom and the arrow keys to move");

    let instance = {
        let extensions = vulkano_win::required_extensions();
        Instance::new(None, &extensions, None).unwrap()
    };

    let physical = PhysicalDevice::enumerate(&instance).next().unwrap();
    println!("Using device: {} (type: {:?})", physical.name(), physical.ty());

    let mut events_loop = EventsLoop::new();
    let surface = WindowBuilder::new().build_vk_surface(&events_loop, instance.clone()).unwrap();
    let window = surface.window();

    let queue_family = physical.queue_families().find(|&q| {
        q.supports_graphics() && surface.is_supported(q).unwrap_or(false)
    }).unwrap();


    let device_ext = DeviceExtensions { khr_swapchain: true, .. DeviceExtensions::none() };
    let (device, mut queues) = Device::new(physical, physical.supported_features(), &device_ext,
        [(queue_family, 0.5)].iter().cloned()).unwrap();


    let queue = queues.next().unwrap(); // we use only one queue, so we just retrieve the first

    let (mut swapchain, images) = {
        let caps = surface.capabilities(physical).unwrap();

        let usage = caps.supported_usage_flags;

        let alpha = caps.supported_composite_alpha.iter().next().unwrap();

        // Choosing the internal format that the images will have.
        let format = caps.supported_formats[0].0;

        let initial_dimensions = if let Some(dimensions) = window.get_inner_size() {
            let dimensions: (u32, u32) = dimensions.to_physical(window.get_hidpi_factor()).into();
            [dimensions.0, dimensions.1]
        } else {
            return;
        };

        Swapchain::new(device.clone(), surface.clone(), caps.min_image_count, format,
            initial_dimensions, 1, usage, &queue, SurfaceTransform::Identity, alpha,
            PresentMode::Fifo, true, None).unwrap()

    };

    let vertex_buffer = {
        #[derive(Debug, Clone)]
        struct Vertex { position: [f32; 2] }
        impl_vertex!(Vertex, position);

        CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(), [
            Vertex { position: [-1., -1.] },
            Vertex { position: [-1.,  1.] },
            Vertex { position: [ 1., -1.] },

            Vertex { position: [ 1., -1.] },
            Vertex { position: [ 1.,  1.] },
            Vertex { position: [-1.,  1.] }
        ].iter().cloned()).unwrap()
    };

    let uniform_buffer = CpuBufferPool::uniform_buffer(device.clone());

    let render_pass = Arc::new(single_pass_renderpass!( // describes where the output of the graphics pipeline will go
        device.clone(),
        attachments: {
            // custom name we give to the first and only attachment
            color: {
                // clear the content of this attachment at the start of the drawing
                load: Clear,
                // store the output of the draw in the actual image
                store: Store,
                // set the format of the image as the same as the swapchain
                format: swapchain.format(),
                samples: 1,
            }
        },
        pass: {
            color: [color],
            // No depth-stencil attachment -> empty brackets
            depth_stencil: {}
        }
    ).unwrap());

    let vs = vs::Shader::load(device.clone()).unwrap();
    let fs = fs::Shader::load(device.clone()).unwrap();

    let pipeline = Arc::new(GraphicsPipeline::start()
        .vertex_input_single_buffer()
        .vertex_shader(vs.main_entry_point(), ())
        .triangle_list()
        .viewports_dynamic_scissors_irrelevant(1)   // Use a resizable viewport set to draw over the entire window
        .fragment_shader(fs.main_entry_point(), ())
        .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
        .build(device.clone())
        .unwrap());

    let mut dynamic_state = DynamicState { line_width: None, viewports: None, scissors: None };

    let mut framebuffers = window_size_dependent_setup(&images, render_pass.clone(), &mut dynamic_state);

    // Initialization is finally over!

    let mut recreate_swapchain = false;
    let mut previous_frame_end = Box::new(sync::now(device.clone())) as Box<GpuFuture>;

    // variables needed to draw the fractal
    let mut zoom = 0.5;
    let mut pos_x = -1.;
    let mut pos_y = 0.;

    loop {
        // Calling this function polls various fences in order to determine what the GPU has
        // already processed, and frees the resources that are no longer needed.
        previous_frame_end.cleanup_finished();

        if recreate_swapchain {
            let dimensions = if let Some(dimensions) = window.get_inner_size() {
                let dimensions: (u32, u32) = dimensions.to_physical(window.get_hidpi_factor()).into();
                [dimensions.0, dimensions.1]
            } else {
                return;
            };

            let (new_swapchain, new_images) = match swapchain.recreate_with_dimension(dimensions) {
                Ok(r) => r,
                Err(SwapchainCreationError::UnsupportedDimensions) => continue,
                Err(err) => panic!("{:?}", err)
            };

            swapchain = new_swapchain;
            // Because framebuffers contains an Arc on the old swapchain, we need to
            // recreate framebuffers as well.
            framebuffers = window_size_dependent_setup(&new_images, render_pass.clone(), &mut dynamic_state);
            recreate_swapchain = false;
        }

        let uniform_buffer_subbuffer = {
            uniform_buffer.next(
                EnvUniform {
                    zoom: zoom,
                    position_x: pos_x,
                    position_y: pos_y,
                }).unwrap()
        };

        let set = Arc::new(PersistentDescriptorSet::start(pipeline.clone(), 0)
            .add_buffer(uniform_buffer_subbuffer).unwrap()
            .build().unwrap()
        );

        // Before we can draw on the output, we have to *acquire* an image from the swapchain
        //  the function will block if too many requests are sent,
        //  the optional param is a timer after which the function returns an error
        let (image_num, acquire_future) = match swapchain::acquire_next_image(swapchain.clone(), None) {
            Ok(r) => r,
            Err(AcquireError::OutOfDate) => {
                recreate_swapchain = true;
                continue;
            },
            Err(err) => panic!("{:?}", err)
        };

        // color to clear the framebuffer with
        let clear_values = vec!([0.0, 0.0, 0.0, 1.0].into());

        let command_buffer = AutoCommandBufferBuilder::primary_one_time_submit(device.clone(), queue.family()).unwrap()
            .begin_render_pass(framebuffers[image_num].clone(), false, clear_values)
            .unwrap()
            .draw(pipeline.clone(), &dynamic_state, vertex_buffer.clone(), set.clone(), ())
            .unwrap()
            .end_render_pass()
            .unwrap()
            .build().unwrap();

        let future = previous_frame_end.join(acquire_future)
            .then_execute(queue.clone(), command_buffer).unwrap()
            .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                previous_frame_end = Box::new(future) as Box<_>;
            }
            Err(FlushError::OutOfDate) => {
                recreate_swapchain = true;
                previous_frame_end = Box::new(sync::now(device.clone())) as Box<_>;
            }
            Err(e) => {
                println!("{:?}", e);
                previous_frame_end = Box::new(sync::now(device.clone())) as Box<_>;
            }
        }

        // is likely that one of `acquire_next_image`,
        // `command_buffer::submit`, or `present` will block for some time. This happens when the
        // GPU's queue is full and the driver has to wait until the GPU finished some work.

        let mut done = false;
        events_loop.poll_events(|ev| {
            match ev {
                Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => done = true,
                Event::WindowEvent { event: WindowEvent::Resized(_), .. } => recreate_swapchain = true,
                Event::WindowEvent { event: WindowEvent::KeyboardInput{input, ..}, ..} => {
                    let key = input.virtual_keycode.unwrap();
                    if input.state == winit::ElementState::Pressed {
                        println!("pressed -> {:?}", key);
                        match key {
                            winit::VirtualKeyCode::W => zoom /= 1.25,
                            winit::VirtualKeyCode::S => if zoom < 2. {zoom *= 1.25},
                            winit::VirtualKeyCode::Left => pos_x -= 0.25 * zoom,
                            winit::VirtualKeyCode::Right => pos_x += 0.25 * zoom,
                            winit::VirtualKeyCode::Up => pos_y -= 0.25 * zoom,
                            winit::VirtualKeyCode::Down => pos_y += 0.25 * zoom,
                            winit::VirtualKeyCode::Escape => done = true,
                            _ => ()
                        }
                    }
                }
                _ => ()
            }
        });
        if done { return; }
    }
}

/// This method is called once during initialization, then again whenever the window is resized
fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<RenderPassAbstract + Send + Sync>,
    dynamic_state: &mut DynamicState
) -> Vec<Arc<FramebufferAbstract + Send + Sync>> {
    let dimensions = images[0].dimensions();

    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0 .. 1.0,
    };
    dynamic_state.viewports = Some(vec!(viewport));

    images.iter().map(|image| {
        Arc::new(
            Framebuffer::start(render_pass.clone())
                .add(image.clone()).unwrap()
                .build().unwrap()
        ) as Arc<FramebufferAbstract + Send + Sync>
    }).collect::<Vec<_>>()
}

#[derive(Clone)]
struct EnvUniform {
    zoom: f32,
    position_x: f32,
    position_y: f32,
}

mod vs {
    vulkano_shaders::shader!{
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
    vulkano_shaders::shader!{
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

vec2	c_div(vec2 c, vec2 divi)
{
	float	re;

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
