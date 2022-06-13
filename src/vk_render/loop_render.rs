use crate::vk_render::*;
use std::sync::{Arc, Mutex};
use vulkano::buffer::TypedBufferAccess;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::descriptor_set::persistent::PersistentDescriptorSet;
use vulkano::descriptor_set::WriteDescriptorSet;
use vulkano::pipeline::Pipeline;
use vulkano::swapchain;
use vulkano::swapchain::AcquireError;
use vulkano::sync;
use vulkano::sync::{FlushError, GpuFuture};

impl Graphics {
    pub fn loop_render(&mut self, uniform: Arc<Mutex<Uniform>>) {
        let mut previous_frame_end = Box::new(sync::now(self.device.clone())) as Box<dyn GpuFuture>;

        loop {
            // Calling this function polls various fences in order to determine what the GPU has
            // already processed, and frees the resources that are no longer needed.
            previous_frame_end.cleanup_finished();

            if self.exit.load(Relaxed) {
                return;
            }
            if self.recreate_swapchain.load(Relaxed) {
                self.resize();
                self.recreate_swapchain.store(false, Relaxed);
            }

            let uniform_read_window = *uniform.lock().unwrap();

            let uniform_buffer_subbuffer =
                { self.uniform_buffer.next(uniform_read_window).unwrap() };
            // drop(uniform_read_window);

            let _set = PersistentDescriptorSet::new(
                self.pipeline.layout().set_layouts().get(0).unwrap().clone(),
                [WriteDescriptorSet::buffer(0, uniform_buffer_subbuffer)],
            )
            .unwrap();

            // Before we can draw on the output, we have to *acquire* an image from the swapchain
            //  the function will block if too many requests are sent,
            //  the optional param is a timer after which the function returns an error
            let (image_num, _suboptimal, acquire_future) =
                match swapchain::acquire_next_image(self.swapchain.clone(), None) {
                    Ok(r) => r,
                    Err(AcquireError::OutOfDate) => {
                        self.recreate_swapchain.store(true, Relaxed);
                        continue;
                    }
                    Err(err) => panic!("{:?}", err),
                };

            // color to clear the framebuffer with
            let clear_values = vec![[0.0, 0.0, 0.0, 1.0].into()];

            let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
                self.device.clone(),
                self.queue.family(),
                vulkano::command_buffer::CommandBufferUsage::OneTimeSubmit,
            )
            .unwrap();
            command_buffer_builder
                .begin_render_pass(
                    self.framebuffers[image_num].clone(),
                    vulkano::command_buffer::SubpassContents::Inline,
                    clear_values,
                )
                .unwrap()
                .draw(self.vertex_buffer.len() as u32, 1, 0, 0)
                .unwrap()
                .end_render_pass()
                .unwrap();

            let command_buffer = command_buffer_builder.build().unwrap();

            let future = previous_frame_end
                .join(acquire_future)
                .then_execute(self.queue.clone(), command_buffer)
                .unwrap()
                .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), image_num)
                .then_signal_fence_and_flush();

            match future {
                Ok(future) => {
                    previous_frame_end = Box::new(future) as Box<_>;
                }
                Err(FlushError::OutOfDate) => {
                    self.recreate_swapchain.store(true, Relaxed);
                    previous_frame_end = Box::new(sync::now(self.device.clone())) as Box<_>;
                }
                Err(e) => {
                    println!("{:?}", e);
                    previous_frame_end = Box::new(sync::now(self.device.clone())) as Box<_>;
                }
            }
        }
    }
}
