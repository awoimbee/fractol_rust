extern crate vulkano_win;
extern crate winit;
extern crate vulkano;
extern crate vulkano_shaders;
extern crate image;

use vulkano_win::VkSurfaceBuild;
use winit::EventsLoop;
use winit::WindowBuilder;

use vulkano::instance::Instance;
use vulkano::instance::InstanceExtensions;
use vulkano::instance::PhysicalDevice;

use vulkano::device;
use vulkano::buffer::CpuAccessibleBuffer;
use vulkano::buffer::BufferUsage;
use vulkano::command_buffer::AutoCommandBufferBuilder;

use std::sync::Arc;
use vulkano::pipeline::ComputePipeline;

use vulkano::descriptor::descriptor_set::PersistentDescriptorSet;

use vulkano::command_buffer::CommandBuffer;             //execute command buffers
use vulkano::sync::GpuFuture;                           //fences (wait for end of command)

use vulkano::format::Format;
use vulkano::format::ClearValue;
use vulkano::image::Dimensions;
use vulkano::image::StorageImage;

use image::{ImageBuffer, Rgba};                          //export to png

mod cs {
    vulkano_shaders::shader!{
        ty: "compute",
        src: "
#version 450

layout(local_size_x = 8, local_size_y = 8, local_size_z = 1) in;

layout(set = 0, binding = 0, rgba8) uniform writeonly image2D img;

void main() {
    vec2 norm_coordinates = (gl_GlobalInvocationID.xy + vec2(0.5)) / vec2(imageSize(img));
    vec2 c = (norm_coordinates - vec2(0.5)) * 2.0 - vec2(1.0, 0.0);

    vec2 z = vec2(0.0, 0.0);
    float i;
    for (i = 0.0; i < 1.0; i += 0.005) {
        z = vec2(
            z.x * z.x - z.y * z.y + c.x,
            z.y * z.x + z.x * z.y + c.y
        );

        if (length(z) > 100.0) {
            break;
        }
    }

    vec4 to_write = vec4(vec3(i), 1.0);
    imageStore(img, ivec2(gl_GlobalInvocationID.xy), to_write);
}"
    }
}

fn get_physical_device(instance : &Arc<Instance>) -> PhysicalDevice {
    println!("Available devices :");
    for (i, physical_device) in PhysicalDevice::enumerate(instance).enumerate() {
        println!("\tDevice number {}: {} (type: {:?})",
              i, physical_device.name(), physical_device.ty());
    }
    println!("Please select the device you want to use :");
    let mut string = String::new();
    std::io::stdin().read_line(&mut string).unwrap();
    return  PhysicalDevice::enumerate(instance).skip(string.trim().parse().expect("Bad input !"))
                                                .next().expect("No device corresponds to this id !");
}

fn main() {

    let instance = {
        let extensions = vulkano_win::required_extensions();
        Instance::new(None, &extensions, None).expect("failed to create Vulkan instance")
    };

    let mut events_loop = EventsLoop::new();
    let surface = WindowBuilder::new().build_vk_surface(&events_loop, instance.clone()).unwrap();

    let physical : PhysicalDevice = get_physical_device(&instance);

    for family in physical.queue_families() {
        println!("Found a queue family with {:?} queue(s)", family.queues_count());
    }

    let queue_family = physical.queue_families()
                                .find(|&q| q.supports_graphics())
                                .expect("couldn't find a graphical queue family");

    let (device, mut queues) = {
        device::Device::new(physical, &device::Features::none(), &device::DeviceExtensions::none(),
                    [(queue_family, 0.5)].iter().cloned()).expect("failed to create device")
    };

    let queue = queues.next().unwrap();


    let shader = cs::Shader::load(device.clone())
                .expect("failed to create shader module");

    let compute_pipeline = Arc::new(ComputePipeline::new(device.clone(), &shader.main_entry_point(), &())
        .expect("failed to create compute pipeline"));

    let image = StorageImage::new(device.clone(), Dimensions::Dim2d { width: 1024, height: 1024 },
                              Format::R8G8B8A8Unorm, Some(queue.family())).unwrap();

    let set = Arc::new(PersistentDescriptorSet::start(compute_pipeline.clone(), 0)
                                            .add_image(image.clone()).unwrap()
                                            .build().unwrap()
                        );

    let buf = CpuAccessibleBuffer::from_iter(device.clone(), BufferUsage::all(),
                                            (0 .. 1024 * 1024 * 4).map(|_| 0u8))
                                            .expect("failed to create buffer");

    let command_buffer = AutoCommandBufferBuilder::new(device.clone(), queue.family()).unwrap()
                        .dispatch([1024 / 8, 1024 / 8, 1], compute_pipeline.clone(), set.clone(), ()).unwrap()
                        .copy_image_to_buffer(image.clone(), buf.clone()).unwrap()
                        .build().unwrap();

    let finished = command_buffer.execute(queue.clone()).unwrap();
    finished.then_signal_fence_and_flush().unwrap()
            .wait(None).unwrap();

    let buffer_content = buf.read().unwrap();
    let image = ImageBuffer::<Rgba<u8>, _>::from_raw(1024, 1024, &buffer_content[..]).unwrap();
    image.save("image.png").unwrap();


    println!("Everything succeeded!");

    events_loop.run_forever(|event| {
        match event {
            winit::Event::WindowEvent { event: winit::WindowEvent::CloseRequested, .. } => {
                winit::ControlFlow::Break
            },
            _ => winit::ControlFlow::Continue,
        }
    });
}
