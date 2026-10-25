//! Replays a scripted input recording (`tests/scripts/*.txt`, format in `kh2::replay`) against the
//! disc on a flat floor with a wall and prints the per-tick trace and its digest.
//! `cargo run --release -p libkh2 --example replay -- <disc image> tests/scripts/basic.txt`

use std::sync::Arc;

use glam::Vec3;
use kh2::{world_from_surfaces, Surface};

fn main() {
    let mut args = std::env::args().skip(1);
    let iso = args.next().expect("disc image");
    let script = std::fs::read_to_string(args.next().expect("script file")).expect("script readable");
    let mut disc = kh2::data::Disc::open(std::path::Path::new(&iso)).unwrap_or_else(|e| panic!("{e}"));
    let game = Arc::new(disc.game().unwrap_or_else(|e| panic!("{e}")));
    let world = replay_world();
    let mut sora = kh2::sora::Sora::new(game, Vec3::ZERO);
    sora.motor.grounded = true;
    let steps = kh2::replay::parse(&script).unwrap_or_else(|e| panic!("{e}"));
    let trace = kh2::replay::run(&mut sora, &world, &steps);
    print!("{trace}");
    eprintln!("digest {:016x}", kh2::replay::digest(&trace));
}

/// The replay room: a 4000-unit floor and a wall across x = 800 facing -x.
pub fn replay_world() -> kh2::World {
    let s = 2000.0;
    let t = |v, flags| Surface { v, flags, ..Default::default() };
    world_from_surfaces(&[
        t([[-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s]], 1),
        t([[-s, 0.0, -s], [s, 0.0, s], [s, 0.0, -s]], 1),
        t([[800.0, -100.0, -s], [800.0, -100.0, s], [800.0, 600.0, s]], kh2::SURF_WALL),
        t([[800.0, -100.0, -s], [800.0, 600.0, s], [800.0, 600.0, -s]], kh2::SURF_WALL),
    ])
}
