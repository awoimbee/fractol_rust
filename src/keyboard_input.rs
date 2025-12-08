use std::sync::atomic::{AtomicU32, Ordering::*};
use std::sync::Arc;

use winit::event::{ElementState, KeyEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::vs;

pub enum KeyMask {
    UP = 0b1,
    DWN = 0b10,
    LFT = 0b100,
    RGT = 0b1000,
    W = 0b1_0000,
    S = 0b10_0000,
}

pub struct PressedKeys {
    pressed_keys: AtomicU32,
}

impl PressedKeys {
    pub fn new() -> PressedKeys {
        let p_keys = AtomicU32::new(0);
        PressedKeys { pressed_keys: p_keys }
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

pub fn input_loop(
    event: KeyEvent,
    p_keys: Arc<PressedKeys>,
) -> bool {
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
        KeyCode::ArrowLeft => fn_ptr(&p_keys, KeyMask::LFT),
        KeyCode::ArrowRight => fn_ptr(&p_keys, KeyMask::RGT),
        KeyCode::ArrowUp => fn_ptr(&p_keys, KeyMask::UP),
        KeyCode::ArrowDown => fn_ptr(&p_keys, KeyMask::DWN),
        KeyCode::Escape => return true, // Signal to exit
        _ => (),
    }

    false
}


pub fn process_input(duration_ms: u32, p_keys: &PressedKeys, uniform: &mut vs::Data) {
    const TICK_TIME: f32 = 5.0; // 5ms <=> 200Hz

    let mult = duration_ms as f32 / TICK_TIME;

    if p_keys.contains(KeyMask::W) {
        uniform.zoom /= 1.10_f32.powf(mult);
    }
    if p_keys.contains(KeyMask::S) && uniform.zoom < 2.0 {
        uniform.zoom *= 1.10_f32.powf(mult);
    }
    if p_keys.contains(KeyMask::LFT) {
        uniform.pos_x -= 0.05 * uniform.zoom * mult;
    }
    if p_keys.contains(KeyMask::RGT) {
        uniform.pos_x += 0.05 * uniform.zoom * mult;
    }
    if p_keys.contains(KeyMask::UP) {
        uniform.pos_y -= 0.05 * uniform.zoom * mult;
    }
    if p_keys.contains(KeyMask::DWN) {
        uniform.pos_y += 0.05 * uniform.zoom * mult;
    }
    // println!(
    //     "pos_x: {}, pos_y: {}, zoom: {}",
    //     uniform.pos_x, uniform.pos_y, uniform.zoom
    // );
}
