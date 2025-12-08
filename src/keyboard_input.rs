use std::sync::atomic::{AtomicU32, Ordering::*};
use std::sync::Arc;

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::vs;

pub enum KeyMask {
    Up = 0b1,
    Down = 0b10,
    Left = 0b100,
    Right = 0b1000,
    W = 0b1_0000,
    S = 0b10_0000,
}

pub struct PressedKeys {
    pressed_keys: AtomicU32,
}

impl PressedKeys {
    pub fn new() -> PressedKeys {
        let p_keys = AtomicU32::new(0);
        PressedKeys {
            pressed_keys: p_keys,
        }
    }

    pub fn add(&self, add: KeyMask) {
        let new_pkeys = self.pressed_keys.load(Relaxed) | (add as u32);
        self.pressed_keys.store(new_pkeys, Relaxed);
    }

    pub fn rm(&self, remove: KeyMask) {
        let new_pkeys = self.pressed_keys.load(Relaxed) & !(remove as u32);
        self.pressed_keys.store(new_pkeys, Relaxed);
    }

    pub fn contains(&self, key: KeyMask) -> bool {
        (self.pressed_keys.load(Relaxed) & (key as u32)) != 0
    }
}

pub fn input_loop(event: KeyEvent, p_keys: Arc<PressedKeys>) -> bool {
    let PhysicalKey::Code(key_code) = event.physical_key else {
        return false;
    };

    let fn_ptr = match event.state {
        ElementState::Pressed => PressedKeys::add,
        ElementState::Released => PressedKeys::rm,
    };

    match key_code {
        KeyCode::KeyW => fn_ptr(&p_keys, KeyMask::W),
        KeyCode::KeyS => fn_ptr(&p_keys, KeyMask::S),
        KeyCode::ArrowLeft => fn_ptr(&p_keys, KeyMask::Left),
        KeyCode::ArrowRight => fn_ptr(&p_keys, KeyMask::Right),
        KeyCode::ArrowUp => fn_ptr(&p_keys, KeyMask::Up),
        KeyCode::ArrowDown => fn_ptr(&p_keys, KeyMask::Down),
        KeyCode::Escape => return true, // Signal to exit
        _ => (),
    }

    false
}

pub fn process_input(duration_ms: u32, p_keys: &PressedKeys, uniform: &mut vs::Data) {
    const TICK_TIME: f32 = 10.0;

    let mult = duration_ms as f32 / TICK_TIME;

    if p_keys.contains(KeyMask::W) {
        uniform.zoom /= 1.10_f32.powf(mult);
    }
    if p_keys.contains(KeyMask::S) && uniform.zoom < 2.0 {
        uniform.zoom *= 1.10_f32.powf(mult);
    }
    if p_keys.contains(KeyMask::Left) {
        uniform.pos_x -= 0.05 * uniform.zoom * mult;
    }
    if p_keys.contains(KeyMask::Right) {
        uniform.pos_x += 0.05 * uniform.zoom * mult;
    }
    if p_keys.contains(KeyMask::Up) {
        uniform.pos_y -= 0.05 * uniform.zoom * mult;
    }
    if p_keys.contains(KeyMask::Down) {
        uniform.pos_y += 0.05 * uniform.zoom * mult;
    }
}
