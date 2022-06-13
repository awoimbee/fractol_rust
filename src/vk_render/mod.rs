use bytemuck::{Pod, Zeroable};
use std::sync::atomic::{AtomicBool, Ordering::*};
use std::sync::Arc;
use vulkano::buffer::{CpuAccessibleBuffer, CpuBufferPool};
use vulkano::device::Device;
use vulkano::device::Queue;
use vulkano::image::view::ImageView;
use vulkano::image::ImageAccess;
use vulkano::image::SwapchainImage;
use vulkano::pipeline::graphics::viewport::Viewport;
use vulkano::pipeline::{DynamicState, GraphicsPipeline};
use vulkano::render_pass::{Framebuffer, RenderPass};
use vulkano::swapchain::Swapchain;

#[repr(C)]
#[derive(Default, Copy, Clone, Zeroable, Pod)]
pub struct Vertex {
    position: [f32; 2],
}

#[repr(C)]
#[derive(Default, Clone, Copy, Pod, Zeroable)]
pub struct Uniform {
    pub zoom: f32,
    pub position_x: f32,
    pub position_y: f32,
}

pub struct Graphics {
    pub surface: Arc<vulkano::swapchain::Surface<winit::window::Window>>,

    pub instance: Arc<vulkano::instance::Instance>,
    pub device_ext: vulkano::device::DeviceExtensions,
    pub device: Arc<Device>,
    pub queue: Arc<Queue>,
    pub swapchain: Arc<Swapchain<winit::window::Window>>,

    pub vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
    pub uniform_buffer: CpuBufferPool<Uniform>,

    pub render_pass: Arc<RenderPass>,
    pub pipeline: Arc<GraphicsPipeline>,

    pub dynamic_state: DynamicState,
    pub framebuffers: Vec<Arc<Framebuffer>>,

    pub recreate_swapchain: Arc<AtomicBool>,
    pub exit: Arc<AtomicBool>,
    // camera
    // fps_counter
}

/// This method is called once during initialization, then again whenever the window is resized
fn window_size_dependent_setup(
    images: &[Arc<SwapchainImage<winit::window::Window>>],
    render_pass: Arc<RenderPass>,
) -> Vec<Arc<Framebuffer>> {
    let dimensions = images[0].dimensions().width_height();
    let _viewport = Viewport {
        origin: [0.0, 0.0],
        dimensions: [dimensions[0] as f32, dimensions[1] as f32],
        depth_range: 0.0..1.0,
    };
    // dynamic_state.viewports = Some(vec![viewport]);

    images
        .iter()
        .map(|image| {
            Framebuffer::new(
                render_pass.clone(),
                vulkano::render_pass::FramebufferCreateInfo {
                    attachments: vec![ImageView::new(image.clone(), Default::default()).unwrap()],
                    ..Default::default()
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>()
}

pub mod loop_render;
pub mod new;
pub mod resize;
