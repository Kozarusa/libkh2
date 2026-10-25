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
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    /// MP charge frames left (0 when no charge runs).
    pub mp_charge: f32,
    /// Drive: the active form (0 none), gauge points, fraction 0..99, form time left / total.
    pub form: i32,
    pub drive_points: i32,
    pub drive_fraction: i32,
    pub form_time: f32,
    pub form_time_total: f32,
    pub level: i32,
    pub exp: i32,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
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
    forms: std::collections::HashMap<(u8, u16), Arc<crate::drive::FormAssets>>,
    cameras: Vec<Option<crate::camera::Camera>>,
    party: Option<Arc<crate::party::PartyData>>,
    followers: std::collections::HashMap<i32, Vec<crate::party::Follower>>,
}

static LIB: Mutex<Option<Lib>> = Mutex::new(None);
static ERROR: Mutex<String> = Mutex::new(String::new());

fn set_error(e: impl std::fmt::Display) -> i32 {
    if let Ok(mut s) = ERROR.lock() {
        *s = e.to_string();
    }
    -1
}

/// Runs `f` and turns a panic into an error return: no panic crosses the C ABI (the release
/// profile unwinds for this). The panic message goes to `kh2_last_error`.
fn guard<R>(fallback: R, f: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(p) => {
            let msg = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default();
            set_error(format!("internal error (panic): {msg}"));
            fallback
        }
    }
}

/// The library state, locked. A lock poisoned by an earlier panic is taken over: the state is
/// plain data and every entry point leaves it consistent between ticks.
fn lib() -> std::sync::MutexGuard<'static, Option<Lib>> {
    LIB.lock().unwrap_or_else(|e| e.into_inner())
}

fn with<R>(fallback: R, f: impl FnOnce(&mut Lib) -> R) -> R {
    let mut g = lib();
    match g.as_mut() {
        Some(l) => {
            let mut fallback = Some(fallback);
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(l)));
            match r {
                Ok(v) => v,
                Err(p) => {
                    let msg = p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_default();
                    set_error(format!("internal error (panic): {msg}"));
                    fallback.take().unwrap()
                }
            }
        }
        None => {
            set_error("kh2_init has not run");
            fallback
        }
    }
}

/// `kh2_init`: opens the disc (NULL: the `KH2_ISO` environment variable) and loads Sora. 0 on
/// success, -1 on error (`kh2_last_error`).
///
/// # Safety
/// `iso_path` is NULL or a NUL-terminated UTF-8 path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_init(iso_path: *const c_char) -> i32 {
    guard(-1, || unsafe { init(iso_path) })
}

unsafe fn init(iso_path: *const c_char) -> i32 {
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
    *lib() = Some(Lib { game, disc, world: world_from_surfaces(&[]), soras: Vec::new(), cameras: Vec::new(), forms: Default::default(), party: None, followers: Default::default() });
    0
}

/// The library version: major << 16 | minor << 8 | patch (compare with `KH2_VERSION` in kh2.h).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_version() -> u32 {
    let n = |s: &str| s.parse::<u32>().unwrap_or(0);
    (n(env!("CARGO_PKG_VERSION_MAJOR")) << 16) | (n(env!("CARGO_PKG_VERSION_MINOR")) << 8) | n(env!("CARGO_PKG_VERSION_PATCH"))
}

/// Frees everything `kh2_init` loaded.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_terminate() {
    guard((), || *lib() = None);
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
        hp: s.hp,
        max_hp: s.max_hp,
        mp: s.mp,
        max_mp: s.max_mp,
        mp_charge: s.charge.left,
        form: if s.drive.in_form { i32::from(s.drive.form) } else { 0 },
        drive_points: i32::from(s.drive.points.whole),
        drive_fraction: i32::from(s.drive.points.fraction),
        form_time: s.drive.timer.left,
        form_time_total: s.drive.timer.total,
        level: s.level,
        exp: s.exp,
        strength: s.strength,
        magic: s.magic,
        defense: s.defense,
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

/// A hit taken: `atkp` the attack row id (from the disc's `atkp` table), `damage` the HP it takes,
/// (dir_x, dir_y, dir_z) the hit direction from the attacker toward Sora. Returns 0 ignored
/// (invincible or down), 1 guarded, 2 damage without flinch, 3 flinch, 4 blown, -1 unknown row.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_hit_taken(handle: i32, atkp: i32, damage: i32, dir_x: f32, dir_y: f32, dir_z: f32) -> i32 {
    sora_do(handle, -1, |s, _| s.hit(atkp as i16, damage, Vec3::new(dir_x, dir_y, dir_z)) as i32)
}

/// Sets HP and maximum HP (defaults: the disc's level 1 values).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_hp(handle: i32, hp: i32, max_hp: i32) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.max_hp = max_hp.max(1);
        s.hp = hp.clamp(0, s.max_hp);
        0
    })
}

/// Stands Sora up from the knock-out (full HP). `auto` != 0 makes him stand up by himself after
/// 180 ticks down, as kh2-rs does where no game over runs.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_revive(handle: i32, auto: i32) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.auto_revive = auto != 0;
        if s.state == crate::sora::State::Down {
            s.revive();
        }
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

/// The equipped abilities (ability item ids, `count` of them; the Kingdom Key stays equipped):
/// the attack table's ability bits, the combo lengths (Combo Plus, Air Combo Plus, Finishing Plus,
/// Negative Combo), the growth levels, Berserk Charge, Second Chance / Once More, Leaf Bracer.
/// Replaces `kh2_set_abilities` / `kh2_set_growth`. -1 without the ability tables.
///
/// # Safety
/// `ids` points to `count` u16 values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_set_loadout(handle: i32, ids: *const u16, count: u32) -> i32 {
    let v: Vec<u16> = if ids.is_null() || count == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(ids, count.min(64) as usize) }.to_vec() };
    sora_do(handle, -1, |s, _| if s.set_loadout(&v) { 0 } else { set_error("no ability tables") })
}

/// The block the loadout gives: out = [ground combo, air combo, finishers, high jump, quick run,
/// aerial dodge, glide] levels/lengths.
///
/// # Safety
/// `out` points to 7 ints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_get_loadout_block(handle: i32, out: *mut i32) -> i32 {
    if out.is_null() {
        return -1;
    }
    sora_do(handle, -1, |s, _| {
        let a = s.abilities;
        let g = s.growth;
        let v = [a.max_ground, a.max_air, a.max_finishers, g.high_jump, g.quick_run, g.aerial_dodge, g.glide].map(i32::from);
        unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), out, 7) };
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
        2 => s.weapon2.as_ref().map(|w| (w.pose.as_slice(), s.weapon2_matrix())),
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
        let Some(rig) = rig_of(s, which) else { return -1 };
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
        let Some(rig) = rig_of(s, which) else { return -1 };
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2CamInput {
    pub stick_x: f32,
    pub stick_y: f32,
    pub zoom: f32,
    /// KH2_CAM_RESET (R3), KH2_CAM_SELECT (look-around), KH2_CAM_MANUAL (no auto-camera).
    pub buttons: u32,
    /// 1 per 60 Hz tick (0 means 1), 2 per 30 Hz field frame.
    pub step: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2CamView {
    pub pos: [f32; 3],
    pub look: [f32; 3],
    pub fov: f32,
    pub roll: f32,
    pub yaw: f32,
    pub mode: i32,
}

/// One field-camera tick for Sora `handle` (call after `kh2_tick`). The camera follows him, turns
/// with the right stick, locks onto `kh2_set_target`'s point and collides with the static
/// collision. Feed `view.look - view.pos` back as the next `kh2_inputs` camera look direction.
///
/// # Safety
/// `input` is NULL or points to a `kh2_cam_input`; `view` points to a writable `kh2_cam_view`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_camera_tick(handle: i32, input: *const Kh2CamInput, view: *mut Kh2CamView) -> i32 {
    if view.is_null() {
        return set_error("no view record");
    }
    let i = if input.is_null() { Kh2CamInput::default() } else { unsafe { *input } };
    let ci = crate::camera::CamInput { stick: [i.stick_x, i.stick_y], reset: i.buttons & 1 != 0, select: i.buttons & 2 != 0, zoom: i.zoom, auto: i.buttons & 4 == 0, step: i.step };
    with(-1, |l| {
        let Some(p) = l.game.params.camera.as_ref() else { return set_error("the camera constants could not be read from the executable") };
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) else { return set_error(format!("no Sora with handle {handle}")) };
        let k = handle as usize;
        if l.cameras.len() <= k {
            l.cameras.resize_with(k + 1, || None);
        }
        let cam = l.cameras[k].get_or_insert_with(|| crate::camera::Camera::new(0));
        let v = cam.tick(p, s, &l.world, &ci);
        unsafe { *view = Kh2CamView { pos: v.pos.to_array(), look: v.look.to_array(), fov: v.fov, roll: v.roll, yaw: v.yaw, mode: v.mode } };
        0
    })
}

/// The camera table row (0 default, 1 indoor, 2, 3: the area camera types of the game) and a reset
/// of the camera behind Sora on the next tick.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_camera_reset(handle: i32, row: i32) -> i32 {
    with(-1, |l| {
        let Ok(k) = usize::try_from(handle) else { return -1 };
        if l.cameras.len() <= k {
            l.cameras.resize_with(k + 1, || None);
        }
        l.cameras[k] = Some(crate::camera::Camera::new(row.clamp(0, 3) as usize));
        0
    })
}

/// The lock-on helper: the index of the nearest of `count` points (x, y, z each) within the disc's
/// lock-on range of Sora, or -1. Pass the chosen point to `kh2_set_target`.
///
/// # Safety
/// `points` points to `3 * count` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_lock_on_pick(handle: i32, points: *const f32, count: u32) -> i32 {
    if points.is_null() {
        return -1;
    }
    let pts: Vec<Vec3> = unsafe { std::slice::from_raw_parts(points, 3 * count as usize) }.chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
    sora_do(handle, -1, |s, _| crate::camera::lock_on_pick(s, &pts).map_or(-1, |i| i as i32))
}

/// Asks for a cast of magic slot `slot` (0 Fire, 1 Blizzard, 2 Thunder, 3 Cure, 4 Magnet, 5
/// Reflect). It is tried on the next ticks (up to 30) until the state and MP allow it.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_cast(handle: i32, slot: i32) -> i32 {
    if !(0..6).contains(&slot) {
        return set_error("magic slot is 0..5");
    }
    sora_do(handle, -1, |s, _| {
        if s.spell(slot as u8).is_none() {
            return set_error("the slot has no spell at its level");
        }
        s.cast_request = Some((slot as u8, 30));
        0
    })
}

/// Spell levels of the six magic slots (0 = not owned; default Fire, Blizzard, Thunder, Cure at 1).
///
/// # Safety
/// `levels` points to 6 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_set_magic_levels(handle: i32, levels: *const u8) -> i32 {
    if levels.is_null() {
        return -1;
    }
    let l: [u8; 6] = unsafe { std::ptr::read(levels as *const [u8; 6]) };
    sora_do(handle, -1, |s, _| {
        s.magic_levels = l;
        0
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_mp(handle: i32, mp: i32, max_mp: i32) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.max_mp = max_mp.max(0);
        s.mp = mp.clamp(0, s.max_mp);
        0
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2Event {
    /// KH2_EV_*: 1 cast started, 2 spell fired, 3 cast cancelled, 4 MP restored, 5 form changed,
    /// 6 spell attack, 7 healed.
    pub kind: i32,
    pub slot: i32,
    /// The cast kind (atkp sub-row key / magi row).
    pub cast_kind: i32,
    pub motion: i32,
    pub voice: i32,
    pub charge: i32,
    /// Spell attack: stage, atkp id and sub id, size, life (ticks), speed, start and direction.
    pub stage: i32,
    pub atkp: i32,
    pub sub_id: i32,
    pub radius: f32,
    pub height: f32,
    pub life: f32,
    pub speed: f32,
    pub pos: [f32; 3],
    pub dir: [f32; 3],
    /// Healed: the amount and the HP after it.
    pub amount: i32,
    pub hp: i32,
}

/// Takes the oldest pending event; returns 1 when one was written, 0 when none is left.
///
/// # Safety
/// `out` points to a writable `kh2_event`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_poll_event(handle: i32, out: *mut Kh2Event) -> i32 {
    if out.is_null() {
        return -1;
    }
    sora_do(handle, -1, |s, _| {
        if s.events.is_empty() {
            return 0;
        }
        let e = s.events.remove(0);
        use crate::sora::Event as E;
        let d = Kh2Event { motion: -1, voice: -1, cast_kind: -1, slot: -1, stage: -1, atkp: -1, sub_id: -1, ..Default::default() };
        let v = match e {
            E::CastStarted { slot, kind, motion, voice } => Kh2Event { kind: 1, slot: slot.into(), cast_kind: kind as i32, motion: motion.into(), voice: voice.into(), ..d },
            E::SpellFired { slot, kind, charge } => Kh2Event { kind: 2, slot: slot.into(), cast_kind: kind as i32, charge: charge as i32, ..d },
            E::CastCancelled { slot } => Kh2Event { kind: 3, slot: slot.into(), ..d },
            E::MpRestored => Kh2Event { kind: 4, ..d },
            E::FormChanged { from, to } => Kh2Event { kind: 5, slot: to.into(), cast_kind: from.into(), ..d },
            E::SpellAttack { slot, stage, atkp, sub_id, radius, height, life, speed, pos, dir } => {
                Kh2Event { kind: 6, slot: slot.into(), stage: stage.into(), atkp: atkp.into(), sub_id: sub_id.into(), radius, height, life, speed, pos: pos.to_array(), dir: dir.to_array(), ..d }
            }
            E::Healed { amount, hp } => Kh2Event { kind: 7, amount, hp, ..d },
            E::ItemUsed { item, hp, mp, drive } => Kh2Event { kind: 8, slot: item.into(), amount: hp, hp: mp, charge: drive, ..d },
            E::Experience { amount, total } => Kh2Event { kind: 9, amount, hp: total, ..d },
            E::LevelUp { level } => Kh2Event { kind: 10, amount: level, ..d },
        };
        unsafe { *out = v };
        1
    })
}

/// Drive gauge and forms: gauge points and maximum, the owned forms (bit 1 Valor, 2 Wisdom,
/// 3 Master, 4 Final; bit 5 Anti is never owned) and the form levels (6 bytes by form id).
///
/// # Safety
/// `levels` is NULL or points to 6 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_drive_setup(handle: i32, points: u8, max_points: u8, owned_mask: u32, levels: *const u8) -> i32 {
    let lv: Option<[u8; 6]> = (!levels.is_null()).then(|| unsafe { std::ptr::read(levels as *const [u8; 6]) });
    sora_do(handle, -1, |s, _| {
        let d = &mut s.drive;
        d.points.max = max_points;
        d.points.whole = points.min(max_points);
        d.points.fraction = 0;
        d.owned = std::array::from_fn(|i| owned_mask >> i & 1 != 0 && i != 5);
        if let Some(l) = lv {
            d.level = l;
        }
        0
    })
}

/// Adds drive gauge (percent of a point; 100 = one point), outside a form.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_drive_gain(handle: i32, percent: i32) -> i32 {
    sora_do(handle, -1, |s, _| {
        if !s.drive.in_form {
            s.drive.points.gain(percent);
        }
        0
    })
}

/// A landed hit with the attack row id: the combo latch (as `kh2_report_hit`) and the row's
/// DriveDrain (gauge gain, or form time lost in a form).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_report_hit_atkp(handle: i32, atkp: i32) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.connected = true;
        s.drive_hit(atkp as i16);
        0
    })
}

/// Enters drive form `form` (1 Valor, 2 Wisdom, 3 Master, 4 Final) through the entry checks and
/// the Anti / Final roll. Returns the form entered (5 for Anti), or -1 (`kh2_last_error`). The
/// body swaps when the change motion ends (`KH2_EV_FORM_CHANGED`).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_drive_enter(handle: i32, form: i32) -> i32 {
    let r = with(Err("no Sora".to_string()), |l| {
        let Some(data) = l.game.drive.clone() else { return Err("no drive tables".to_string()) };
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) else { return Err(format!("no Sora with handle {handle}")) };
        let (disc, cache) = (&mut l.disc, &mut l.forms);
        let hand1 = s.hand1_item;
        let mut load = |f: u8| -> Option<Arc<crate::drive::FormAssets>> {
            if let Some(a) = cache.get(&(f, hand1)) {
                return Some(a.clone());
            }
            let a = Arc::new(disc.form_assets(&data, f, 0, hand1).ok()?);
            cache.insert((f, hand1), a.clone());
            Some(a)
        };
        s.drive_enter(form.clamp(0, 255) as u8, &mut load)
    });
    match r {
        Ok(f) => i32::from(f),
        Err(e) => set_error(e),
    }
}

/// Reverts to base Sora (refused in Anti form while in battle, or during a change).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_drive_revert(handle: i32) -> i32 {
    sora_do(handle, -1, |s, _| if s.drive_revert(true) { 0 } else { set_error("cannot revert now") })
}

/// The active form's object and weapon object ids (objentry ids): out[0] body, out[1] hand 0,
/// out[2] hand 1 (-1 none). Returns 0, or -1 when no form is active.
///
/// # Safety
/// `out` points to 3 writable ints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_drive_objects(handle: i32, out: *mut i32) -> i32 {
    if out.is_null() {
        return -1;
    }
    sora_do(handle, -1, |s, _| {
        let Some(f) = s.drive.files.as_ref() else { return -1 };
        let v = [i32::from(f.object), f.hands[0].as_ref().map_or(-1, |h| h.0 as i32), f.hands[1].as_ref().map_or(-1, |h| h.0 as i32)];
        unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), out, 3) };
        0
    })
}

/// The damage Sora deals in his form (kh2-rs `rules::outgoing_damage`: Anti scales non-magic
/// attack types).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_drive_outgoing_damage(handle: i32, atk_type: i32, damage: i32) -> i32 {
    sora_do(handle, -1, |s, _| match s.game.drive.as_ref() {
        Some(d) => kh2_game_core::drive_rules::outgoing_damage(&d.tuning, if s.drive.in_form { s.drive.form } else { 0 }, atk_type as u8, damage),
        None => damage,
    })
}

fn rig_of(s: &Sora, which: i32) -> Option<Arc<crate::anim::Rig>> {
    match which {
        0 => Some(s.body.rig.clone()),
        1 => s.weapon.as_ref().map(|w| w.rig.clone()),
        2 => s.weapon2.as_ref().map(|w| w.rig.clone()),
        _ => None,
    }
}

/// Number of slots in the motion set of the body (0) or keyblade (1); slots may be empty.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_motion_slot_count(handle: i32, which: i32) -> i32 {
    sora_do(handle, -1, |s, _| rig_of(s, which).map_or(-1, |r| r.mset.slots.len() as i32))
}

/// The slot of a motion id in a variant (0 battle + weapon, 1 battle, 2 field, 3 field + weapon),
/// with the game's variant fallback; -1 when the motion set has none.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_motion_slot(handle: i32, which: i32, motion_id: i32, variant: i32) -> i32 {
    use kh2_anim::PlayerState as P;
    let v = match variant {
        0 => P::BattleWeapon,
        1 => P::BattleNoWeapon,
        2 => P::FieldNoWeapon,
        _ => P::FieldWeapon,
    };
    sora_do(handle, -1, |s, _| rig_of(s, which).and_then(|r| r.mset.slot(motion_id.max(0) as usize, v)).map_or(-1, |x| x as i32))
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2MotionInfo {
    pub frame_start: f32,
    pub frame_end: f32,
    pub frame_return: f32,
    /// Frames per second the motion is authored at (the animator steps fps / 60 frames per tick).
    pub fps: f32,
    pub has_root_motion: i32,
    pub tag: [u8; 8],
}

/// A slot's frame range, rate and tag. Returns 0, or -1 for an empty slot.
///
/// # Safety
/// `out` points to a writable `kh2_motion_info`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_motion_info(handle: i32, which: i32, slot: i32, out: *mut Kh2MotionInfo) -> i32 {
    if out.is_null() || slot < 0 {
        return -1;
    }
    sora_do(handle, -1, |s, _| {
        let Some(r) = rig_of(s, which) else { return -1 };
        let Some(sl) = r.mset.get(slot as usize) else { return -1 };
        let Some(m) = sl.motion.as_ref() else { return -1 };
        let mut tag = [0u8; 8];
        for (i, b) in sl.tag.bytes().take(7).enumerate() {
            tag[i] = b;
        }
        let root = matches!(&m.data, kh2_anim::motion::Data::Interpolated(ip) if ip.root.as_ref().is_some_and(|x| x.enabled));
        unsafe { *out = Kh2MotionInfo { frame_start: m.frame_start, frame_end: m.frame_end, frame_return: m.frame_return, fps: m.fps, has_root_motion: root as i32, tag } };
        0
    })
}

/// Samples any motion slot at `frame` (the motion's own frame units): up to `max` bone matrices
/// (model space, relative to the motion's root, 16 floats column-major) and the root as
/// (x, y, z, yaw) in `root` (may be NULL). Independent of what Sora plays. Returns the bone count.
///
/// # Safety
/// `out` points to `16 * max` floats; `root` is NULL or points to 4 floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_sample_motion(handle: i32, which: i32, slot: i32, frame: f32, out: *mut f32, max: u32, root: *mut f32) -> i32 {
    if slot < 0 {
        return -1;
    }
    sora_do(handle, -1, |s, _| {
        let Some(r) = rig_of(s, which) else { return -1 };
        let Some((pose, rp)) = crate::anim::sample(&r, slot as usize, frame) else { return -1 };
        if !root.is_null() {
            let v = [rp.translation.x, rp.translation.y, rp.translation.z, rp.yaw];
            unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), root, 4) };
        }
        unsafe { write_mats(pose.into_iter(), out, max) }
    })
}

/// The damage of one of Sora's attack rows (`sub_id` -1: the row's first variant) against a target
/// with `defense`, `max_hp` (type 5 rows) and the percents for the row's element and in general
/// (100 = neutral). kh2-rs `GameData::damage`. Returns -1 for an unknown row.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_damage(handle: i32, atkp: i32, sub_id: i32, defense: i32, max_hp: i32, weakness: i32, general: i32) -> i32 {
    sora_do(handle, -1, |s, _| s.damage(atkp as u16, (sub_id >= 0).then_some(sub_id as u16), defense, max_hp, weakness, general).unwrap_or(-1))
}

/// Sora's level (1..99): rebuilds strength, magic and defense from the disc's `lvup` row.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_level(handle: i32, level: i32) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.set_level(level);
        0
    })
}

/// The keyblade item (item id) a drive form's second hand takes (Valor, Master, Final; default the
/// Kingdom Key, 41). Applies to the next entry.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_drive_set_second_keyblade(handle: i32, item: i32) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.hand1_item = item.clamp(0, u16::MAX as i32) as u16;
        0
    })
}

/// Uses item `item` (item id of the disc's item table: Potion 1, Hi-Potion 2, Ether 3, Elixir 4,
/// Drive Recovery 274, ...): the use motion plays and the effect lands 30 frames after its trigger
/// (`KH2_EV_ITEM_USED`). The host keeps the inventory. -1 when Sora is busy or the item is unknown.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_use_item(handle: i32, item: i32) -> i32 {
    sora_do(handle, -1, |s, _| if s.use_item(item.clamp(0, u16::MAX as i32) as u16) { 0 } else { set_error("cannot use the item now") })
}

/// A defeat by Sora's side: `lvpm_experience` the lvpm row's Experience for the world level,
/// `enmp_experience` the enemy's. Returns the experience paid (level-ups arrive as events).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_report_defeat(handle: i32, lvpm_experience: i32, enmp_experience: i32) -> i32 {
    sora_do(handle, -1, |s, _| s.report_defeat(lvpm_experience.clamp(0, 65535) as u16, enmp_experience.clamp(0, 65535) as u16))
}

/// Adds a party member following Sora `handle`: 0 Donald, 1 Goofy (at most two). Returns the
/// member's slot (0 or 1), or -1. The first call loads the party data from the disc.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_party_add(handle: i32, member: i32) -> i32 {
    with(-1, |l| {
        if l.party.is_none() {
            match l.disc.party() {
                Ok(p) => l.party = Some(Arc::new(p)),
                Err(e) => return set_error(e),
            }
        }
        let Some(p) = l.party.clone() else { return -1 };
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref) else { return set_error(format!("no Sora with handle {handle}")) };
        if !(0..p.members.len() as i32).contains(&member) {
            return set_error("member is 0 (Donald) or 1 (Goofy)");
        }
        let at = s.motor.pos - s.motor.forward() * 150.0 + Vec3::new(if member == 0 { -100.0 } else { 100.0 }, 0.0, 0.0);
        let list = l.followers.entry(handle).or_default();
        if list.len() >= 2 {
            return set_error("two members already follow");
        }
        list.push(crate::party::Follower::new(&p, member as usize, at));
        list.len() as i32 - 1
    })
}

/// One field tick of Sora `handle`'s followers (call after `kh2_tick`).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_party_tick(handle: i32) -> i32 {
    with(-1, |l| {
        let (Some(p), Some(s)) = (l.party.clone(), usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref)) else { return -1 };
        let (pos, yaw, fall, pose) = (s.motor.pos, s.motor.yaw, s.game.params.sstm.fall_max, s.pose);
        if let Some(list) = l.followers.get_mut(&handle) {
            crate::party::tick(&p, list, pos, yaw, &l.world, fall, pose);
        }
        0
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2MemberState {
    pub pos: [f32; 3],
    pub yaw: f32,
    pub member: i32,
    pub motion_slot: i32,
    pub frame: f32,
}

/// The state of follower `slot` of Sora `handle`.
///
/// # Safety
/// `out` points to a writable `kh2_member_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_party_state(handle: i32, slot: i32, out: *mut Kh2MemberState) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let Some(f) = l.followers.get(&handle).and_then(|v| v.get(usize::try_from(slot).ok()?)) else { return -1 };
        unsafe { *out = Kh2MemberState { pos: f.motor.pos.to_array(), yaw: f.motor.yaw, member: f.member as i32, motion_slot: f.body.slot.map_or(-1, |x| x as i32), frame: f.body.frame } };
        0
    })
}

/// The follower's drawn pose (as `kh2_get_bones` for the body: `space` 0 model, 1 world).
///
/// # Safety
/// `out` points to `16 * max` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_party_bones(handle: i32, slot: i32, space: i32, out: *mut f32, max: u32) -> i32 {
    with(-1, |l| {
        let Some(f) = l.followers.get(&handle).and_then(|v| v.get(usize::try_from(slot).ok()?)) else { return -1 };
        let root = if space == 1 { Mat4::from_rotation_translation(glam::Quat::from_rotation_y(f.motor.yaw), f.motor.pos) } else { Mat4::IDENTITY };
        unsafe { write_mats(f.body.pose.iter().map(|m| root * *m), out, max) }
    })
}
