//! Speed and memory of a battle tick: Sora, Donald and Goofy against N enemies on a flat floor.
//!
//! `cargo run --release -p libkh2 --example bench -- [disc image]` (or KH2_ISO). For each N it runs
//! 600 ticks (10 s of game time) and prints the ticks per second of the whole tick (Sora, party,
//! enemies, posed skeletons) and the heap the library holds (a counting allocator: live bytes after
//! loading, and the peak).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use glam::Vec3;
use kh2::enemy::Enemies;
use kh2::party::{tick_battle, Follower, PartyRng};
use kh2::sora::{Input, Sora};
use kh2::{world_from_surfaces, Surface};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let now = LIVE.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static A: Counting = Counting;

fn mib(b: usize) -> f64 {
    b as f64 / (1024.0 * 1024.0)
}

fn main() {
    let iso = std::env::args().nth(1).or_else(|| std::env::var("KH2_ISO").ok()).expect("pass the disc image or set KH2_ISO");
    let t0 = Instant::now();
    let base = LIVE.load(Ordering::Relaxed);
    let mut d = kh2::data::Disc::open(std::path::Path::new(&iso)).expect("disc");
    let game = Arc::new(d.game().expect("Sora"));
    let after_sora = LIVE.load(Ordering::Relaxed);
    let data = Arc::new(d.enemies().expect("enemies"));
    let party = d.party().expect("party");
    let loaded = LIVE.load(Ordering::Relaxed);
    println!("load {:.2} s; heap: Sora {:.1} MiB, all enemy kinds and the party {:.1} MiB more, {:.1} MiB in all", t0.elapsed().as_secs_f64(), mib(after_sora - base), mib(loaded - after_sora), mib(loaded - base));
    let s = 6000.0;
    let t = |v| Surface { v, flags: 1, ..Default::default() };
    let w = world_from_surfaces(&[t([[-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s]]), t([[-s, 0.0, -s], [s, 0.0, s], [s, 0.0, -s]])]);
    let kinds = ["M_EX020", "M_EX010", "M_EX990"];
    println!("| enemies | ticks/s | ms/tick | x real time (60 Hz) | heap during the run (MiB) |");
    println!("| --- | --- | --- | --- | --- |");
    for n in [0usize, 1, 5, 10, 20, 50] {
        let mut sora = Sora::new(game.clone(), Vec3::ZERO);
        sora.motor.grounded = true;
        sora.max_hp = 9999;
        sora.hp = 9999;
        let mut en = Enemies::new(data.clone());
        for k in 0..n {
            let a = k as f32 * 0.7;
            let r = 400.0 + 60.0 * (k % 10) as f32;
            en.spawn(kinds[k % 3], Vec3::new(a.sin() * r, 0.0, a.cos() * r), a + std::f32::consts::PI, &w);
        }
        for e in en.list.iter_mut() {
            e.hp = 1_000_000;
            e.stats.max_hp = 1_000_000;
        }
        let mut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, -100.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, -100.0))];
        let mut rng = PartyRng::default();
        PEAK.store(LIVE.load(Ordering::Relaxed), Ordering::Relaxed);
        let ticks = 600;
        let start = Instant::now();
        for i in 0..ticks {
            sora.hp = 9999;
            sora.tick(&w, &Input { attack: i % 6 == 0, cam_look: [0.0, 1.0], ..Default::default() });
            tick_battle(&party, &mut fs, &mut rng, &mut sora, &w, Some(&mut en));
            en.tick(&mut sora, &w, game.params.sstm.fall_max);
            en.events.clear();
            sora.events.clear();
        }
        let secs = start.elapsed().as_secs_f64();
        let tps = ticks as f64 / secs;
        println!("| {n} | {tps:.0} | {:.3} | {:.0} | {:.1} |", 1000.0 / tps, tps / 60.0, mib(PEAK.load(Ordering::Relaxed) - base));
    }
}
