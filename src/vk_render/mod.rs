use std::sync::atomic::{AtomicBool, Ordering::*};
use std::sync::Arc;
use vulkano::buffer::{CpuAccessibleBuffer, CpuBufferPool};
use vulkano::command_buffer::DynamicState;
use vulkano::device::Device;
use vulkano::device::Queue;
use vulkano::framebuffer::{Framebuffer, FramebufferAbstract, RenderPassAbstract};
use vulkano::image::SwapchainImage;
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::GraphicsPipelineAbstract;
use vulkano::swapchain::Swapchain;

#[derive(Default, Copy, Clone, Debug)]
pub struct Vertex {
    position: [f32; 2],
}

#[derive(Clone, Copy)]
pub struct Uniform {
    pub zoom: f32,
    pub position_x: f32,
    pub position_y: f32,
}

pub struct Graphics {
    pub surface: Arc<vulkano::swapchain::Surface<winit::Window>>,

    pub instance: Arc<vulkano::instance::Instance>,
    pub device_ext: vulkano::device::DeviceExtensions,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain<winit::Window>>,

    pub vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
    pub uniform_buffer: CpuBufferPool<Uniform>,

    pub render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    pub pipeline: Arc<dyn GraphicsPipelineAbstract + Send + Sync>,

    pub dynamic_state: DynamicState,
    pub framebuffers: Vec<Arc<dyn FramebufferAbstract + Send + Sync>>,

    pub recreate_swapchain: Arc<AtomicBool>,
    pub exit: Arc<AtomicBool>,
    // camera
    // fps_counter
}

/// This method is called once during initialization, then again whenever the window is resized
fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage<winit::Window>>],
    render_pass: Arc<dyn RenderPassAbstract + Send + Sync>,
    dynamic_state: &mut DynamicState,
) -> Vec<Arc<dyn FramebufferAbstract + Send + Sync>> {
    let dimensions = images[0].dimensions();
    let viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0..1.0,
    };
    dynamic_state.viewports = Some(vec![viewport]);

    images
        .iter()
        .map(|image| {
            Arc::new(
                Framebuffer::start(render_pass.clone())
                    .add(image.clone())
                    .unwrap()
                    .build()
                    .unwrap(),
            ) as Arc<dyn FramebufferAbstract + Send + Sync>
        })
        .collect::<Vec<_>>()
}

pub mod loop_render;
pub mod new;
pub mod resize;
