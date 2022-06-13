use crate::vk_render::*;
use vulkano::swapchain::{SwapchainCreateInfo, SwapchainCreationError};

impl Graphics {
    pub fn resize(&mut self) {
        let window = self.surface.window();
        let dimensions = window.inner_size();

        let swap = self.swapchain.recreate(SwapchainCreateInfo {
            image_extent: [dimensions.width, dimensions.height],
            ..Default::default()
        });

        let (new_swapchain, new_images) = match swap {
            Ok(r) => r,
            Err(SwapchainCreationError::ImageExtentNotSupported { .. }) => return,
            Err(err) => panic!("{:?}", err),
        };

        self.swapchain = new_swapchain;
        // Because framebuffers contains an Arc on the old swapchain, we need to recreate framebuffers as well.
        self.framebuffers = window_size_dependent_setup(&new_images, self.render_pass.clone());
    }
}
