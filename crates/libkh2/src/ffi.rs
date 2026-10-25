//! The C ABI (`include/kh2.h`). One library state per process, as libsm64 has: the disc data, the
//! collision world and the Sora instances live behind a mutex; every call locks it.

use std::ffi::{c_char, CStr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use glam::{Mat4, Vec3};

use crate::data::{Disc, Game};
use crate::sora::{Input, Sora, Target};
use crate::{world_from_surfaces, Surface, World};

pub const KH2_BTN_ATTACK: u32 = 1;
pub const KH2_BTN_JUMP: u32 = 2;
pub const KH2_BTN_GUARD: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2Inputs {
    pub stick_x: f32,
    pub stick_y: f32,
    pub cam_look_x: f32,
    pub cam_look_z: f32,
    pub buttons: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2State {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub yaw: f32,
    pub grounded: i32,
    pub state: i32,
    pub motion_slot: i32,
    pub motion_id: i32,
    pub frame: f32,
    pub gframe: f32,
    pub motion_finished: i32,
    pub blending: i32,
    pub combo: i32,
    pub attack_active: i32,
    pub attack_id: i32,
    pub attack_group: i32,
    pub combo_window: i32,
    pub finisher_window: i32,
    pub weapon_out: i32,
    pub ground_material: i32,
    pub invulnerable: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2Volume {
    pub a: [f32; 3],
    pub b: [f32; 3],
    pub radius: f32,
    pub attack_id: i32,
    pub group: i32,
}

struct Lib {
    game: Arc<Game>,
    disc: Disc,
    world: World,
    soras: Vec<Option<Sora>>,
}

static LIB: Mutex<Option<Lib>> = Mutex::new(None);
static ERROR: Mutex<String> = Mutex::new(String::new());

fn set_error(e: impl std::fmt::Display) -> i32 {
    if let Ok(mut s) = ERROR.lock() {
        *s = e.to_string();
    }
    -1
}

fn with<R>(fallback: R, f: impl FnOnce(&mut Lib) -> R) -> R {
    match LIB.lock() {
        Ok(mut g) => match g.as_mut() {
            Some(l) => f(l),
            None => {
                set_error("kh2_init has not run");
                fallback
            }
        },
        Err(_) => fallback,
    }
}

/// `kh2_init`: opens the disc (NULL: the `KH2_ISO` environment variable) and loads Sora. 0 on
/// success, -1 on error (`kh2_last_error`).
///
/// # Safety
/// `iso_path` is NULL or a NUL-terminated UTF-8 path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_init(iso_path: *const c_char) -> i32 {
    let path = if iso_path.is_null() {
        match std::env::var_os("KH2_ISO") {
            Some(p) => PathBuf::from(p),
            None => return set_error("no disc path given and KH2_ISO is not set"),
        }
    } else {
        match unsafe { CStr::from_ptr(iso_path) }.to_str() {
            Ok(s) => PathBuf::from(s),
            Err(e) => return set_error(e),
        }
    };
    let mut disc = match Disc::open(&path) {
        Ok(d) => d,
        Err(e) => return set_error(e),
    };
    let game = match disc.game() {
        Ok(g) => Arc::new(g),
        Err(e) => return set_error(e),
    };
    match LIB.lock() {
        Ok(mut g) => {
            *g = Some(Lib { game, disc, world: world_from_surfaces(&[]), soras: Vec::new() });
            0
        }
        Err(e) => set_error(e),
    }
}

/// Frees everything `kh2_init` loaded.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_terminate() {
    if let Ok(mut g) = LIB.lock() {
        *g = None;
    }
}

/// Copies the last error message (NUL-terminated, cut to `len`); returns its full length.
///
/// # Safety
/// `buf` is NULL or points to `len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_last_error(buf: *mut c_char, len: u32) -> u32 {
    let s = ERROR.lock().map(|s| s.clone()).unwrap_or_default();
    unsafe { write_str(&s, buf, len) };
    s.len() as u32
}

unsafe fn write_str(s: &str, buf: *mut c_char, len: u32) {
    if buf.is_null() || len == 0 {
        return;
    }
    let n = s.len().min(len as usize - 1);
    unsafe {
        std::ptr::copy_nonoverlapping(s.as_ptr() as *const c_char, buf, n);
        *buf.add(n) = 0;
    }
}

/// Replaces the static collision with `count` host triangles. Returns the number kept (degenerate
/// ones are dropped), or -1.
///
/// # Safety
/// `tris` points to `count` `kh2_surface` records (or `count` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_surfaces_load(tris: *const Surface, count: u32) -> i32 {
    let tris = if count == 0 || tris.is_null() { &[][..] } else { unsafe { std::slice::from_raw_parts(tris, count as usize) } };
    with(-1, |l| {
        l.world = world_from_surfaces(tris);
        l.world.tris.len() as i32
    })
}

/// Replaces the static collision with an area's own COCT from the disc (`"tt00"`: `map/us/tt00.map`).
/// Returns the triangle count, or -1.
///
/// # Safety
/// `area` is a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_surfaces_load_map(area: *const c_char) -> i32 {
    if area.is_null() {
        return set_error("no area name");
    }
    let Ok(area) = unsafe { CStr::from_ptr(area) }.to_str().map(str::to_string) else { return set_error("area name is not UTF-8") };
    with(-1, |l| match l.disc.coct(&area) {
        Ok(c) => {
            l.world = World::from_coct(&c);
            l.world.tris.len() as i32
        }
        Err(e) => set_error(e),
    })
}

/// Switches mesh group `group` (0..63) of the collision on or off.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_collision(group: u32, solid: i32) {
    with((), |l| l.world.set_group(group, solid != 0));
}

/// Creates a Sora standing at (x, y, z). Returns its handle, or -1.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_create(x: f32, y: f32, z: f32) -> i32 {
    with(-1, |l| {
        let mut s = Sora::new(l.game.clone(), Vec3::new(x, y, z));
        // Settle on the floor below the start point, if there is one within a step.
        if let Some(g) = l.world.ground(s.motor.pos, y + s.motor.capsule.step, s.motor.capsule.step * 2.0) {
            s.motor.pos.y = g.y;
            s.motor.grounded = true;
            s.motor.ground = Some(g);
        } else {
            s.motor.grounded = false;
            s.state = crate::sora::State::Air;
            s.motor.start_fall();
        }
        match l.soras.iter().position(Option::is_none) {
            Some(i) => {
                l.soras[i] = Some(s);
                i as i32
            }
            None => {
                l.soras.push(Some(s));
                l.soras.len() as i32 - 1
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kh2_delete(handle: i32) {
    with((), |l| {
        if let Some(slot) = usize::try_from(handle).ok().and_then(|i| l.soras.get_mut(i)) {
            *slot = None;
        }
    });
}

fn sora_do<R>(h: i32, fallback: R, f: impl FnOnce(&mut Sora, &World) -> R) -> R {
    let mut f = Some(f);
    let r = with(None, |l| {
        let world = &l.world;
        let s = usize::try_from(h).ok().and_then(|i| l.soras.get_mut(i)).and_then(Option::as_mut)?;
        Some((f.take().unwrap())(s, world))
    });
    match r {
        Some(v) => v,
        None => {
            set_error(format!("no Sora with handle {h}"));
            fallback
        }
    }
}

fn input_of(i: &Kh2Inputs) -> Input {
    Input {
        stick: [i.stick_x, i.stick_y],
        cam_look: [i.cam_look_x, i.cam_look_z],
        attack: i.buttons & KH2_BTN_ATTACK != 0,
        jump: i.buttons & KH2_BTN_JUMP != 0,
        guard: i.buttons & KH2_BTN_GUARD != 0,
    }
}

fn state_of(s: &Sora) -> Kh2State {
    let attacks = s.attack_triggers();
    Kh2State {
        pos: s.motor.pos.to_array(),
        vel: s.motor.vel.to_array(),
        yaw: s.motor.yaw,
        grounded: s.motor.grounded as i32,
        state: s.state.code(),
        motion_slot: s.body.slot.map_or(-1, |x| x as i32),
        motion_id: s.motion_id().map_or(-1, |x| x as i32),
        frame: s.body.frame,
        gframe: s.body.gframe(),
        motion_finished: s.body.finished as i32,
        blending: s.body.blending() as i32,
        combo: i32::from(s.hits),
        attack_active: !attacks.is_empty() as i32,
        attack_id: attacks.first().map_or(-1, |a| i32::from(a.0)),
        attack_group: attacks.first().map_or(-1, |a| i32::from(a.1)),
        combo_window: s.combo_window() as i32,
        finisher_window: s.finisher_window() as i32,
        weapon_out: (s.battle_ticks > 0.0) as i32,
        ground_material: s.motor.ground.map_or(-1, |g| i32::from(g.material)),
        invulnerable: (s.invuln > 0.0) as i32,
    }
}

/// One 60 Hz logic tick. `state` may be NULL. Returns 0, or -1 for a bad handle.
///
/// # Safety
/// `inputs` points to a `kh2_inputs`; `state` is NULL or points to a writable `kh2_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_tick(handle: i32, inputs: *const Kh2Inputs, state: *mut Kh2State) -> i32 {
    let input = if inputs.is_null() { Input::default() } else { input_of(unsafe { &*inputs }) };
    sora_do(handle, -1, |s, w| {
        s.tick(w, &input);
        if !state.is_null() {
            unsafe { *state = state_of(s) };
        }
        0
    })
}

/// Two logic ticks with the same input (the 30 Hz field cadence: a frame of step 2 is two ticks of
/// step 1, kh2-rs `schedule::FrameClock`). Presses count once.
///
/// # Safety
/// As `kh2_tick`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_tick_field(handle: i32, inputs: *const Kh2Inputs, state: *mut Kh2State) -> i32 {
    let r = unsafe { kh2_tick(handle, inputs, std::ptr::null_mut()) };
    if r != 0 {
        return r;
    }
    unsafe { kh2_tick(handle, inputs, state) }
}

/// The current state without ticking.
///
/// # Safety
/// `state` points to a writable `kh2_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_get_state(handle: i32, state: *mut Kh2State) -> i32 {
    if state.is_null() {
        return -1;
    }
    sora_do(handle, -1, |s, _| {
        unsafe { *state = state_of(s) };
        0
    })
}

/// Moves Sora to (x, y, z) (a warp: velocity and jump cleared, state ground or air by the floor).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_position(handle: i32, x: f32, y: f32, z: f32, yaw: f32) -> i32 {
    sora_do(handle, -1, |s, w| {
        let capsule = s.motor.capsule;
        s.motor = kh2_game_core::motor::Motor { pos: Vec3::new(x, y, z), yaw, capsule, ..Default::default() };
        s.spawn = s.motor.pos;
        match w.ground(s.motor.pos, y + capsule.step, capsule.step * 2.0) {
            Some(g) => {
                s.motor.pos.y = g.y;
                s.motor.grounded = true;
                s.motor.ground = Some(g);
                s.state = crate::sora::State::Ground;
            }
            None => {
                s.motor.start_fall();
                s.state = crate::sora::State::Air;
            }
        }
        0
    })
}

/// The host's target (lock-on) point: an enemy's target-shape centre with its radius and half
/// height, world units. Feeds the attack table's range tests, the attack lunge, turn and leap.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_target(handle: i32, x: f32, y: f32, z: f32, radius: f32, half_height: f32) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.target = Some(Target { pos: Vec3::new(x, y, z), radius, half_height });
        0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kh2_clear_target(handle: i32) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.target = None;
        0
    })
}

/// The host's hit test found that the current swing touched something: sets the "a hit landed"
/// latch (actor +0x108 bit 0x2000000) that lets the combo chain go on. Cleared when the next attack
/// starts.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_report_hit(handle: i32) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.connected = true;
        0
    })
}

/// The ability block the attack table reads: bits 0..127 (`lo` = 0..63, `hi` = 64..127; bit 1 is
/// Guard) and the combo lengths (ground, air, finishers). Defaults: Guard, 3, 3, 1.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_abilities(handle: i32, lo: u64, hi: u64, max_ground: u8, max_air: u8, max_finishers: u8) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.abilities = crate::sora::Abilities { bits: u128::from(lo) | u128::from(hi) << 64, max_ground, max_air, max_finishers };
        0
    })
}

/// The High Jump level (0 = not learned, 1..5; the `fmab` row of that level gives the height).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_high_jump(handle: i32, level: u8) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.growth.high_jump = level.min(5);
        0
    })
}

/// All growth-ability levels (0..5): High Jump, Quick Run, Aerial Dodge, Glide.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_growth(handle: i32, high_jump: u8, quick_run: u8, aerial_dodge: u8, glide: u8) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.growth = crate::sora::Growth { high_jump: high_jump.min(5), quick_run: quick_run.min(5), aerial_dodge: aerial_dodge.min(5), glide: glide.min(5) };
        0
    })
}

/// Turns the per-tick skeleton evaluation on (default) or off. Off: no bone matrices, no attack
/// volumes, and the air leaps aim from Sora's feet instead of his target shape.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_pose_enabled(handle: i32, on: i32) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.pose = on != 0;
        0
    })
}

fn pose_of(s: &Sora, which: i32) -> Option<(&[Mat4], Mat4)> {
    match which {
        0 => Some((&s.body.pose, s.body_matrix())),
        1 => s.weapon.as_ref().map(|w| (w.pose.as_slice(), s.weapon_matrix())),
        _ => None,
    }
}

/// Number of bones of the body (`which` 0) or the keyblade (1); -1 when there is none.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_bone_count(handle: i32, which: i32) -> i32 {
    sora_do(handle, -1, |s, _| pose_of(s, which).map_or(-1, |(p, _)| p.len() as i32))
}

unsafe fn write_mats(mats: impl Iterator<Item = Mat4>, out: *mut f32, max: u32) -> i32 {
    if out.is_null() {
        return -1;
    }
    let mut n = 0;
    for m in mats.take(max as usize) {
        unsafe { std::ptr::copy_nonoverlapping(m.to_cols_array().as_ptr(), out.add(16 * n), 16) };
        n += 1;
    }
    n as i32
}

/// The drawn pose: up to `max` bone matrices (16 floats each, column-major) of the body (`which`
/// 0) or the keyblade (1). `space` 0: model space (the body's pose relative to its root; the
/// keyblade's relative to the hand bone); 1: world space. Returns the count written.
///
/// # Safety
/// `out` points to `16 * max` writable floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_get_bones(handle: i32, which: i32, space: i32, out: *mut f32, max: u32) -> i32 {
    sora_do(handle, -1, |s, _| {
        let Some((pose, root)) = pose_of(s, which) else { return -1 };
        let root = if space == 1 { root } else { Mat4::IDENTITY };
        unsafe { write_mats(pose.iter().map(|m| root * *m), out, max) }
    })
}

/// The rest pose (model space) of the body or the keyblade: bind matrices for skinning (their
/// inverses are the inverse bind matrices).
///
/// # Safety
/// `out` points to `16 * max` writable floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_get_rest_bones(handle: i32, which: i32, out: *mut f32, max: u32) -> i32 {
    sora_do(handle, -1, |s, _| {
        let rig = match which {
            0 => &s.body.rig,
            1 => match &s.weapon {
                Some(w) => &w.rig,
                None => return -1,
            },
            _ => return -1,
        };
        unsafe { write_mats(kh2_model::rest_pose(&rig.model.bones).into_iter(), out, max) }
    })
}

/// Parent index of each bone (-1 for a root), up to `max`.
///
/// # Safety
/// `out` points to `max` writable ints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_bone_parents(handle: i32, which: i32, out: *mut i32, max: u32) -> i32 {
    if out.is_null() {
        return -1;
    }
    sora_do(handle, -1, |s, _| {
        let rig = match which {
            0 => &s.body.rig,
            1 => match &s.weapon {
                Some(w) => &w.rig,
                None => return -1,
            },
            _ => return -1,
        };
        let n = rig.model.bones.len().min(max as usize);
        for (i, b) in rig.model.bones.iter().take(n).enumerate() {
            unsafe { *out.add(i) = b.parent };
        }
        n as i32
    })
}

/// The live attack volumes (world-space capsules of the attack shapes of the active attack
/// windows), up to `max`. Returns the number written; the host tests them against its targets and
/// calls `kh2_report_hit` on contact.
///
/// # Safety
/// `out` points to `max` writable `kh2_volume` records.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_get_attack_volumes(handle: i32, out: *mut Kh2Volume, max: u32) -> i32 {
    if out.is_null() {
        return -1;
    }
    sora_do(handle, -1, |s, _| {
        let v = s.attack_volumes();
        let n = v.len().min(max as usize);
        for (i, x) in v.iter().take(n).enumerate() {
            unsafe { *out.add(i) = Kh2Volume { a: x.a.to_array(), b: x.b.to_array(), radius: x.r, attack_id: i32::from(x.atkp), group: i32::from(x.group) } };
        }
        n as i32
    })
}

/// The playing motion's tag (`"A300"`), NUL-terminated, cut to `len`. Returns its length.
///
/// # Safety
/// `buf` points to `len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_motion_tag(handle: i32, buf: *mut c_char, len: u32) -> i32 {
    sora_do(handle, -1, |s, _| {
        let t = s.body.tag().to_string();
        unsafe { write_str(&t, buf, len) };
        t.len() as i32
    })
}

/// Sora's jump height from the disc (`prty` JumpHeight, or the High Jump row when set).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_jump_height(handle: i32) -> f32 {
    sora_do(handle, -1.0, |s, _| s.jump_height())
}
