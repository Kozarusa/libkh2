//! The C ABI (`include/kh2.h`). One library state per process, as libsm64 has: the disc data, the
//! collision world and the Sora instances live behind a mutex; every call locks it.
//!
//! Safety (round 6 audit, docs/SAFETY.md): every `unsafe` block here is an output record write after a
//! NULL check, `write_mats` / `write_str` (NULL-checked, bounded by the caller's count), a `from_raw_parts`
//! over a host buffer (NULL / 0 empty, counts capped), `CStr::from_ptr` after a NULL check, or a bounded
//! `copy_nonoverlapping`; the records are `#[repr(C)]` and match kh2.h (tests/layout.rs).

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
    party_rng: std::collections::HashMap<i32, crate::party::PartyRng>,
    limit_data: Option<Arc<crate::limit::LimitData>>,
    auto_shown: std::collections::HashMap<i32, u16>,
    summon_data: Option<Arc<crate::summon::SummonData>>,
    area_runs: std::collections::HashMap<i32, (crate::area::AreaRun, Vec<(i32, i32)>)>,
    area_objects: Vec<crate::area::AreaObject>,
    /// The texture table of `kh2_get_mesh`: (rig, texture index of its model), by global id.
    textures: Vec<(Arc<crate::anim::Rig>, usize)>,
    /// The last mesh `kh2_mesh_prepare` skinned.
    mesh: crate::mesh::Skinned,
    /// The field HUD of each Sora (`kh2_hud_load`) and its last draw list.
    huds: std::collections::HashMap<i32, (crate::hud::Hud, Vec<crate::hud::Sprite>)>,
    /// Moving collision objects (`kh2_surface_object_*`) and the static surfaces they join.
    objects: Vec<crate::surface_object::Object>,
    /// The audio engine (`kh2_audio_init`).
    audio: Option<crate::audio::Audio>,
    base_surfaces: Option<Vec<Surface>>,
    next_object: u32,
    summons: std::collections::HashMap<i32, crate::summon::Summons>,
    limits: std::collections::HashMap<i32, crate::limit::Limits>,
    enemy_data: Option<Arc<crate::enemy::EnemyData>>,
    enemies: std::collections::HashMap<i32, crate::enemy::Enemies>,
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

/// `kh2_init`: opens the disc (NULL: the `KH2_ISO` environment variable; the Final Mix disc only) and loads Sora. 0 on
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
    *lib() = Some(Lib { game, disc, world: world_from_surfaces(&[]), soras: Vec::new(), cameras: Vec::new(), forms: Default::default(), party: None, followers: Default::default(), party_rng: Default::default(), limit_data: None, auto_shown: Default::default(), summon_data: None, area_runs: Default::default(), area_objects: Vec::new(), audio: None, textures: Vec::new(), mesh: Default::default(), huds: Default::default(), objects: Vec::new(), base_surfaces: None, next_object: 1, summons: Default::default(), limits: Default::default(), enemy_data: None, enemies: Default::default() });
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
    if !tris.iter().all(|t| t.v.iter().all(|&v| in_world(v))) {
        return set_error("a vertex is not finite or lies beyond 1e7 units");
    }
    with(-1, |l| {
        l.world = world_from_surfaces(tris);
        l.base_surfaces = None;
        rebuild_objects(l);
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
            l.base_surfaces = None;
            rebuild_objects(l);
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

/// decided: places the library accepts: finite and within 1e7 units (100 km) of the origin. The
/// collision grid and the movers are sized by positions; a stray huge one must not reach them.
const WORLD_LIMIT: f32 = 1.0e7;

fn in_world(p: [f32; 3]) -> bool {
    p.iter().all(|c| c.is_finite() && c.abs() <= WORLD_LIMIT)
}

/// A stick or look value: NaN and infinities become 0, the rest is held to -1..1 (stick) or kept.
fn finite_or_zero(v: f32) -> f32 {
    if v.is_finite() { v } else { 0.0 }
}

/// Creates a Sora standing at (x, y, z). Returns its handle, or -1.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_create(x: f32, y: f32, z: f32) -> i32 {
    if !in_world([x, y, z]) {
        return set_error("the position is not finite or lies beyond 1e7 units");
    }
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
        stick: [finite_or_zero(i.stick_x).clamp(-1.0, 1.0), finite_or_zero(i.stick_y).clamp(-1.0, 1.0)],
        cam_look: [finite_or_zero(i.cam_look_x).clamp(-1.0e6, 1.0e6), finite_or_zero(i.cam_look_z).clamp(-1.0e6, 1.0e6)],
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
    // A body on a moving object rides with it (`surface_object::ride`), before the tick.
    with((), |l| {
        if let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut)
            && let Some((at, yaw)) = crate::surface_object::ride(&l.objects, s.motor.pos, s.motor.grounded)
        {
            s.motor.pos = at;
            s.motor.yaw = kh2_game_core::motor::fix_radian(s.motor.yaw + yaw);
        }
        for o in &mut l.objects {
            o.before = o.now;
        }
    });
    sora_do(handle, -1, |s, w| {
        cap(&mut s.events);
        cap(&mut s.cues);
        s.tick(w, &input);
        s.collect_cues();
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
    if !in_world([x, y, z]) || !yaw.is_finite() {
        return set_error("the position is not finite or lies beyond 1e7 units");
    }
    let yaw = kh2_game_core::motor::wrap_angle(yaw % std::f32::consts::TAU);
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
    if !in_world([x, y, z]) || !in_world([radius, half_height, 0.0]) {
        return set_error("the target is not finite or lies beyond 1e7 units");
    }
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
        // decided: maxima held to 9999 (the save record's range; the game's stay far below).
        s.max_hp = max_hp.clamp(1, 9999);
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
        s.growth = crate::sora::Growth { high_jump: high_jump.min(5), quick_run: quick_run.min(5), aerial_dodge: aerial_dodge.min(5), glide: glide.min(5), dodge_roll: s.growth.dodge_roll };
        0
    })
}

/// Final Mix's Dodge Roll level (0..5): kh2fm-rs `square_ground` / `dash_step`.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_dodge_roll(handle: i32, level: u8) -> i32 {
    sora_do(handle, -1, |s, _| {
        s.growth.dodge_roll = level.min(5);
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
        // Only for a live Sora (a stray handle must not size the camera table).
        let Some(k) = usize::try_from(handle).ok().filter(|&k| l.soras.get(k).is_some_and(Option::is_some)) else { return set_error(format!("no Sora with handle {handle}")) };
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
        s.max_mp = max_mp.clamp(0, 9999);
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
            E::Bonus { id, hp, mp, drive, items } => Kh2Event { kind: 11, slot: id.into(), amount: hp, hp: mp, charge: drive, sub_id: items[0].into(), atkp: items[1].into(), ..d },
            E::LevelUp { level, strength, magic, defense, ap, ability } => Kh2Event { kind: 10, amount: level, sub_id: ability.into(), atkp: strength, stage: magic, slot: defense, charge: ap, ..d },
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
    let lv: Option<[u8; 7]> = (!levels.is_null()).then(|| unsafe { std::ptr::read(levels as *const [u8; 7]) });
    sora_do(handle, -1, |s, _| {
        let d = &mut s.drive;
        d.points.max = max_points;
        d.points.whole = points.min(max_points);
        d.points.fraction = 0;
        d.owned = std::array::from_fn(|i| owned_mask >> i & 1 != 0 && i != crate::drive::IDS.anti() as usize);
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
        Some(d) => kh2_game_core::drive_rules::outgoing_damage(&d.tuning, crate::drive::IDS, if s.drive.in_form { s.drive.form } else { 0 }, atk_type as u8, damage),
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
        cap_queues(l, handle);
        let (Some(p), Some(s)) = (l.party.clone(), usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut)) else { return -1 };
        let away = l.summons.get(&handle).is_some_and(|m| m.companions_away);
        if let Some(list) = l.followers.get_mut(&handle).filter(|_| !away) {
            // With spawned enemies the members fight (the battle part of `party.rs`).
            let mut en = l.enemies.get_mut(&handle);
            let before = en.as_ref().map_or(0, |e| e.events.len());
            crate::party::tick_battle(&p, list, l.party_rng.entry(handle).or_default(), s, &l.world, en.as_deref_mut());
            if let (Some(e), Some(lim)) = (en, l.limits.get_mut(&handle)) {
                for _ in e.events[before.min(e.events.len())..].iter().filter(|x| matches!(x, crate::enemy::EnemyEvent::Hit { .. })) {
                    lim.count_hit();
                }
            }
        }
        // The members' levels follow the shared experience (kh2-rs `pay_experience`: every slot).
        if let (Some(list), Some(p), Some(s)) = (l.followers.get_mut(&handle), l.party.as_ref(), usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref)) {
            crate::party::raise_levels(p, list, s.exp);
        }
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) else { return -1 };
        // The summon (kh2-rs `summon::tick`): its arrival, timer and object.
        if let (Some(sm), Some(d)) = (l.summons.get_mut(&handle), l.summon_data.clone()) {
            let battle = l.enemies.get(&handle).is_some_and(|e| e.list.iter().any(|x| x.alive()));
            sm.tick(&d, &mut l.disc, s, &l.world, battle, l.enemies.get_mut(&handle));
        }
        // The limit task's countdown and script (after the party).
        if let Some(lim) = l.limits.get_mut(&handle) {
            let had = lim.events.len();
            lim.tick(s);
            lim.kh1_tick(s);
            for ev in &lim.events[had.min(lim.events.len())..] {
                if let crate::limit::LimitEvent::Effect { who, id } = ev {
                    s.cue(crate::cue::Cue { kind: crate::cue::Kind::ScriptEffect, owner: crate::cue::Owner::Sora, model: "limit".to_string(), params: [*id as i16, *who as u16 as i16, 0, 0], pos: s.motor.pos, frame: 0 });
                }
            }
            if let (Some(list), Some(p)) = (l.followers.get_mut(&handle), l.party.as_ref()) {
                lim.play_motions(s, list, p);
            }
        }
        // The members' and the summon's sound and effect cues (`crate::cue`).
        if let (Some(list), Some(p)) = (l.followers.get(&handle), l.party.as_ref()) {
            for (k, f) in list.iter().enumerate() {
                let model = p.members.get(f.member).map_or("", |m| m.model.as_str());
                s.cues_extend(crate::cue::of_anim(&f.body, crate::cue::Owner::Member(k as u32), model, f.motor.pos));
            }
        }
        if let Some(o) = l.summons.get(&handle).and_then(|m| m.object.as_ref()) {
            s.cues_extend(crate::cue::of_anim(&o.body, crate::cue::Owner::Summon(0), &o.model, o.pos));
        }
        cap(&mut s.cues);
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
    pub hp: i32,
    pub max_hp: i32,
    pub mp: i32,
    pub max_mp: i32,
    /// 1 while knocked out.
    pub ko: i32,
    pub level: i32,
    pub style: i32,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
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
        let (strength, magic, defense) = l.party.as_ref().map_or((0, 0, 0), |p| p.members[f.member].status(f.level));
        unsafe {
            *out = Kh2MemberState {
                pos: f.motor.pos.to_array(),
                yaw: f.motor.yaw,
                member: f.member as i32,
                motion_slot: f.body.slot.map_or(-1, |x| x as i32),
                frame: f.body.frame,
                hp: f.hp,
                max_hp: f.hp_max,
                mp: f.mp,
                max_mp: f.mp_max,
                ko: f.ko as i32,
                level: i32::from(f.level),
                style: i32::from(f.style),
                strength,
                magic,
                defense,
            }
        };
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

/// Spawns an enemy of kind `kind` (its model name, e.g. "M_EX020" Shadow, "M_EX010" Soldier,
/// "M_EX990" Dusk, or kh2-rs's kind name) at (x, y, z) facing `yaw`, fighting Sora `handle`.
/// It runs under its own BDX script. Returns the enemy id (> 0), or -1. The first call loads the
/// enemy data from the disc.
///
/// # Safety
/// `kind` is a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_enemy_spawn(handle: i32, kind: *const c_char, x: f32, y: f32, z: f32, yaw: f32) -> i32 {
    if kind.is_null() {
        return set_error("no enemy kind");
    }
    let Ok(kind) = unsafe { CStr::from_ptr(kind) }.to_str().map(str::to_string) else { return set_error("kind is not UTF-8") };
    if !in_world([x, y, z]) || !yaw.is_finite() {
        return set_error("the position is not finite or lies beyond 1e7 units");
    }
    let yaw = yaw % std::f32::consts::TAU;
    with(-1, |l| {
        if l.enemy_data.is_none() {
            match l.disc.enemies() {
                Ok(d) => l.enemy_data = Some(Arc::new(d)),
                Err(e) => return set_error(e),
            }
        }
        let Some(data) = l.enemy_data.clone() else { return -1 };
        if usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref).is_none() {
            return set_error(format!("no Sora with handle {handle}"));
        }
        let en = l.enemies.entry(handle).or_insert_with(|| crate::enemy::Enemies::new(data));
        match en.spawn(&kind, Vec3::new(x, y, z), yaw, &l.world) {
            Some(id) => id as i32,
            None => set_error(format!("no enemy kind {kind:?}")),
        }
    })
}

/// One tick of Sora `handle`'s enemies (call after `kh2_tick`): their scripts, movement, hit
/// reactions, and the hit tests both ways (Sora's swings hurt them, theirs reach `kh2_hit_taken`'s
/// path), defeats paying EXP to Sora.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_enemies_tick(handle: i32) -> i32 {
    with(-1, |l| {
        cap_queues(l, handle);
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) else { return -1 };
        let fall = s.game.params.sstm.fall_max;
        if let Some(en) = l.enemies.get_mut(&handle) {
            let before = en.events.len();
            en.tick(s, &l.world, fall);
            // Collision groups the objects' scripts switched (hosts 1.36 / 1.37, 1.46 / 1.47 with the area mask).
            let edits = std::mem::take(&mut *kh2_enemy_host::shared::enemy::GROUP_EDITS.lock().unwrap_or_else(|e| e.into_inner()));
            for (g, on) in edits {
                l.world.set_group(g as u32, on);
                s.cue(crate::cue::Cue { kind: crate::cue::Kind::CollisionGroup, owner: crate::cue::Owner::Sora, model: String::new(), params: [g as i16, on as i16, 0, 0], pos: s.motor.pos, frame: 0 });
            }
            // The enemies' sound and effect cues (`crate::cue`), their scripts' effect starts.
            for e in en.list.iter().filter(|e| !e.removed) {
                s.cues_extend(crate::cue::of_anim(&e.anim, crate::cue::Owner::Enemy(e.id), e.data.kind.model, e.pos()));
            }
            // Hit cues (the hit sounds): Sora's hits with his keyblade's bank, the enemies' with their own.
            let weapon = if s.drive.in_form { s.drive.files.as_ref().and_then(|f| f.hands[0].as_ref().map(|h| h.1.clone())) } else { None }.unwrap_or_else(|| crate::data::KEYBLADE_MODEL.to_string());
            let mut hits = Vec::new();
            for ev in &en.events[before.min(en.events.len())..] {
                match ev {
                    crate::enemy::EnemyEvent::Hit { id, atkp, .. } => {
                        if let (Some(row), Some(e)) = (s.game.params.atkp(*atkp), en.list.iter().find(|e| e.id == *id)) {
                            hits.push(crate::cue::Cue { kind: crate::cue::Kind::Hit, owner: crate::cue::Owner::Sora, model: weapon.clone(), params: [row.enemy_reaction as i16, *atkp, 0, 0], pos: e.pos(), frame: 0 });
                        }
                    }
                    crate::enemy::EnemyEvent::HitSora { id, atkp, .. } => {
                        if let (Some(row), Some(e)) = (s.game.params.atkp(*atkp), en.list.iter().find(|e| e.id == *id)) {
                            hits.push(crate::cue::Cue { kind: crate::cue::Kind::Hit, owner: crate::cue::Owner::Enemy(*id), model: e.data.kind.model.to_string(), params: [row.enemy_reaction as i16, *atkp, 0, 0], pos: s.motor.pos, frame: 0 });
                        }
                    }
                    _ => {}
                }
            }
            s.cues_extend(hits);
            for ev in &en.events[before.min(en.events.len())..] {
                if let crate::enemy::EnemyEvent::Effect { id, effect } = ev
                    && let Some(e) = en.list.iter().find(|e| e.id == *id)
                {
                    s.cue(crate::cue::Cue { kind: crate::cue::Kind::ScriptEffect, owner: crate::cue::Owner::Enemy(*id), model: e.data.kind.model.to_string(), params: [*effect as i16, 0, 0, 0], pos: e.pos(), frame: 0 });
                }
            }
            cap(&mut s.cues);
            // The area's own groups and waves (kh2-rs `spawn_waves`).
            if let Some((run, raised)) = l.area_runs.get_mut(&handle) {
                // The mission manager a group started (kh2-rs struggle.rs: orbs, clock, script, end).
                let had = run.mission.as_ref().map_or(0, |m| m.sounds.len());
                run.tick_mission(s, en, &l.world);
                if let Some(m) = run.mission.as_ref() {
                    for (se, at) in m.sounds.iter().skip(had.min(m.sounds.len())) {
                        s.cue(crate::cue::Cue { kind: crate::cue::Kind::SoundNumber, owner: crate::cue::Owner::Sora, model: String::new(), params: [*se as u16 as i16, (*se >> 16) as u16 as i16, 0, 0], pos: at.unwrap_or(s.motor.pos), frame: 0 });
                    }
                }
                let r = std::mem::take(raised);
                run.tick(en, &l.world, s.motor.pos, &r);
            }
            // Sora's effective hits count for a running limit (FUN_001b3dc8).
            if let Some(lim) = l.limits.get_mut(&handle) {
                for _ in en.events[before.min(en.events.len())..].iter().filter(|x| matches!(x, crate::enemy::EnemyEvent::Hit { .. })) {
                    lim.count_hit();
                }
            }
            // Auto Limit (FUN_001d25e0): with nothing else offered, a partner's limit in slot 1.
            if en.offer.is_none()
                && !en.list.is_empty()
                && let (Some(list), Some(p), Some(d)) = (l.followers.get(&handle), l.party.as_ref(), l.limit_data.clone())
            {
                let shown = l.auto_shown.get(&handle).copied();
                let lim = l.limits.entry(handle).or_default();
                if let Some((cmd, mp_ok)) = lim.auto_offer(&d, s, list, p, shown) {
                    let message = en.data.cmds.iter().find(|c| c.id == cmd).map_or(0, |c| c.message_id as u32);
                    en.offer = Some(crate::enemy::ReactionOffer { cmd, pos: 0, from: 0, status: if mp_ok { crate::enemy::Status::Available } else { crate::enemy::Status::NoMp }, message });
                    l.auto_shown.insert(handle, cmd);
                } else {
                    l.auto_shown.remove(&handle);
                }
            }
            en.list.len() as i32
        } else {
            0
        }
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2EnemyState {
    pub id: i32,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub hp: i32,
    pub max_hp: i32,
    /// 0 idle (the script runs it), 1 hurt, 2 blown.
    pub state: i32,
    pub motion_slot: i32,
    pub frame: f32,
    pub grounded: i32,
    /// The draw alpha the script asks for (fades), 0..1.
    pub alpha: f32,
    pub enmp: i32,
}

/// The number of enemies of Sora `handle`.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_enemy_count(handle: i32) -> i32 {
    with(-1, |l| l.enemies.get(&handle).map_or(0, |e| e.list.len() as i32))
}

/// The state of the `index`-th enemy (0 .. kh2_enemy_count).
///
/// # Safety
/// `out` points to a writable `kh2_enemy_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_enemy_state(handle: i32, index: i32, out: *mut Kh2EnemyState) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let Some(e) = l.enemies.get(&handle).and_then(|en| en.list.get(usize::try_from(index).ok()?)) else { return -1 };
        let state = match e.state {
            kh2_enemy_host::shared::enemy::EState::Idle => 0,
            kh2_enemy_host::shared::enemy::EState::Hurt => 1,
            kh2_enemy_host::shared::enemy::EState::Blown => 2,
        };
        unsafe {
            *out = Kh2EnemyState {
                id: e.id as i32,
                pos: e.pos().to_array(),
                yaw: e.obj.motor.yaw,
                hp: e.hp,
                max_hp: e.stats.max_hp,
                state,
                motion_slot: e.anim.slot.map_or(-1, |x| x as i32),
                frame: e.anim.frame,
                grounded: e.obj.motor.grounded as i32,
                alpha: e.obj.shown_alpha(),
                enmp: i32::from(e.enmp),
            }
        };
        0
    })
}

/// The drawn pose of the `index`-th enemy (space 0 model, 1 world), 16 floats per bone.
///
/// # Safety
/// `out` points to `16 * max` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_enemy_bones(handle: i32, index: i32, space: i32, out: *mut f32, max: u32) -> i32 {
    with(-1, |l| {
        let Some(e) = l.enemies.get(&handle).and_then(|en| en.list.get(usize::try_from(index).ok()?)) else { return -1 };
        let root = if space == 1 { e.matrix() } else { Mat4::IDENTITY };
        unsafe { write_mats(e.anim.pose.iter().map(|m| root * *m), out, max) }
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2EnemyEvent {
    /// 1 Sora hit an enemy, 2 an enemy hit Sora, 3 defeated, 4 effect, 5 prize drop, 6 removed,
    /// 7 reaction command delivered (`atkp` = command id, `value` = 1 when the script handled it).
    pub kind: i32,
    pub id: i32,
    pub atkp: i32,
    pub damage: i32,
    /// Hit: HP left; hit Sora: `kh2_hit_taken`'s result code; defeated: EXP paid; effect: element.
    pub value: i32,
    pub enmp: i32,
    pub lvpm_experience: i32,
    pub enmp_experience: i32,
    /// Drop: the prize counts per kind (HP, MP, munny, drive, ...).
    pub prizes: [u32; 9],
}

/// Takes the oldest enemy event: 1 written, 0 none left.
///
/// # Safety
/// `out` points to a writable `kh2_enemy_event`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_poll_enemy_event(handle: i32, out: *mut Kh2EnemyEvent) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let Some(en) = l.enemies.get_mut(&handle) else { return 0 };
        if en.events.is_empty() {
            return 0;
        }
        use crate::enemy::EnemyEvent as E;
        let v = match en.events.remove(0) {
            E::Hit { id, atkp, damage, hp } => Kh2EnemyEvent { kind: 1, id: id as i32, atkp: atkp.into(), damage, value: hp, ..Default::default() },
            E::HitSora { id, atkp, damage, result } => Kh2EnemyEvent { kind: 2, id: id as i32, atkp: atkp.into(), damage, value: result, ..Default::default() },
            E::Defeated { id, enmp, lvpm_experience, enmp_experience, paid } => {
                Kh2EnemyEvent { kind: 3, id: id as i32, enmp: enmp.into(), lvpm_experience: lvpm_experience.into(), enmp_experience: enmp_experience.into(), value: paid, ..Default::default() }
            }
            E::Effect { id, effect } => Kh2EnemyEvent { kind: 4, id: id as i32, value: effect.into(), ..Default::default() },
            E::Drop { id, row, prizes, .. } => Kh2EnemyEvent { kind: 5, id: id as i32, value: row.into(), prizes, ..Default::default() },
            E::Removed { id } => Kh2EnemyEvent { kind: 6, id: id as i32, ..Default::default() },
            E::Reaction { id, cmd, handled } => Kh2EnemyEvent { kind: 7, id: id as i32, atkp: cmd.into(), value: handled as i32, ..Default::default() },
        };
        unsafe { *out = v };
        1
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2ReactionOffer {
    /// The `cmd` row id offered in slot 1.
    pub cmd: i32,
    /// The offering enemy's id.
    pub from: i32,
    /// 0 available, 1 blocked (flags/lock/form), 2 too far, 3 not enough MP, 4 not ported.
    pub status: i32,
    /// The command's message id (the prompt's text).
    pub message: u32,
}

/// The reaction command offered to Sora `handle` this tick (kh2-rs `reaction_cmd`): 1 written,
/// 0 none offered.
///
/// # Safety
/// `out` points to a writable `kh2_reaction_offer`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_reaction_offer(handle: i32, out: *mut Kh2ReactionOffer) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let Some(o) = l.enemies.get(&handle).and_then(|e| e.offer) else { return 0 };
        use crate::enemy::Status as S;
        let status = match o.status {
            S::Available => 0,
            S::Blocked => 1,
            S::TooFar => 2,
            S::NoMp => 3,
            S::NotPorted => 4,
        };
        unsafe { *out = Kh2ReactionOffer { cmd: o.cmd.into(), from: o.from as i32, status, message: o.message } };
        1
    })
}

/// Accepts the offered reaction command (the Triangle press): returns the command id, or -1 when
/// nothing available is offered. The target's script receives it on the next `kh2_enemies_tick`.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_reaction_accept(handle: i32) -> i32 {
    with(-1, |l| reaction_accept(l, handle))
}

/// The body of `kh2_reaction_accept` (also the HUD's reaction choice).
fn reaction_accept(l: &mut Lib, handle: i32) -> i32 {
    {
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) else { return -1 };
        let Some(en) = l.enemies.get_mut(&handle) else { return -1 };
        // An Auto Limit offer: the limit starts (FUN_001cf030 case 0x51).
        if let (Some(o), Some(d)) = (en.offer, l.limit_data.clone())
            && o.from == 0
            && d.rows.iter().any(|r| r.command == o.cmd)
        {
            if o.status != crate::enemy::Status::Available {
                return -1;
            }
            let present = match (l.followers.get(&handle), l.party.as_ref()) {
                (Some(f), Some(p)) => crate::limit::present(f, p),
                _ => Vec::new(),
            };
            return match l.limits.entry(handle).or_default().start(&d, &mut l.disc, s, &present, o.cmd) {
                Ok(()) => i32::from(o.cmd),
                Err(_) => -1,
            };
        }
        en.accept(s).map_or(-1, i32::from)
    }
}

/// Test harness (kh2-rs `FakeOffer`): every living enemy of Sora `handle` offers `cmd` (0 stops).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_reaction_force_offer(handle: i32, cmd: i32) -> i32 {
    with(-1, |l| {
        let Some(en) = l.enemies.get_mut(&handle) else { return -1 };
        en.fake_offer = u16::try_from(cmd).ok().filter(|&c| c != 0);
        0
    })
}

/// The weapon chosen at the start (0 Sword, 1 Shield, 2 Staff): which `lvup` ability column the
/// level-ups give. Default 0.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_set_weapon_set(handle: i32, set: i32) -> i32 {
    sora_do(handle, -1, |s, _| match u8::try_from(set) {
        Ok(v) if v <= 2 => {
            s.weapon_set = v;
            0
        }
        _ => set_error("weapon set is 0, 1 or 2"),
    })
}

/// The abilities the level-ups gave (ability item ids, oldest first): writes up to `max`, returns
/// the total count. Equip them with `kh2_set_loadout`.
///
/// # Safety
/// `out` is NULL or points to `max` u16 values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_granted_abilities(handle: i32, out: *mut u16, max: u32) -> i32 {
    sora_do(handle, -1, |s, _| {
        if !out.is_null() {
            for (k, &a) in s.granted.iter().take(max as usize).enumerate() {
                unsafe { *out.add(k) = a };
            }
        }
        s.granted.len() as i32
    })
}

/// Sora's save record (level, EXP, HP/MP, AP, spells, weapon set, abilities, drive, place; not the
/// motion or the enemies). Writes it when `cap` is large enough; returns its size in bytes.
///
/// # Safety
/// `out` is NULL or points to `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_state_save(handle: i32, out: *mut u8, cap: u32) -> i32 {
    sora_do(handle, -1, |s, _| {
        let b = s.save();
        if !out.is_null() && b.len() <= cap as usize {
            unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), out, b.len()) };
        }
        b.len() as i32
    })
}

/// Applies a save record from `kh2_state_save`: 0, or -1 (nothing changed) when it is not a valid one.
///
/// # Safety
/// `bytes` points to `len` readable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_state_load(handle: i32, bytes: *const u8, len: u32) -> i32 {
    if bytes.is_null() || len > 1 << 20 {
        return set_error("no save record");
    }
    let b = unsafe { std::slice::from_raw_parts(bytes, len as usize) }.to_vec();
    sora_do(handle, -1, |s, _| match s.load(&b) {
        Ok(()) => 0,
        Err(e) => set_error(e),
    })
}

fn member_do<R>(handle: i32, slot: i32, fallback: R, f: impl FnOnce(&mut crate::party::Follower, &crate::party::PartyData) -> R) -> R {
    let mut f = Some(f);
    let r = with(None, |l| {
        let p = l.party.clone()?;
        let m = l.followers.get_mut(&handle)?.get_mut(usize::try_from(slot).ok()?)?;
        Some((f.take().unwrap())(m, &p))
    });
    match r {
        Some(v) => v,
        None => {
            set_error(format!("no party member {slot} of Sora {handle}"));
            fallback
        }
    }
}

/// A member's level (1..99): its status is the `lvup` row of the level plus the new game's bytes.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_party_set_level(handle: i32, slot: i32, level: i32) -> i32 {
    member_do(handle, slot, -1, |m, _| {
        m.level = level.clamp(1, 99) as u8;
        0
    })
}

/// A member's HP / MP and maxima (maxima 1..9999 / 0..9999). HP 0 knocks the member out.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_party_set_vitals(handle: i32, slot: i32, hp: i32, max_hp: i32, mp: i32, max_mp: i32) -> i32 {
    member_do(handle, slot, -1, |m, p| {
        m.hp_max = max_hp.clamp(1, 9999);
        m.mp_max = max_mp.clamp(0, 9999);
        m.mp = mp.clamp(0, m.mp_max);
        let want = hp.clamp(0, m.hp_max);
        if m.ko && want > 0 {
            m.change_hp(want, p);
        } else if !m.ko {
            m.change_hp(want - m.hp, p);
        }
        0
    })
}

/// A member's battle style (setting byte 0; 0..3: the target choice and battle distance of the style).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_party_set_style(handle: i32, slot: i32, style: i32) -> i32 {
    member_do(handle, slot, -1, |m, _| match u8::try_from(style) {
        Ok(s) if s <= 3 => {
            m.style = s;
            0
        }
        _ => set_error("style is 0..3"),
    })
}

/// A member's battle item stock (`count` item ids and their counts): its HP / MP item handlers use them.
///
/// # Safety
/// `ids` and `counts` point to `count` values each (or `count` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_party_set_items(handle: i32, slot: i32, ids: *const u16, counts: *const u8, count: u32) -> i32 {
    let n = count.min(64) as usize;
    let v: Vec<(u16, u8)> = if n == 0 || ids.is_null() || counts.is_null() { Vec::new() } else { unsafe { std::slice::from_raw_parts(ids, n).iter().copied().zip(std::slice::from_raw_parts(counts, n).iter().copied()).collect() } };
    member_do(handle, slot, -1, |m, _| {
        m.items = v;
        0
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2MemberEvent {
    /// The member's slot.
    pub slot: i32,
    /// 1 hit by an enemy, 2 knocked out, 3 revived, 4 used an item.
    pub kind: i32,
    /// Hit: the enemy id; item: the item id.
    pub id: i32,
    /// Hit: atkp row; item: the target (0 Sora, 1 the member, 2 the other member).
    pub atkp: i32,
    pub damage: i32,
    /// HP after (hit, revived).
    pub hp: i32,
    /// Script mode: its name (NUL-terminated, cut).
    pub text: [u8; 32],
}

/// Takes the oldest member event of Sora `handle`'s party: 1 written, 0 none left.
///
/// # Safety
/// `out` points to a writable `kh2_member_event`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_poll_member_event(handle: i32, out: *mut Kh2MemberEvent) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let Some(list) = l.followers.get_mut(&handle) else { return 0 };
        for (slot, f) in list.iter_mut().enumerate() {
            if f.events.is_empty() {
                continue;
            }
            use crate::party::MemberEvent as E;
            let s = slot as i32;
            let v = match f.events.remove(0) {
                E::Hit { by, atkp, damage, hp } => Kh2MemberEvent { slot: s, kind: 1, id: by as i32, atkp: atkp.into(), damage, hp, ..Default::default() },
                E::KnockedOut => Kh2MemberEvent { slot: s, kind: 2, ..Default::default() },
                E::Revived { hp } => Kh2MemberEvent { slot: s, kind: 3, hp, ..Default::default() },
                E::ItemUsed { item, who } => Kh2MemberEvent { slot: s, kind: 4, id: item.into(), atkp: who.into(), ..Default::default() },
                E::LevelUp { level, ability } => Kh2MemberEvent { slot: s, kind: 5, id: ability.into(), hp: level.into(), ..Default::default() },
                E::Bonus { id, hp, mp, items } => Kh2MemberEvent { slot: s, kind: 6, id: id.into(), damage: hp, hp: mp, atkp: items[0].into(), ..Default::default() },
                E::ScriptMode { mode } => {
                    let mut text = [0u8; 32];
                    for (k, b) in mode.bytes().take(31).enumerate() {
                        text[k] = b;
                    }
                    Kh2MemberEvent { slot: s, kind: 7, text, ..Default::default() }
                }
                E::Effect { id } => Kh2MemberEvent { slot: s, kind: 8, id: id.into(), ..Default::default() },
                E::Healed { who, amount } => Kh2MemberEvent { slot: s, kind: 9, atkp: who.into(), damage: amount, ..Default::default() },
            };
            unsafe { *out = v };
            return 1;
        }
        0
    })
}

fn limit_code(r: &kh2_enemy_host::limit::Refusal) -> i32 {
    use kh2_enemy_host::limit::Refusal as R;
    match r {
        R::WrongWorld => 1,
        R::Locked => 2,
        R::MemberAbsent(_) => 3,
        R::AbilityMissing(_) => 4,
        R::NotEnoughMp => 5,
        R::SummonGaugeEmpty => 6,
        R::Busy => 7,
    }
}

fn limit_ready(l: &mut Lib) -> Result<Arc<crate::limit::LimitData>, i32> {
    if l.limit_data.is_none() {
        match l.disc.limits() {
            Ok(d) => l.limit_data = Some(Arc::new(d)),
            Err(e) => return Err(set_error(e)),
        }
    }
    Ok(l.limit_data.clone().unwrap())
}

/// The limit commands and their status for Sora `handle` with the party that follows him: writes up to
/// `max` command ids and status codes (0 available, 1 wrong world, 2 locked, 3 a member is absent or
/// knocked out, 4 the limit ability is not learned, 5 not enough MP, 6 summon gauge, 7 a limit runs);
/// returns the number of commands.
///
/// # Safety
/// `cmds` and `status` are NULL or point to `max` values each.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_limit_status(handle: i32, cmds: *mut u16, status: *mut i32, max: u32) -> i32 {
    with(-1, |l| {
        let d = match limit_ready(l) {
            Ok(d) => d,
            Err(e) => return e,
        };
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref) else { return set_error(format!("no Sora with handle {handle}")) };
        let present = match (l.followers.get(&handle), l.party.as_ref()) {
            (Some(f), Some(p)) => crate::limit::present(f, p),
            _ => Vec::new(),
        };
        let st = l.limits.entry(handle).or_default().status(&d, s, &present);
        for (k, (c, r)) in st.iter().take(max as usize).enumerate() {
            unsafe {
                if !cmds.is_null() {
                    *cmds.add(k) = *c;
                }
                if !status.is_null() {
                    *status.add(k) = r.as_ref().map_or_else(limit_code, |_| 0);
                }
            }
        }
        st.len() as i32
    })
}

/// Starts limit command `cmd` (the accepted command): MP spent, the countdown and the limit's own
/// script start. Returns 0, or minus the status code of `kh2_limit_status`. A sub-command of the
/// running limit goes to its script (event 23).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_limit_start(handle: i32, cmd: i32) -> i32 {
    with(-1, |l| {
        let d = match limit_ready(l) {
            Ok(d) => d,
            Err(e) => return e,
        };
        let Ok(cmd) = u16::try_from(cmd) else { return set_error("no such command") };
        let present = match (l.followers.get(&handle), l.party.as_ref()) {
            (Some(f), Some(p)) => crate::limit::present(f, p),
            _ => Vec::new(),
        };
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) else { return set_error(format!("no Sora with handle {handle}")) };
        match l.limits.entry(handle).or_default().start(&d, &mut l.disc, s, &present, cmd) {
            Ok(()) => 0,
            Err(r) => {
                set_error(format!("limit refused: {r:?}"));
                -limit_code(&r)
            }
        }
    })
}

/// The world id the limit rows are tested against (0 by default: world-bound rows are refused).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_limit_set_world(handle: i32, world: i32) -> i32 {
    with(-1, |l| {
        l.limits.entry(handle).or_default().world = world.clamp(0, u16::MAX as i32) as u16;
        0
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2LimitState {
    /// The running command, 0 none.
    pub command: i32,
    pub ticks_left: i32,
    pub hits: i32,
}

/// # Safety
/// `out` points to a writable `kh2_limit_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_limit_state(handle: i32, out: *mut Kh2LimitState) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let st = l.limits.get(&handle).map_or(Kh2LimitState::default(), |m| Kh2LimitState { command: m.task.map_or(0, |t| i32::from(t.0)), ticks_left: m.task.map_or(0, |t| t.1 as i32), hits: m.hits() as i32 });
        unsafe { *out = st };
        0
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2LimitEvent {
    /// 1 started, 2 ended, 3 participant motion, 4 PAX element, 5 named event, 6 script stopped.
    pub kind: i32,
    /// Started / ended: the command; motion / effect: the participant handle.
    pub a: i32,
    /// Started: time; ended: hits; motion / effect: the id.
    pub b: i32,
    /// Motion: the blend.
    pub blend: f32,
    /// Started: the file; named: the event name; stopped: the reason (NUL-terminated, cut).
    pub text: [u8; 32],
}

/// Takes the oldest limit event: 1 written, 0 none left.
///
/// # Safety
/// `out` points to a writable `kh2_limit_event`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_poll_limit_event(handle: i32, out: *mut Kh2LimitEvent) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let Some(m) = l.limits.get_mut(&handle) else { return 0 };
        if m.events.is_empty() {
            return 0;
        }
        let text = |s: &str| {
            let mut t = [0u8; 32];
            for (k, b) in s.bytes().take(31).enumerate() {
                t[k] = b;
            }
            t
        };
        use crate::limit::LimitEvent as E;
        let v = match m.events.remove(0) {
            E::Started { command, file, time, .. } => Kh2LimitEvent { kind: 1, a: command.into(), b: time.into(), text: text(&file), ..Default::default() },
            E::Ended { command, hits } => Kh2LimitEvent { kind: 2, a: command.into(), b: hits as i32, ..Default::default() },
            E::Motion { who, id, blend } => Kh2LimitEvent { kind: 3, a: who as i32, b: id as i32, blend, ..Default::default() },
            E::Effect { who, id } => Kh2LimitEvent { kind: 4, a: who as i32, b: id.into(), ..Default::default() },
            E::Named(n) => Kh2LimitEvent { kind: 5, text: text(&n), ..Default::default() },
            E::ScriptStopped(w) => Kh2LimitEvent { kind: 6, text: text(&w), ..Default::default() },
        };
        unsafe { *out = v };
        1
    })
}

/// Gives bonus reward `id` (the `bons` rows, kh2-rs `Progress::give_bonus`): Sora's maximum HP / MP /
/// drive gauge and items, and the following members' maximum HP / MP. 1 given, 0 given before, -1.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_give_bonus(handle: i32, id: i32) -> i32 {
    with(-1, |l| {
        let Ok(id) = u16::try_from(id) else { return set_error("bonus id is 0..255") };
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) else { return set_error(format!("no Sora with handle {handle}")) };
        let Some(others) = s.give_bonus(id) else { return 0 };
        if let (Some(list), Some(p)) = (l.followers.get_mut(&handle), l.party.as_ref()) {
            for (who, hp, mp, items) in others {
                for f in list.iter_mut().filter(|f| p.members[f.member].neo == u16::from(who)) {
                    f.hp_max = (f.hp_max + hp).min(9999);
                    f.mp_max = (f.mp_max + mp).min(9999);
                    f.mp = (f.mp + mp).clamp(0, f.mp_max);
                    if !f.ko {
                        f.hp = (f.hp + hp).clamp(0, f.hp_max);
                    }
                    f.events.push(crate::party::MemberEvent::Bonus { id, hp, mp, items });
                }
            }
        }
        1
    })
}

/// A drive form's ability list (kh2-rs `drive::form_ability_list`: the `fmlv` grants up to the form's
/// level and its growth ability): writes up to `max` ability item ids, returns the count.
///
/// # Safety
/// `out` is NULL or points to `max` u16 values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_form_abilities(handle: i32, form: i32, out: *mut u16, max: u32) -> i32 {
    sora_do(handle, -1, |s, _| {
        let Ok(f) = u8::try_from(form) else { return set_error("form is 1..5") };
        if !(1..=5).contains(&f) {
            return set_error("form is 1..5");
        }
        let v = s.form_ability_list(f);
        if !out.is_null() {
            for (k, &a) in v.iter().take(max as usize).enumerate() {
                unsafe { *out.add(k) = a };
            }
        }
        v.len() as i32
    })
}

fn summon_ready(l: &mut Lib) -> Result<Arc<crate::summon::SummonData>, i32> {
    if l.summon_data.is_none() {
        match l.disc.summons() {
            Ok(d) => l.summon_data = Some(Arc::new(d)),
            Err(e) => return Err(set_error(e)),
        }
    }
    Ok(l.summon_data.clone().unwrap())
}

fn summon_code(r: crate::summon::SummonRefusal) -> i32 {
    use crate::summon::SummonRefusal as R;
    match r {
        R::NotASummon => 1,
        R::CharmNotOwned => 2,
        R::Busy => 3,
        R::PartyMissing => 4,
        R::InForm => 5,
        R::NotEnoughPoints => 6,
        R::Airborne => 7,
    }
}

/// The charms the player owns (`sumn` row items): the summons the menu lists.
///
/// # Safety
/// `items` points to `count` u16 values (or `count` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_summon_set_owned(handle: i32, items: *const u16, count: u32) -> i32 {
    let v: Vec<u16> = if items.is_null() || count == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(items, count.min(16) as usize) }.to_vec() };
    with(-1, |l| {
        if let Err(e) = summon_ready(l) {
            return e;
        }
        l.summons.entry(handle).or_default().owned = v;
        0
    })
}

fn party_ok(l: &Lib, handle: i32) -> bool {
    l.followers.get(&handle).is_some_and(|f| f.len() >= 2 && f.iter().all(|m| !m.ko))
}

/// The summon commands and their status (0 available, 1 not a summon, 2 charm not owned, 3 busy,
/// 4 Donald and Goofy not both in the party and standing, 5 in a form, 6 not enough drive points,
/// 7 airborne); returns the count.
///
/// # Safety
/// `cmds` and `status` are NULL or point to `max` values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_summon_status(handle: i32, cmds: *mut u16, status: *mut i32, max: u32) -> i32 {
    with(-1, |l| {
        let d = match summon_ready(l) {
            Ok(d) => d,
            Err(e) => return e,
        };
        let ok = party_ok(l, handle);
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref) else { return set_error(format!("no Sora with handle {handle}")) };
        let st = l.summons.entry(handle).or_default().status(&d, s, ok);
        for (k, (c, r)) in st.iter().take(max as usize).enumerate() {
            unsafe {
                if !cmds.is_null() {
                    *cmds.add(k) = *c;
                }
                if !status.is_null() {
                    *status.add(k) = r.map_or_else(summon_code, |_| 0);
                }
            }
        }
        st.len() as i32
    })
}

/// The Summon Effects option: 1 plays the arrival as an event (the script's `appear`, the longer wait).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_summon_set_arrival_event(handle: i32, on: i32) -> i32 {
    with(-1, |l| {
        l.summons.entry(handle).or_default().arrival_event = on != 0;
        0
    })
}

/// Summon command `cmd` (Execute 72): 0, or minus the status code. `KH2_SUMMON_DISMISS` (0x13c)
/// dismisses the summon that is out.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_summon(handle: i32, cmd: i32) -> i32 {
    with(-1, |l| {
        let d = match summon_ready(l) {
            Ok(d) => d,
            Err(e) => return e,
        };
        let ok = party_ok(l, handle);
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) else { return set_error(format!("no Sora with handle {handle}")) };
        let sm = l.summons.entry(handle).or_default();
        if cmd == i32::from(crate::summon::DISMISS) {
            return if sm.dismiss() { 0 } else { set_error("no summon to dismiss") };
        }
        let Ok(c) = u16::try_from(cmd) else { return -1 };
        match sm.summon(&d, s, ok, c) {
            Ok(()) => 0,
            Err(r) => {
                set_error(format!("summon refused: {r:?}"));
                -summon_code(r)
            }
        }
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2SummonState {
    /// 0 none, 1 arrival event, 2 out, 3 leaving.
    pub state: i32,
    /// 1-based `sumn` index, 0 none.
    pub index: i32,
    /// 1 while the arrival wait runs.
    pub arriving: i32,
    pub timer: f32,
    pub timer_total: f32,
    pub segments: i32,
    pub level: i32,
    pub exp: i32,
    /// The object: 1 when out, its place, yaw and motion slot.
    pub object: i32,
    pub pos: [f32; 3],
    pub yaw: f32,
    pub motion_slot: i32,
    /// Donald and Goofy are away while the summon runs.
    pub companions_away: i32,
}

/// # Safety
/// `out` points to a writable `kh2_summon_state`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_summon_state(handle: i32, out: *mut Kh2SummonState) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let v = l.summons.get(&handle).map_or(Kh2SummonState::default(), |m| Kh2SummonState {
            state: m.state.into(),
            index: m.index.into(),
            arriving: m.arrival.is_some() as i32,
            timer: m.timer.left,
            timer_total: m.timer.total,
            segments: m.segments.into(),
            level: m.exp.level.into(),
            exp: m.exp.exp,
            object: m.object.is_some() as i32,
            pos: m.object.as_ref().map_or([0.0; 3], |o| o.pos.to_array()),
            yaw: m.object.as_ref().map_or(0.0, |o| o.yaw),
            motion_slot: m.object.as_ref().and_then(|o| o.body.slot).map_or(-1, |x| x as i32),
            companions_away: m.companions_away as i32,
        });
        unsafe { *out = v };
        0
    })
}

/// The summon object's drawn pose (space 0 model, 1 world), 16 floats per bone.
///
/// # Safety
/// `out` points to `16 * max` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_summon_bones(handle: i32, space: i32, out: *mut f32, max: u32) -> i32 {
    with(-1, |l| {
        let Some(o) = l.summons.get(&handle).and_then(|m| m.object.as_ref()) else { return -1 };
        let root = if space == 1 { o.matrix() } else { Mat4::IDENTITY };
        unsafe { write_mats(o.body.pose.iter().map(|m| root * *m), out, max) }
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2SummonEvent {
    /// 1 started, 2 arrived, 3 script mode, 4 level up, 5 leaving, 6 gone.
    pub kind: i32,
    /// Started: command; arrived: segments; level up: the level; leaving: 1 dismissed.
    pub value: i32,
    /// Arrived: the timer.
    pub timer: f32,
    /// Arrived: the model; mode: its name.
    pub text: [u8; 32],
}

/// Takes the oldest summon event: 1 written, 0 none left.
///
/// # Safety
/// `out` points to a writable `kh2_summon_event`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_poll_summon_event(handle: i32, out: *mut Kh2SummonEvent) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let Some(m) = l.summons.get_mut(&handle) else { return 0 };
        if m.events.is_empty() {
            return 0;
        }
        let text = |s: &str| {
            let mut t = [0u8; 32];
            for (k, b) in s.bytes().take(31).enumerate() {
                t[k] = b;
            }
            t
        };
        use crate::summon::SummonEvent as E;
        let v = match m.events.remove(0) {
            E::Started { command, .. } => Kh2SummonEvent { kind: 1, value: command.into(), ..Default::default() },
            E::Arrived { model, timer, segments } => Kh2SummonEvent { kind: 2, value: segments.into(), timer, text: text(&model) },
            E::Mode(md) => Kh2SummonEvent { kind: 3, text: text(&md), ..Default::default() },
            E::LevelUp(lv) => Kh2SummonEvent { kind: 4, value: lv.into(), ..Default::default() },
            E::Leaving { dismissed } => Kh2SummonEvent { kind: 5, value: dismissed as i32, ..Default::default() },
            E::Gone => Kh2SummonEvent { kind: 6, ..Default::default() },
            E::Record { index } => Kh2SummonEvent { kind: 7, value: index as i32, ..Default::default() },
            E::Blast { id } => Kh2SummonEvent { kind: 8, value: id as i32, ..Default::default() },
            E::Refill => Kh2SummonEvent { kind: 9, ..Default::default() },
            E::Heal { hp } => Kh2SummonEvent { kind: 10, value: hp, ..Default::default() },
            E::Potion => Kh2SummonEvent { kind: 11, ..Default::default() },
        };
        unsafe { *out = v };
        1
    })
}

/// Spawns the enemies of area `area`'s battle program `program` (the groups its `btl` program loads,
/// from the area file's spawn records with their spawn kinds and parameters; -1: every program) for
/// Sora `handle`. Disabled records (waiting for a signal) are skipped unless `with_disabled`.
/// Returns the number spawned. Load the area's collision first (`kh2_surfaces_load_map`).
///
/// # Safety
/// `area` is a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_area_spawn(handle: i32, area: *const c_char, program: i32, with_disabled: i32) -> i32 {
    if area.is_null() {
        return set_error("no area name");
    }
    let Ok(area) = unsafe { CStr::from_ptr(area) }.to_str().map(str::to_string) else { return set_error("area name is not UTF-8") };
    with(-1, |l| {
        if l.enemy_data.is_none() {
            match l.disc.enemies() {
                Ok(d) => l.enemy_data = Some(Arc::new(d)),
                Err(e) => return set_error(e),
            }
        }
        if usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref).is_none() {
            return set_error(format!("no Sora with handle {handle}"));
        }
        let b = l.disc.area_battle(&area, u16::try_from(program).ok());
        let Some(data) = l.enemy_data.clone() else { return -1 };
        let en = l.enemies.entry(handle).or_insert_with(|| crate::enemy::Enemies::new(data));
        let mut n = 0;
        for e in b.enemies.iter().filter(|e| with_disabled != 0 || !e.disabled) {
            if en.list.len() >= 256 {
                break;
            }
            if en.spawn_with(&e.model, e.at, e.yaw, &l.world, e.spawn_kind, Some(e.spawn_params)).is_some() {
                n += 1;
            }
        }
        n
    })
}

/// Drops Sora `handle`'s enemies that are gone (defeated and faded, or fallen out; their indices
/// shift) and frees the enemy kinds no enemy uses any more (models, motions, scripts). Returns the
/// number of enemies dropped.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_enemies_prune(handle: i32) -> i32 {
    with(-1, |l| match l.enemies.get_mut(&handle) {
        Some(en) => en.prune().0 as i32,
        None => 0,
    })
}

/// Removes every enemy of Sora `handle` and frees the kinds no other Sora's enemies use.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_enemies_clear(handle: i32) -> i32 {
    with(-1, |l| {
        l.enemies.remove(&handle);
        l.enemy_data.as_ref().map_or(0, |d| d.release_unused() as i32)
    })
}

/// The mission counters the enemies' scripts read (host 4.29: slot 0 the player's orbs, slot 1 the
/// opponent's in the Struggle); `on` 0 removes the mission. The host keeps the counters.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_enemies_set_mission(handle: i32, on: i32, c0: i32, c1: i32, c2: i32) -> i32 {
    with(-1, |l| match l.enemies.get_mut(&handle) {
        Some(en) => {
            en.mission = (on != 0).then_some([c0, c1, c2]);
            0
        }
        None => set_error("no enemies for this Sora (spawn one first)"),
    })
}

/// # Safety
/// `ids` points to `count` u16 values (or is NULL).
unsafe fn ids_of(ids: *const u16, count: u32) -> Vec<u16> {
    if ids.is_null() || count == 0 { Vec::new() } else { unsafe { std::slice::from_raw_parts(ids, count.min(16) as usize) }.to_vec() }
}

/// Sora's equipment (weapon, armor, accessories; item ids): the status is rebuilt from the level row
/// plus the items' `item_stat` rows (attack, magic, defense, AP, resistances) and their abilities join
/// the loadout. Default: the Kingdom Key.
///
/// # Safety
/// `ids` points to `count` u16 values (or `count` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_set_equipment(handle: i32, ids: *const u16, count: u32) -> i32 {
    let v = unsafe { ids_of(ids, count) };
    sora_do(handle, -1, |s, _| {
        s.set_equipment(&v);
        0
    })
}

/// A party member's equipment (their `item_stat` rows add to the member's status).
///
/// # Safety
/// `ids` points to `count` u16 values (or `count` is 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_party_set_equipment(handle: i32, slot: i32, ids: *const u16, count: u32) -> i32 {
    let v = unsafe { ids_of(ids, count) };
    member_do(handle, slot, -1, |m, _| {
        m.equipment = v;
        0
    })
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2ItemInfo {
    pub id: u16,
    /// `item.Type` (2 keyblade, 3 staff, 4 shield, 19 ability ...).
    pub kind: u8,
    pub ap: u8,
    pub command: u16,
    pub name: u16,
    pub description: u16,
    /// `Flag1 | Rank << 8`.
    pub amount: u16,
    pub shop_buy: u16,
    pub shop_sell: u16,
    pub attack: u8,
    pub magic: u8,
    pub defense: u8,
    /// Physical, fire, blizzard, thunder, dark, light, general.
    pub resist: [u8; 7],
    pub ability: u16,
}

/// What item `id` is (the `item` row and its `item_stat` row): 0, or -1 for no such item.
///
/// # Safety
/// `out` points to a writable `kh2_item_info`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_item_info(id: i32, out: *mut Kh2ItemInfo) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let p = &l.game.params;
        let Some(i) = u16::try_from(id).ok().and_then(|id| crate::equipment::info(&p.items, &p.item_stats, id)) else { return set_error("no such item") };
        unsafe {
            *out = Kh2ItemInfo { id: i.id, kind: i.kind, ap: i.ap, command: i.command, name: i.name, description: i.description, amount: i.amount, shop_buy: i.shop_buy, shop_sell: i.shop_sell, attack: i.attack, magic: i.magic, defense: i.defense, resist: i.resist, ability: i.ability }
        };
        0
    })
}

/// Runs area `area`'s battle program `program` for Sora `handle` (kh2-rs `spawn_waves`): the groups'
/// members created at load appear now, the kind-2 groups create their waves while Sora stands in
/// their volumes (in `kh2_enemies_tick`), the signals switch groups on the global events. Load the
/// area's collision first. Returns the number of enemies created now.
///
/// # Safety
/// `area` is a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_area_run(handle: i32, area: *const c_char, program: i32) -> i32 {
    if area.is_null() {
        return set_error("no area name");
    }
    let Ok(area) = unsafe { CStr::from_ptr(area) }.to_str().map(str::to_string) else { return set_error("area name is not UTF-8") };
    let Ok(program) = u16::try_from(program) else { return set_error("program is 0..65535") };
    with(-1, |l| {
        if l.enemy_data.is_none() {
            match l.disc.enemies() {
                Ok(d) => l.enemy_data = Some(Arc::new(d)),
                Err(e) => return set_error(e),
            }
        }
        if usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref).is_none() {
            return set_error(format!("no Sora with handle {handle}"));
        }
        let Some(data) = l.enemy_data.clone() else { return -1 };
        let en = l.enemies.entry(handle).or_insert_with(|| crate::enemy::Enemies::new(data));
        let before = en.list.len();
        let run = crate::area::AreaRun::set_up(&mut l.disc, &area, program, en, &l.world);
        let n = en.list.len() - before;
        l.area_runs.insert(handle, (run, Vec::new()));
        n as i32
    })
}

/// Raises a global event (`FUN_00197b70`: id and argument, -1 any) on Sora `handle`'s area run: the
/// groups its signals name are enabled or disabled.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_area_raise_event(handle: i32, event: i32, arg: i32) -> i32 {
    with(-1, |l| match l.area_runs.get_mut(&handle) {
        Some((_, raised)) => {
            raised.push((event, arg));
            0
        }
        None => set_error("no area run for this Sora"),
    })
}

/// Takes the global events the area's groups raised (event 3: a group record's enemies are all
/// gone, argument the record's id; event 2: all the groups' enemies are gone): writes up to `max`
/// (id, argument) pairs into `out` (2 ints each), returns the count written.
///
/// # Safety
/// `out` is NULL or points to `2 * max` ints.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_area_take_events(handle: i32, out: *mut i32, max: u32) -> i32 {
    with(-1, |l| {
        let Some((run, _)) = l.area_runs.get_mut(&handle) else { return 0 };
        let n = run.raised.len().min(max as usize);
        let taken: Vec<(u32, i32)> = run.raised.drain(..n).collect();
        if !out.is_null() {
            for (k, (id, arg)) in taken.iter().enumerate() {
                unsafe {
                    *out.add(2 * k) = *id as i32;
                    *out.add(2 * k + 1) = *arg;
                }
            }
        }
        taken.len() as i32
    })
}

/// Ends Sora `handle`'s area run (its enemies stay; `kh2_enemies_clear` removes them).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_area_end(handle: i32) -> i32 {
    with(-1, |l| {
        l.area_runs.remove(&handle);
        0
    })
}

/// The mission manager of Sora `handle`'s area run (kh2-rs `struggle::Active`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2Mission {
    /// 1 while a manager exists (started by the area run's program, not ended).
    pub running: i32,
    /// Host 4.29's counters: slot 0 the player's orbs, slot 1 the opponent's.
    pub counters: [i32; 3],
    /// The clock's time shown (frames, 60 a second) and 1 when it counts down.
    pub clock_frames: i32,
    pub clock_down: i32,
    /// The manager's update state (0 wait to show, 1 running, 2 to 4 the end).
    pub state: i32,
    /// 1 while a layout event plays (the groups wait, host 1.34).
    pub event_playing: i32,
    /// 1 when the mission stopped (`FUN_001b9940`).
    pub stopped: i32,
    /// How the match ended: 0 not yet, 1 a full counter on the player's side, 2 on the opponent's,
    /// 3 the clock, 4 Sora's knock-out.
    pub end_cause: i32,
    /// The result the script sent: 0 none, 1 won (global event 159), 2 lost (160).
    pub result: i32,
    /// The end's last step: 1 the manager ended, 1 in `game_over` with the game-over path, 1 in
    /// `menu` when it asked for the result menu (record 0x1a) and stays.
    pub ended: i32,
    pub game_over: i32,
    pub menu: i32,
    /// Orbs in the pool.
    pub orbs: u32,
    /// The mission id (opcode 0x15).
    pub id: u32,
}

/// The mission manager of Sora `handle`'s area run into `out`: 1 when one exists, 0 when none.
///
/// # Safety
/// `out` is NULL or points to a `Kh2Mission`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_mission_state(handle: i32, out: *mut Kh2Mission) -> i32 {
    with(-1, |l| {
        let Some(m) = l.area_runs.get(&handle).and_then(|(r, _)| r.mission.as_ref()) else { return 0 };
        let a = &m.a;
        let st = Kh2Mission {
            running: i32::from(!m.ended),
            counters: m.counters(),
            clock_frames: a.clock.shown,
            clock_down: i32::from(a.clock.down),
            state: a.state.into(),
            event_playing: i32::from(a.play.is_some()),
            stopped: i32::from(a.stopped),
            end_cause: match a.end_cause {
                Some("win") => 1,
                Some("lose") => 2,
                Some("time") => 3,
                Some("knock-out") => 4,
                _ => 0,
            },
            result: match m.outcome() {
                Some(crate::struggle::Outcome::Won) => 1,
                Some(crate::struggle::Outcome::Lost) => 2,
                None => 0,
            },
            ended: m.ended.into(),
            game_over: m.game_over.into(),
            menu: m.menu.into(),
            orbs: a.orbs.len() as u32,
            id: a.id,
        };
        if !out.is_null() {
            unsafe { *out = st };
        }
        1
    })
}

/// The orbs of the mission's pool: up to `max` of (x, y, z cm, kind) as 4 floats each into `out`;
/// returns the count written (kinds 0 the player's, 2 the opponent's).
///
/// # Safety
/// `out` is NULL or points to `4 * max` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_mission_orbs(handle: i32, out: *mut f32, max: u32) -> i32 {
    with(-1, |l| {
        let Some(m) = l.area_runs.get(&handle).and_then(|(r, _)| r.mission.as_ref()) else { return 0 };
        let orbs = m.orbs();
        let n = orbs.len().min(max as usize);
        if !out.is_null() {
            for (k, (p, kind)) in orbs.iter().take(n).enumerate() {
                unsafe {
                    *out.add(4 * k) = p.x;
                    *out.add(4 * k + 1) = p.y;
                    *out.add(4 * k + 2) = p.z;
                    *out.add(4 * k + 3) = f32::from(*kind);
                }
            }
        }
        n as i32
    })
}

/// Takes the sounds the mission asked for: up to `max` of (sound number, has place, x, y, z) as 5
/// floats each; returns the count written (the interface sounds have no place).
///
/// # Safety
/// `out` is NULL or points to `5 * max` floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_mission_take_sounds(handle: i32, out: *mut f32, max: u32) -> i32 {
    with(-1, |l| {
        let Some(m) = l.area_runs.get_mut(&handle).and_then(|(r, _)| r.mission.as_mut()) else { return 0 };
        let n = m.sounds.len().min(max as usize);
        let taken: Vec<(u32, Option<Vec3>)> = m.sounds.drain(..n).collect();
        if !out.is_null() {
            for (k, (se, at)) in taken.iter().enumerate() {
                let p = at.unwrap_or_default();
                for (j, v) in [*se as f32, if at.is_some() { 1.0 } else { 0.0 }, p.x, p.y, p.z].into_iter().enumerate() {
                    unsafe { *out.add(5 * k + j) = v };
                }
            }
        }
        taken.len() as i32
    })
}

/// A townsperson Sora interacts with (the host's NPC).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2TalkTarget {
    pub pos: [f32; 3],
    pub yaw: f32,
    /// The talk part of the offer (reaction part index).
    pub part: i32,
    /// Script array bit 4 (keeps its facing, camera in its own frame) / bit 6 (camera in its frame).
    pub keep_facing: i32,
    pub own_frame: i32,
    /// 1: Execute 0x1e (Examine): no camera, no turn, no talk motion.
    pub examine: i32,
    /// The spawn record's talk id, handed back.
    pub talk_id: u32,
}

/// The talk request the library hands the host.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2TalkRequest {
    pub talk_id: u32,
    pub examine: i32,
    /// The target's new facing and Sora's.
    pub target_yaw: f32,
    pub sora_yaw: f32,
    /// The motion the target plays (0x32), -1 for none.
    pub target_motion: i32,
    /// 1 when the talk camera started; its look-at point, distance and height.
    pub camera: i32,
    pub look: [f32; 3],
    pub dist: f32,
    pub height: f32,
}

fn talk_parts(l: &mut Lib, model: &str) -> (Option<kh2_game_core::reaction_parts::Table>, Vec<glam::Mat4>) {
    let Ok(file) = l.disc.read(&format!("obj/{model}.mdlx")) else { return (None, Vec::new()) };
    let parts = kh2_game_core::reaction_parts::Table::from_mdlx(&file);
    let rest = kh2_model::mdlx(&file).map(|m| kh2_model::rest_pose(&m.bones)).unwrap_or_default();
    (parts, rest)
}

/// The host reports that Sora interacts with a townsperson (kh2-rs `npc::talk_step`, Execute 0x1f
/// talk / 0x1e examine): Sora stops and faces it, it faces Sora (unless `keep_facing`), the talk
/// camera (mode 4) starts from its type-0x13 record of `obj/<model>.mdlx` (on a bone: through
/// `bones`, `bone_count` column-major 4x4 matrices of its pose in model space, else the rest pose),
/// and Sora's control is locked until `kh2_talk_end`. Writes the request into `out`; returns 0.
///
/// # Safety
/// `model` is NULL or NUL-terminated; `target` points to a `kh2_talk_target`; `bones` is NULL or
/// points to `16 * bone_count` floats; `out` is NULL or points to a `kh2_talk_request`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_talk_begin(handle: i32, model: *const c_char, target: *const Kh2TalkTarget, bones: *const f32, bone_count: u32, out: *mut Kh2TalkRequest) -> i32 {
    if target.is_null() {
        return set_error("no talk target");
    }
    let t = unsafe { *target };
    let model = if model.is_null() { None } else { unsafe { CStr::from_ptr(model) }.to_str().ok().map(str::to_string) };
    let given: Vec<glam::Mat4> = if bones.is_null() {
        Vec::new()
    } else {
        let f = unsafe { std::slice::from_raw_parts(bones, 16 * bone_count.min(1024) as usize) };
        f.as_chunks::<16>().0.iter().map(glam::Mat4::from_cols_array).collect()
    };
    with(-1, |l| {
        if usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref).is_none() {
            return set_error(format!("no Sora with handle {handle}"));
        }
        let (parts, rest) = match &model {
            Some(m) => talk_parts(l, m),
            None => (None, Vec::new()),
        };
        let bones = if given.is_empty() { rest } else { given };
        let target = crate::talk::Target {
            pos: Vec3::from(t.pos),
            yaw: t.yaw,
            part: t.part.clamp(0, 255) as u8,
            keep_facing: t.keep_facing != 0,
            own_frame: t.own_frame != 0,
            examine: t.examine != 0,
            talk_id: t.talk_id,
        };
        let Some(s) = l.soras.get_mut(handle as usize).and_then(Option::as_mut) else { return -1 };
        let talk = crate::talk::begin(s, parts.as_ref(), &bones, &target);
        if let (Some(shot), Some(p)) = (talk.req.shot, l.game.params.camera.as_ref()) {
            let k = handle as usize;
            if l.cameras.len() <= k {
                l.cameras.resize_with(k + 1, || None);
            }
            l.cameras[k].get_or_insert_with(|| crate::camera::Camera::new(0)).start_talk(p, shot);
        }
        let r = talk.req;
        if !out.is_null() {
            unsafe {
                *out = Kh2TalkRequest {
                    talk_id: r.talk_id,
                    examine: r.examine.into(),
                    target_yaw: r.target_yaw,
                    sora_yaw: r.sora_yaw,
                    target_motion: r.target_motion.map_or(-1, |m| m as i32),
                    camera: i32::from(r.shot.is_some() && l.game.params.camera.is_some()),
                    look: r.shot.map_or([0.0; 3], |s| s.look.to_array()),
                    dist: r.shot.map_or(0.0, |s| s.dist),
                    height: r.shot.map_or(0.0, |s| s.height),
                }
            };
        }
        0
    })
}

/// The talk ends (`FUN_00189ff0`, `FUN_00159768`, `FUN_001b7d18`): the camera warps back to the
/// follow (or lock-on) view and Sora's control is released.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_talk_end(handle: i32) -> i32 {
    with(-1, |l| {
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) else { return set_error(format!("no Sora with handle {handle}")) };
        crate::talk::end(s);
        if let Some(Some(c)) = l.cameras.get_mut(handle as usize) {
            c.end_talk();
        }
        0
    })
}

/// The most events a queue keeps for a host that does not poll (oldest dropped first).
pub const QUEUE_CAP: usize = 1024;

fn cap<T>(v: &mut Vec<T>) {
    if v.len() > QUEUE_CAP {
        let n = v.len() - QUEUE_CAP;
        v.drain(..n);
    }
}

/// Bounds Sora `handle`'s event queues (members, enemies, limits, summons, the area run's events).
fn cap_queues(l: &mut Lib, handle: i32) {
    if let Some(list) = l.followers.get_mut(&handle) {
        for f in list {
            cap(&mut f.events);
        }
    }
    if let Some(en) = l.enemies.get_mut(&handle) {
        cap(&mut en.events);
    }
    if let Some(m) = l.limits.get_mut(&handle) {
        cap(&mut m.events);
    }
    if let Some(m) = l.summons.get_mut(&handle) {
        cap(&mut m.events);
    }
    if let Some((run, raised)) = l.area_runs.get_mut(&handle) {
        cap(&mut run.raised);
        cap(raised);
    }
}
/// A sound or effect cue (`crate::cue`; docs/SOUND-AND-EFFECTS.md).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2Cue {
    /// KH2_CUE_SOUND 1 (trigger 8: params bank, index), KH2_CUE_VOICE 2 (13 / 14: line, priority),
    /// KH2_CUE_EFFECT 3 (trigger 1: element), KH2_CUE_FOOTSTEP 4 (2: kind), KH2_CUE_DUST 5 (19:
    /// kind), KH2_CUE_SCRIPT_EFFECT 6 (a script's effect start: element), KH2_CUE_SOUND_NUMBER 7
    /// (a numbered sound: params 0 / 1 the low / high 16 bits).
    pub kind: i32,
    /// 0 Sora, 1 a member (owner_id = follower slot), 2 an enemy (its id), 3 the summon.
    pub owner: i32,
    pub owner_id: i32,
    pub params: [i16; 4],
    pub frame: i16,
    pub _pad: i16,
    pub pos: [f32; 3],
    /// The owner's model, NUL-terminated (its effect file: obj/<model>.apdx); "limit" for a limit's.
    pub model: [u8; 16],
}

/// Takes the oldest sound / effect cue of Sora `handle` and his party, enemies and summon into
/// `out`: 1 when one was taken, 0 when none is waiting (at most 1024 wait).
///
/// # Safety
/// `out` points to a `kh2_cue`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_poll_cue(handle: i32, out: *mut Kh2Cue) -> i32 {
    if out.is_null() {
        return -1;
    }
    sora_do(handle, -1, |s, _| {
        if s.cues.is_empty() {
            return 0;
        }
        let c = s.cues.remove(0);
        let (owner, owner_id) = c.owner.code();
        let mut model = [0u8; 16];
        for (d, b) in model.iter_mut().zip(c.model.bytes().take(15)) {
            *d = b;
        }
        unsafe { *out = Kh2Cue { kind: c.kind as i32, owner, owner_id, params: c.params, frame: c.frame, _pad: 0, pos: c.pos.to_array(), model } };
        1
    })
}

/// An area's set-up (`kh2_area_setup`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2AreaInfo {
    /// Objects the loaded groups create for the entrance (read them with `kh2_area_get_object`).
    pub object_count: u32,
    pub group_count: u32,
    /// `DAT_0035cd18` (area opcode 0xb): the collision groups hosts 1.46 / 1.47 switch.
    pub collision_mask: u64,
    pub camera_type: u32,
    /// The (field, battle) music pair, 0xffff each when the scripts set none.
    pub music: [u16; 2],
    pub has_player: i32,
    pub player_pos: [f32; 3],
    pub player_yaw: f32,
}

/// One object of the last `kh2_area_setup`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2AreaObject {
    /// `00objentry` id and type (4 enemy, 5 townsperson, ...), NUL-terminated model name.
    pub object: u32,
    pub object_type: i32,
    pub model: [u8; 32],
    /// 0 map, 1 btl, 2 evt; the program; the spawn group's tag.
    pub script: i32,
    pub program: i32,
    pub group: [u8; 8],
    pub pos: [f32; 3],
    pub yaw: f32,
    /// Spawn record +0x28 the talk id, +0x2c the command its reaction part 0x83 offers, +0x1c the
    /// spawn kind, +0x1e the serial.
    pub talk_id: u32,
    pub part_command: u32,
    pub spawn_kind: i32,
    pub serial: u32,
}

fn fill<const N: usize>(s: &str) -> [u8; N] {
    let mut b = [0u8; N];
    for (d, c) in b.iter_mut().zip(s.bytes().take(N - 1)) {
        *d = c;
    }
    b
}

/// Sets area `area` up as the game does (kh2-rs `kh2_ard::run::run_area`, orig 0x181148: the map,
/// btl and evt programs, then FUN_001981a0's entrance test): the objects of its spawn groups
/// (townspeople, field objects, save points, chests, enemies, the player's start) for the host to
/// place, the collision mask the objects' hosts 1.46 / 1.47 switch (the library's enemy and member
/// scripts use it), the camera type, the music. Fills `out`; returns the object count.
///
/// # Safety
/// `area` is NUL-terminated; `out` is NULL or points to a `kh2_area_info`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_area_setup(area: *const c_char, map: i32, btl: i32, evt: i32, entrance: i32, out: *mut Kh2AreaInfo) -> i32 {
    if area.is_null() {
        return set_error("no area name");
    }
    let Ok(area) = unsafe { CStr::from_ptr(area) }.to_str().map(str::to_string) else { return set_error("area name is not UTF-8") };
    let byte = |v: i32| v.clamp(0, 255) as u8;
    with(-1, |l| {
        let s = l.disc.area_setup(&area, [byte(map), byte(btl), byte(evt)], byte(entrance));
        kh2_enemy_host::shared::enemy::set_area_mask(s.collision_mask);
        let info = Kh2AreaInfo {
            object_count: s.objects.len() as u32,
            group_count: s.groups.len() as u32,
            collision_mask: s.collision_mask,
            camera_type: s.camera_type,
            music: s.music.unwrap_or([0xffff; 2]),
            has_player: s.player.is_some().into(),
            player_pos: s.player.map_or([0.0; 3], |p| p.0.to_array()),
            player_yaw: s.player.map_or(0.0, |p| p.1),
        };
        if !out.is_null() {
            unsafe { *out = info };
        }
        let n = s.objects.len() as i32;
        l.area_objects = s.objects;
        n
    })
}

/// Object `index` of the last `kh2_area_setup` into `out`: 0, or -1 for an index out of range.
///
/// # Safety
/// `out` points to a `kh2_area_object`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_area_get_object(index: i32, out: *mut Kh2AreaObject) -> i32 {
    if out.is_null() {
        return set_error("no object record");
    }
    with(-1, |l| {
        let Some(o) = usize::try_from(index).ok().and_then(|k| l.area_objects.get(k)) else { return set_error("no such object") };
        let r = Kh2AreaObject {
            object: o.object,
            object_type: o.object_type.into(),
            model: fill(&o.model),
            script: match o.script.as_str() {
                "map" => 0,
                "btl" => 1,
                _ => 2,
            },
            program: o.program.into(),
            group: fill(&o.group),
            pos: o.at.to_array(),
            yaw: o.yaw,
            talk_id: o.talk_id.into(),
            part_command: o.part_command.into(),
            spawn_kind: o.spawn_kind.into(),
            serial: o.serial.into(),
        };
        unsafe { *out = r };
        0
    })
}

/// The body of an actor: its rig, pose (model space) and root matrix. `owner` 0 Sora (`part` 0
/// body, 1 keyblade, 2 second keyblade), 1 a member (`part` = follower slot), 2 an enemy (`part`
/// = index in the list), 3 the summon.
fn actor_body(l: &Lib, handle: i32, owner: i32, part: i32) -> Option<(Arc<crate::anim::Rig>, Vec<Mat4>, Mat4)> {
    match owner {
        0 => {
            let s = usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref)?;
            let (pose, root) = pose_of(s, part)?;
            Some((rig_of(s, part)?, pose.to_vec(), root))
        }
        1 => {
            let f = l.followers.get(&handle)?.get(usize::try_from(part).ok()?)?;
            Some((f.body.rig.clone(), f.body.pose.clone(), Mat4::from_rotation_translation(glam::Quat::from_rotation_y(f.motor.yaw), f.motor.pos)))
        }
        2 => {
            let e = l.enemies.get(&handle)?.list.get(usize::try_from(part).ok()?)?;
            Some((e.anim.rig.clone(), e.anim.pose.clone(), e.matrix()))
        }
        3 => {
            let o = l.summons.get(&handle)?.object.as_ref()?;
            Some((o.body.rig.clone(), o.body.pose.clone(), o.matrix()))
        }
        _ => None,
    }
}

/// What `kh2_mesh_prepare` skinned.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2MeshInfo {
    pub vertex_count: u32,
    pub index_count: u32,
    pub submesh_count: u32,
    pub _pad: u32,
}

/// One draw call of a skinned mesh.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2Submesh {
    pub first_index: u32,
    pub index_count: u32,
    /// Global texture id (`kh2_texture_info` / `kh2_texture_get`), -1 none.
    pub texture: i32,
    /// 1: alpha-blended (draw after the opaque ones).
    pub transparent: i32,
}

/// Skins the current pose of an actor's body (`owner` 0 Sora with `part` 0 body / 1 keyblade /
/// 2 second keyblade, 1 a member slot, 2 an enemy index, 3 the summon) on the CPU, in world space
/// (`space` 1) or model space (0), and keeps it for `kh2_mesh_get`. Fills `out`; returns 0.
///
/// # Safety
/// `out` is NULL or points to a `kh2_mesh_info`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_mesh_prepare(handle: i32, owner: i32, part: i32, space: i32, out: *mut Kh2MeshInfo) -> i32 {
    with(-1, |l| {
        let Some((rig, pose, root)) = actor_body(l, handle, owner, part) else { return set_error("no such body") };
        let mut m = crate::mesh::skin(&rig, &pose, if space == 1 { root } else { Mat4::IDENTITY });
        // Global texture ids: each (rig, texture) once.
        for s in &mut m.subs {
            let local = s.texture as usize;
            if local >= rig.model.textures.len() {
                s.texture = u32::MAX;
                continue;
            }
            let id = match l.textures.iter().position(|(r, k)| Arc::ptr_eq(r, &rig) && *k == local) {
                Some(i) => i,
                None => {
                    l.textures.push((rig.clone(), local));
                    l.textures.len() - 1
                }
            };
            s.texture = id as u32;
        }
        let info = Kh2MeshInfo { vertex_count: m.positions.len() as u32, index_count: m.indices.len() as u32, submesh_count: m.subs.len() as u32, _pad: 0 };
        l.mesh = m;
        if !out.is_null() {
            unsafe { *out = info };
        }
        0
    })
}

/// Copies the mesh `kh2_mesh_prepare` skinned: positions (3 floats a vertex, cm), normals (3),
/// uvs (2), colors (4, 1.0 = neutral), indices (triangle list), submeshes; any pointer may be
/// NULL; each buffer is written only when its capacity (in vertices / indices / submeshes) holds
/// the whole mesh. Returns 0, or -1 when a capacity is too small.
///
/// # Safety
/// Each non-NULL pointer holds its capacity's worth of elements.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn kh2_mesh_get(positions: *mut f32, normals: *mut f32, uvs: *mut f32, colors: *mut f32, max_vertices: u32, indices: *mut u32, max_indices: u32, subs: *mut Kh2Submesh, max_subs: u32) -> i32 {
    with(-1, |l| {
        let m = &l.mesh;
        let (nv, ni, ns) = (m.positions.len(), m.indices.len(), m.subs.len());
        let any_v = !positions.is_null() || !normals.is_null() || !uvs.is_null() || !colors.is_null();
        if (any_v && (max_vertices as usize) < nv) || (!indices.is_null() && (max_indices as usize) < ni) || (!subs.is_null() && (max_subs as usize) < ns) {
            return set_error("a buffer is smaller than the mesh (see kh2_mesh_info)");
        }
        unsafe {
            if !positions.is_null() {
                std::ptr::copy_nonoverlapping(m.positions.as_ptr() as *const f32, positions, 3 * nv);
            }
            if !normals.is_null() {
                std::ptr::copy_nonoverlapping(m.normals.as_ptr() as *const f32, normals, 3 * nv);
            }
            if !uvs.is_null() {
                std::ptr::copy_nonoverlapping(m.uvs.as_ptr() as *const f32, uvs, 2 * nv);
            }
            if !colors.is_null() {
                std::ptr::copy_nonoverlapping(m.colors.as_ptr() as *const f32, colors, 4 * nv);
            }
            if !indices.is_null() {
                std::ptr::copy_nonoverlapping(m.indices.as_ptr(), indices, ni);
            }
            if !subs.is_null() {
                for (k, s) in m.subs.iter().enumerate() {
                    *subs.add(k) = Kh2Submesh { first_index: s.first, index_count: s.count, texture: if s.texture == u32::MAX { -1 } else { s.texture as i32 }, transparent: s.transparent.into() };
                }
            }
        }
        0
    })
}

/// Size of texture `id` (from `kh2_submesh.texture`): writes width and height; returns 0 or -1.
///
/// # Safety
/// `w` and `h` are NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_texture_info(id: i32, w: *mut u32, h: *mut u32) -> i32 {
    with(-1, |l| {
        let Some((rig, k)) = usize::try_from(id).ok().and_then(|i| l.textures.get(i)) else { return set_error("no such texture") };
        let t = &rig.model.textures[*k];
        unsafe {
            if !w.is_null() {
                *w = t.width;
            }
            if !h.is_null() {
                *h = t.height;
            }
        }
        0
    })
}

/// Texture `id` as RGBA8 (row-major, straight alpha; the PS2's alpha 0..0x80 mapped to 0..255)
/// into `out` of `cap` bytes; returns the byte count (width * height * 4) or -1.
///
/// # Safety
/// `out` points to `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_texture_get(id: i32, out: *mut u8, cap: u32) -> i32 {
    if out.is_null() {
        return set_error("no buffer");
    }
    with(-1, |l| {
        let Some((rig, k)) = usize::try_from(id).ok().and_then(|i| l.textures.get(i)) else { return set_error("no such texture") };
        let t = &rig.model.textures[*k];
        if (cap as usize) < t.rgba.len() {
            return set_error("buffer smaller than the texture");
        }
        unsafe { std::ptr::copy_nonoverlapping(t.rgba.as_ptr(), out, t.rgba.len()) };
        t.rgba.len() as i32
    })
}

/// Loads the field HUD for Sora `handle` (art of world `world`, two letters, e.g. "tt"; NULL "zz"):
/// the command menu with its pages and the reaction capsule (docs/HUD.md). Returns 0.
///
/// # Safety
/// `world` is NULL or NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_hud_load(handle: i32, world: *const c_char) -> i32 {
    let world = if world.is_null() { "zz".to_string() } else { unsafe { CStr::from_ptr(world) }.to_str().unwrap_or("zz").chars().take(2).collect() };
    with(-1, |l| {
        if usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref).is_none() {
            return set_error(format!("no Sora with handle {handle}"));
        }
        match crate::hud::Hud::load(&mut l.disc, &world) {
            Ok(h) => {
                l.huds.insert(handle, (h, Vec::new()));
                0
            }
            Err(e) => set_error(e),
        }
    })
}

/// HUD buttons for `kh2_hud_input`.
pub const KH2_HUD_UP: u32 = 1;
pub const KH2_HUD_DOWN: u32 = 2;
pub const KH2_HUD_CONFIRM: u32 = 4;
pub const KH2_HUD_BACK: u32 = 8;
pub const KH2_HUD_REACTION: u32 = 16;

/// The menu's input of one tick (KH2_HUD_* pressed this tick).
#[unsafe(no_mangle)]
pub extern "C" fn kh2_hud_input(handle: i32, buttons: u32) -> i32 {
    with(-1, |l| {
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get(k)).and_then(Option::as_ref) else { return -1 };
        let Some((h, _)) = l.huds.get_mut(&handle) else { return set_error("no HUD (kh2_hud_load)") };
        h.input(crate::hud::MenuInput { up: buttons & KH2_HUD_UP != 0, down: buttons & KH2_HUD_DOWN != 0, confirm: buttons & KH2_HUD_CONFIRM != 0, back: buttons & KH2_HUD_BACK != 0, reaction: buttons & KH2_HUD_REACTION != 0 }, s);
        0
    })
}

/// The host's items for the Items page: `count` (item id, count) pairs.
///
/// # Safety
/// `ids` and `counts` point to `count` elements (or are NULL with `count` 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_hud_set_items(handle: i32, ids: *const u16, counts: *const u8, count: u32) -> i32 {
    let n = count.min(64) as usize;
    let list: Vec<(u16, u8)> = if ids.is_null() || counts.is_null() { Vec::new() } else { (0..n).map(|k| unsafe { (*ids.add(k), *counts.add(k)) }).collect() };
    with(-1, |l| match l.huds.get_mut(&handle) {
        Some((h, _)) => {
            h.items = list;
            0
        }
        None => set_error("no HUD (kh2_hud_load)"),
    })
}

/// One HUD tick (call after `kh2_enemies_tick`): the offered reaction command into the capsule, the
/// menu's layout and players, then the choices made: Attack presses attack on Sora's next tick,
/// a spell starts the cast, the reaction command is accepted; items and drive forms are reported
/// (`kh2_hud_poll_event`) for the host to use (`kh2_use_item`, `kh2_drive_enter`). Builds the
/// draw list; returns its sprite count.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_hud_tick(handle: i32) -> i32 {
    with(-1, |l| {
        let offer = l.enemies.get(&handle).and_then(|e| e.offer).filter(|o| o.status == crate::enemy::Status::Available).map(|o| o.cmd);
        let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) else { return -1 };
        let foes: Vec<crate::hud::Foe> = l.enemies.get(&handle).map(|en| en.list.iter().filter(|e| e.alive() && !e.removed).map(|e| crate::hud::Foe { id: e.id, hp: e.hp, max_hp: e.stats.max_hp, point: e.pos() + glam::Vec3::Y * 100.0 }).collect()).unwrap_or_default();
        let view = l.cameras.get(handle as usize).and_then(Option::as_ref).and_then(|c| c.rig.as_ref()).map(|r| crate::hud::View { eye: r.pos, look: r.look, fov: r.fov });
        let Some((h, list)) = l.huds.get_mut(&handle) else { return set_error("no HUD (kh2_hud_load)") };
        h.observe(&foes, s.target.map(|t| t.pos), view.as_ref());
        h.reaction_cmd = offer;
        h.tick(s);
        // The panel's sounds (the danger alarm, system sound 9 of se000) as numbered sound cues.
        for se in std::mem::take(&mut h.sounds) {
            s.cue(crate::cue::Cue { kind: crate::cue::Kind::SoundNumber, owner: crate::cue::Owner::Sora, model: String::new(), params: [se as i16, 0, 0, 0], pos: s.motor.pos, frame: 0 });
        }
        let mut accept = false;
        for c in std::mem::take(&mut h.fresh) {
            match c {
                crate::hud::Choice::Attack => s.menu_attack = true,
                crate::hud::Choice::Magic { slot } => {
                    if s.spell(slot).is_some() {
                        s.cast_request = Some((slot, 30));
                    }
                }
                crate::hud::Choice::Reaction { .. } => accept = true,
                _ => {}
            }
        }
        *list = h.draw_list();
        let n = list.len() as i32;
        if accept {
            reaction_accept(l, handle);
        }
        n
    })
}

/// A command the player chose in the HUD menu.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2HudEvent {
    /// 1 Attack, 2 magic (`value` = magic slot), 3 item (`value` = item id), 4 drive (`value` =
    /// form), 5 reaction command (`value` = command id).
    pub kind: i32,
    pub value: i32,
}

/// Takes the oldest choice: 1 when one was taken, 0 when none waits.
///
/// # Safety
/// `out` points to a `kh2_hud_event`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_hud_poll_event(handle: i32, out: *mut Kh2HudEvent) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let Some((h, _)) = l.huds.get_mut(&handle) else { return 0 };
        if h.choices.is_empty() {
            return 0;
        }
        use crate::hud::Choice as C;
        let e = match h.choices.remove(0) {
            C::Attack => Kh2HudEvent { kind: 1, value: 0 },
            C::Magic { slot } => Kh2HudEvent { kind: 2, value: slot.into() },
            C::Item { id } => Kh2HudEvent { kind: 3, value: id.into() },
            C::Drive { form } => Kh2HudEvent { kind: 4, value: form.into() },
            C::Reaction { cmd } => Kh2HudEvent { kind: 5, value: cmd.into() },
        };
        unsafe { *out = e };
        1
    })
}

/// One sprite of the HUD draw list.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2HudSprite {
    /// HUD texture id (`kh2_hud_texture_info` / `kh2_hud_texture_get`).
    pub texture: i32,
    /// 0 alpha, 1 additive, 2 subtractive.
    pub blend: i32,
    /// 1: nearest filtering; 1 in `text`: a glyph or icon of the font atlas.
    pub nearest: i32,
    pub text: i32,
    /// Corners top-left, top-right, bottom-left, bottom-right: x, y in the 512 x 448 screen (y down).
    pub pos: [f32; 8],
    /// Texel coordinates of the corners.
    pub uv: [f32; 8],
    /// RGBA per corner, 0x80 = 1.0 (multiply the texel by colour / 128).
    pub color: [u8; 16],
}

/// Copies up to `max` sprites of the last `kh2_hud_tick`'s draw list, in draw order; returns the
/// count written.
///
/// # Safety
/// `out` points to `max` `kh2_hud_sprite`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_hud_draw_list(handle: i32, out: *mut Kh2HudSprite, max: u32) -> i32 {
    if out.is_null() {
        return -1;
    }
    with(-1, |l| {
        let Some((_, list)) = l.huds.get(&handle) else { return set_error("no HUD (kh2_hud_load)") };
        let n = list.len().min(max as usize);
        for (k, s) in list.iter().take(n).enumerate() {
            let mut color = [0u8; 16];
            for (i, c) in s.color.iter().enumerate() {
                color[4 * i..4 * i + 4].copy_from_slice(c);
            }
            let flat = |v: [[f32; 2]; 4]| [v[0][0], v[0][1], v[1][0], v[1][1], v[2][0], v[2][1], v[3][0], v[3][1]];
            unsafe { *out.add(k) = Kh2HudSprite { texture: s.texture as i32, blend: s.blend as i32, nearest: s.nearest.into(), text: s.text.into(), pos: flat(s.pos), uv: flat(s.uv), color } };
        }
        n as i32
    })
}

/// Number of HUD textures (the menu art, the font pages, the icons) and the size of texture `id`
/// into `w` / `h` (when `id` >= 0). Returns the texture count.
///
/// # Safety
/// `w` and `h` are NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_hud_texture_info(handle: i32, id: i32, w: *mut u32, h: *mut u32) -> i32 {
    with(-1, |l| {
        let Some((hud, _)) = l.huds.get(&handle) else { return set_error("no HUD (kh2_hud_load)") };
        if let Some(t) = usize::try_from(id).ok().and_then(|i| hud.textures.get(i)) {
            unsafe {
                if !w.is_null() {
                    *w = t.width;
                }
                if !h.is_null() {
                    *h = t.height;
                }
            }
        }
        hud.textures.len() as i32
    })
}

/// HUD texture `id` as RGBA8 (row-major from the top, straight alpha, the PS2 texel values) into
/// `out` of `cap` bytes; returns the byte count or -1.
///
/// # Safety
/// `out` points to `cap` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_hud_texture_get(handle: i32, id: i32, out: *mut u8, cap: u32) -> i32 {
    if out.is_null() {
        return set_error("no buffer");
    }
    with(-1, |l| {
        let Some((hud, _)) = l.huds.get(&handle) else { return set_error("no HUD (kh2_hud_load)") };
        let Some(t) = usize::try_from(id).ok().and_then(|i| hud.textures.get(i)) else { return set_error("no such texture") };
        if (cap as usize) < t.rgba.len() {
            return set_error("buffer smaller than the texture");
        }
        unsafe { std::ptr::copy_nonoverlapping(t.rgba.as_ptr(), out, t.rgba.len()) };
        t.rgba.len() as i32
    })
}

/// The collision world from the static surfaces plus the objects at their places.
fn rebuild_objects(l: &mut Lib) {
    if l.objects.is_empty() {
        if let Some(base) = l.base_surfaces.take() {
            let groups = l.world.enabled_groups;
            l.world = world_from_surfaces(&base);
            l.world.enabled_groups = groups;
        }
        return;
    }
    if l.base_surfaces.is_none() {
        l.base_surfaces = Some(crate::surface_object::surfaces_of(&l.world));
    }
    let groups = l.world.enabled_groups;
    l.world = crate::surface_object::combined(l.base_surfaces.as_deref().unwrap_or(&[]), &l.objects, groups);
}

/// A surface object's placement: position (cm) and Euler rotation (radians, X then Y then Z).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Kh2ObjectTransform {
    pub pos: [f32; 3],
    pub rot: [f32; 3],
}

fn transform_of(t: *const Kh2ObjectTransform) -> Option<crate::surface_object::Transform> {
    if t.is_null() {
        return Some(crate::surface_object::Transform::default());
    }
    let t = unsafe { *t };
    (in_world(t.pos) && t.rot.iter().all(|r| r.is_finite())).then(|| crate::surface_object::Transform { pos: Vec3::from(t.pos), rot: Vec3::from(t.rot) })
}

/// Creates a moving collision object from `count` triangles in its own space (as
/// `kh2_surfaces_load` takes them) placed at `transform` (NULL: the origin). Returns its id (> 0).
///
/// # Safety
/// `tris` points to `count` surfaces; `transform` is NULL or points to a `kh2_object_transform`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_surface_object_create(tris: *const Surface, count: u32, transform: *const Kh2ObjectTransform) -> i32 {
    if tris.is_null() || count == 0 {
        return set_error("no triangles");
    }
    let tris = unsafe { std::slice::from_raw_parts(tris, count.min(65536) as usize) }.to_vec();
    if !tris.iter().all(|t| t.v.iter().all(|&v| in_world(v))) {
        return set_error("a vertex is not finite or lies beyond 1e7 units");
    }
    let Some(tf) = transform_of(transform) else { return set_error("the transform is not finite") };
    with(-1, |l| {
        let id = l.next_object;
        l.next_object += 1;
        let m = tf.matrix();
        l.objects.push(crate::surface_object::Object { id, tris, now: m, before: m });
        rebuild_objects(l);
        id as i32
    })
}

/// Moves object `id` to `transform`; a body standing on it rides along on its next `kh2_tick`.
///
/// # Safety
/// `transform` points to a `kh2_object_transform`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_surface_object_move(id: i32, transform: *const Kh2ObjectTransform) -> i32 {
    if transform.is_null() {
        return set_error("no transform");
    }
    let Some(tf) = transform_of(transform) else { return set_error("the transform is not finite") };
    with(-1, |l| {
        let Some(o) = l.objects.iter_mut().find(|o| o.id as i32 == id) else { return set_error("no such object") };
        o.now = tf.matrix();
        rebuild_objects(l);
        0
    })
}

/// Deletes object `id`.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_surface_object_delete(id: i32) -> i32 {
    with(-1, |l| {
        let before = l.objects.len();
        l.objects.retain(|o| o.id as i32 != id);
        if l.objects.len() == before {
            return set_error("no such object");
        }
        rebuild_objects(l);
        0
    })
}

/// Starts the audio engine (docs/SOUND-AND-EFFECTS.md "The audio engine"): kh2-rs's software sound
/// driver with the disc's tables; from now on Sora's sound cues are also played by
/// `kh2_audio_tick`. Returns the output rate (48000) or -1.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_audio_init() -> i32 {
    with(-1, |l| match crate::audio::Audio::new(&mut l.disc) {
        Ok(a) => {
            l.audio = Some(a);
            for s in l.soras.iter_mut().flatten() {
                s.audio_cues.get_or_insert_with(Vec::new);
            }
            crate::audio::RATE as i32
        }
        Err(e) => set_error(e),
    })
}

/// Plays Sora `handle`'s new sound cues and renders `frames` stereo frames of 16-bit PCM at 48 kHz
/// into `out` (interleaved left, right: `2 * frames` samples), like libsm64's `sm64_audio_tick`.
/// Returns the frames written.
///
/// # Safety
/// `out` points to `2 * frames` writable samples.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_audio_tick(handle: i32, out: *mut i16, frames: u32) -> i32 {
    if out.is_null() {
        return set_error("no buffer");
    }
    let frames = frames.min(48000) as usize;
    let buf = unsafe { std::slice::from_raw_parts_mut(out, 2 * frames) };
    with(-1, |l| {
        let Some(a) = l.audio.as_mut() else { return set_error("no audio engine (kh2_audio_init)") };
        if let Some(s) = usize::try_from(handle).ok().and_then(|k| l.soras.get_mut(k)).and_then(Option::as_mut) {
            let cues = s.audio_cues.get_or_insert_with(Vec::new);
            a.pending.append(cues);
        }
        a.tick(&mut l.disc, buf, frames);
        frames as i32
    })
}

/// The listener for the audio engine's volume and pan (kh2-rs `sound::placement`): position (cm)
/// and the forward and up directions (the right axis is forward x up). Pass the camera's.
///
/// # Safety
/// The pointers each point to 3 floats.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn kh2_audio_set_listener(pos: *const f32, forward: *const f32, up: *const f32) -> i32 {
    if pos.is_null() || forward.is_null() || up.is_null() {
        return set_error("no listener vectors");
    }
    let v = |p: *const f32| unsafe { Vec3::new(*p, *p.add(1), *p.add(2)) };
    let (at, f, u) = (v(pos), v(forward).normalize_or_zero(), v(up).normalize_or_zero());
    if !at.is_finite() || f == Vec3::ZERO || u == Vec3::ZERO {
        return set_error("the listener is not finite");
    }
    let r = f.cross(u).normalize_or_zero();
    with(-1, |l| match l.audio.as_mut() {
        Some(a) => {
            a.listener = Some([at, r, u, f]);
            0
        }
        None => set_error("no audio engine (kh2_audio_init)"),
    })
}

/// Plays area music `music<number>` (`bgm/musicNNN.bgm`; an area's pair is in `kh2_area_info.music`)
/// through the audio engine; a negative number stops it. Returns 0, or -1 when the music is missing.
#[unsafe(no_mangle)]
pub extern "C" fn kh2_audio_play_bgm(number: i32) -> i32 {
    with(-1, |l| {
        let Some(a) = l.audio.as_mut() else { return set_error("no audio engine (kh2_audio_init)") };
        if number < 0 {
            a.stop_bgm();
            return 0;
        }
        if a.play_bgm(&mut l.disc, number as u32) { 0 } else { set_error(format!("no music {number}")) }
    })
}
