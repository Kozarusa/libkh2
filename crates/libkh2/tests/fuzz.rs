//! A fuzz run of the C API: random call orders with NULLs, out-of-range handles and indices, huge
//! and non-finite numbers, before `kh2_init` and (with `KH2_ISO`) after it. Every call must return
//! (no hang: a watchdog fails the run), no panic may reach the guard (the guard's
//! "internal error (panic)" message is counted as a failure), and the library must stay usable
//! (a fresh Sora still ticks at the end).
//!
//! Pointers are NULL or valid buffers of the size the call is told: passing a bad pointer is the
//! caller's fault in any C API, so it is not fuzzed. `KH2_FUZZ_STEPS` sets the number of calls
//! (default 4000 per phase); `KH2_FUZZ_SEED` the seed.

use std::ffi::{c_char, CString};
use std::sync::mpsc;
use std::time::Duration;

use kh2::ffi::*;
use kh2::Surface;

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
    fn chance(&mut self, k: u64) -> bool {
        self.below(k) == 0
    }
    fn int(&mut self) -> i32 {
        match self.below(8) {
            0 => i32::MIN,
            1 => i32::MAX,
            2 => -1,
            3 => 0,
            4 => self.below(4) as i32,
            5 => self.below(1000) as i32,
            _ => self.next() as i32,
        }
    }
    fn float(&mut self) -> f32 {
        match self.below(10) {
            0 => f32::NAN,
            1 => f32::INFINITY,
            2 => f32::NEG_INFINITY,
            3 => f32::MAX,
            4 => -f32::MAX,
            5 => 1.0e-40,
            6 => 0.0,
            _ => (self.below(20000) as f32 - 10000.0) * 0.37,
        }
    }
    fn byte(&mut self) -> u8 {
        self.next() as u8
    }
    fn handle(&mut self, live: &[i32]) -> i32 {
        if !live.is_empty() && self.below(4) != 0 {
            live[self.below(live.len() as u64) as usize]
        } else {
            self.int()
        }
    }
}

const BUF: usize = 16 * 1024;

struct Bufs {
    f: Vec<f32>,
    i: Vec<i32>,
    u16s: Vec<u16>,
    u8s: Vec<u8>,
    c: Vec<c_char>,
}

/// One random call. Returns false when the error message says a panic was caught.
unsafe fn call(r: &mut Rng, live: &mut Vec<i32>, b: &mut Bufs) {
    let h = r.handle(live);
    let fp = |r: &mut Rng, b: &mut Bufs| if r.chance(5) { std::ptr::null_mut() } else { b.f.as_mut_ptr() };
    let max = |r: &mut Rng, cap: usize| if r.chance(3) { cap as u32 } else { r.below(cap as u64 + 1) as u32 };
    let kinds = ["M_EX020", "M_EX010", "M_EX990", "", "P_EX100", "nonsense"];
    unsafe {
        let arm = r.below(65);
        if std::env::var_os("KH2_FUZZ_TRACE").is_some() {
            eprintln!("arm {arm} handle {h}");
        }
        match arm {
            0 => {
                let n = kh2_create(r.float(), r.float(), r.float());
                if n >= 0 {
                    live.push(n);
                }
            }
            1 => {
                kh2_delete(h);
                live.retain(|&x| x != h);
            }
            2..=5 => {
                let i = Kh2Inputs { stick_x: r.float(), stick_y: r.float(), cam_look_x: r.float(), cam_look_z: r.float(), buttons: r.next() as u32 };
                let mut s = Kh2State::default();
                let ip = if r.chance(6) { std::ptr::null() } else { &i as *const _ };
                let sp = if r.chance(6) { std::ptr::null_mut() } else { &mut s as *mut _ };
                if r.chance(2) {
                    kh2_tick(h, ip, sp);
                } else {
                    kh2_tick_field(h, ip, sp);
                }
            }
            6 => {
                let mut s = Kh2State::default();
                kh2_get_state(h, if r.chance(4) { std::ptr::null_mut() } else { &mut s });
            }
            7 => {
                kh2_set_position(h, r.float(), r.float(), r.float(), r.float());
            }
            8 => {
                kh2_set_target(h, r.float(), r.float(), r.float(), r.float(), r.float());
            }
            9 => {
                kh2_clear_target(h);
                kh2_report_hit(h);
            }
            10 => {
                kh2_hit_taken(h, r.int(), r.int(), r.float(), r.float(), r.float());
            }
            11 => {
                kh2_set_hp(h, r.int(), r.int());
                kh2_revive(h, r.int());
            }
            12 => {
                kh2_set_abilities(h, r.next(), r.next(), r.byte(), r.byte(), r.byte());
            }
            13 => {
                for v in b.u16s.iter_mut().take(64) {
                    *v = r.next() as u16 % 700;
                }
                let p = if r.chance(4) { std::ptr::null() } else { b.u16s.as_ptr() };
                kh2_set_loadout(h, p, r.below(65) as u32);
                kh2_get_loadout_block(h, if r.chance(4) { std::ptr::null_mut() } else { b.i.as_mut_ptr() });
            }
            14 => {
                kh2_set_high_jump(h, r.byte());
                kh2_set_growth(h, r.byte(), r.byte(), r.byte(), r.byte());
                kh2_set_pose_enabled(h, r.int());
            }
            15 => {
                let which = r.int();
                kh2_bone_count(h, which);
                let m = max(r, BUF / 16);
                kh2_get_bones(h, which, r.int(), fp(r, b), m);
                kh2_get_rest_bones(h, which, fp(r, b), m);
                let ip = if r.chance(5) { std::ptr::null_mut() } else { b.i.as_mut_ptr() };
                kh2_bone_parents(h, which, ip, max(r, BUF));
            }
            16 => {
                let mut v = [Kh2Volume::default(); 64];
                kh2_get_attack_volumes(h, if r.chance(4) { std::ptr::null_mut() } else { v.as_mut_ptr() }, max(r, 64));
                let cp = if r.chance(4) { std::ptr::null_mut() } else { b.c.as_mut_ptr() };
                kh2_motion_tag(h, cp, max(r, b.c.len()));
                kh2_jump_height(h);
            }
            17 => {
                let i = Kh2CamInput { stick_x: r.float(), stick_y: r.float(), zoom: r.float(), buttons: r.next() as u32, step: r.float() };
                let mut v = Kh2CamView::default();
                kh2_camera_tick(h, if r.chance(5) { std::ptr::null() } else { &i }, if r.chance(5) { std::ptr::null_mut() } else { &mut v });
            }
            18 => {
                kh2_camera_reset(h, r.int());
                for v in b.f.iter_mut().take(64 * 3) {
                    *v = r.float();
                }
                kh2_lock_on_pick(h, if r.chance(4) { std::ptr::null() } else { b.f.as_ptr() }, r.below(65) as u32);
            }
            19..=20 => {
                kh2_cast(h, r.int());
                kh2_set_mp(h, r.int(), r.int());
            }
            21 => {
                for v in b.u8s.iter_mut().take(8) {
                    *v = r.byte();
                }
                kh2_set_magic_levels(h, if r.chance(4) { std::ptr::null() } else { b.u8s.as_ptr() });
            }
            22 => {
                let mut e = Kh2Event::default();
                for _ in 0..r.below(8) {
                    kh2_poll_event(h, if r.chance(8) { std::ptr::null_mut() } else { &mut e });
                }
            }
            23 => {
                kh2_drive_setup(h, r.byte(), r.byte(), r.next() as u32, if r.chance(3) { std::ptr::null() } else { b.u8s.as_ptr() });
                kh2_drive_gain(h, r.int());
                kh2_report_hit_atkp(h, r.int());
            }
            24 => {
                kh2_drive_enter(h, r.int());
            }
            25 => {
                kh2_drive_revert(h);
                kh2_drive_objects(h, if r.chance(4) { std::ptr::null_mut() } else { b.i.as_mut_ptr() });
                kh2_drive_outgoing_damage(h, r.int(), r.int());
            }
            26 => {
                let which = r.int();
                kh2_motion_slot_count(h, which);
                kh2_motion_slot(h, which, r.int(), r.int());
                let mut m = Kh2MotionInfo::default();
                kh2_motion_info(h, which, r.int(), if r.chance(4) { std::ptr::null_mut() } else { &mut m });
            }
            27 => {
                let mut root = [0f32; 16];
                kh2_sample_motion(h, r.int(), r.int(), r.float(), fp(r, b), max(r, BUF / 16), if r.chance(3) { std::ptr::null_mut() } else { root.as_mut_ptr() });
            }
            28 => {
                kh2_damage(h, r.int(), r.int(), r.int(), r.int(), r.int(), r.int());
                kh2_set_level(h, r.int());
            }
            29 => {
                kh2_drive_set_second_keyblade(h, r.int());
                kh2_use_item(h, r.int());
            }
            30 => {
                kh2_report_defeat(h, r.int(), r.int());
            }
            31 => {
                kh2_party_add(h, r.int());
            }
            32..=33 => {
                kh2_party_tick(h);
            }
            34 => {
                let mut m = Kh2MemberState::default();
                let slot = r.int();
                kh2_party_state(h, slot, if r.chance(4) { std::ptr::null_mut() } else { &mut m });
                kh2_party_bones(h, slot, r.int(), fp(r, b), max(r, BUF / 16));
                kh2_party_set_level(h, slot, r.int());
                kh2_party_set_vitals(h, slot, r.int(), r.int(), r.int(), r.int());
                kh2_party_set_style(h, slot, r.int());
                let ids = [1u16, 2, 3, r.next() as u16];
                let counts = [r.byte(), r.byte(), r.byte(), r.byte()];
                kh2_party_set_items(h, slot, if r.chance(4) { std::ptr::null() } else { ids.as_ptr() }, counts.as_ptr(), r.below(5) as u32);
                let mut ev = Kh2MemberEvent::default();
                kh2_poll_member_event(h, if r.chance(5) { std::ptr::null_mut() } else { &mut ev });
            }
            35..=36 => {
                let k = CString::new(kinds[r.below(kinds.len() as u64) as usize]).unwrap();
                kh2_enemy_spawn(h, if r.chance(6) { std::ptr::null() } else { k.as_ptr() }, r.float(), r.float(), r.float(), r.float());
            }
            37..=40 => {
                kh2_enemies_tick(h);
            }
            41 => {
                let n = kh2_enemy_count(h);
                let i = if n > 0 && r.chance(2) { r.below(n as u64) as i32 } else { r.int() };
                let mut s = Kh2EnemyState::default();
                kh2_enemy_state(h, i, if r.chance(4) { std::ptr::null_mut() } else { &mut s });
                kh2_enemy_bones(h, i, r.int(), fp(r, b), max(r, BUF / 16));
            }
            42 => {
                let mut e = Kh2EnemyEvent::default();
                for _ in 0..r.below(8) {
                    kh2_poll_enemy_event(h, if r.chance(8) { std::ptr::null_mut() } else { &mut e });
                }
            }
            43 => {
                let mut o = Kh2ReactionOffer::default();
                kh2_reaction_offer(h, if r.chance(4) { std::ptr::null_mut() } else { &mut o });
                kh2_reaction_accept(h);
                kh2_reaction_force_offer(h, r.int());
            }
            51 => {
                let mut c = [0u16; 32];
                let mut st = [0i32; 32];
                kh2_limit_status(h, if r.chance(4) { std::ptr::null_mut() } else { c.as_mut_ptr() }, if r.chance(4) { std::ptr::null_mut() } else { st.as_mut_ptr() }, max(r, 32));
                kh2_limit_start(h, if r.chance(2) { [282, 344, 255, 338, 484][r.below(5) as usize] } else { r.int() });
                kh2_limit_set_world(h, r.int());
                let mut s = Kh2LimitState::default();
                kh2_limit_state(h, if r.chance(5) { std::ptr::null_mut() } else { &mut s });
                let mut e = Kh2LimitEvent::default();
                kh2_poll_limit_event(h, if r.chance(5) { std::ptr::null_mut() } else { &mut e });
            }
            52 => {
                let items = [383u16, 159, 25, 160, r.next() as u16];
                kh2_summon_set_arrival_event(h, r.int());
                kh2_summon_set_owned(h, if r.chance(4) { std::ptr::null() } else { items.as_ptr() }, r.below(6) as u32);
                let mut c = [0u16; 8];
                let mut st = [0i32; 8];
                kh2_summon_status(h, if r.chance(4) { std::ptr::null_mut() } else { c.as_mut_ptr() }, st.as_mut_ptr(), max(r, 8));
                kh2_summon(h, if r.chance(2) { [447, 89, 88, 306, 0x13c][r.below(5) as usize] } else { r.int() });
                let mut s = Kh2SummonState::default();
                kh2_summon_state(h, if r.chance(5) { std::ptr::null_mut() } else { &mut s });
                kh2_summon_bones(h, r.int(), fp(r, b), max(r, BUF / 16));
                let mut e = Kh2SummonEvent::default();
                kh2_poll_summon_event(h, if r.chance(5) { std::ptr::null_mut() } else { &mut e });
            }
            44 => {
                kh2_give_bonus(h, if r.chance(2) { r.below(40) as i32 } else { r.int() });
                kh2_form_abilities(h, r.int(), if r.chance(4) { std::ptr::null_mut() } else { b.u16s.as_mut_ptr() }, max(r, b.u16s.len()));
                kh2_set_weapon_set(h, r.int());
                let p = if r.chance(4) { std::ptr::null_mut() } else { b.u16s.as_mut_ptr() };
                kh2_granted_abilities(h, p, max(r, b.u16s.len()));
            }
            45 => {
                let cap = max(r, b.u8s.len());
                let p = if r.chance(4) { std::ptr::null_mut() } else { b.u8s.as_mut_ptr() };
                let n = kh2_state_save(h, p, cap);
                // Load it back, damaged sometimes.
                if n > 0 && (n as usize) <= b.u8s.len() {
                    if r.chance(2) {
                        let k = r.below(n as u64) as usize;
                        b.u8s[k] = r.byte();
                    }
                    let len = if r.chance(3) { r.below(n as u64 + 1) as u32 } else { n as u32 };
                    kh2_state_load(h, b.u8s.as_ptr(), len);
                }
            }
            46 => {
                for v in b.u8s.iter_mut().take(256) {
                    *v = r.byte();
                }
                kh2_state_load(h, if r.chance(4) { std::ptr::null() } else { b.u8s.as_ptr() }, r.below(257) as u32);
            }
            47 => {
                let n = r.below(8) as usize;
                let tris: Vec<Surface> = (0..n).map(|_| Surface { v: [[r.float(), r.float(), r.float()], [r.float(), r.float(), r.float()], [r.float(), r.float(), r.float()]], flags: r.next() as u32, group: r.byte(), material: r.byte(), _pad: 0 }).collect();
                kh2_surfaces_load(if r.chance(5) { std::ptr::null() } else { tris.as_ptr() }, n as u32);
                // Keep a floor most of the time so the run does something.
                if r.below(3) != 0 {
                    floor();
                }
            }
            48 => {
                let a = CString::new(["tt00", "", "zz99", "al00"][r.below(4) as usize]).unwrap();
                if r.chance(8) {
                    kh2_surfaces_load_map(if r.chance(4) { std::ptr::null() } else { a.as_ptr() });
                    floor();
                }
            }
            53 => {
                let a = CString::new(["tt04", "", "zz99", "tt03"][r.below(4) as usize]).unwrap();
                if r.chance(10) {
                    kh2_area_spawn(h, if r.chance(4) { std::ptr::null() } else { a.as_ptr() }, r.int(), r.int());
                }
                if r.chance(12) {
                    kh2_area_run(h, if r.chance(4) { std::ptr::null() } else { a.as_ptr() }, if r.chance(2) { 1 } else { r.int() });
                }
                kh2_area_raise_event(h, r.int(), r.int());
                let mut ev = [0i32; 16];
                kh2_area_take_events(h, if r.chance(4) { std::ptr::null_mut() } else { ev.as_mut_ptr() }, max(r, 8));
                if r.chance(20) {
                    kh2_area_end(h);
                }
                let mut ms = Kh2Mission::default();
                kh2_mission_state(h, if r.chance(4) { std::ptr::null_mut() } else { &mut ms });
                let mut fl = [0f32; 40];
                kh2_mission_orbs(h, if r.chance(4) { std::ptr::null_mut() } else { fl.as_mut_ptr() }, max(r, 10));
                kh2_mission_take_sounds(h, if r.chance(4) { std::ptr::null_mut() } else { fl.as_mut_ptr() }, max(r, 8));
                if r.chance(8) {
                    let tg = Kh2TalkTarget { pos: [r.float(), r.float(), r.float()], yaw: r.float(), part: r.int(), keep_facing: r.int(), own_frame: r.int(), examine: r.int(), talk_id: r.int() as u32 };
                    let mut rq = Kh2TalkRequest::default();
                    let m = CString::new(["N_BB010_RTN", "", "zz"][r.below(3) as usize]).unwrap();
                    kh2_talk_begin(h, if r.chance(4) { std::ptr::null() } else { m.as_ptr() }, if r.chance(6) { std::ptr::null() } else { &tg }, if r.chance(2) { std::ptr::null() } else { fl.as_ptr() }, max(r, 2), if r.chance(4) { std::ptr::null_mut() } else { &mut rq });
                }
                if r.chance(4) {
                    kh2_talk_end(h);
                }
                let mut cue = Kh2Cue::default();
                kh2_poll_cue(h, if r.chance(4) { std::ptr::null_mut() } else { &mut cue });
            }
            55 => {
                let ids = [41u16, 499, r.next() as u16, 0, 526];
                kh2_set_equipment(h, if r.chance(4) { std::ptr::null() } else { ids.as_ptr() }, r.below(6) as u32);
                kh2_party_set_equipment(h, r.int(), ids.as_ptr(), r.below(6) as u32);
                let mut info = Kh2ItemInfo::default();
                kh2_item_info(r.int(), if r.chance(5) { std::ptr::null_mut() } else { &mut info });
            }
            54 => {
                kh2_enemies_set_mission(h, r.int(), r.int(), r.int(), r.int());
                if r.chance(3) {
                    kh2_enemies_prune(h);
                } else if r.chance(6) {
                    kh2_enemies_clear(h);
                }
            }
            49 => {
                kh2_set_collision(r.next() as u32, r.int());
            }
            50 => {
                let cp = if r.chance(4) { std::ptr::null_mut() } else { b.c.as_mut_ptr() };
                kh2_last_error(cp, max(r, b.c.len()));
                kh2_version();
            }
            _ => {
                // Plain ticks of everything, the common case of a host.
                let i = Kh2Inputs { stick_x: (r.below(3) as f32) - 1.0, stick_y: 1.0, cam_look_x: 0.0, cam_look_z: 1.0, buttons: r.below(16) as u32 };
                let mut s = Kh2State::default();
                kh2_tick(h, &i, &mut s);
                kh2_party_tick(h);
                kh2_enemies_tick(h);
            }
        }
    }
}

fn floor() {
    let s = 3000.0;
    let t = |v| Surface { v, flags: 1, ..Default::default() };
    let tris = [t([[-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s]]), t([[-s, 0.0, -s], [s, 0.0, s], [s, 0.0, -s]])];
    unsafe { kh2_surfaces_load(tris.as_ptr(), 2) };
}

fn last_error() -> String {
    let mut buf = [0 as c_char; 512];
    let n = unsafe { kh2_last_error(buf.as_mut_ptr(), buf.len() as u32) } as usize;
    let bytes: Vec<u8> = buf[..n.min(511)].iter().map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Every panic of the run, caught by the guard or not (the hook sees them all).
static PANICS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

/// Runs `steps` random calls; returns the caught panics' messages.
fn phase(seed: u64, steps: usize) -> Vec<String> {
    let mut r = Rng(seed | 1);
    let mut live = Vec::new();
    let mut b = Bufs { f: vec![0.0; BUF * 16], i: vec![0; BUF], u16s: vec![0; 256], u8s: vec![0; 4096], c: vec![0; 256] };
    let mut panics = Vec::new();
    for k in 0..steps {
        unsafe { call(&mut r, &mut live, &mut b) };
        let caught: Vec<String> = std::mem::take(&mut *PANICS.lock().unwrap_or_else(|e| e.into_inner()));
        for e in caught {
            panics.push(format!("step {k}: {e}"));
        }
    }
    panics
}

#[test]
fn the_c_api_survives_random_calls() {
    let steps: usize = std::env::var("KH2_FUZZ_STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(4000);
    let seed: u64 = std::env::var("KH2_FUZZ_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(0x5eed_1234_abcd);
    let iso = std::env::var("KH2_ISO").ok();
    std::panic::set_hook(Box::new(|info| {
        PANICS.lock().unwrap_or_else(|e| e.into_inner()).push(info.to_string());
    }));
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut panics = phase(seed, steps / 4);
        let mut ran_disc = false;
        if let Some(iso) = iso {
            let p = CString::new(iso).unwrap();
            if unsafe { kh2_init(p.as_ptr()) } == 0 {
                ran_disc = true;
                floor();
                let h = kh2_create(0.0, 0.0, 0.0);
                unsafe { kh2_enemy_spawn(h, c"M_EX990".as_ptr(), 0.0, 0.0, 300.0, 3.0) };
                kh2_party_add(h, 0);
                kh2_party_add(h, 1);
                panics.extend(phase(seed ^ 0x9e37_79b9, steps));
                // Still usable: a fresh Sora on a fresh floor ticks.
                floor();
                let h = kh2_create(0.0, 0.0, 0.0);
                let mut s = Kh2State::default();
                let i = Kh2Inputs { stick_y: 1.0, cam_look_z: 1.0, ..Default::default() };
                let ok = h >= 0 && (0..30).all(|_| unsafe { kh2_tick(h, &i, &mut s) } >= 0) && s.pos[2] > 1.0;
                if !ok {
                    panics.push(format!("the library is not usable after the run: handle {h}, state {s:?}, error {}", last_error()));
                }
            }
        }
        kh2_terminate();
        let _ = tx.send((panics, ran_disc));
    });
    let (panics, ran_disc) = rx.recv_timeout(Duration::from_secs(600)).expect("a call hung (no answer in 600 s)");
    eprintln!("fuzz: {} steps, disc phase {ran_disc}, {} caught panics", steps, panics.len());
    for p in &panics {
        eprintln!("  {p}");
    }
    assert!(panics.is_empty(), "panics reached the C API guard");
}
