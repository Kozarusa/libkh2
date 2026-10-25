//! A tiny pure-Rust host on the Rust API (no C ABI): a floor, Sora runs, jumps and attacks, one
//! line per 20 ticks. `cargo run --release -p libkh2 --example host -- <disc image>` (or KH2_ISO).

use std::sync::Arc;

use glam::Vec3;
use kh2::sora::{Input, Sora};
use kh2::{world_from_surfaces, Surface};

fn main() {
    let iso = std::env::args().nth(1).or_else(|| std::env::var("KH2_ISO").ok()).expect("pass the disc image or set KH2_ISO");
    let mut disc = kh2::data::Disc::open(std::path::Path::new(&iso)).unwrap_or_else(|e| panic!("{e}"));
    let game = Arc::new(disc.game().unwrap_or_else(|e| panic!("{e}")));
    let s = 5000.0;
    let t = |v| Surface { v, flags: 1, ..Default::default() };
    let world = world_from_surfaces(&[t([[-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s]]), t([[-s, 0.0, -s], [s, 0.0, s], [s, 0.0, -s]])]);
    let mut sora = Sora::new(game, Vec3::ZERO);
    sora.motor.grounded = true;
    for t in 0..240 {
        let input = Input { stick: if t < 60 { [0.0, 1.0] } else { [0.0; 2] }, cam_look: [0.0, 1.0], jump: (80..110).contains(&t), attack: t >= 150 && t % 5 == 0, guard: false };
        sora.tick(&world, &input);
        if !sora.attack_triggers().is_empty() {
            sora.connected = true;
        }
        if t % 20 == 0 {
            println!("t={t:3} {:?} {} pos ({:.1} {:.1} {:.1}) combo {}", sora.state, sora.body.tag(), sora.motor.pos.x, sora.motor.pos.y, sora.motor.pos.z, sora.hits);
        }
    }
}
