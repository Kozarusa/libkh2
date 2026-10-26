//! A long soak of the C API with valid calls (round 8): random inputs, enemy spawns, area runs,
//! Struggle missions, party, drive forms, spells, save / load, prune / clear, camera, talks, for
//! `KH2_SOAK_SECS` seconds of wall time (skipped when it is unset; needs `KH2_ISO`).
//!
//! Checks: no panic reaches the C guard (`kh2_last_error` never says "panic"), the heap stays
//! within a bound (live bytes counted by this binary's allocator, which the statically linked
//! library uses too: after the first area cycle, then at the end, growth under 64 MiB), and the
//! same seed gives the same Sora state hash over a fixed stretch before and after the soak.

use std::alloc::{GlobalAlloc, Layout, System};
use std::ffi::CString;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::{Duration, Instant};

use kh2::ffi::*;

struct Counting;
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            let n = LIVE.fetch_add(l.size() as isize, Ordering::Relaxed) + l.size() as isize;
            PEAK.fetch_max(n, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, new) };
        if !q.is_null() {
            let n = LIVE.fetch_add(new as isize - l.size() as isize, Ordering::Relaxed) + new as isize - l.size() as isize;
            PEAK.fetch_max(n, Ordering::Relaxed);
        }
        q
    }
}

#[global_allocator]
static A: Counting = Counting;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n.max(1)
    }
    fn unit(&mut self) -> f32 {
        self.below(2001) as f32 / 1000.0 - 1.0
    }
}

fn mib(n: isize) -> f64 {
    n as f64 / (1024.0 * 1024.0)
}

fn last_error() -> String {
    let mut b = [0i8; 256];
    unsafe { kh2_last_error(b.as_mut_ptr(), b.len() as u32) };
    let s: Vec<u8> = b.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    String::from_utf8_lossy(&s).into_owned()
}

fn input(r: &mut Rng) -> Kh2Inputs {
    let mut i = Kh2Inputs { stick_x: r.unit(), stick_y: r.unit(), cam_look_x: 0.0, cam_look_z: 1.0, ..Default::default() };
    if r.below(5) == 0 {
        i.buttons |= 1;
    }
    if r.below(20) == 0 {
        i.buttons |= 2;
    }
    if r.below(30) == 0 {
        i.buttons |= 4;
    }
    i
}

/// A fixed stretch from a fresh Sora on tt04: the hash of every tick's position, state and motion.
fn replay_hash(h: i32, seed: u64) -> u64 {
    let mut r = Rng(seed);
    kh2_set_position(h, 0.0, 0.0, 0.0, 0.0);
    let mut hash = 0xcbf29ce484222325u64;
    let mut st = Kh2State::default();
    for _ in 0..600 {
        let i = input(&mut r);
        unsafe { kh2_tick(h, &i, &mut st) };
        for v in [st.pos[0].to_bits() as u64, st.pos[1].to_bits() as u64, st.pos[2].to_bits() as u64, st.state as u64, st.motion_id as u64] {
            hash = (hash ^ v).wrapping_mul(0x100000001b3);
        }
    }
    hash
}

#[test]
fn a_long_soak_stays_bounded_and_deterministic() {
    let Some(secs) = std::env::var("KH2_SOAK_SECS").ok().and_then(|s| s.parse::<u64>().ok()) else {
        eprintln!("skipped: set KH2_SOAK_SECS (and KH2_ISO)");
        return;
    };
    let Ok(iso) = std::env::var("KH2_ISO_FM").or_else(|_| std::env::var("KH2_ISO")) else { return eprintln!("skipped: no KH2_ISO") };
    let iso = CString::new(iso).unwrap();
    assert_eq!(unsafe { kh2_init(iso.as_ptr()) }, 0, "{}", last_error());
    let tt04 = CString::new("tt04").unwrap();
    assert!(unsafe { kh2_surfaces_load_map(tt04.as_ptr()) } >= 0, "{}", last_error());
    let probe = kh2_create(0.0, 0.0, 0.0);
    let before = replay_hash(probe, 7);
    let kinds: Vec<CString> = ["M_EX990", "M_EX870", "M_EX010", "N_EX570_BTL"].iter().map(|k| CString::new(*k).unwrap()).collect();
    let areas = [("tt04", 184), ("tt04", 80), ("tt04", 1), ("tt04", 182)];
    let mut r = Rng(0x5eed);
    let h = kh2_create(0.0, 0.0, 0.0);
    unsafe {
        kh2_party_add(h, 0);
        kh2_party_add(h, 1);
        kh2_drive_setup(h, 9, 9, 0x3f, std::ptr::null());
    }
    let start = Instant::now();
    let (mut ticks, mut cycles, mut warm) = (0u64, 0u64, None::<isize>);
    let mut save = vec![0u8; 4096];
    let mut st = Kh2State::default();
    let mut view = Kh2CamView::default();
    while start.elapsed() < Duration::from_secs(secs) {
        let i = input(&mut r);
        unsafe {
            kh2_tick(h, &i, &mut st);
            if st.attack_active != 0 && r.below(2) == 0 {
                kh2_report_hit(h);
            }
            kh2_party_tick(h);
            kh2_enemies_tick(h);
            kh2_camera_tick(h, std::ptr::null(), &mut view);
            match r.below(400) {
                0 => {
                    let k = &kinds[r.below(kinds.len() as u64) as usize];
                    kh2_enemy_spawn(h, k.as_ptr(), st.pos[0] + 300.0 * r.unit(), st.pos[1], st.pos[2] + 300.0 * r.unit(), 0.0);
                }
                1 => {
                    let (a, p) = areas[r.below(areas.len() as u64) as usize];
                    let a = CString::new(a).unwrap();
                    kh2_area_run(h, a.as_ptr(), p);
                }
                2 => {
                    kh2_area_end(h);
                    kh2_enemies_clear(h);
                    cycles += 1;
                    if cycles == 1 {
                        warm = Some(LIVE.load(Ordering::Relaxed));
                    }
                }
                3 => {
                    let n = kh2_state_save(h, save.as_mut_ptr(), save.len() as u32);
                    if n > 0 {
                        kh2_state_load(h, save.as_ptr(), n as u32);
                    }
                }
                4 => {
                    kh2_drive_gain(h, 100);
                    kh2_drive_enter(h, r.below(6) as i32);
                }
                5 => {
                    kh2_drive_revert(h);
                }
                6 => {
                    kh2_set_mp(h, 100, 100);
                    kh2_cast(h, r.below(4) as i32);
                }
                7 => {
                    kh2_enemies_prune(h);
                }
                8 => {
                    kh2_set_position(h, 0.0, 0.0, 0.0, 0.0);
                }
                _ => {}
            }
            if st.hp <= 0 {
                kh2_revive(h, 0);
            }
        }
        ticks += 1;
        let e = last_error();
        assert!(!e.contains("panic"), "tick {ticks}: {e}");
    }
    kh2_area_end(h);
    kh2_enemies_clear(h);
    kh2_delete(h);
    let end = LIVE.load(Ordering::Relaxed);
    let after = replay_hash(probe, 7);
    let warm = warm.unwrap_or(end);
    eprintln!(
        "soak: {ticks} ticks in {secs} s, {cycles} area cycles, live heap after the first cycle {:.1} MiB, at the end {:.1} MiB, peak {:.1} MiB; replay hash {before:#x} / {after:#x}",
        mib(warm),
        mib(end),
        mib(PEAK.load(Ordering::Relaxed))
    );
    assert_eq!(before, after, "the same seed replays the same after the soak");
    assert!(end - warm < 64 << 20, "heap grew {:.1} MiB", mib(end - warm));
    kh2_terminate();
}
