use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::*};
use std::sync::Arc;
use winit::VirtualKeyCode as KeyCode;
use winit::{ControlFlow, Event, WindowEvent};

pub enum BTKey {
    UP = 0b1,
    DWN = 0b10,
    LFT = 0b100,
    RGT = 0b1000,
    W = 0b1_0000,
    S = 0b10_0000,
}

pub struct PKeys {
    p_keys: AtomicU32,
}

impl PKeys {
    pub fn new() -> PKeys {
        let p_keys = AtomicU32::new(0);
        PKeys { p_keys }
    }

    pub fn add(&self, add: BTKey) {
        let new_pkeys = self.p_keys.load(Relaxed) | (add as u32);
        self.p_keys.store(new_pkeys, Relaxed);
    }

    pub fn rm(&self, remove: BTKey) {
        let new_pkeys = self.p_keys.load(Relaxed) & !(remove as u32);
        self.p_keys.store(new_pkeys, Relaxed);
    }

    pub fn contains(&self, key: BTKey) -> bool {
        (self.p_keys.load(Relaxed) & (key as u32)) != 0
    }
}

pub fn input_loop(
    mut events_loop: winit::EventsLoop,
    recreate_swapchain: Arc<AtomicBool>,
    exit: Arc<AtomicBool>,
    p_keys: Arc<PKeys>,
) {
    events_loop.run_forever(|ev| {
        match ev {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => exit.store(true, Relaxed),
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => recreate_swapchain.store(true, Relaxed),

            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { input, .. },
                ..
            } => {
                let key = input.virtual_keycode.unwrap();
                let fn_ptr = match input.state {
                    winit::ElementState::Pressed => PKeys::add,
                    winit::ElementState::Released => PKeys::rm,
                };

                match key {
                    KeyCode::W => fn_ptr(&p_keys, BTKey::W),
                    KeyCode::S => fn_ptr(&p_keys, BTKey::S),
                    KeyCode::Left => fn_ptr(&p_keys, BTKey::LFT),
                    KeyCode::Right => fn_ptr(&p_keys, BTKey::RGT),
                    KeyCode::Up => fn_ptr(&p_keys, BTKey::UP),
                    KeyCode::Down => fn_ptr(&p_keys, BTKey::DWN),
                    KeyCode::Escape => exit.store(true, Relaxed),
                    _ => (),
                }
            }
            _ => (),
        };
        if exit.load(Relaxed) {
            ControlFlow::Break
        } else {
            ControlFlow::Continue
        }
    });
}
