use crate::vk_render::*;
use vulkano::swapchain::SwapchainCreationError;

impl Graphics {
    pub fn resize(&mut self) {
        let window = self.surface.window();
        let dimensions: [u32; 2] = match window.get_inner_size() {
            Some(dims) => {
                let d: (u32, u32) = dims.to_physical(window.get_hidpi_factor()).into();
                [d.0, d.1]
            }
            None => panic!("Could not get inner window dimensions"),
        };

        let (new_swapchain, new_images) = match self.swapchain.recreate_with_dimension(dimensions) {
            Ok(r) => r,
            Err(SwapchainCreationError::UnsupportedDimensions) => return,
            Err(err) => panic!("{:?}", err),
        };

        self.swapchain = new_swapchain;
        // Because framebuffers contains an Arc on the old swapchain, we need to recreate framebuffers as well.
        self.framebuffers = window_size_dependent_setup(
            &new_images,
            self.render_pass.clone(),
            &mut self.dynamic_state,
        );
    }
}
