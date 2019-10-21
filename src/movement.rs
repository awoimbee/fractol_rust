use std::sync::atomic::{AtomicBool, Ordering::*};
use std::sync::{Arc, Mutex};
use std::{thread, time};

use crate::input::*;
use crate::vk_render::*;

const PHYSICS_TIME: u64 = 5; // 5ms <=> 200Hz

pub fn game_loop(exit: Arc<AtomicBool>, p_keys: Arc<PKeys>, uniform: Arc<Mutex<Uniform>>) {
    let mut zoom = 0.5;
    let mut pos_x = -1.;
    let mut pos_y = 0.;
    loop {
        let now = time::Instant::now();
        if exit.load(Relaxed) {
            return;
        }
        if p_keys.contains(BTKey::W) {
            zoom /= 1.10;
        }
        if p_keys.contains(BTKey::S) && zoom < 2. {
            zoom *= 1.10;
        }
        if p_keys.contains(BTKey::LFT) {
            pos_x -= 0.05 * zoom;
        }
        if p_keys.contains(BTKey::RGT) {
            pos_x += 0.05 * zoom;
        }
        if p_keys.contains(BTKey::UP) {
            pos_y -= 0.05 * zoom;
        }
        if p_keys.contains(BTKey::DWN) {
            pos_y += 0.05 * zoom;
        }

        let mut u = uniform.lock().unwrap(); // This lock here is causing some bad delays :/
        u.zoom = zoom;
        u.position_x = pos_x;
        u.position_y = pos_y;
        drop(u); // otherwise mutex is not unlocked

        let sleep_dur = match time::Duration::from_millis(PHYSICS_TIME).checked_sub(now.elapsed()) {
            Some(t) => t,
            None => {
                println!("Physics can't keep up !");
                time::Duration::from_secs(0)
            }
        };
        thread::sleep(sleep_dur);
    }
}
