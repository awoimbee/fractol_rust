extern crate vulkano_shaders;
extern crate vulkano_win;
#[macro_use]
extern crate vulkano;
extern crate bitfield;
extern crate winit;

mod input;
mod movement;
mod vk_render;

use std::sync::{Arc, Mutex};
use std::thread;

use input::*;
use movement::game_loop;

fn main() {
    let events_loop = winit::EventsLoop::new();
    let pressed_keys = Arc::new(PKeys::new());

    let mut vk = vk_render::Graphics::new(&events_loop);
    let exit = vk.exit.clone();
    let rs = vk.recreate_swapchain.clone();

    let zoom = 0.5;
    let pos_x = -1.;
    let pos_y = 0.;

    let uniform = Arc::new(Mutex::new(vk_render::Uniform {
        zoom,
        position_x: pos_x,
        position_y: pos_y,
    }));

    let u = uniform.clone();
    thread::spawn(move || vk.loop_render(u));

    let e = exit.clone();
    let pk = pressed_keys.clone();
    let u = uniform.clone();
    thread::spawn(move || game_loop(e, pk, u));

    input::input_loop(events_loop, rs, exit.clone(), pressed_keys.clone());
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
