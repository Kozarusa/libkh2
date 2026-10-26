//! The spells after their trigger: Fire's attack around Sora, Blizzard's homing shots, Thunder's
//! strikes, against the enemies.
//!
//! A transcription of kh2-rs `kh2-player/src/magic.rs`'s hand port of the spell scripts (FIRE,
//! BLIZZARD and THUNDER `*.mag`, event 21 and their request threads): `blizzard_plan`,
//! `thunder_plan`, `rotate_y_game`, `turn_toward`, `step_shot`, `step_strike`, `new_attack`,
//! `move_attack`, `sweep_points`, `attack_volume`, `shot_ends`, and the hit loop with the victims'
//! hit memory (`magic_rules::HitMemory`, `variant`: kh2-rs's pure rules). Cure is applied at the
//! trigger (`Sora::spell_stages`).
//!
//! decided: the effects' own positions are not known (libkh2 loads no effect files): Fire's attack
//! and Blizzard's shots start at Sora's chest (100 above his feet) and ahead of it; Final form's
//! scatter (host 1.248) keeps the place; Reflect and Magnet are not simulated.

use std::collections::{HashMap, VecDeque};

use glam::{Quat, Vec3};
use kh2_game_core::magic_rules::{self as rules, HitMemory, MagiRow};
use kh2_tables::tables::Atkp;

use crate::enemy::{overlap, Volume};

pub const ATKP_FIRE: u16 = 502;
pub const ATKP_BLIZZARD: u16 = 248;
pub const ATKP_THUNDER_GROUND: u16 = 250;
pub const ATKP_THUNDER_AIR: u16 = 251;
pub const ATKP_THUNDER_COLUMN: u16 = 249;
const MAGI_RADIUS: usize = 0x0;
const MAGI_HEIGHT: usize = 0x4;
const MAGI_TIME: usize = 0x8;
const MAGI_TURN: usize = 0xc;
const MAGI_LIFE: usize = 0x10;
const MAGI_SPEED: usize = 0x14;
const MAGI_SHOT_SIZE: usize = 0x18;
const MAGI_PUSH: usize = 0x1c;
const MAGI_THUNDER_SNAP: usize = 0x20;
const MAGI_THUNDER_RADIUS: usize = 0x24;
const MAGI_THUNDER_COLUMN_H: usize = 0x28;
const MAGI_THUNDER_COLUMN_R: usize = 0x2c;
const MAGI_THUNDER_WAIT: usize = 0x30;
const MAGI_THUNDER_TIME: usize = 0x34;
const SHOT_FADE: u32 = 12;
const SWEEP_STEP: f32 = 0.5;
const SWEEP_MIN: f32 = 2.0;
const CHEST: f32 = 100.0;

/// One spell attack object (kh2-rs `magic::SpellAttack`), centimetres.
#[derive(Clone, Debug)]
pub struct SpellAttack {
    pub group: u32,
    pub base_row: usize,
    pub pos: Option<Vec3>,
    pub prev: Option<Vec3>,
    pub radius: f32,
    pub height: f32,
    pub live: bool,
    pub hit_any: bool,
    pub wall: bool,
    pub slot: u8,
}

#[derive(Clone, Debug, PartialEq)]
struct BlizzardShot {
    angle: f32,
    y: f32,
    flag: u32,
    scale: f32,
}

/// kh2-rs `magic::blizzard_plan` (the shots part; elements are effects).
fn blizzard_shots(kind: u32, counter: u32) -> (Vec<BlizzardShot>, bool) {
    let (k10, fin) = (kind / 10, kind & 1 == 1);
    let mode = if k10 == 3 {
        if fin {
            match counter {
                1 | 3 => 1,
                4 => 2,
                _ => 0,
            }
        } else {
            2
        }
    } else {
        0
    };
    let two = mode == 2;
    let shot = |angle: f32, y: f32, flag: u32, scale: f32| BlizzardShot { angle, y, flag, scale };
    let (a35, a70, a45) = (0.610_865_f32, 1.221_73_f32, std::f32::consts::FRAC_PI_4);
    if k10 == 1 && fin {
        return (vec![shot(0.0, 0.0, 0, 0.5), shot(a35, 0.0, 0, 1.0), shot(-a35, 0.0, 0, 1.0), shot(a70, 0.0, 0, 2.0), shot(-a70, 0.0, 0, 2.0)], true);
    }
    if k10 == 2 {
        return (vec![shot(0.0, 0.0, 0, 1.0), shot(0.0, -0.96, 0, 1.0), shot(0.0, 0.96, 0, 1.0), shot(a45, 0.4, 0, 1.0), shot(-a45, 0.4, 0, 1.0), shot(a45, -0.4, 0, 1.0), shot(-a45, -0.4, 0, 1.0)], true);
    }
    let mut shots = vec![shot(0.0, 0.0, 1, 1.0)];
    if two {
        shots.push(shot(0.0, 0.0, 1, 1.0));
    }
    (shots, !two)
}

/// kh2-rs `magic::rotate_y_game` (host 0.13).
fn rotate_y_game(v: Vec3, angle: f32) -> Vec3 {
    let (sn, cs) = angle.sin_cos();
    let (x, z) = (v.x, -v.z);
    let (x2, z2) = (x * cs + z * sn, z * cs - x * sn);
    Vec3::new(x2, v.y, -z2)
}

/// kh2-rs `magic::thunder_plan` in centimetres: (frames to wait after it, place, probe height).
fn thunder_plan(kind: u32, here: Vec3, facing: Vec3, target: Option<Vec3>, radius: f32) -> Vec<(f32, Vec3, f32)> {
    let (k10, fin) = (kind / 10, kind & 1 == 1);
    let (place, rise) = match target {
        Some(t) => (t, 100.0),
        None => {
            let ahead = if k10 == 2 { rotate_y_game(facing, std::f32::consts::FRAC_PI_4) * 800.0 } else { facing * 300.0 };
            (here + ahead + Vec3::Y * 80.0, 330.0)
        }
    };
    let mut out = Vec::new();
    if k10 == 2 {
        for f in [0.0, 0.47, 0.78] {
            out.push((16.0, here + (place - here) * f, rise));
        }
    }
    out.push((0.0, place, rise));
    if k10 == 1 && fin {
        out.last_mut().unwrap().0 = 8.0;
        let mut dir = rotate_y_game(facing * radius * 1.5, 2.094_395);
        for i in 0..3 {
            out.push((if i < 2 { 8.0 } else { 0.0 }, place + dir, 330.0));
            dir = rotate_y_game(dir, 2.094_395);
        }
    }
    if k10 == 3 {
        let n = if fin { 8 } else { 3 };
        out.last_mut().unwrap().0 = 8.0;
        for i in 1..n {
            out.push((if i < n - 1 { 8.0 } else { 0.0 }, place, 330.0));
        }
    }
    out
}

/// kh2-rs `magic::turn_toward` (FUN_0017c0f8).
pub fn turn_toward(from: Vec3, to: Vec3, rate: f32) -> Vec3 {
    let (a, b) = (from.normalize_or_zero(), to.normalize_or_zero());
    if a == Vec3::ZERO || b == Vec3::ZERO {
        return a;
    }
    let angle = a.dot(b).clamp(-1.0, 1.0).acos();
    let step = angle.clamp(-rate, rate);
    let axis = a.cross(b).normalize_or_zero();
    if axis == Vec3::ZERO {
        return a;
    }
    Quat::from_axis_angle(axis, step) * a
}

/// kh2-rs `magic::attack_volume` (FUN_00189610), centimetres.
fn attack_volume(c: Vec3, radius: f32, height: f32) -> Volume {
    let half = (height - radius).max(0.0);
    Volume { a: c - Vec3::Y * half, b: c + Vec3::Y * half, r: radius }
}

/// kh2-rs `magic::sweep_points` (FUN_001a2cc0), centimetres.
fn sweep_points(a: &SpellAttack, cur: Vec3) -> Vec<Vec3> {
    let mut out = vec![cur];
    let Some(prev) = a.prev else { return out };
    let d = prev - cur;
    let len = d.length();
    let step = a.radius.min(a.height);
    if len <= SWEEP_MIN || step <= 0.0 {
        return out;
    }
    let dir = d / len;
    let mut left = len;
    let mut p = cur;
    while left > step * SWEEP_STEP {
        p += dir * step;
        left -= step;
        out.push(p);
    }
    out
}

fn shot_ends(wall: bool, clash: bool, target: bool, flag: u32) -> bool {
    wall || clash || ((target || wall || clash) && flag == 0)
}

struct Shot {
    kind: u32,
    magi: MagiRow,
    pos: Vec3,
    dir: Vec3,
    target: Option<Vec3>,
    attack: Option<usize>,
    origin: Vec3,
    life: f32,
    fade: Option<u32>,
    done: bool,
    turn_scale: f32,
    flag: u32,
}

struct Strike {
    kind: u32,
    magi: MagiRow,
    pos: Vec3,
    rise: f32,
    atkp: u16,
    started: bool,
    wait: f32,
    attacks: Option<(usize, usize)>,
    done: bool,
}

/// Magnet's attack (MAGNET_*.mag: `attack_create(ctx, 658, -1, kind)`, reaction class 5: the pull).
pub const ATKP_MAGNET: u16 = 658;
/// Magnet's second attack in Wisdom, Master and Final, and Final's last one.
pub const ATKP_MAGNET_FORM: u16 = 976;
pub const ATKP_MAGNET_FINAL: u16 = 1440;
/// Reflect's guard (REFLECT_*.mag: atkp 573, Type 3, Power 0) and its counter-bursts by tier
/// (574 / 576 / 578, then 667 / 668 / 669).
pub const ATKP_REFLECT_GUARD: u16 = 573;
pub const REFLECT_BURST: [u16; 3] = [574, 576, 578];
pub const REFLECT_BURST2: [u16; 3] = [667, 668, 669];
/// The barrier's life when the spell file's element cannot be read: REFLECT_1..3.mag's barrier elements
/// (0, 2, 12) all name particle data with a manager life of 27 ticks on the Final Mix disc.
pub const REFLECT_BARRIER_LIFE: f32 = 27.0;

enum Step {
    /// Magnet (kh2fm-rs magic-turn.md "Magnet", the script's order): after a yield attack 658 of the
    /// kind at the place (a sphere of `magi` +0x4c), 16 frames to the sphere, `magi` +0x40 frames of
    /// growth and `magi` +0x44 of hold, then the attack is finished.
    Magnet { yielded: bool, attack: Option<usize>, t: f32, age: u32, extra: Option<usize> },
    /// Reflect (REFLECT_*.mag, kh2fm-rs magic-turn.md "Reflect"): after a yield the guard follows the
    /// barrier while it lives (`life` ticks, the barrier element's particle-data life); when it ends
    /// the clashes pick the counter-burst: tier 1 / 2 / 3 by the count against `magi` +0x64 / +0x70,
    /// attack 574 / 576 / 578 (radius and height `magi` +0x5c / +0x60, +0x68 / +0x6c, +0x74 / +0x78) for
    /// `magi` +0x54 frames, then 667 / 668 / 669 20 larger for `magi` +0x58 frames; no clash, no burst.
    Reflect { yielded: bool, life: f32, burst: Option<(usize, u8, f32, bool)> },
    Fire { attacks: Option<(usize, Option<usize>)>, t: f32, yielded: bool },
    Blizzard { yielded: bool },
    Thunder { plan: Option<VecDeque<(f32, Vec3, f32)>>, wait: f32 },
    Done,
}

struct Cast {
    kind: u32,
    magi: MagiRow,
    step: Step,
}

/// A spell attack's hit on a victim (the caller applies damage and reaction).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpellHit {
    pub victim: usize,
    pub slot: u8,
    pub row: usize,
    pub dir: Vec3,
    /// The attack's group (the object a pulled victim follows, `Spells::attack_at`).
    pub group: u32,
}

/// The running spells of one caster (kh2-rs `MagicRun`'s spell part).
pub struct Spells {
    casts: Vec<Cast>,
    shots: Vec<Shot>,
    strikes: Vec<Strike>,
    pub attacks: Vec<SpellAttack>,
    next_group: u32,
    bliz_counter: u32,
    /// Hit memory per victim id.
    memory: HashMap<u32, HitMemory>,
    /// Reflect's guard while the barrier lives: centre (cm) and radius; the clashes it took.
    pub guard: Option<(Vec3, f32)>,
    pub clashes: u32,
    /// The barrier's life in ticks (`set_barrier_life`, from the spell file's element).
    pub barrier_life: f32,
}

#[allow(clippy::too_many_arguments)]
fn new_attack(attacks: &mut Vec<SpellAttack>, next: &mut u32, atkp: &[Atkp], id: u16, kind: u32, radius: f32, height: f32, slot: u8) -> Option<usize> {
    let base_row = rules::attack_row(atkp, id, kind)?;
    *next += 1;
    let height = if height > 0.0 { height } else { radius };
    attacks.push(SpellAttack { group: *next, base_row, pos: None, prev: None, radius, height, live: true, hit_any: false, wall: false, slot });
    Some(attacks.len() - 1)
}

fn move_attack(a: &mut SpellAttack, p: Vec3) {
    a.prev = a.pos.or(Some(p));
    a.pos = Some(p);
}

/// What one tick reads of the caster and the world.
pub struct Caster {
    pub pos: Vec3,
    pub facing: Vec3,
    pub target: Option<Vec3>,
    pub grounded: bool,
}

impl Default for Spells {
    fn default() -> Spells {
        Spells { casts: Vec::new(), shots: Vec::new(), strikes: Vec::new(), attacks: Vec::new(), next_group: 0, bliz_counter: 0, memory: HashMap::new(), guard: None, clashes: 0, barrier_life: REFLECT_BARRIER_LIFE }
    }
}

impl Spells {
    /// Event 21 of a cast whose trigger fired (`slot` 0 Fire, 1 Blizzard, 2 Thunder).
    pub fn fire(&mut self, slot: u8, kind: u32, magi: MagiRow) {
        let step = match slot {
            0 => Step::Fire { attacks: None, t: magi.f(MAGI_TIME), yielded: false },
            1 => Step::Blizzard { yielded: false },
            2 => Step::Thunder { plan: None, wait: 0.0 },
            4 => Step::Magnet { yielded: false, attack: None, t: 16.0 + magi.f(0x40).ceil() + magi.f(0x44), age: 0, extra: None },
            5 => {
                self.guard = None;
                self.clashes = 0;
                Step::Reflect { yielded: false, life: self.barrier_life, burst: None }
            }
            _ => return,
        };
        self.casts.push(Cast { kind, magi, step });
    }

    /// Reflect's guard as an attack volume (it hits no object; enemy attacks that touch it clash).
    pub fn guard_volume(&self) -> Option<Volume> {
        self.guard.map(|(c, r)| attack_volume(c, r, r))
    }

    /// The barrier's life (ticks) from the Reflect file's barrier element (see `REFLECT_BARRIER_LIFE`).
    pub fn set_barrier_life(&mut self, ticks: f32) {
        self.barrier_life = ticks;
    }

    /// The position of the live attack of `group` (None once it is finished).
    pub fn attack_at(&self, group: u32) -> Option<Vec3> {
        self.attacks.iter().find(|a| a.live && a.group == group).and_then(|a| a.pos)
    }

    pub fn active(&self) -> bool {
        !self.casts.is_empty() || !self.shots.is_empty() || !self.strikes.is_empty() || self.attacks.iter().any(|a| a.live)
    }

    /// One tick: the casts' steps, the shots and strikes, then the hit tests against `victims`
    /// (id, hurt volumes, target point, position). Returns the hits, and Blizzard's push of the
    /// caster (direction, power) when one starts.
    pub fn tick(&mut self, c: &Caster, atkp: &[Atkp], world: &kh2_game_core::world::World, victims: &[(u32, Vec<Volume>, Vec3, Vec3)]) -> (Vec<SpellHit>, Option<(Vec3, f32)>) {
        let mut push = None;
        let chest = c.pos + Vec3::Y * CHEST;
        let (attacks, next) = (&mut self.attacks, &mut self.next_group);
        for cast in self.casts.iter_mut() {
            match &mut cast.step {
                Step::Fire { attacks: at, t, yielded } => {
                    if !*yielded {
                        *yielded = true;
                        continue;
                    }
                    let (a1, a2) = match *at {
                        Some(a) => a,
                        None => {
                            let a1 = new_attack(attacks, next, atkp, ATKP_FIRE, cast.kind, cast.magi.f(MAGI_RADIUS), cast.magi.f(MAGI_HEIGHT), 0);
                            let mut a2 = None;
                            if cast.kind / 10 == 3 {
                                a2 = new_attack(attacks, next, atkp, ATKP_FIRE, cast.kind, cast.magi.f(MAGI_RADIUS) - 30.0, cast.magi.f(MAGI_HEIGHT) - 30.0, 0);
                                if cast.kind & 1 == 1
                                    && let Some(i) = a1
                                {
                                    attacks[i].radius = 450.0;
                                }
                            }
                            let Some(a1) = a1 else {
                                cast.step = Step::Done;
                                continue;
                            };
                            *at = Some((a1, a2));
                            (a1, a2)
                        }
                    };
                    if *t > 0.0 {
                        move_attack(&mut attacks[a1], chest);
                        if let Some(i2) = a2 {
                            move_attack(&mut attacks[i2], chest);
                        }
                        *t -= 1.0;
                    } else {
                        attacks[a1].live = false;
                        if let Some(i2) = a2 {
                            attacks[i2].live = false;
                        }
                        cast.step = Step::Done;
                    }
                }
                Step::Blizzard { yielded } => {
                    if !*yielded {
                        *yielded = true;
                        continue;
                    }
                    push = Some((-c.facing, cast.magi.f(MAGI_PUSH)));
                    let (shots, counts) = blizzard_shots(cast.kind, self.bliz_counter);
                    if counts {
                        self.bliz_counter += 1;
                    }
                    for sh in shots {
                        let dir = rotate_y_game(c.facing.with_y(-sh.y), sh.angle);
                        self.shots.push(Shot {
                            kind: cast.kind,
                            magi: cast.magi.clone(),
                            pos: chest + c.facing * 50.0,
                            origin: chest,
                            dir,
                            target: c.target,
                            attack: None,
                            life: cast.magi.f(MAGI_LIFE) - SHOT_FADE as f32,
                            fade: None,
                            done: false,
                            turn_scale: sh.scale,
                            flag: sh.flag,
                        });
                    }
                    cast.step = Step::Done;
                }
                Step::Thunder { plan, wait } => {
                    let plan = plan.get_or_insert_with(|| thunder_plan(cast.kind, c.pos, c.facing, c.target, cast.magi.f(MAGI_THUNDER_RADIUS)).into());
                    if *wait > 0.0 {
                        *wait -= 1.0;
                        if *wait > 0.0 {
                            continue;
                        }
                    }
                    match plan.pop_front() {
                        Some((w, pos, rise)) => {
                            self.strikes.push(Strike { kind: cast.kind, magi: cast.magi.clone(), pos, rise, atkp: 0, started: false, wait: 0.0, attacks: None, done: false });
                            *wait = w;
                            if plan.is_empty() {
                                cast.step = Step::Done;
                            }
                        }
                        None => cast.step = Step::Done,
                    }
                }
                Step::Magnet { yielded, attack, t, age, extra } => {
                    if !*yielded {
                        *yielded = true;
                        continue;
                    }
                    let ai = match *attack {
                        Some(a) => a,
                        None => {
                            // The place: the target's position, else in front of the caster [decided: the
                            // script's fallback is element 1's position, near the caster]; host 1.141 puts it
                            // `magi` +0x3c above the ground found within twice that.
                            let up = cast.magi.f(0x3c);
                            let mut at = c.target.unwrap_or(c.pos + c.facing * 300.0);
                            if let Some(g) = world.ground(at, at.y + up, 2.0 * up) {
                                at.y = g.y + up;
                            }
                            let Some(a) = new_attack(attacks, next, atkp, ATKP_MAGNET, cast.kind, cast.magi.f(0x4c), 0.0, 4) else {
                                cast.step = Step::Done;
                                continue;
                            };
                            move_attack(&mut attacks[a], at);
                            *attack = Some(a);
                            a
                        }
                    };
                    let p = attacks[ai].pos.unwrap_or(c.pos);
                    move_attack(&mut attacks[ai], p);
                    if let Some(x) = *extra {
                        move_attack(&mut attacks[x], p);
                    }
                    *age += 1;
                    // 16 frames after the first attack, in Wisdom, Master and Final (kind / 10 = 1, 2, 3) a
                    // second attack 976 of radius `magi` +0x3c * 0.5 at the same place.
                    if *age == 16
                        && (1..=3).contains(&(cast.kind / 10))
                        && let Some(x) = new_attack(attacks, next, atkp, ATKP_MAGNET_FORM, cast.kind, cast.magi.f(0x3c) * 0.5, 0.0, 4)
                    {
                        move_attack(&mut attacks[x], p);
                        *extra = Some(x);
                    }
                    *t -= 1.0;
                    if *t <= 0.0 {
                        attacks[ai].live = false;
                        if let Some(x) = *extra {
                            attacks[x].live = false;
                        }
                        // Final adds attack 1440 (radius `magi` +0x3c * 0.6) for one frame at the end.
                        if cast.kind / 10 == 3
                            && let Some(x) = new_attack(attacks, next, atkp, ATKP_MAGNET_FINAL, cast.kind, cast.magi.f(0x3c) * 0.6, 0.0, 4)
                        {
                            move_attack(&mut attacks[x], p);
                            *attack = Some(x);
                            *extra = None;
                            *t = 1.0;
                            cast.kind = 0;
                            continue;
                        }
                        cast.step = Step::Done;
                    }
                }
                Step::Reflect { yielded, life, burst } => {
                    if !*yielded {
                        *yielded = true;
                        continue;
                    }
                    match burst {
                        None if *life > 0.0 => {
                            self.guard = Some((chest, cast.magi.f(0x50)));
                            *life -= 1.0;
                        }
                        None => {
                            self.guard = None;
                            let n = self.clashes;
                            if n == 0 {
                                cast.step = Step::Done;
                                continue;
                            }
                            // +0x64 / +0x70 are integers (5 and 10 on the disc), not floats like the row's other fields.
                            let tier = if n >= cast.magi.f(0x70).to_bits() { 2 } else if n >= cast.magi.f(0x64).to_bits() { 1 } else { 0 };
                            let (r, h) = [(0x5c, 0x60), (0x68, 0x6c), (0x74, 0x78)][tier];
                            let Some(a) = new_attack(attacks, next, atkp, REFLECT_BURST[tier], cast.kind, cast.magi.f(r), cast.magi.f(h), 5) else {
                                cast.step = Step::Done;
                                continue;
                            };
                            move_attack(&mut attacks[a], chest);
                            *burst = Some((a, tier as u8, cast.magi.f(0x54), false));
                        }
                        Some((a, tier, t, second)) => {
                            move_attack(&mut attacks[*a], chest);
                            *t -= 1.0;
                            if *t > 0.0 {
                                continue;
                            }
                            attacks[*a].live = false;
                            if *second {
                                cast.step = Step::Done;
                                continue;
                            }
                            let k = *tier as usize;
                            let (r, h) = [(0x5c, 0x60), (0x68, 0x6c), (0x74, 0x78)][k];
                            match new_attack(attacks, next, atkp, REFLECT_BURST2[k], cast.kind, cast.magi.f(r) + 20.0, cast.magi.f(h) + 20.0, 5) {
                                Some(b) => {
                                    move_attack(&mut attacks[b], chest);
                                    *burst = Some((b, *tier, cast.magi.f(0x58), true));
                                }
                                None => cast.step = Step::Done,
                            }
                        }
                    }
                }
                Step::Done => {}
            }
        }
        self.casts.retain(|c| !matches!(c.step, Step::Done));
        // The strikes (kh2-rs `step_strike`).
        for k in self.strikes.iter_mut() {
            if !k.started {
                k.started = true;
                k.atkp = ATKP_THUNDER_AIR;
                if let Some(g) = world.ground(k.pos, k.pos.y + k.rise, k.rise + 10_000.0)
                    && (k.pos.y - g.y) - k.magi.f(MAGI_THUNDER_SNAP) < 0.0
                {
                    k.atkp = ATKP_THUNDER_GROUND;
                    k.pos.y = g.y;
                }
                k.wait = k.magi.f(MAGI_THUNDER_WAIT);
            }
            if k.wait > 0.0 {
                k.wait -= 1.0;
                continue;
            }
            match k.attacks {
                None => {
                    let Some(a1) = new_attack(attacks, next, atkp, k.atkp, k.kind, k.magi.f(MAGI_THUNDER_RADIUS), 0.0, 2) else {
                        k.done = true;
                        continue;
                    };
                    move_attack(&mut attacks[a1], k.pos);
                    let group = attacks[a1].group;
                    let Some(a2) = new_attack(attacks, next, atkp, ATKP_THUNDER_COLUMN, k.kind, k.magi.f(MAGI_THUNDER_COLUMN_R), k.magi.f(MAGI_THUNDER_COLUMN_H), 2) else {
                        k.done = true;
                        continue;
                    };
                    attacks[a2].group = group;
                    let h = attacks[a2].height;
                    move_attack(&mut attacks[a2], k.pos + Vec3::Y * h);
                    k.attacks = Some((a1, a2));
                    k.wait = k.magi.f(MAGI_THUNDER_TIME);
                    if k.wait > 0.0 {
                        k.wait -= 1.0;
                    }
                }
                Some((a1, a2)) => {
                    attacks[a1].live = false;
                    attacks[a2].live = false;
                    k.done = true;
                }
            }
        }
        self.strikes.retain(|k| !k.done);
        // The shots (kh2-rs `step_shot`).
        for shot in self.shots.iter_mut() {
            let Some(ai) = shot.attack else {
                let size = shot.magi.f(MAGI_SHOT_SIZE);
                shot.attack = new_attack(attacks, next, atkp, ATKP_BLIZZARD, shot.kind, size, 0.0, 1);
                match shot.attack {
                    Some(ai) => move_attack(&mut attacks[ai], shot.origin),
                    None => shot.done = true,
                }
                continue;
            };
            let rate = shot.magi.f(MAGI_TURN) * shot.turn_scale;
            if rate > 0.0
                && let Some(t) = shot.target
            {
                shot.dir = turn_toward(shot.dir, t - shot.pos, rate);
            }
            shot.pos += shot.dir.normalize_or_zero() * shot.magi.f(MAGI_SPEED);
            match shot.fade.as_mut() {
                Some(f) => *f = f.saturating_sub(1),
                None => {
                    shot.life -= 1.0;
                    if shot.life <= 0.0 {
                        shot.fade = Some(SHOT_FADE);
                    }
                }
            }
            let a = &attacks[ai];
            if shot.fade == Some(0) || shot_ends(a.wall, false, a.hit_any, shot.flag) {
                attacks[ai].live = false;
                shot.done = true;
                continue;
            }
            move_attack(&mut attacks[ai], shot.pos);
        }
        self.shots.retain(|s| !s.done);
        // The hit loop (kh2-rs `run`'s attacks against the enemies' hurt shapes).
        let mut hits = Vec::new();
        for a in self.attacks.iter_mut().filter(|a| a.live) {
            let Some(cur) = a.pos else { continue };
            let row = &atkp[a.base_row];
            let moved = a.prev.map_or(0.0, |p| (p - cur).length());
            if row.flags & 1 != 0
                && moved > SWEEP_MIN
                && !a.hit_any
                && !a.wall
                && let Some(prev) = a.prev
                && world.touching_m(prev * 0.01, cur * 0.01, 0.01, kh2_map::flags::ATTACK_COLLIDE).is_some()
            {
                a.wall = true;
            }
            for (vi, (id, shapes, _target, vpos)) in victims.iter().enumerate() {
                let mem = self.memory.entry(*id).or_default();
                if !mem.may_hit(row.r#type, a.group) {
                    continue;
                }
                let hit = sweep_points(a, cur).into_iter().any(|p| {
                    let v = attack_volume(p, a.radius, a.height);
                    shapes.iter().any(|h| overlap(&v, h))
                });
                if !hit {
                    continue;
                }
                let n = mem.count_hit(a.group);
                let vrow = rules::variant(atkp, a.base_row, n);
                mem.mark(a.group, atkp[vrow].interval);
                let dir = (*vpos - c.pos).with_y(0.0).normalize_or(Vec3::Z);
                hits.push(SpellHit { victim: vi, slot: a.slot, row: vrow, dir, group: a.group });
                a.hit_any = true;
            }
        }
        for m in self.memory.values_mut() {
            m.tick();
        }
        if self.casts.is_empty() && self.shots.is_empty() && self.strikes.is_empty() {
            self.attacks.retain(|a| a.live);
        }
        (hits, push)
    }

    /// The live spell attacks: (slot, atkp row index, centre, radius, height).
    pub fn live(&self) -> Vec<(u8, usize, Vec3, f32, f32)> {
        self.attacks.iter().filter(|a| a.live).filter_map(|a| Some((a.slot, a.base_row, a.pos?, a.radius, a.height))).collect()
    }
}
