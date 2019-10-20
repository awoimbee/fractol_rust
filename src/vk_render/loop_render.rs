use crate::vk_render::*;
use std::sync::{Arc, Mutex};
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;
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

            let uniform_buffer_subbuffer =
                { self.uniform_buffer.next(*uniform.lock().unwrap()).unwrap() };

            let set = Arc::new(
                PersistentDescriptorSet::start(self.pipeline.clone(), 0)
                    .add_buffer(uniform_buffer_subbuffer)
                    .unwrap()
                    .build()
                    .unwrap(),
            );

            // Before we can draw on the output, we have to *acquire* an image from the swapchain
            //  the function will block if too many requests are sent,
            //  the optional param is a timer after which the function returns an error
            let (image_num, acquire_future) =
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

            let command_buffer = AutoCommandBufferBuilder::primary_one_time_submit(
                self.device.clone(),
                self.queue.family(),
            )
            .unwrap()
            .begin_render_pass(self.framebuffers[image_num].clone(), false, clear_values)
            .unwrap()
            .draw(
                self.pipeline.clone(),
                &self.dynamic_state,
                vec![self.vertex_buffer.clone()],
                set.clone(),
                (),
            )
            .unwrap()
            .end_render_pass()
            .unwrap()
            .build()
            .unwrap();

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
