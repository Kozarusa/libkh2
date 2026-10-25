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
    pub target: Option<Target>,
    pub body: Anim,
    pub weapon: Option<Anim>,
    /// Evaluate the skeleton every tick (the bone matrices, the leap's own target point and the
    /// attack volumes need it).
    pub pose: bool,
    pub spawn: Vec3,
    prev: Input,
    fall_motion: bool,
}

impl Sora {
    pub fn new(game: Arc<Game>, at: Vec3) -> Sora {
        let body = Anim::new(game.sora.clone());
        let weapon = game.keyblade.clone().map(Anim::new);
        let max_hp = game.params.max_hp;
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
            target: None,
            body,
            weapon,
            pose: true,
            spawn: at,
            prev: Input::default(),
            fall_motion: false,
        }
    }

    fn motion(&self, id: usize, pstate: PlayerState) -> Option<usize> {
        self.game.sora.mset.slot(id, pstate)
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
        let s = self.game.sora.model.shapes.iter().find(|s| s.kind == kh2_model::collision::kind::TARGET && s.argument & 1 == 0)?;
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
        let leap = self.game.leaps.of(a);
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
            if let Ok(a) = self.game.moves.explicit(k, &chain, w) {
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
        let ledge = ledge_flagged(world, &self.motor, air, body_shape(&self.game.sora), reference, self.game.params.plyr.attack_j_front);
        let scene = Scene { target: c.target.map(|t| probe(&t, self.motor.pos, reference)), others: c.target.iter().map(|t| probe(t, self.motor.pos, reference)).collect(), ledge_probe: ledge, jump_height: c.jump_height };
        let (_, idx) = self.game.moves.select(&chain, &scene, playing);
        let Some(a) = idx.map(|i| self.game.moves.actions[i].clone()) else { return false };
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
        let motion = |id: usize| game.sora.mset.slot(id, pstate);

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
                    } else if let Some(g) = game.moves.by_key(key::GUARD).filter(|g| self.abilities.chain(self.hits, self.finishers, false, false).check(g).is_ok()).cloned()
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
            State::Guard | State::Counter => {
                self.motor.vel = Vec3::ZERO;
                if !self.motor.grounded {
                    airborne_dy = Some(self.motor.jump_dy(fall_max));
                }
                // An attack after a guard hit answers with the counter action (key 39, its ability).
                let counter = (self.state == State::Counter && attack)
                    .then(|| game.moves.by_key(key::COUNTER).filter(|c| self.abilities.chain(self.hits, self.finishers, false, false).check(c).is_ok()).cloned())
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
        self.body.step(self.pose);
        if let Some(w) = self.weapon.as_mut() {
            w.follow(&self.body);
            if self.pose {
                w.pose_now();
            }
        }
    }

    /// Stands up from the knock-out with full HP (kh2-rs `State::Down` stand-up: 120 invincible
    /// ticks, motion 40 with blend 6).
    pub fn revive(&mut self) {
        self.hp = self.max_hp;
        self.invuln = 120.0;
        self.state = State::Hurt;
        let slot = self.game.sora.mset.slot(id::DAMAGE_BLOW_RECOV, PlayerState::BattleWeapon);
        self.body.restart(slot, false, 6.0);
    }

    /// A hit taken (kh2-rs `sora::receive_hits`): `atkp` is the attack row id, `damage` the HP it
    /// takes (the host computes it, it knows the attacker's stats), `dir` the hit direction (from
    /// the attacker toward Sora). Guard from the front repels it; otherwise the victim reaction of
    /// the row (`reaction_rules::victim_outcome`) plays.
    pub fn hit(&mut self, atkp: i16, damage: i32, dir: Vec3) -> HitResult {
        if self.invuln > 0.0 || self.state == State::Down {
            return HitResult::Ignored;
        }
        let game = self.game.clone();
        let facing = self.motor.forward();
        let from_front = facing.dot(-dir) > 0.0;
        if self.state == State::Guard && !self.body.trigger(range::BLEND_TO_IDLE) && from_front {
            let slot = game.sora.mset.slot(id::AUTOGUARD, PlayerState::BattleWeapon);
            self.body.restart(slot, false, 2.0);
            self.state = State::Counter;
            self.invuln = 18.0;
            return HitResult::Guarded;
        }
        let (Some(atk), Some(reaction)) = (game.params.atkp(atkp), game.params.reaction.as_ref()) else { return HitResult::NoRow };
        use kh2_game_core::reaction_rules::{self as rules, Blow, Push, VictimAction, VictimCtx};
        let useless = rules::is_ineffective(100, 100);
        self.hp = (self.hp - damage.max(0)).max(0);
        let ctx = VictimCtx { status: 0, grounded: self.motor.grounded, attacker_flag_4000: false, from_front, hit_part_plain: true };
        let out = rules::victim_outcome(&rules::HitRow::from_atkp(atk), useless, &ctx, &reaction.patn, &reaction.defaults, &reaction.consts);
        let d = &reaction.defaults;
        let action = match out.action {
            a @ VictimAction::Blow { .. } => a,
            _ if self.hp == 0 => VictimAction::Blow { motion: rules::blow_motion(from_front), up: d.blow_up, horiz: d.blow_speed, face_attacker: from_front },
            a => a,
        };
        let pstate = PlayerState::BattleWeapon;
        let rig = game.sora.clone();
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
        &self.game.moves
    }
}

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
