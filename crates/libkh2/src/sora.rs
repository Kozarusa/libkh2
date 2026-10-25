//! Sora's controller at the game's 60 Hz logic rate, on plain structs.
//!
//! This is a reduced transcription of kh2-rs `kh2-player/src/sora.rs::tick` (a Bevy system) for
//! the v1 states: ground, air, attack and guard. The branch order, the blend lengths and the calls
//! into the rule functions are kh2-rs's; the rules themselves are kh2-rs's code compiled in place
//! (`kh2_game_core::motor`, `crate::controller_rules`) or its pure crates (`kh2_anim::moveset`,
//! the motion triggers). What kh2-rs gets from the ECS (enemies, the camera, the weapon entities)
//! comes from the host here: the stick and the camera's look direction, an optional target point,
//! and the hit reports.
//!
//! Left out (not in v1): hurt, blow and down states, magic, drive forms, reaction commands, Quick
//! Run, Glide, Aerial Dodge, the dodge roll, the motion viewer, the auto-target, the mission and
//! summon input locks, sounds and effects.

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use kh2_anim::moveset::{self, key, Action, Chain, Probe, Scene, Windows};
use kh2_anim::{id, triggers::range, PlayerState};
use kh2_game_core::motor::{ground_motion, ground_step, wrap_angle, GroundConsts, GroundMotionIn, GroundStep, GroundStepIn, Jump, Motor};
use kh2_game_core::world::World;

use crate::anim::{Anim, Rig};
use crate::controller_rules::{stick_vector, view_matrix};
use crate::data::{Game, Leap, SwingConsts};

/// Ticks the keyblade stays out after the last sign of a fight (kh2-rs `sora::BATTLE_TICKS`, the
/// app's stand-in for the game's battle flag).
const BATTLE_TICKS: f32 = 240.0;
/// Range trigger 30: suspends gravity like trigger 3 (kh2-rs `TRIGGER_NO_GRAVITY_30`, FUN_00188030).
const TRIGGER_NO_GRAVITY_30: u8 = 30;
/// Range trigger 49: the landing callbacks play no landing motion (kh2-rs, FUN_0017b9e0).
const TRIGGER_NO_LANDING_MOTION_49: u8 = 49;
/// Blend lengths in ticks (kh2-rs `sora.rs`; 8 is DAT_0037a4e4).
const BLEND_LOCOMOTION: f32 = 8.0;
const BLEND_FALL: f32 = 8.0;
/// The attack start requests its motion with blend 0 (FUN_00176050).
const BLEND_ATTACK: f32 = 0.0;
/// The frame step of one tick.
const TICK_STEP: f32 = 1.0;
/// Motion id of the High Jump (kh2-rs `sora::mid::HIGH_JUMP`).
const HIGH_JUMP_MOTION: usize = 201;
/// Growth motion ids (kh2-rs `sora::mid`).
const AERIAL_DODGE_MOTION: usize = 202;
const QUICK_RUN_MOTION: usize = 203;
const QUICK_RUN_END_MOTION: usize = 204;
const GLIDE_MOTION: usize = 205;
const GLIDE_LEFT_MOTION: usize = 206;
const GLIDE_RIGHT_MOTION: usize = 207;
const GLIDE_HOVER_MOTION: usize = 208;
const BLEND_GLIDE: f32 = 16.0;

/// Growth-ability levels (kh2-rs `sora::Growth`; 0 = not learned, 5 = MAX). The USA rules: no
/// dodge roll (Final Mix only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Growth {
    pub high_jump: u8,
    pub quick_run: u8,
    pub aerial_dodge: u8,
    pub glide: u8,
}
/// The bone the keyblade hangs from (kh2-rs `shared::sora::HAND_BONES[0]`).
pub const HAND_BONE: usize = 178;
/// The left hand, where a form's second keyblade hangs (kh2-rs `HAND_BONES[1]`).
pub const HAND_BONE_2: usize = 86;

/// Controller state (kh2-rs `shared::sora::State`, the v1 subset).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Ground,
    Air,
    Attack { air: bool, finisher: bool },
    Guard,
    QuickRun,
    Glide,
    /// A flinch (hit reaction motion) plays.
    Hurt,
    /// Launched by a blow, until landing.
    Blown,
    /// Knocked out (HP 0): stays down until `revive`.
    Down,
    /// The guard repelled a hit; an attack press answers with the counter.
    Counter,
    /// A spell's cast motion plays.
    Magic,
    /// A drive form change plays (the change motions): no input.
    FormChange,
}

impl State {
    /// The id the C API reports.
    pub fn code(self) -> i32 {
        match self {
            State::Ground => 0,
            State::Air => 1,
            State::Attack { air: false, .. } => 2,
            State::Attack { air: true, .. } => 3,
            State::Guard => 4,
            State::QuickRun => 5,
            State::Glide => 6,
            State::Hurt => 7,
            State::Blown => 8,
            State::Down => 9,
            State::Counter => 10,
            State::Magic => 11,
            State::FormChange => 12,
        }
    }
}

/// One tick of host input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// Left stick: x right, y forward (away from the camera); length 0..1.
    pub stick: [f32; 2],
    /// The camera's horizontal look direction (x, z) in world space; (0, 0) means +Z.
    pub cam_look: [f32; 2],
    /// Buttons held this tick: attack (X / Triangle-less), jump (Circle), guard (Square).
    pub attack: bool,
    pub jump: bool,
    pub guard: bool,
}

/// A target point (kh2-rs `sora::TargetPoint`): an enemy's target shape centre, its radius and
/// half height, in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub pos: Vec3,
    pub radius: f32,
    pub half_height: f32,
}

/// The ability block as the attack table reads it (kh2-rs `shared::sora::Abilities`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Abilities {
    pub bits: u128,
    pub max_ground: u8,
    pub max_air: u8,
    pub max_finishers: u8,
}

impl Default for Abilities {
    /// kh2-rs's baseline: Guard (bit 1) and the status block's base lengths 3 / 3 / 1.
    fn default() -> Abilities {
        Abilities { bits: 1 << 1, max_ground: 3, max_air: 3, max_finishers: 1 }
    }
}

impl Abilities {
    pub fn chain(&self, hits: u8, finishers: u8, airborne: bool, finisher_window: bool) -> Chain {
        Chain { airborne, abilities: self.bits, combo: hits, finishers, max_ground: self.max_ground, max_air: self.max_air, max_finishers: self.max_finishers, finisher_window }
    }
}

/// The motion-set variant (kh2-rs `sora::motion_state`, the rule of FUN_0017da30).
pub fn motion_state(weapon_out: bool, battle_flag: bool) -> PlayerState {
    match (weapon_out, battle_flag) {
        (true, true) => PlayerState::BattleWeapon,
        (true, false) => PlayerState::FieldWeapon,
        (false, _) => PlayerState::FieldNoWeapon,
    }
}

/// The launch of an attack's leap (kh2-rs `sora::leap_launch`, FUN_00176050): the horizontal unit
/// direction, the horizontal speed per tick and the jump height.
pub fn leap_launch(l: &Leap, flags: u32, jump_height: f32, facing: Vec3, target: Option<(Vec3, f32)>) -> (Vec3, f32, f32) {
    let Some((to, radii)) = target else { return (facing.with_y(0.0).normalize_or_zero(), l.max_speed, l.height) };
    let reach = l.reach + if flags & 2 != 0 { jump_height } else { 0.0 };
    let up = to.y;
    let h = if l.min_height <= up { if reach < up { reach } else { up } } else { l.min_height };
    let flat = to.with_y(0.0);
    let gap = flat.length() - radii + l.gap;
    let rise = (2.0 * h / kh2_game_core::motor::G).sqrt();
    let s = gap / rise;
    let speed = if l.min_speed <= s { if s <= l.max_speed { s } else { l.max_speed } } else { l.min_speed };
    (flat.normalize_or_zero(), speed, h)
}

/// The turn of range triggers 22 and 28 (kh2-rs `sora::attack_turn_rate`, FUN_00186018).
pub fn attack_turn_rate(param: i16, turn_speed: f32, to_target: Vec3, c: Option<&SwingConsts>) -> Option<f32> {
    let c = c?;
    if to_target.with_y(0.0).length() <= c.turn_min_gap {
        return None;
    }
    let rate = param as f32 * c.turn_unit;
    Some(if rate == 0.0 { turn_speed } else { rate })
}

/// How a target point lies from `from` (kh2-rs `sora::probe`, FUN_001cba68).
fn probe(p: &Target, from: Vec3, reference: Vec3) -> Probe {
    let d = p.pos - from;
    let flat = d.with_y(0.0);
    let len = flat.length();
    Probe { dist: (len - p.radius).max(0.0), dy: -d.y, dot: (flat.normalize_or_zero().dot(reference)).min(1.0), half_height: p.half_height }
}

/// kh2-rs `sora::reference_direction`.
fn reference_direction(m: &Motor, target: Option<Target>) -> Vec3 {
    target.map(|t| (t.pos - m.pos).with_y(0.0).normalize_or_zero()).filter(|d| *d != Vec3::ZERO).unwrap_or_else(|| m.forward().with_y(0.0).normalize_or_zero())
}

/// Sora's body collision shape (kh2-rs `sora::BodyShape`, `body_shape`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct BodyShape {
    radius: f32,
    height: f32,
    lift: f32,
}

fn body_shape(rig: &Rig) -> Option<BodyShape> {
    rig.model.shapes.iter().find(|s| s.kind == 0 || s.kind == 4).map(|s| BodyShape { radius: s.radius.abs(), height: s.height, lift: s.lift })
}

/// The ledge probe of the table selection (kh2-rs `sora::ledge_probe` / `ledge_at`, FUN_001cbcf8 ->
/// FUN_0019a8e8): true when no floor (or one too low) is ahead.
fn ledge_flagged(world: &World, m: &Motor, airborne: bool, shape: Option<BodyShape>, reference: Vec3, j_front: f32) -> bool {
    if airborne {
        return false;
    }
    let Some(b) = shape else { return false };
    let normal = m.ground.map_or(Vec3::Y, |g| g.normal);
    let dir = normal.cross(reference).cross(normal).normalize_or_zero();
    if dir == Vec3::ZERO {
        return false;
    }
    let v = dir * (b.radius + j_front);
    let start = m.pos;
    let len = v.length();
    let centre = start + Vec3::Y * b.lift;
    let travel = if len > 1e-4 {
        let d = v / len;
        kh2_game_core::motor::sweep_wall(world, centre, centre + v, b.radius, kh2_map::flags::PARTY_COLLIDE)
            .map(|(q, _)| (q - centre).dot(d))
            .filter(|ahead| *ahead > 0.0)
            .map_or(len, |ahead| (ahead - b.radius).clamp(0.0, len))
    } else {
        0.0
    };
    let end = if len > 1e-4 { start + v * (travel / len) } else { start };
    let top = end.y + b.lift + v.y.abs().min(100.0);
    let bottom = end.y - 3.0 * b.lift;
    let floor = world.ground(end, top, top - bottom).map(|g| g.y);
    let limit = start.y - (-v.y).max(0.0) - (b.lift - b.height) + 1.0;
    floor.is_none_or(|y| y < limit)
}

/// Height of the ground below Sora (kh2-rs `sora::height_above_ground`).
pub fn height_above_ground(world: &World, m: &Motor) -> f32 {
    world.ground(m.pos, m.pos.y + 1.0, 10_000.0).map_or(0.0, |g| (m.pos.y - g.y).max(0.0))
}

/// Per-tick values the attack code needs (the closure captures of kh2-rs `tick`).
struct Ctx {
    pstate: PlayerState,
    dir: Vec3,
    target: Option<Target>,
    own_point: Option<Target>,
    jump_height: f32,
}

/// One Sora.
pub struct Sora {
    pub game: Arc<Game>,
    pub state: State,
    pub motor: Motor,
    pub hits: u8,
    pub finishers: u8,
    pub queued_attack: bool,
    /// A hit of the current swing landed (actor +0x108 bit 0x2000000); the host reports it.
    pub connected: bool,
    pub timer: f32,
    pub hover: bool,
    pub last_move: Vec3,
    pub lunge_left: f32,
    pub lunge_speed: f32,
    /// The wanted yaw (actor +0x66c, `controller_rules::update_wanted_yaw`).
    pub wanted_yaw: f32,
    pub battle_ticks: f32,
    pub abilities: Abilities,
    /// Growth-ability levels (`fmab` rows).
    pub growth: Growth,
    pub dash_time: f32,
    pub dash_brake: f32,
    pub aerial_dodge_ready: bool,
    /// Invincible ticks left (Quick Run's AirSlideStarTime, hurt reactions).
    pub invuln: f32,
    pub hp: i32,
    pub max_hp: i32,
    /// Decaying push (kh2-rs `reaction_rules::Push`) and a blow's flight.
    pub push: Option<kh2_game_core::reaction_rules::Push>,
    pub blow: Option<kh2_game_core::reaction_rules::Blow>,
    pub blown_back: bool,
    /// Stand up by itself after 180 ticks down (kh2-rs's stand-in where no game over runs).
    pub auto_revive: bool,
    pub mp: i32,
    pub max_mp: i32,
    pub charge: kh2_game_core::magic_rules::Charge,
    /// Spell levels per magic slot (0 = not owned): Fire, Blizzard, Thunder, Cure, Magnet, Reflect.
    pub magic_levels: [u8; 6],
    /// Sora's level and status (kh2-rs `GameData::sora_stats`: `lvup` row plus the Kingdom Key).
    pub level: i32,
    pub exp: i32,
    /// The weapon chosen at the start (record +0xe: 0 Sword, 1 Shield, 2 Staff): the `lvup` ability
    /// column a level gives (kh2-rs `Progress::raise_level`).
    pub weapon_set: u8,
    /// AP from the `lvup` rows (the row's AP of the level).
    pub ap: i32,
    /// Abilities the level-ups gave (kh2-rs `give`); equipping them is the host's (`set_loadout`).
    pub granted: Vec<u16>,
    /// The ability ids of the last `set_loadout`, for the save record.
    pub loadout_ids: Vec<u16>,
    pub strength: i32,
    pub magic: i32,
    pub defense: i32,
    /// A cast request: (slot, ticks it is still tried).
    pub cast_request: Option<(u8, u8)>,
    pub cast: Option<Cast>,
    pub events: Vec<Event>,
    /// Spells whose trigger fired this tick (slot, kind), for the spell simulation (`spell.rs`).
    pub fired: Vec<(u8, u32)>,
    pub target: Option<Target>,
    pub body: Anim,
    pub weapon: Option<Anim>,
    /// A drive form's second keyblade.
    pub weapon2: Option<Anim>,
    /// The equipped abilities (kh2-rs `shared::abilities::Abilities`), when the host set a list.
    pub loadout: Option<kh2_game_core::abilities::Abilities>,
    /// Leaf Bracer's untouchable ticks (kh2-rs `Sora::leaf_bracer`).
    pub leaf_bracer: f32,
    /// An item use: (item id, the wait after frame trigger 6 when started).
    pub item_use: Option<(u16, Option<f32>)>,
    /// The keyblade item a form's second hand takes (Kingdom Key by default).
    pub hand1_item: u16,
    /// The attack table and its leaps (the base body's, or a drive form's).
    pub moves: Arc<moveset::Moveset>,
    pub leaps: Arc<crate::data::AttackLeaps>,
    pub drive: crate::drive::DriveState,
    /// The form being entered (loaded before the change motion ends).
    pub pending_form: Option<Arc<crate::drive::FormAssets>>,
    /// The base body while a form is active: body, keyblade, attack table, leaps, growth.
    pub base: Option<BaseBody>,
    /// Evaluate the skeleton every tick (the bone matrices, the leap's own target point and the
    /// attack volumes need it).
    pub pose: bool,
    pub spawn: Vec3,
    prev: Input,
    fall_motion: bool,
}

impl Sora {
    pub fn new(game: Arc<Game>, at: Vec3) -> Sora {
        let mut s = Sora::bare(game, at);
        s.set_level(1);
        s
    }

    fn bare(game: Arc<Game>, at: Vec3) -> Sora {
        let body = Anim::new(game.sora.clone());
        let weapon = game.keyblade.clone().map(Anim::new);
        let max_hp = game.params.max_hp;
        let (moves, leaps) = (game.moves.clone(), Arc::new(game.leaps.clone()));
        let max_mp = game.params.max_mp;
        // The capsule stays kh2-rs's `Capsule::default()` (radius 35, Sora's body shape).
        Sora {
            game,
            state: State::Ground,
            motor: Motor { pos: at, ..Motor::default() },
            hits: 0,
            finishers: 0,
            queued_attack: false,
            connected: false,
            timer: 0.0,
            hover: false,
            last_move: Vec3::ZERO,
            lunge_left: 0.0,
            lunge_speed: 0.0,
            wanted_yaw: 0.0,
            battle_ticks: 0.0,
            abilities: Abilities::default(),
            growth: Growth::default(),
            dash_time: 0.0,
            dash_brake: 0.0,
            aerial_dodge_ready: false,
            invuln: 0.0,
            hp: max_hp,
            max_hp,
            push: None,
            blow: None,
            blown_back: false,
            auto_revive: false,
            mp: max_mp,
            max_mp,
            charge: Default::default(),
            magic_levels: [1, 1, 1, 1, 0, 0],
            level: 1,
            exp: 0,
            weapon_set: 0,
            ap: 0,
            granted: Vec::new(),
            loadout_ids: Vec::new(),
            strength: 0,
            magic: 0,
            defense: 0,
            cast_request: None,
            cast: None,
            events: Vec::new(),
            fired: Vec::new(),
            target: None,
            moves,
            leaps,
            drive: crate::drive::DriveState::default(),
            pending_form: None,
            base: None,
            body,
            weapon,
            weapon2: None,
            hand1_item: 41,
            loadout: None,
            leaf_bracer: 0.0,
            item_use: None,
            pose: true,
            spawn: at,
            prev: Input::default(),
            fall_motion: false,
        }
    }

    fn motion(&self, id: usize, pstate: PlayerState) -> Option<usize> {
        self.body.rig.mset.slot(id, pstate)
    }

    /// The jump height of the jump action and of the flag-bit-1 reach (kh2-rs `sora::jump_height`,
    /// FUN_0016be20).
    pub fn jump_height(&self) -> f32 {
        let p = &self.game.params;
        p.fmab(self.growth.high_jump).map_or(p.prty.jump_height, |f| f.high_jump_height)
    }

    /// The body's world matrix (kh2-rs `sora::place`: position and yaw; poses are root-relative).
    pub fn body_matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(Quat::from_rotation_y(self.motor.yaw), self.motor.pos)
    }

    /// Sora's own target point (FUN_0016b558(actor, 3, -1): his first target-kind shape), from the
    /// drawn pose (kh2-rs `sora::target_points`).
    fn own_point(&self) -> Option<Target> {
        let body = self.body_matrix();
        let s = self.body.rig.model.shapes.iter().find(|s| s.kind == kh2_model::collision::kind::TARGET && s.argument & 1 == 0)?;
        let m = match s.bone {
            Some(b) => body * self.body.pose.get(b).copied().unwrap_or(Mat4::IDENTITY),
            None => body,
        };
        let mut pos = m.transform_point3(Vec3::from(s.offset));
        pos.y += s.lift;
        Some(Target { pos, radius: s.radius, half_height: s.height })
    }

    /// Starts the table action `a` (kh2-rs `tick`'s `start_attack`, FUN_00176050).
    fn start_attack(&mut self, a: &Action, air: bool, c: &Ctx, keep_out: &mut bool) -> bool {
        let plyr = &self.game.params.plyr;
        let finisher = a.finisher();
        let v0 = if finisher {
            plyr.attack_finish_v0
        } else if self.hits <= 1 {
            plyr.attack_first_v0
        } else {
            plyr.attack_combo_v0
        };
        let attack_radius = plyr.attack_radius;
        let Some(slot) = self.motion(a.motion as usize, c.pstate) else { return false };
        self.body.restart(Some(slot), false, BLEND_ATTACK);
        self.state = State::Attack { air, finisher };
        self.timer = 0.0;
        self.queued_attack = false;
        self.connected = false;
        *keep_out = true;
        let airborne = !self.motor.grounded;
        if !airborne {
            self.motor.vel = Vec3::ZERO;
        }
        self.lunge_left = 0.0;
        if let Some(tp) = c.target.map(|t| t.pos) {
            let to = (tp - self.motor.pos).with_y(0.0);
            self.motor.yaw = to.x.atan2(to.z);
            self.lunge_left = (to.length() - attack_radius.max(100.0)).max(0.0);
            self.lunge_speed = v0;
        } else if c.dir != Vec3::ZERO {
            self.motor.yaw = c.dir.x.atan2(c.dir.z);
        }
        let leap = self.leaps.of(a);
        if leap.height > 0.0 {
            if !airborne {
                self.hits = 1;
                self.finishers = 1;
            }
            let own = c.own_point.unwrap_or(Target { pos: self.motor.pos, radius: 0.0, half_height: 0.0 });
            let to_target = c.target.map(|t| (t.pos - own.pos, t.radius + own.radius));
            let (d, speed, h) = leap_launch(&leap, a.flags, c.jump_height, self.motor.forward(), to_target);
            self.motor.vel = d * speed;
            self.motor.start_leap(h);
            self.lunge_left = 0.0;
        }
        true
    }

    /// The Square press's explicit actions (kh2-rs `tick`'s `square_explicit`, FUN_00177ea0).
    fn square_explicit(&mut self, air: bool, w: Windows, fin_window: bool, c: &Ctx, keep_out: &mut bool) -> bool {
        let mut chain = self.abilities.chain(self.hits, self.finishers, air, fin_window);
        if w.free {
            chain.restart();
        }
        let mut chosen = None;
        for k in key::EXPLICIT {
            if let Ok(a) = self.moves.explicit(k, &chain, w) {
                let mut cc = chain;
                if cc.press(w, self.connected).is_some() {
                    chain = cc;
                    chosen = Some(a.clone());
                    break;
                }
            }
        }
        let Some(a) = chosen else { return false };
        self.hits = chain.combo;
        self.finishers = chain.finishers;
        self.start_attack(&a, air, c, keep_out)
    }

    /// The attack button (kh2-rs `tick`'s `request`: FUN_00176b00, then the table selection of
    /// FUN_00177d58).
    fn request(&mut self, world: &World, air: bool, w: Windows, fin_window: bool, c: &Ctx, keep_out: &mut bool) -> bool {
        let playing = self.body.slot.map_or(usize::MAX, |sl| sl / 4);
        let mut chain = self.abilities.chain(self.hits, self.finishers, air, fin_window);
        // Kept from kh2-rs (its comment: the idle-range restart belongs to FUN_00177ea0; kept so
        // the ground combo and air chain stay as they were).
        if w.free {
            chain.restart();
        }
        if chain.press(w, self.connected).is_none() {
            return false;
        }
        let reference = reference_direction(&self.motor, c.target);
        let ledge = ledge_flagged(world, &self.motor, air, body_shape(&self.body.rig), reference, self.game.params.plyr.attack_j_front);
        let scene = Scene { target: c.target.map(|t| probe(&t, self.motor.pos, reference)), others: c.target.iter().map(|t| probe(t, self.motor.pos, reference)).collect(), ledge_probe: ledge, jump_height: c.jump_height };
        let (_, idx) = self.moves.select(&chain, &scene, playing);
        let Some(a) = idx.map(|i| self.moves.actions[i].clone()) else { return false };
        self.hits = chain.combo;
        self.finishers = chain.finishers;
        self.start_attack(&a, air, c, keep_out)
    }

    fn start_jump(&mut self, c: &Ctx, move_speed: f32) {
        let p = &self.game.params;
        let high = p.fmab(self.growth.high_jump);
        let h = high.map_or(p.prty.jump_height, |f| f.high_jump_height);
        self.motor.vel = c.dir * move_speed;
        self.motor.start_jump(h);
        self.aerial_dodge_ready = true;
        self.state = State::Air;
        self.timer = 0.0;
        let slot = if high.is_some() { self.motion(HIGH_JUMP_MOTION, c.pstate).or(self.motion(id::JUMP, c.pstate)) } else { self.motion(id::JUMP, c.pstate) };
        self.body.restart(slot, false, 0.0);
    }

    /// One 60 Hz tick.
    // The empty branches keep kh2-rs's order of tries (a started action ends the chain of `else if`).
    #[allow(clippy::if_same_then_else)]
    pub fn tick(&mut self, world: &World, input: &Input) {
        let attack = input.attack && !self.prev.attack;
        let jump = input.jump && !self.prev.jump;
        let square = input.guard && !self.prev.guard;
        self.prev = *input;
        self.timer += 1.0;
        self.invuln = (self.invuln - 1.0).max(0.0);
        if self.leaf_bracer > 0.0 {
            self.leaf_bracer -= 1.0;
        }
        let game = self.game.clone();
        let prty = &game.params.prty;

        // The stick through the camera view (controller_rules, oracle-tested): FUN_00106f78,
        // FUN_001774d8 -> FUN_00177360. The stick record is (x right, y down, -, magnitude).
        let look = glam::Vec2::from(input.cam_look);
        let look = if look.length_squared() > 1e-12 { look.normalize() } else { glam::Vec2::Y };
        let view = view_matrix([-look.x, 0.0, -look.y], [0.0; 3], [0.0, 1.0, 0.0]);
        let s = glam::Vec2::from(input.stick);
        let mag = s.length().min(1.0);
        let stick = if mag > 0.0 { let u = s / s.length(); [u.x, -u.y, 0.0, mag] } else { [0.0; 4] };
        let mv = stick_vector(stick, &view, self.wanted_yaw, prty.walk_speed, prty.run_speed);
        let (dir, move_speed) = if stick[3] > 0.0 && (mv[0] != 0.0 || mv[2] != 0.0) { (Vec3::new(mv[0], 0.0, mv[2]), mv[3]) } else { (Vec3::ZERO, 0.0) };
        if move_speed > 0.0 {
            self.wanted_yaw = crate::controller_rules::wanted_yaw(mv);
        }

        // Battle mode (keyblade out) while a target is in lock-on range or after attacking.
        let mut keep_out = false;
        if self.target.is_some_and(|t| t.pos.distance(self.motor.pos) <= game.params.sstm.lock_on_range) {
            self.battle_ticks = BATTLE_TICKS;
        }
        self.battle_ticks = (self.battle_ticks - 1.0).max(0.0);
        let pstate = motion_state(self.battle_ticks > 0.0, true);
        let c = Ctx { pstate, dir, target: self.target, own_point: if self.pose { self.own_point() } else { None }, jump_height: self.jump_height() };
        let rig = self.body.rig.clone();
        let moves = self.moves.clone();
        let motion = |id: usize| rig.mset.slot(id, pstate);

        let mut airborne_dy: Option<f32> = None;
        let fall_max = game.params.sstm.fall_max;
        let mut lunge_move = Vec3::ZERO;
        let hover_now = !self.motor.grounded && (self.body.trigger(range::NO_GRAVITY) || self.body.trigger(range::NO_GRAVITY_KEEP_KINETICS) || self.body.trigger(TRIGGER_NO_GRAVITY_30));
        if self.hover && !hover_now && !self.motor.grounded && matches!(self.state, State::Attack { .. } | State::Air) {
            self.motor.vel = self.last_move.with_y(0.0);
            self.motor.jump = Jump::falling_at(-self.last_move.y, self.motor.pos.y);
        }
        self.hover = hover_now;
        self.fall_motion = false;
        // The MP charge (FUN_00175f48): at its end MP is refilled.
        if self.charge.tick(1.0) {
            self.mp = self.max_mp;
            self.events.push(Event::MpRestored);
        }
        if let Some((slot, left)) = self.cast_request.take()
            && !self.try_cast(slot, &c)
            && left > 1
        {
            self.cast_request = Some((slot, left - 1));
        }
        match self.state {
            State::Ground => {
                self.hits = 0;
                self.finishers = 1;
                let ground_in = GroundStepIn { dir: [dir.x, 0.0, dir.z, move_speed], ..Default::default() };
                self.motor.vel = match ground_step(&ground_in, &GroundConsts::USA, 1.0) {
                    GroundStep::Walk { out, .. } if move_speed > 0.0 && dir.length_squared() > 1e-12 => Vec3::new(out[0], 0.0, out[2]),
                    _ => Vec3::ZERO,
                };
                let class = ground_motion(&GroundMotionIn { speed: move_speed, walk: prty.walk_speed, run: prty.run_speed, ..Default::default() }, &GroundConsts::USA).class;
                if move_speed > 0.0 {
                    self.motor.turn_to_step(dir, prty.turn_speed, TICK_STEP);
                }
                let landing = self.body.slot == motion(id::LAND) && !self.body.finished;
                if move_speed > 0.0 || !landing {
                    let m = match class {
                        0 => id::IDLE,
                        1 => id::WALK,
                        _ => id::RUN,
                    };
                    self.body.play(motion(m), true, BLEND_LOCOMOTION);
                }
                if jump {
                    self.start_jump(&c, move_speed);
                } else if attack {
                    self.request(world, false, Windows { free: true, combo: false }, false, &c, &mut keep_out);
                } else if square {
                    // FUN_00177ea0's order: the explicit actions, then (no growth abilities in v1)
                    // the guard (FUN_00178080, key 11).
                    // USA rule (kh2-rs `square_ground_for` without the roll): moving with Quick Run
                    // learned = Quick Run at the press (FUN_00178108), else the guard.
                    let quick = move_speed > 0.0 && self.growth.quick_run >= 1;
                    if self.square_explicit(false, Windows { free: true, combo: false }, false, &c, &mut keep_out) {
                    } else if quick && let Some(f) = game.params.fmab(self.growth.quick_run) {
                        self.state = State::QuickRun;
                        self.timer = 0.0;
                        self.motor.yaw = dir.x.atan2(dir.z);
                        self.motor.vel = dir * f.air_slide_speed;
                        self.dash_time = f.air_slide_time;
                        self.dash_brake = f.air_slide_brake;
                        self.invuln = f.air_slide_star_time;
                        self.body.restart(motion(QUICK_RUN_MOTION), false, 0.0);
                    } else if let Some(g) = moves.by_key(key::GUARD).filter(|g| self.abilities.chain(self.hits, self.finishers, false, false).check(g).is_ok()).cloned()
                        && let Some(slot) = motion(g.motion as usize)
                    {
                        self.state = State::Guard;
                        self.timer = 0.0;
                        self.body.restart(Some(slot), false, BLEND_ATTACK);
                        self.motor.vel = Vec3::ZERO;
                        keep_out = true;
                    }
                }
            }
            State::Air => {
                self.hits = 0;
                self.finishers = 1;
                self.motor.air_velocity(dir, move_speed);
                if move_speed > 0.0 {
                    self.motor.turn_to(dir, prty.turn_speed);
                }
                if self.motor.jump_control(input.jump) {
                    self.body.play(motion(id::FALL), true, BLEND_FALL);
                    self.fall_motion = true;
                }
                let can_air_act = self.motor.jump.t > self.motor.jump.rise.max(0.0) + 6.0 || (self.motor.jump.rise == 0.0 && self.motor.jump.t > 15.0);
                let dodge = game.params.fmab(self.growth.aerial_dodge).filter(|_| self.aerial_dodge_ready || self.growth.aerial_dodge >= 5);
                let glide = game.params.fmab(self.growth.glide).filter(|f| can_air_act && height_above_ground(world, &self.motor) > f.glide_start_height);
                if jump && let Some(f) = dodge {
                    // Aerial Dodge: a second jump of AirDodgeHeight at AirDodgeSpeed.
                    self.aerial_dodge_ready = false;
                    if move_speed > 0.0 {
                        self.motor.vel = dir * f.air_dodge_speed;
                    }
                    self.motor.start_jump(f.air_dodge_height);
                    self.body.restart(motion(AERIAL_DODGE_MOTION), false, 0.0);
                } else if square && glide.is_some() {
                    self.state = State::Glide;
                    self.timer = 0.0;
                    self.body.play(motion(if move_speed > 0.0 { GLIDE_MOTION } else { GLIDE_HOVER_MOTION }), true, BLEND_GLIDE);
                } else if square && self.square_explicit(true, Windows { free: true, combo: false }, false, &c, &mut keep_out) {
                } else if attack {
                    self.request(world, true, Windows { free: true, combo: false }, false, &c, &mut keep_out);
                }
                if self.state == State::Air {
                    airborne_dy = Some(self.motor.jump_dy(fall_max));
                }
            }
            State::Attack { .. } => {
                if attack {
                    self.queued_attack = true;
                }
                let combo_window = self.body.trigger(range::ALLOW_COMBO);
                let finisher_window = self.body.trigger(range::ALLOW_FINISHER);
                let window = combo_window || finisher_window;
                let step = self.lunge_speed.min(self.lunge_left);
                self.lunge_left -= step;
                let idle = self.body.trigger(range::GROUNDED) || self.body.trigger(range::AIR) || self.body.trigger(range::BLEND_TO_IDLE);
                if self.motor.grounded {
                    self.motor.vel = self.motor.forward() * step;
                } else {
                    if idle {
                        self.motor.air_velocity(dir, move_speed);
                    } else {
                        self.motor.air_velocity(Vec3::ZERO, 0.0);
                    }
                    lunge_move = self.motor.forward() * step;
                }
                if let Some(tp) = c.target.map(|t| t.pos) {
                    let gframe = self.body.gframe();
                    let rates: Vec<f32> = self
                        .body
                        .triggers()
                        .map(|t| {
                            t.active(gframe)
                                .filter(|r| r.id == range::TURN_TO_LOCK || r.id == range::TURN_TO_TARGET)
                                .filter_map(|r| attack_turn_rate(r.params.first().copied().unwrap_or(0), prty.turn_speed, tp - self.motor.pos, game.params.swing.as_ref()))
                                .collect()
                        })
                        .unwrap_or_default();
                    for rate in rates {
                        self.motor.turn_to(tp - self.motor.pos, rate);
                    }
                }
                if !self.motor.grounded {
                    airborne_dy = Some(if hover_now { 0.0 } else { self.motor.jump_dy(fall_max) });
                }
                let air_now = !self.motor.grounded || self.body.trigger(range::NO_GRAVITY);
                let windows = Windows { free: idle, combo: combo_window };
                if square && self.square_explicit(air_now, windows, finisher_window, &c, &mut keep_out) {
                } else if self.queued_attack && (windows.free || windows.combo || finisher_window) && self.request(world, air_now, windows, finisher_window, &c, &mut keep_out) {
                } else if jump && window && self.motor.grounded {
                    self.start_jump(&c, move_speed);
                } else if self.body.finished || (self.body.trigger(range::BLEND_TO_IDLE) && dir != Vec3::ZERO) {
                    self.state = if self.motor.grounded { State::Ground } else { State::Air };
                    if self.state == State::Air {
                        self.body.play(motion(id::FALL), true, BLEND_FALL);
                    }
                }
            }
            State::QuickRun => {
                // FUN_001c1660: the clock runs while Square is held; a release or the end stops.
                if input.guard {
                    self.dash_time -= 1.0;
                }
                let end = motion(QUICK_RUN_END_MOTION);
                if self.body.slot != end && (!input.guard || self.dash_time <= 0.0) {
                    if let Some(f) = game.params.fmab(self.growth.quick_run.max(1)) {
                        self.dash_brake = f.air_slide_stop_brake;
                    }
                    self.body.restart(end, false, 0.0);
                }
                self.motor.vel *= self.dash_brake;
                if self.body.slot == end && (self.body.finished || self.motor.vel.length() < prty.walk_speed) {
                    self.state = State::Ground;
                }
            }
            State::Glide => {
                let f = game.params.fmab(self.growth.glide.max(1)).cloned();
                let h = height_above_ground(world, &self.motor);
                match f {
                    Some(f) if input.guard && h >= f.glide_end_height => {
                        if move_speed > 0.0 {
                            self.motor.turn_to(dir, f.glide_turn_speed);
                            let speed = self.motor.vel.length() * f.glide_acceleration + f.glide_speed * (1.0 - f.glide_acceleration);
                            self.motor.vel = self.motor.forward() * speed;
                            let facing = self.motor.forward();
                            let m = if facing.dot(dir) < 0.98 { if facing.cross(dir).y > 0.0 { GLIDE_LEFT_MOTION } else { GLIDE_RIGHT_MOTION } } else { GLIDE_MOTION };
                            self.body.play(motion(m), true, BLEND_GLIDE);
                        } else {
                            self.motor.vel *= 0.9;
                            self.body.play(motion(GLIDE_HOVER_MOTION), true, BLEND_GLIDE);
                        }
                        let sink = ((h - f.glide_fall_height).max(0.0) * f.glide_fall_ratio).min(f.glide_fall_max);
                        airborne_dy = Some(-sink);
                    }
                    _ => {
                        self.state = State::Air;
                        self.motor.start_fall();
                        self.body.play(motion(id::FALL), true, BLEND_GLIDE);
                    }
                }
            }
            // Handled after this match (no input reaches them).
            State::Hurt | State::Blown | State::Down => {}
            State::FormChange => {
                // kh2-rs: no input, the drive code ends the state; gravity goes on.
                self.motor.vel = Vec3::ZERO;
                if !self.motor.grounded {
                    airborne_dy = Some(self.motor.jump_dy(fall_max));
                }
            }
            State::Magic => {
                // The cast motion's turn triggers 22 / 28 (kh2-rs `magic.rs`, FUN_00186018 ->
                // `magic_rules::cast_turn`): toward the target, by the trigger's degrees per frame; Cure
                // (slot 3) is cast on Sora himself and does not turn.
                if let (Some(cast), Some(t)) = (self.cast, self.target) {
                    let g = self.body.gframe();
                    let turns: Vec<f32> = self.body.triggers().map(|tr| tr.active(g).filter(|r| matches!(r.id, 22 | 28)).map(|r| r.params.first().copied().unwrap_or(0) as u16 as f32).collect()).unwrap_or_default();
                    for deg in turns {
                        self.motor.yaw = kh2_game_core::magic_rules::cast_turn(self.motor.yaw, self.motor.pos, t.pos, cast.slot == 3, deg);
                    }
                }
                // kh2-rs `sora::tick` State::Magic: gravity unless the motion suspends it; control
                // comes back when it ends or its blend-to-idle range opens with a press or a direction.
                let hover = self.body.trigger(range::NO_GRAVITY) || self.body.trigger(range::NO_GRAVITY_KEEP_KINETICS);
                if !self.motor.grounded {
                    airborne_dy = Some(if hover { 0.0 } else { self.motor.jump_dy(fall_max) });
                    if hover {
                        self.motor.jump = Jump::fall(0.0, self.motor.pos.y);
                    }
                }
                if self.body.finished || (self.body.trigger(range::BLEND_TO_IDLE) && (dir != Vec3::ZERO || attack || jump)) {
                    self.state = if self.motor.grounded { State::Ground } else { State::Air };
                    if self.state == State::Air {
                        self.motor.start_fall();
                        self.body.play(motion(id::FALL), true, BLEND_FALL);
                    } else if attack {
                        self.request(world, false, Windows { free: true, combo: false }, false, &c, &mut keep_out);
                    }
                }
            }
            State::Guard | State::Counter => {
                self.motor.vel = Vec3::ZERO;
                if !self.motor.grounded {
                    airborne_dy = Some(self.motor.jump_dy(fall_max));
                }
                // An attack after a guard hit answers with the counter action (key 39, its ability).
                let counter = (self.state == State::Counter && attack)
                    .then(|| moves.by_key(key::COUNTER).filter(|c| self.abilities.chain(self.hits, self.finishers, false, false).check(c).is_ok()).cloned())
                    .flatten()
                    .and_then(|c| motion(c.motion as usize).map(|slot| (c, slot)));
                if let Some((_, slot)) = counter {
                    self.body.restart(Some(slot), false, BLEND_ATTACK);
                    self.state = State::Attack { air: false, finisher: false };
                    self.timer = 0.0;
                    self.hits = 1;
                    self.finishers = 1;
                    self.queued_attack = false;
                    self.connected = false;
                } else if self.body.finished || (self.body.trigger(range::BLEND_TO_IDLE) && (dir != Vec3::ZERO || attack)) {
                    self.state = if self.motor.grounded { State::Ground } else { State::Air };
                    if attack && self.motor.grounded {
                        self.request(world, false, Windows { free: true, combo: false }, false, &c, &mut keep_out);
                    }
                }
            }
        }
        match self.state {
            State::Hurt => {
                self.motor.vel *= 0.85;
                if !self.motor.grounded {
                    airborne_dy = Some(self.motor.jump_dy(fall_max));
                }
                if self.body.finished {
                    self.state = if self.motor.grounded { State::Ground } else { State::Air };
                }
            }
            State::Blown => {
                // The flight of a blow (FUN_0019e4d8); without the reaction data the jump model.
                if let (Some(b), Some(r)) = (self.blow.as_mut(), game.params.reaction.as_ref()) {
                    let (h, vy) = b.step(&r.defaults, &r.consts);
                    self.motor.vel = b.dir * h;
                    airborne_dy = Some(vy);
                } else {
                    airborne_dy = (!self.motor.grounded).then(|| self.motor.jump_dy(fall_max));
                }
                if self.motor.grounded && self.timer > 2.0 {
                    self.blow = None;
                    self.motor.vel = Vec3::ZERO;
                    if self.hp <= 0 {
                        self.state = State::Down;
                        self.timer = 0.0;
                    } else {
                        self.state = State::Hurt;
                        self.body.restart(motion(if self.blown_back { id::DAMAGE_BLOW_BACK_LAND } else { id::DAMAGE_BLOW_FRONT_LAND }), false, 3.0);
                    }
                }
            }
            State::Down => {
                self.motor.vel = Vec3::ZERO;
                if self.auto_revive && self.timer > 180.0 {
                    self.revive();
                }
            }
            _ => {}
        }
        if keep_out {
            self.battle_ticks = BATTLE_TICKS;
        }
        if airborne_dy.is_none() && !self.motor.grounded && matches!(self.state, State::Attack { .. } | State::Air) {
            airborne_dy = Some(self.motor.jump_dy(fall_max));
        }

        // Root motion: yaw turns the facing; translation is rotated by the facing the motion
        // started from.
        let (root_t, root_yaw) = self.body.root_delta;
        self.motor.yaw = wrap_angle(self.motor.yaw + root_yaw);
        let start_yaw = self.motor.yaw - self.body.root.yaw;
        let root_move = Quat::from_rotation_y(start_yaw) * root_t;
        let mut delta = self.motor.vel + root_move.with_y(0.0) + lunge_move;
        if let (Some(p), Some(r)) = (self.push.as_mut(), game.params.reaction.as_ref()) {
            delta += p.step(&r.consts);
            if p.finished() {
                self.push = None;
            }
        }
        let dy = airborne_dy.map(|d| d + root_move.y);
        let was_air = airborne_dy.is_some();
        let before = self.motor.pos;
        let landed = self.motor.move_by(world, delta, dy);
        self.last_move = self.motor.pos - before;
        if landed && was_air && matches!(self.state, State::Attack { .. }) {
            // Landing in an air swing: the jump mover's landing callback plays the landing motion
            // unless range trigger 49 is set (FUN_0017b9e0).
            if !self.body.trigger(TRIGGER_NO_LANDING_MOTION_49) {
                self.state = State::Ground;
                self.timer = 0.0;
                self.hits = 0;
                self.finishers = 1;
                self.queued_attack = false;
                self.lunge_left = 0.0;
                self.motor.vel = Vec3::ZERO;
                self.body.restart(motion(id::LAND), false, 0.0);
            }
        } else if landed && was_air {
            // kh2-rs `sora::land`.
            self.hits = 0;
            self.finishers = 1;
            if matches!(self.state, State::Air | State::Glide) {
                self.state = State::Ground;
                if dir == Vec3::ZERO {
                    self.body.restart(motion(id::LAND), false, 0.0);
                }
            }
        } else if !self.motor.grounded && self.state == State::Ground {
            // Walked off a ledge.
            self.state = State::Air;
            self.motor.start_fall();
            self.body.play(motion(id::FALL), true, BLEND_FALL);
        }
        if self.motor.pos.y < world.min.y - 5000.0 || !self.motor.pos.is_finite() {
            self.motor = Motor { pos: self.spawn, capsule: self.motor.capsule, ..Motor::default() };
            self.state = State::Ground;
        }

        // The pose step of the same tick (kh2-rs `Sim::Pose` runs after `Sim::Sora`), then the
        // keyblade follows the body's slot and frame (kh2-rs `sync_weapon`, run before the next
        // controller tick there).
        self.drive_tick();
        let before = (self.body.slot, self.body.frame);
        self.body.step(self.pose);
        self.magic_trigger(before);
        self.item_tick(before);
        for w in [self.weapon.as_mut(), self.weapon2.as_mut()].into_iter().flatten() {
            w.follow(&self.body);
            if self.pose {
                w.pose_now();
            }
        }
    }

    /// The `magc` row and command cost of a magic slot at its level (kh2-rs `MagicData::slots`,
    /// `magic_rules::pick_row` with world 0).
    pub fn spell(&self, slot: u8) -> Option<(kh2_tables::tables::Magc, u8)> {
        let p = &self.game.params;
        let row = kh2_game_core::magic_rules::pick_row(&p.magc, slot, *self.magic_levels.get(slot as usize)?, 0)?.clone();
        let cost = p.cmd.iter().find(|c| c.id == row.command).map_or(0, |c| c.cost);
        Some((row, cost))
    }

    /// Accepts a cast now if the state allows it (kh2-rs `magic.rs`: FUN_00174cf8 with the hit latch
    /// forced, then FUN_00179400 / FUN_001a4798). True when it started.
    fn try_cast(&mut self, slot: u8, c: &Ctx) -> bool {
        use kh2_game_core::magic_rules as rules;
        let airborne = !self.motor.grounded || self.body.trigger(range::NO_GRAVITY);
        let (free, combo, fin) = match self.state {
            State::Ground | State::Air => (true, false, false),
            State::Attack { .. } | State::Magic => (
                self.body.trigger(range::GROUNDED) || self.body.trigger(range::AIR) || self.body.trigger(range::BLEND_TO_IDLE),
                self.body.trigger(range::ALLOW_COMBO),
                self.body.trigger(range::ALLOW_FINISHER),
            ),
            _ => (false, false, false),
        };
        let Some((row, cost)) = self.spell(slot) else { return false };
        let status = rules::availability(cost, self.mp, self.charge.active(), false, false);
        let pending = self.cast.is_some_and(|k| !k.paid);
        let mut chain = self.abilities.chain(self.hits, self.finishers, airborne, fin);
        let pressed = (free || combo || fin).then(|| chain.press(Windows { free, combo }, true)).flatten();
        if !status.usable() || pending || pressed.is_none() {
            return false;
        }
        self.hits = chain.combo;
        self.finishers = chain.finishers;
        let finisher = rules::is_finisher(chain.combo, airborne, self.abilities.max_ground, self.abilities.max_air);
        let (motion_id, blend, fin_motion) = rules::cast_motion(&row, airborne, finisher);
        let kind = rules::script_kind(row.id, rules::cast_kind(row.level, fin_motion, 0));
        self.battle_ticks = self.battle_ticks.max(BATTLE_TICKS);
        let mslot = self.body.rig.mset.slot(motion_id as usize, PlayerState::BattleWeapon);
        self.body.restart(mslot, false, blend);
        self.state = State::Magic;
        self.timer = 0.0;
        self.motor.vel = Vec3::ZERO;
        self.queued_attack = false;
        self.push = None;
        if airborne {
            // The mover's method +0xc with sstm MagicJump (FUN_001a4798) [kh2-rs: inferred].
            self.motor.start_jump(self.game.params.sstm.magic_jump);
        }
        let _ = c;
        let voice = rules::cast_voice(&row, fin_motion, row.id == 3);
        self.cast = Some(Cast { slot: row.id, kind, motion_id, motion: mslot, cost, paid: false });
        self.events.push(Event::CastStarted { slot: row.id, kind, motion: motion_id, voice });
        true
    }

    /// Frame trigger 17 crossed by this tick's motion step pays the cast (kh2-rs `magic.rs`,
    /// FUN_00188498 case 0x11 -> FUN_001a4a60 -> FUN_001769e0); a cast whose motion changed first ends.
    fn magic_trigger(&mut self, (slot_before, frame_before): (Option<usize>, f32)) {
        use kh2_game_core::magic_rules as rules;
        let Some(mut cast) = self.cast else { return };
        if cast.paid {
            if self.state != State::Magic || self.body.slot != cast.motion {
                self.cast = None;
            }
            return;
        }
        if self.state != State::Magic || self.body.slot != cast.motion {
            self.events.push(Event::CastCancelled { slot: cast.slot });
            self.cast = None;
            return;
        }
        let Some((m, t)) = self.body.motion().zip(self.body.triggers()) else { return };
        let g = |f: f32| f * crate::anim::LOGIC_HZ / m.fps.max(1.0);
        let from = if slot_before == self.body.slot { g(frame_before) } else { g(self.body.prev_frame) };
        let to = g(self.body.frame);
        if from == to {
            return;
        }
        let fires = t.crossed(from, to).filter(|f| f.id == kh2_anim::triggers::frame::MAGIC_CAST).any(|f| f.params.first().is_none_or(|&p| p as u16 == cast.motion_id));
        if !fires {
            return;
        }
        let charge = match rules::pay(self.mp, self.max_mp, cast.cost, self.game.params.prty.mp_drive) {
            rules::Paid::Mp(v) => {
                self.mp = v;
                false
            }
            rules::Paid::Charge { total } => {
                self.mp = 0;
                self.charge = rules::Charge { left: total, total };
                true
            }
        };
        cast.paid = true;
        self.cast = Some(cast);
        self.events.push(Event::SpellFired { slot: cast.slot, kind: cast.kind, charge });
        self.spell_stages(&cast);
        self.fired.push((cast.slot, cast.kind));
    }

    /// The drive request (kh2-rs `drive_tick`, `DriveRequest::Enter`): the entry checks, the Anti /
    /// Final roll, the cost, then the leaving change motion on the old body. `load` returns the
    /// assets of the form the roll picked. Returns the form entered.
    pub fn drive_enter(&mut self, chosen: u8, load: &mut dyn FnMut(u8) -> Option<Arc<crate::drive::FormAssets>>) -> Result<u8, String> {
        use kh2_game_core::drive_rules as rules;
        let data = self.game.drive.clone().ok_or("no drive tables")?;
        let st = &mut self.drive;
        let can_act = st.change.is_none() && self.hp > 0 && !matches!(self.state, State::Down | State::Blown);
        let ctx = rules::EnterCtx { current_form: st.form, owned: st.owned, points: st.points.whole, donald: st.donald, goofy: st.goofy, locked: !can_act };
        let row = rules::check_enter(&data.forms, chosen, &ctx).map_err(|e| format!("{e:?}"))?;
        let battle = self.battle_ticks > 0.0;
        let roll = rules::RollCtx {
            chosen,
            may_anti: rules::companions_ok(kh2_tables::player::form::ANTI, st.donald, st.goofy),
            battle,
            final_owned: st.owned[kh2_tables::player::form::FINAL as usize],
            story_flag: true,
            points: st.points.whole,
            final_cost: data.form(kh2_tables::player::form::FINAL).map_or(u8::MAX, |f| f.cost),
            anti_factor: 1,
        };
        let rolled = rules::choose_form(&data.tuning, &data.levels, &roll, st.counters, &mut st.rng);
        let entered = rolled.form;
        let assets = load(entered).ok_or_else(|| format!("form {entered}: files not loadable"))?;
        let st = &mut self.drive;
        st.counters = rolled.counters;
        if rolled.gave_final {
            st.owned[kh2_tables::player::form::FINAL as usize] = true;
        }
        st.points.spend(data.form(entered).map_or(row.cost, |f| f.cost));
        st.form = entered;
        self.pending_form = Some(assets);
        let slot = self.body.rig.mset.slot(crate::drive::MOTION_LEAVE, PlayerState::BattleWeapon);
        if slot.is_some() {
            self.body.restart(slot, false, 4.0);
            self.state = State::FormChange;
            self.motor.vel = Vec3::ZERO;
            self.queued_attack = false;
        }
        self.drive.change = Some((entered, crate::drive::Phase::Leaving, 0, slot.is_some()));
        Ok(entered)
    }

    /// Back to base Sora (kh2-rs `do_revert`). False when no form is active or a change runs.
    pub fn drive_revert(&mut self, by_command: bool) -> bool {
        use kh2_game_core::drive_rules as rules;
        if !self.drive.in_form || self.drive.change.is_some() || (by_command && !rules::can_revert(self.drive.form, self.battle_ticks > 0.0)) {
            return false;
        }
        let Some((body, weapon, moves, leaps, growth)) = self.base.take() else { return false };
        let from = self.drive.form;
        self.body = Anim::new(body);
        self.weapon = weapon.map(Anim::new);
        self.weapon2 = None;
        self.moves = moves;
        self.leaps = leaps;
        self.growth = growth;
        self.drive.in_form = false;
        if let Some(l) = self.loadout.as_mut() {
            l.set_form(None);
        }
        self.sync_loadout();
        self.drive.form = 0;
        self.drive.timer = Default::default();
        self.drive.files = None;
        self.state = if self.motor.grounded { State::Ground } else { State::Air };
        if !self.motor.grounded {
            self.motor.start_fall();
        }
        self.events.push(Event::FormChanged { from, to: 0 });
        true
    }

    /// The change task and the form timer (kh2-rs `drive_tick`).
    fn drive_tick(&mut self) {
        use crate::drive::{Phase, CHANGE_TIMEOUT, MOTION_ARRIVE};
        use kh2_game_core::drive_rules as rules;
        let Some(data) = self.game.drive.clone() else { return };
        if let Some((to, phase, ticks, motion)) = self.drive.change.as_mut() {
            *ticks += 1;
            self.invuln = self.invuln.max(2.0);
            let done = match phase {
                Phase::Leaving => !*motion || self.body.finished || *ticks > CHANGE_TIMEOUT,
                Phase::Arriving => *ticks > 1 && (self.body.finished || *ticks > CHANGE_TIMEOUT),
            };
            let (to, phase, t) = (*to, *phase, *ticks);
            if done && phase == Phase::Leaving {
                let Some(a) = self.pending_form.take() else {
                    self.drive.change = None;
                    return;
                };
                if self.base.is_none() {
                    self.base = Some((self.body.rig.clone(), self.weapon.as_ref().map(|w| w.rig.clone()), self.moves.clone(), self.leaps.clone(), self.growth));
                }
                self.body = Anim::new(a.body.clone());
                self.weapon = a.weapon.clone().map(Anim::new);
                self.weapon2 = a.weapon2.clone().map(Anim::new);
                self.moves = a.moves.clone();
                self.leaps = a.leaps.clone();
                self.drive.files = Some(a.files.clone());
                let level = self.drive.level[to as usize];
                // In a form the form's own growth ability is the only one (kh2-rs `Growth`).
                let g = rules::growth_level(&data.levels, to, level);
                self.growth = Growth { high_jump: if to == 1 { g } else { 0 }, quick_run: if to == 2 { g } else { 0 }, aerial_dodge: if to == 3 { g } else { 0 }, glide: if to == 4 { g } else { 0 } };
                self.drive.timer = rules::FormTimer::start(&data.tuning, to, level);
                self.drive.in_form = true;
                if self.loadout.is_some() {
                    let list = self.form_abilities();
                    if let Some(l) = self.loadout.as_mut() {
                        l.set_form(Some(list));
                    }
                    self.sync_loadout();
                }
                // FUN_001a04b0: HP raised to the maximum, MP set to its maximum (a charge ends).
                self.hp = self.max_hp;
                self.mp = self.max_mp;
                self.charge = Default::default();
                self.events.push(Event::FormChanged { from: 0, to });
                self.drive.change = Some((to, Phase::Arriving, 0, true));
                self.state = State::FormChange;
                return;
            }
            if done && phase == Phase::Arriving {
                self.drive.change = None;
                self.invuln = 0.0;
                self.state = if self.motor.grounded { State::Ground } else { State::Air };
                if !self.motor.grounded {
                    self.motor.start_fall();
                }
            } else if phase == Phase::Arriving && t == 1 {
                let slot = self.body.rig.mset.slot(MOTION_ARRIVE, PlayerState::BattleWeapon);
                match slot {
                    Some(_) => self.body.restart(slot, false, 0.0),
                    None => {
                        self.state = if self.motor.grounded { State::Ground } else { State::Air };
                        self.drive.change = None;
                        self.invuln = 0.0;
                    }
                }
            }
        }
        if self.drive.in_form && self.drive.change.is_none() {
            let drain = rules::drain_per_tick(&data.tuning, self.battle_ticks > 0.0, 1.0);
            let form = self.drive.form;
            self.drive.timer.add(form, -drain);
            if self.drive.timer.expired() {
                self.drive_revert(false);
            }
        }
    }

    /// A landed hit's drive effect (kh2-rs `drive_tick`, FUN_00178748): the attack row's DriveDrain
    /// fills the gauge, or takes form time while in a form.
    pub fn drive_hit(&mut self, atkp: i16) {
        let Some(data) = self.game.drive.clone() else { return };
        let Some(drain) = self.game.params.atkp(atkp).map(|a| a.drive_drain).filter(|&d| d != 0) else { return };
        if self.drive.in_form {
            let form = self.drive.form;
            self.drive.timer.add(form, -(drain as f32) * data.tuning.form_hit_loss);
        } else if self.drive.change.is_none() {
            self.drive.points.gain(drain as i32);
        }
    }

    /// Sets the level and rebuilds the status from the `lvup` row (kh2-rs `GameData::sora_stats`:
    /// row `level - 1`, defaults 2 / 6 / 2 without a table, plus the Kingdom Key's attack and magic).
    pub fn set_level(&mut self, level: i32) {
        let p = &self.game.params;
        let level = level.clamp(1, p.levels.len().max(1) as i32);
        let l = p.levels.get((level - 1) as usize);
        self.level = level;
        self.strength = l.map_or(2, |l| l.strength as i32) + p.weapon_attack;
        self.magic = l.map_or(6, |l| l.magic as i32) + p.weapon_magic;
        self.defense = l.map_or(2, |l| l.defense as i32);
        self.ap = l.map_or(0, |l| l.ap as i32);
    }

    /// The attack stages of a fired spell (kh2-rs `magic.rs` event 21 and the spell objects): the
    /// `atkp` ids are the spells' (`ATKP_FIRE` 502, Blizzard 248, Thunder 250 / 251 strikes and
    /// 249 column, Cure 708 on the caster), the row is `attack_row(id, kind)`, the sizes are the
    /// `magi` row's (Fire radius / height / time at 0x0 / 0x4 / 0x8; Blizzard life / speed / shot size
    /// at 0x10 / 0x14 / 0x18; Thunder strike radius 0x24, column height / radius 0x28 / 0x2c, time
    /// 0x34; Cure range 0x38). decided: the spell objects' flight (Blizzard's homing, Thunder's
    /// strike pattern over several targets) is the host's; one strike lands on the target point, or
    /// in front of Sora without one. Cure heals Sora with `heal_amount` / `healed`.
    fn spell_stages(&mut self, cast: &Cast) {
        use kh2_game_core::magic_rules as rules;
        let p = self.game.params.clone();
        let magi = rules::magi_row(&p.magi, cast.kind).cloned().unwrap_or_default();
        let fwd = self.motor.forward();
        let chest = self.motor.pos + Vec3::Y * 100.0;
        let row = |id: u16| rules::attack_row(&p.atkp, id, cast.kind).or_else(|| rules::attack_row(&p.atkp, id, 0)).map(|i| &p.atkp[i]);
        let push = |s: &mut Sora, stage: u8, id: u16, radius: f32, height: f32, life: f32, speed: f32, pos: Vec3, dir: Vec3| {
            if let Some(r) = row(id) {
                s.events.push(Event::SpellAttack { slot: cast.slot, stage, atkp: r.id, sub_id: r.sub_id, radius, height, life, speed, pos, dir });
            }
        };
        match cast.slot {
            0 => push(self, 0, 502, magi.f(0x0), magi.f(0x4), magi.f(0x8), 0.0, chest, fwd),
            1 => push(self, 0, 248, magi.f(0x18), magi.f(0x18), magi.f(0x10), magi.f(0x14), chest, fwd),
            2 => {
                let at = self.target.map_or(self.motor.pos + fwd * 300.0, |t| t.pos);
                let strike = if self.motor.grounded { 250 } else { 251 };
                push(self, 0, strike, magi.f(0x24), 0.0, 0.0, 0.0, at, -Vec3::Y);
                push(self, 1, 249, magi.f(0x2c), magi.f(0x28), magi.f(0x34), 0.0, at, Vec3::Y);
            }
            3 => {
                if let Some(r) = row(708).cloned() {
                    push(self, 0, 708, magi.f(0x38), 0.0, 0.0, 0.0, chest, Vec3::Y);
                    let heal = rules::heal_amount(&r, self.strength, self.magic);
                    let before = self.hp;
                    self.hp = rules::healed(self.hp, self.max_hp, heal);
                    self.events.push(Event::Healed { amount: self.hp - before, hp: self.hp });
                    if let Some(t) = kh2_game_core::abilities::leaf_bracer_frames(self.has_ability_bit(kh2_game_core::abilities::BIT_LEAF_BRACER), true) {
                        self.leaf_bracer = t;
                    }
                }
            }
            _ => {}
        }
    }

    /// The damage one of Sora's attack rows deals to a target (kh2-rs `GameData::damage`, the
    /// game's damage routine): `weakness` is the target's percent for the row's element,
    /// `general` its general percent, `max_hp` for type 5 rows. The caps are the player's (none).
    pub fn damage(&self, atkp: u16, sub_id: Option<u16>, defense: i32, max_hp: i32, weakness: i32, general: i32) -> Option<i32> {
        let p = &self.game.params;
        let a = match sub_id {
            Some(s) => p.atkp.iter().find(|r| r.id == atkp && r.sub_id == s),
            None => p.atkp(atkp as i16),
        }?;
        let berserk = self.loadout.as_ref().map_or(0, |l| kh2_game_core::abilities::berserk_for_row(l.block(), self.charge.active(), a.critical_adjust));
        let stat = if a.critical_adjust == 1 { self.magic } else { self.strength + berserk };
        let power = a.power as i32;
        let dmg = match a.r#type {
            5 => (max_hp * stat * power + 9999) / 10000,
            6 => (stat * power + 99) / 100,
            _ => {
                // decided: the target's damage caps are not known here; the floor is 1 (kh2-rs min_taken).
                let base = (stat - defense).max(1);
                ((base as i64 * power as i64 * weakness as i64 * general as i64 + 999_999) / 1_000_000) as i32
            }
        };
        let dmg = match &self.loadout {
            Some(l) => kh2_game_core::abilities::attacker_bonuses(l.block(), self.hits, a, dmg),
            None => dmg,
        };
        Some(dmg.max(0))
    }

    /// An ability bit of the loadout (bits 0..255); without a loadout only the default Guard bit.
    pub fn has_ability_bit(&self, bit: u16) -> bool {
        match &self.loadout {
            Some(l) => l.has_bit(bit),
            None => bit < 128 && self.abilities.bits >> bit & 1 != 0,
        }
    }

    /// Sets the equipped ability list (item ids; the Kingdom Key stays equipped) and derives the
    /// attack table's bits, the combo lengths and the growth levels from the block (kh2-rs
    /// `Abilities::recompute`, `sync_controller`). False without the ability tables.
    pub fn set_loadout(&mut self, ids: &[u16]) -> bool {
        let Some(rules) = self.game.ability_rules.clone() else { return false };
        let mut l = kh2_game_core::abilities::Abilities::new(rules);
        for &id in ids {
            l.equip_ability(id);
        }
        if self.drive.in_form {
            l.set_form(Some(self.form_abilities()));
        }
        self.loadout = Some(l);
        self.loadout_ids = ids.to_vec();
        self.sync_loadout();
        true
    }

    /// A form's ability list (kh2-rs `drive::form_ability_list`: the fmlv grants up to the level and
    /// the growth ability of the form's chain at its display level).
    fn form_abilities(&self) -> Vec<u16> {
        let (Some(d), Some(r)) = (self.game.drive.as_ref(), self.game.ability_rules.as_ref()) else { return Vec::new() };
        let form = self.drive.form;
        let level = self.drive.level[form as usize];
        let mut list = kh2_game_core::drive_rules::granted_abilities(&d.levels, form, level);
        let g = kh2_game_core::drive_rules::growth_level(&d.levels, form, level);
        if (1..=4).contains(&form) && let Some(id) = r.growth_ability(form as usize - 1, g) {
            list.push(id);
        }
        list
    }

    fn sync_loadout(&mut self) {
        let Some(l) = &self.loadout else { return };
        self.abilities = Abilities { bits: l.bits(), max_ground: l.ground_combo(), max_air: l.air_combo(), max_finishers: l.finishers() };
        self.growth = Growth { high_jump: l.growth(0), quick_run: l.growth(1), aerial_dodge: l.growth(2), glide: l.growth(3) };
    }

    /// Starts an item use (kh2-rs `items.rs`: motion request 9 with blend 8, FUN_00175e20). False
    /// when Sora is busy (only from the ground and air states) or the item is unknown.
    pub fn use_item(&mut self, item: u16) -> bool {
        if !matches!(self.state, State::Ground | State::Air) || !self.game.params.items.iter().any(|i| i.id == item) {
            return false;
        }
        let slot = self.body.rig.mset.slot(9, PlayerState::BattleWeapon);
        self.body.restart(slot, false, 8.0);
        self.state = State::Magic;
        self.timer = 0.0;
        self.motor.vel = Vec3::ZERO;
        self.queued_attack = false;
        self.cast = None;
        self.item_use = Some((item, None));
        true
    }

    /// Frame trigger 6 of the use motion starts the 30-frame wait, at its end the item's effect runs
    /// (kh2-rs `items.rs`: `TRIGGER_USE`, `USE_WAIT`, `item_effect` with 100 plus Item Boost).
    fn item_tick(&mut self, (slot_before, frame_before): (Option<usize>, f32)) {
        let Some((item, wait)) = self.item_use else { return };
        match wait {
            None => {
                if self.state != State::Magic {
                    self.item_use = None;
                    return;
                }
                let Some((m, t)) = self.body.motion().zip(self.body.triggers()) else { return };
                let g = |f: f32| f * crate::anim::LOGIC_HZ / m.fps.max(1.0);
                let from = if slot_before == self.body.slot { g(frame_before) } else { g(self.body.prev_frame) };
                if t.crossed(from, g(self.body.frame)).any(|f| f.id == 6) {
                    self.item_use = Some((item, Some(30.0)));
                }
            }
            Some(left) if left > 1.0 => self.item_use = Some((item, Some(left - 1.0))),
            Some(_) => {
                self.item_use = None;
                self.apply_item(item);
            }
        }
    }

    /// The item's effect on Sora now (FUN_001a12a0 through kh2-rs `item_effect`).
    pub fn apply_item(&mut self, item: u16) -> bool {
        use kh2_game_core::items::{item_effect, Vitals};
        let p = self.game.params.clone();
        let Some(row) = p.items.iter().find(|i| i.id == item) else { return false };
        let amount = row.flag1 as u16 | (row.rank as u16) << 8;
        let mult = 100 + self.loadout.as_ref().map_or(0, |l| l.block().item_boost as i32);
        let d = &self.drive.points;
        let mut v = Vitals { hp: self.hp, hp_max: self.max_hp, hp_min: 0, mp: self.mp, mp_max: self.max_mp, charge_left: self.charge.left, charge_total: self.charge.total, drive_fraction: d.fraction, drive_points: d.whole, drive_max: d.max };
        let before = (self.hp, self.mp, d.whole as i32 * 100 + d.fraction as i32);
        if !item_effect(item, amount, mult, &mut v, p.charge_feed) {
            return false;
        }
        self.hp = v.hp;
        self.mp = v.mp;
        self.charge = kh2_game_core::magic_rules::Charge { left: v.charge_left, total: v.charge_total };
        self.drive.points.whole = v.drive_points;
        self.drive.points.fraction = v.drive_fraction;
        let drive = v.drive_points as i32 * 100 + v.drive_fraction as i32 - before.2;
        self.events.push(Event::ItemUsed { item, hp: self.hp - before.0, mp: self.mp - before.1, drive });
        if self.hp > before.0 {
            self.events.push(Event::Healed { amount: self.hp - before.0, hp: self.hp });
        }
        true
    }

    /// A defeat Sora's side scored: `table` is the `lvpm` Experience of the world level, `row` the
    /// enemy's `enmp` Experience (kh2-rs `defeat_payout`, with the Experience Boost multiplier at
    /// low HP), then the level rises while the `lvup` row of the level is reached (kh2-rs
    /// `Progress::raise_level`, Sora's table, held at its end). Returns the experience paid.
    pub fn report_defeat(&mut self, table: u16, row: u16) -> i32 {
        let p = self.game.params.clone();
        let low = self.max_hp > 0 && self.hp as f32 / self.max_hp as f32 <= p.low_hp_fraction;
        let mult = low.then(|| self.loadout.as_ref().map_or(1, |l| i32::from(l.block().exp_multiplier)));
        let amount = kh2_game_core::items::defeat_payout(table, row, mult);
        if amount <= 0 {
            return 0;
        }
        self.exp = (self.exp + amount).min(9_999_999);
        self.events.push(Event::Experience { amount, total: self.exp });
        // kh2-rs `Progress::raise_level` (FUN_001b52f8 / FUN_001b53d0): while the threshold of the
        // level (row `level - 1`'s Exp) is reached and the next row exists, rise; the rises over the
        // previous row are reported, the new row's ability column for the weapon set is given.
        while self.level < 99 {
            let (Some(old), Some(new)) = (p.levels.get(self.level as usize - 1), p.levels.get(self.level as usize)) else { break };
            if old.exp > self.exp {
                break;
            }
            let rise = |a: u8, b: u8| i32::from(a) - i32::from(b);
            let ability = match self.weapon_set {
                0 => new.sword_ability,
                1 => new.shield_ability,
                2 => new.staff_ability,
                _ => 0,
            };
            let (strength, magic, defense, ap) = (rise(new.strength, old.strength), rise(new.magic, old.magic), rise(new.defense, old.defense), rise(new.ap, old.ap));
            self.set_level(self.level + 1);
            if ability != 0 {
                self.granted.push(ability);
            }
            self.events.push(Event::LevelUp { level: self.level, strength, magic, defense, ap, ability });
        }
        amount
    }

    /// The input of the last tick.
    pub fn last_input(&self) -> Input {
        self.prev
    }

    /// Stands up from the knock-out with full HP (kh2-rs `State::Down` stand-up: 120 invincible
    /// ticks, motion 40 with blend 6).
    pub fn revive(&mut self) {
        self.hp = self.max_hp;
        self.invuln = 120.0;
        self.state = State::Hurt;
        let slot = self.body.rig.mset.slot(id::DAMAGE_BLOW_RECOV, PlayerState::BattleWeapon);
        self.body.restart(slot, false, 6.0);
    }

    /// A hit taken (kh2-rs `sora::receive_hits`): `atkp` is the attack row id, `damage` the HP it
    /// takes (the host computes it, it knows the attacker's stats), `dir` the hit direction (from
    /// the attacker toward Sora). Guard from the front repels it; otherwise the victim reaction of
    /// the row (`reaction_rules::victim_outcome`) plays.
    pub fn hit(&mut self, atkp: i16, damage: i32, dir: Vec3) -> HitResult {
        if self.invuln > 0.0 || self.state == State::Down || self.leaf_bracer > 0.0 {
            return HitResult::Ignored;
        }
        let game = self.game.clone();
        let facing = self.motor.forward();
        let from_front = facing.dot(-dir) > 0.0;
        if self.state == State::Guard && !self.body.trigger(range::BLEND_TO_IDLE) && from_front {
            let slot = self.body.rig.mset.slot(id::AUTOGUARD, PlayerState::BattleWeapon);
            self.body.restart(slot, false, 2.0);
            self.state = State::Counter;
            self.invuln = 18.0;
            return HitResult::Guarded;
        }
        let (Some(atk), Some(reaction)) = (game.params.atkp(atkp), game.params.reaction.as_ref()) else { return HitResult::NoRow };
        use kh2_game_core::reaction_rules::{self as rules, Blow, Push, VictimAction, VictimCtx};
        let useless = rules::is_ineffective(100, 100);
        // FUN_00185a68: a lethal hit asks Second Chance / Once More (kh2-rs `abilities`).
        let dmg = damage.max(0);
        let survives = {
            use kh2_game_core::abilities as ab;
            let damage_motion = if self.state == State::Hurt { self.motion_id() } else { None };
            self.hp - dmg <= 0 && ab::survives_lethal_hit(self.has_ability_bit(ab::BIT_SECOND_CHANCE), self.has_ability_bit(ab::BIT_ONCE_MORE), self.hp, damage_motion)
        };
        let dmg = kh2_game_core::abilities::lethal_hit_damage(self.hp, 0, dmg, survives);
        self.hp = (self.hp - dmg).max(0);
        let ctx = VictimCtx { status: 0, grounded: self.motor.grounded, attacker_flag_4000: false, from_front, hit_part_plain: true };
        let out = rules::victim_outcome(&rules::HitRow::from_atkp(atk), useless, &ctx, &reaction.patn, &reaction.defaults, &reaction.consts);
        let d = &reaction.defaults;
        let action = match out.action {
            a @ VictimAction::Blow { .. } => a,
            _ if self.hp == 0 => VictimAction::Blow { motion: rules::blow_motion(from_front), up: d.blow_up, horiz: d.blow_speed, face_attacker: from_front },
            a => a,
        };
        let pstate = PlayerState::BattleWeapon;
        let rig = self.body.rig.clone();
        let first_slot = |ids: &[usize]| ids.iter().find_map(|&i| rig.mset.slot(i, pstate));
        let away = dir.with_y(0.0).normalize_or_zero();
        match action {
            VictimAction::None | VictimAction::NotDecoded { .. } => {}
            VictimAction::Push { strength } => self.push = Push::new(away, strength, 0.0, &reaction.consts),
            VictimAction::Flinch { motion, fallback } | VictimAction::FlinchPush { motion, fallback, .. } => {
                if let VictimAction::FlinchPush { strength, .. } = action {
                    self.push = Push::new(away, strength, 0.0, &reaction.consts);
                }
                self.body.restart(first_slot(&[motion as usize, fallback as usize]), false, 2.0);
                self.state = State::Hurt;
                self.motor.vel = Vec3::ZERO;
            }
            VictimAction::Class14 { motion, strength, face_attacker } => {
                let to = if face_attacker { -away } else { away };
                self.motor.yaw = to.x.atan2(to.z);
                self.push = Push::new(away, strength, 0.0, &reaction.consts);
                self.body.restart(first_slot(&[motion as usize, id::DAMAGE_S_FRONT_HIGH]), false, 2.0);
                self.state = State::Hurt;
                self.motor.vel = Vec3::ZERO;
            }
            VictimAction::Blow { motion, up, horiz, face_attacker } => {
                let to = if face_attacker { -away } else { away };
                self.motor.yaw = to.x.atan2(to.z);
                self.blow = Some(Blow::new(away, up, horiz, d));
                self.push = None;
                self.motor.vel = Vec3::ZERO;
                self.motor.start_fall();
                self.state = State::Blown;
                self.timer = 0.0;
                self.blown_back = !from_front;
                self.body.restart(rig.mset.slot(motion as usize, pstate), false, 2.0);
            }
        }
        if !matches!(action, VictimAction::None | VictimAction::NotDecoded { .. } | VictimAction::Push { .. }) {
            self.hits = 0;
            self.finishers = 1;
        }
        // kh2-rs: the game's invulnerability after a hit was not found; the app's own timer.
        self.invuln = 36.0;
        match action {
            VictimAction::Blow { .. } => HitResult::Blown,
            VictimAction::None | VictimAction::NotDecoded { .. } | VictimAction::Push { .. } => HitResult::NoFlinch,
            _ => HitResult::Flinched,
        }
    }

    /// The live attack windows of one attack body: (atkp id, shape group) of every range trigger 10
    /// or 33 that covers its current frame (kh2-rs `combat::live_attack_shapes`).
    fn live(a: &Anim) -> Vec<(i16, i16)> {
        let Some(tr) = a.triggers() else { return Vec::new() };
        tr.active(a.gframe())
            .filter(|r| r.id == range::ATTACK || r.id == range::ATTACK_COMBO)
            .filter_map(|r| Some((*r.params.first()?, *r.params.get(1)?)))
            .collect()
    }

    /// The live attack windows of the keyblade and the body (kh2-rs `Fighter::attack_bodies`: the
    /// weapons, then the body; Sora's swing windows are in the keyblade's motions).
    pub fn attack_triggers(&self) -> Vec<(i16, i16)> {
        let mut out = self.weapon.as_ref().map(Self::live).unwrap_or_default();
        out.extend(self.weapon2.as_ref().map(Self::live).unwrap_or_default());
        out.extend(Self::live(&self.body));
        out
    }

    /// The world-space attack volumes of the live windows: capsules (a, b, radius) of the attack
    /// shapes of each window's group, on the keyblade and the body (kh2-rs `combat::shape_volume`;
    /// columns and ellipsoids extend up by their height, spheres and cubes are spheres). Units are
    /// world units. Needs the pose.
    pub fn attack_volumes(&self) -> Vec<Volume> {
        let body_m = self.body_matrix();
        let mut out = Vec::new();
        let mut add = |anim: &Anim, root: Mat4| {
            for (atkp, group) in Self::live(anim) {
                for s in anim.rig.model.shapes.iter().filter(|s| s.kind == kh2_model::collision::kind::ATTACK && s.group as i16 == group) {
                    let m = match s.bone {
                        Some(b) => root * anim.pose.get(b).copied().unwrap_or(Mat4::IDENTITY),
                        None => root,
                    };
                    let a = m.transform_point3(Vec3::from(s.offset));
                    let h = if matches!(s.shape, 0 | 1) { s.height.max(0.0) } else { 0.0 };
                    out.push(Volume { a, b: a + Vec3::Y * h, r: s.radius.abs(), atkp, group });
                }
            }
        };
        if let Some(w) = &self.weapon {
            add(w, self.weapon_matrix());
        }
        if let Some(w) = &self.weapon2 {
            add(w, self.weapon2_matrix());
        }
        add(&self.body, body_m);
        out
    }

    /// The fall motion started this tick (for hosts that want the event).
    pub fn fall_started(&self) -> bool {
        self.fall_motion
    }

    /// The keyblade's world matrix (the hand bone of the drawn pose).
    pub fn weapon_matrix(&self) -> Mat4 {
        self.body_matrix() * self.body.pose.get(HAND_BONE).copied().unwrap_or(Mat4::IDENTITY)
    }

    /// The second keyblade's world matrix (the left hand bone of the drawn pose).
    pub fn weapon2_matrix(&self) -> Mat4 {
        self.body_matrix() * self.body.pose.get(HAND_BONE_2).copied().unwrap_or(Mat4::IDENTITY)
    }

    pub fn combo_window(&self) -> bool {
        self.body.trigger(range::ALLOW_COMBO)
    }

    pub fn finisher_window(&self) -> bool {
        self.body.trigger(range::ALLOW_FINISHER)
    }

    /// The motion id of the playing slot (slot / 4).
    pub fn motion_id(&self) -> Option<usize> {
        self.body.slot.map(|s| s / 4)
    }

    /// The air-only flag of an action is read by `Chain::check`; re-exported for tests.
    pub fn moves(&self) -> &moveset::Moveset {
        &self.moves
    }
}

/// Something the host may want to react to (spawn an effect, play a sound).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Event {
    /// A cast started: magic slot (0 Fire, 1 Blizzard, 2 Thunder, 3 Cure), the cast kind (the
    /// `atkp` sub-row key and `magi` row of the spell), the motion id, the voice (-1 none).
    CastStarted { slot: u8, kind: u32, motion: u16, voice: i8 },
    /// The cast motion reached frame trigger 17: the MP is paid and the spell object should spawn
    /// (event 21 of the spell's script). `charge` is true when the MP charge started.
    SpellFired { slot: u8, kind: u32, charge: bool },
    /// A cast ended before its trigger: no spell, no cost.
    CastCancelled { slot: u8 },
    /// The MP charge ended and MP is full again.
    MpRestored,
    /// One attack stage of a fired spell (kh2-rs `magic.rs` event 21): the `atkp` row (id, sub id)
    /// the host's hit test uses, the stage's size from the spell's `magi` row, where it starts and
    /// which way it goes. `life` is the stage's time in ticks (0: one shot), `speed` units per tick.
    SpellAttack { slot: u8, stage: u8, atkp: u16, sub_id: u16, radius: f32, height: f32, life: f32, speed: f32, pos: Vec3, dir: Vec3 },
    /// HP restored (Cure on Sora, an item): the amount and the HP after it.
    Healed { amount: i32, hp: i32 },
    /// An item took effect (kh2-rs `items.rs`, FUN_001a12a0): HP, MP and drive gauge (hundredths) changes.
    ItemUsed { item: u16, hp: i32, mp: i32, drive: i32 },
    /// Experience paid for a defeat, and the level reached when it rose.
    Experience { amount: i32, total: i32 },
    /// A level reached: the rises over the previous row and the ability given (0 none).
    LevelUp { level: i32, strength: i32, magic: i32, defense: i32, ap: i32, ability: u16 },
    /// The body changed: drive form `to` (0 = base Sora) replaced form `from`.
    FormChanged { from: u8, to: u8 },
}

/// A running cast (kh2-rs `magic::Cast`, the parts without the effect files and the script VM).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cast {
    pub slot: u8,
    pub kind: u32,
    pub motion_id: u16,
    pub motion: Option<usize>,
    pub cost: u8,
    pub paid: bool,
}

/// The base body kept while a form is active: body, keyblade, attack table, leaps, growth.
pub type BaseBody = (Arc<Rig>, Option<Arc<Rig>>, Arc<moveset::Moveset>, Arc<crate::data::AttackLeaps>, Growth);

/// What a hit did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitResult {
    /// Invincible or knocked out: nothing happened.
    Ignored = 0,
    /// The guard repelled it.
    Guarded = 1,
    /// Damage, at most a push.
    NoFlinch = 2,
    Flinched = 3,
    Blown = 4,
    /// No such attack row, or no reaction data (the executable could not be read).
    NoRow = -1,
}

/// An attack capsule in world units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    pub a: Vec3,
    pub b: Vec3,
    pub r: f32,
    pub atkp: i16,
    pub group: i16,
}
