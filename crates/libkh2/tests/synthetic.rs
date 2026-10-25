//! Behaviour tests on synthetic data: made-up movement numbers, a one-bone skeleton with
//! placeholder motions, a hand-built attack table and host triangles. No disc needed. The numbers
//! here are test values, not the game's.

use std::sync::Arc;

use glam::{Mat4, Vec3};
use kh2::anim::Rig;
use kh2::data::{AttackLeaps, Game, Params};
use kh2::sora::{Input, Sora, State};
use kh2::{world_from_surfaces, Surface, World, SURF_WALL};
use kh2_anim::moveset::{Action, Moveset};
use kh2_anim::triggers::{range, RangeTrigger, Triggers};
use kh2_anim::{Mset, Slot};
use kh2_tables::tables::{PrefPlyr, PrefPrty, PrefSstm};

const WALK: f32 = 3.0;
const RUN: f32 = 10.0;
const JUMP: f32 = 150.0;
const FALL_MAX: f32 = 30.0;

/// Motion ids of the test attacks.
const ATTACK_A: u16 = 150;
const ATTACK_B: u16 = 151;
/// The test attacks' combo window (global frames) and their length.
const COMBO: (i16, i16) = (10, 20);
const ATTACK_LEN: f32 = 40.0;

fn motion(len: f32) -> kh2_anim::Motion {
    kh2_anim::Motion { frame_start: 0.0, frame_end: len, fps: 60.0, frame_return: 0.0, data: kh2_anim::motion::Data::Raw { bones: 1, frames: vec![vec![Mat4::IDENTITY]; len as usize + 1] } }
}

fn slot(tag: &str, len: f32, ranges: Vec<RangeTrigger>) -> Option<Slot> {
    Some(Slot { tag: tag.to_string(), motion: Some(motion(len)), triggers: Triggers { ranges, frames: Vec::new() } })
}

fn zeroed<T>() -> T
where
    T: kh2_tables::Record,
{
    T::parse(&mut kh2_tables::rt::Reader::new(&vec![0u8; T::SIZE]))
}

fn game() -> Arc<Game> {
    // Field variant without a weapon is relative slot 2 (`motion_state(false, _)`).
    let mut slots: Vec<Option<Slot>> = (0..(ATTACK_B as usize + 1) * 4).map(|_| None).collect();
    let at = |id: u16| id as usize * 4 + 2;
    for (id, tag) in [(0u16, "A000"), (1, "A001"), (2, "A002"), (3, "A003"), (4, "A004"), (5, "A005")] {
        slots[at(id)] = slot(tag, 30.0, Vec::new());
    }
    let window = |id: u8, (start, end): (i16, i16)| RangeTrigger { start, end, id, params: Vec::new() };
    let attack_ranges = || vec![window(range::ALLOW_COMBO, COMBO), window(range::ATTACK, (4, 12)), window(range::BLEND_TO_IDLE, (30, -1))];
    slots[at(ATTACK_A)] = slot("T150", ATTACK_LEN, attack_ranges());
    slots[at(ATTACK_B)] = slot("T151", ATTACK_LEN, attack_ranges());
    let bone = kh2_model::Bone { parent: -1, scale: Vec3::ONE, rotation: Vec3::ZERO, translation: Vec3::ZERO, length: 0.0 };
    let model = kh2_model::Model { bones: vec![bone], ..Default::default() };
    let action = |key: u8, group: u8, alt: i8, motion: u16| Action { key, group, alt, arg: 0, flags: 0, motion, next_motion: 0, distance: [0.0; 2], height: [0.0; 2], angle: [0.0; 2], ability: 0 };
    // Key 5 (the default ground attack) plays A; while A plays its alternative key 6 plays B.
    let moves = Moveset { actions: vec![action(5, 3, 6, ATTACK_A), action(6, 0, 5, ATTACK_B)] };
    let prty = PrefPrty { walk_speed: WALK, run_speed: RUN, jump_height: JUMP, turn_speed: 0.25, ..zeroed() };
    let sstm = PrefSstm { fall_max: FALL_MAX, lock_on_range: 1000.0, ..zeroed() };
    let plyr: PrefPlyr = zeroed();
    let params = Params { prty, plyr, sstm, fmab: Vec::new(), swing: None, reaction: None, atkp: Vec::new(), max_hp: 20, max_mp: 100, camera: None, magc: Vec::new(), cmd: Vec::new(), magi: Vec::new(), levels: Vec::new(), weapon_attack: 0, weapon_magic: 0 };
    Arc::new(Game { params, sora: Arc::new(Rig { model, mset: Mset { slots } }), keyblade: None, moves: Arc::new(moves), leaps: AttackLeaps::default(), drive: None, ability_rules: None })
}

fn quad(a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3], flags: u32) -> [Surface; 2] {
    let t = |v| Surface { v, flags, ..Default::default() };
    [t([a, b, c]), t([a, c, d])]
}

/// A 4000-unit floor at y = 0 (normal up) and, optionally, a wall across x = `wall` facing -x.
fn room(wall: Option<f32>) -> World {
    let s = 2000.0;
    let mut tris: Vec<Surface> = quad([-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s], [s, 0.0, -s], 1).to_vec();
    if let Some(x) = wall {
        tris.extend(quad([x, -100.0, -s], [x, -100.0, s], [x, 600.0, s], [x, 600.0, -s], SURF_WALL));
    }
    world_from_surfaces(&tris)
}

fn sora(world: &World) -> Sora {
    let mut s = Sora::new(game(), Vec3::ZERO);
    s.pose = false;
    let g = world.ground(Vec3::ZERO, 40.0, 80.0).expect("floor");
    s.motor.pos.y = g.y;
    s.motor.grounded = true;
    s.motor.ground = Some(g);
    s
}

/// Stick fully forward with the camera looking along +x.
fn run_x() -> Input {
    Input { stick: [0.0, 1.0], cam_look: [1.0, 0.0], ..Default::default() }
}

#[test]
fn the_floor_normal_points_up_and_the_wall_is_solid() {
    let w = room(Some(500.0));
    assert_eq!(w.tris.len(), 4);
    assert!(w.ground(Vec3::new(10.0, 0.0, 10.0), 50.0, 100.0).is_some_and(|g| g.y.abs() < 1e-4 && g.normal.y > 0.99));
    assert!(w.tris.iter().filter(|t| t.n.y.abs() < 0.5).all(|t| t.n.x < -0.99), "the wall faces -x");
}

#[test]
fn sora_runs_at_run_speed_and_stops_at_the_wall() {
    let w = room(Some(500.0));
    let mut s = sora(&w);
    // The facing turns at the turn speed first; once facing +x the step is the run speed per tick.
    let mut last = s.motor.pos;
    let mut steps = Vec::new();
    for _ in 0..30 {
        s.tick(&w, &run_x());
        steps.push((s.motor.pos - last).length());
        last = s.motor.pos;
    }
    assert!(steps.iter().all(|d| (d - RUN).abs() < 1e-3), "{steps:?}");
    assert!((s.motor.yaw - std::f32::consts::FRAC_PI_2).abs() < 1e-4, "facing +x after turning: {}", s.motor.yaw);
    assert_eq!(s.state, State::Ground);
    for _ in 0..200 {
        s.tick(&w, &run_x());
    }
    // Stopped one capsule radius (35) in front of the wall, still on the floor.
    let x = s.motor.pos.x;
    assert!(x < 500.0 - 35.0 + 0.1 && x > 500.0 - 35.0 - 1.0, "x = {x}");
    assert!(s.motor.grounded && s.motor.pos.y.abs() < 1e-3);
    // Without the wall flag the same triangles let him through (the party wall filter).
    let s2 = 2000.0;
    let mut tris: Vec<Surface> = quad([-s2, 0.0, -s2], [-s2, 0.0, s2], [s2, 0.0, s2], [s2, 0.0, -s2], 1).to_vec();
    tris.extend(quad([500.0, -100.0, -s2], [500.0, -100.0, s2], [500.0, 600.0, s2], [500.0, 600.0, -s2], 0x82));
    let open = world_from_surfaces(&tris);
    let mut s = sora(&open);
    for _ in 0..100 {
        s.tick(&open, &run_x());
    }
    assert!(s.motor.pos.x > 600.0, "an attack-only wall does not stop Sora: {}", s.motor.pos.x);
}

#[test]
fn a_half_stick_walks() {
    let w = room(None);
    let mut s = sora(&w);
    let input = Input { stick: [0.0, 0.5], cam_look: [0.0, 1.0], ..Default::default() };
    s.tick(&w, &input);
    let a = s.motor.pos;
    s.tick(&w, &input);
    assert!(((s.motor.pos - a).length() - WALK).abs() < 1e-3);
    // Stick right with the camera along +z moves toward -x (right-handed, Y up).
    let mut s = sora(&w);
    for _ in 0..20 {
        s.tick(&w, &Input { stick: [1.0, 0.0], cam_look: [0.0, 1.0], ..Default::default() });
    }
    assert!(s.motor.pos.x < -100.0 && s.motor.pos.z.abs() < 1e-3, "{:?}", s.motor.pos);
}

#[test]
fn a_held_jump_peaks_at_the_jump_height_and_lands() {
    let w = room(None);
    let mut s = sora(&w);
    let hold = Input { jump: true, ..Default::default() };
    let mut top: f32 = 0.0;
    let mut landed_at = None;
    for t in 0..200 {
        s.tick(&w, &hold);
        top = top.max(s.motor.pos.y);
        if t > 5 && s.motor.grounded && landed_at.is_none() {
            landed_at = Some(t);
        }
    }
    assert!((top - JUMP).abs() < 0.5, "apex {top}");
    let t = landed_at.expect("landed");
    // Rise sqrt(2h/g) ticks, then the same parabola down (capped at FallMax per tick).
    let rise = (2.0 * JUMP / kh2_game_core::motor::G).sqrt();
    assert!((t as f32) > 2.0 * rise - 3.0 && (t as f32) < 2.0 * rise + 6.0, "landed after {t} ticks, rise {rise}");
    assert_eq!(s.state, State::Ground);
}

#[test]
fn an_early_release_hops_short() {
    let w = room(None);
    let mut s = sora(&w);
    let mut top: f32 = 0.0;
    for t in 0..120 {
        s.tick(&w, &Input { jump: t < 8, ..Default::default() });
        top = top.max(s.motor.pos.y);
    }
    assert!(top > 0.0 && top < JUMP * 0.75, "short hop apex {top}");
}

#[test]
fn walking_off_a_ledge_falls_with_the_fall_cap() {
    // A floor ending at x = 100, a lower floor 2000 below.
    let s2 = 2000.0;
    let mut tris: Vec<Surface> = quad([-s2, 0.0, -s2], [-s2, 0.0, s2], [100.0, 0.0, s2], [100.0, 0.0, -s2], 1).to_vec();
    tris.extend(quad([-s2, -2000.0, -s2], [-s2, -2000.0, 3.0 * s2], [3.0 * s2, -2000.0, 3.0 * s2], [3.0 * s2, -2000.0, -s2], 1));
    let w = world_from_surfaces(&tris);
    let mut s = sora(&w);
    let mut worst: f32 = 0.0;
    let mut was_air = false;
    for _ in 0..300 {
        let y = s.motor.pos.y;
        s.tick(&w, &run_x());
        was_air |= s.state == State::Air;
        worst = worst.max(y - s.motor.pos.y);
    }
    assert!(was_air);
    assert!((worst - FALL_MAX).abs() < 1e-3, "largest drop per tick {worst}");
    assert!(s.motor.grounded && (s.motor.pos.y + 2000.0).abs() < 1e-3);
}

fn press(s: &mut Sora, w: &World) {
    s.tick(w, &Input { attack: true, ..Default::default() });
    s.tick(w, &Input::default());
}

#[test]
fn the_combo_advances_only_inside_the_window_after_a_hit() {
    let w = room(None);
    let mut s = sora(&w);
    s.tick(&w, &Input::default());
    press(&mut s, &w);
    assert!(matches!(s.state, State::Attack { air: false, .. }));
    assert_eq!(s.motion_id(), Some(ATTACK_A as usize));
    assert_eq!(s.hits, 1);
    // The attack window of the motion (range trigger 10) is reported with the motion's frames.
    assert!(s.attack_triggers().is_empty() || s.body.gframe() >= 4.0);
    // A press before the combo window opens is held, not started; the hit lands.
    s.connected = true;
    press(&mut s, &w);
    assert!(s.body.gframe() < COMBO.0 as f32);
    assert_eq!(s.motion_id(), Some(ATTACK_A as usize), "no chain before the window");
    assert!(s.queued_attack);
    // The held press starts the next attack on the first tick the window covers.
    let mut started_at = None;
    for _ in 0..40 {
        let before = s.body.gframe();
        s.tick(&w, &Input::default());
        if s.motion_id() == Some(ATTACK_B as usize) {
            started_at = Some(before);
            break;
        }
    }
    let f = started_at.expect("the chain went on");
    assert!(f + 1.0 >= COMBO.0 as f32 && f <= COMBO.1 as f32, "started at global frame {f}");
    assert_eq!(s.hits, 2);
}

#[test]
fn without_a_hit_the_window_does_not_chain() {
    let w = room(None);
    let mut s = sora(&w);
    s.tick(&w, &Input::default());
    press(&mut s, &w);
    // Wait into the combo window, press without a landed hit: refused.
    while s.body.gframe() < (COMBO.0 + 2) as f32 {
        s.tick(&w, &Input::default());
    }
    press(&mut s, &w);
    assert_eq!(s.motion_id(), Some(ATTACK_A as usize));
    assert_eq!(s.hits, 1);
    // After the motion ends Sora is back on the ground state.
    for _ in 0..60 {
        s.tick(&w, &Input::default());
    }
    assert_eq!(s.state, State::Ground);
}

#[test]
fn hidden_groups_are_not_solid() {
    let s2 = 2000.0;
    let mut tris: Vec<Surface> = quad([-s2, 0.0, -s2], [-s2, 0.0, s2], [s2, 0.0, s2], [s2, 0.0, -s2], 1).to_vec();
    let mut wall = quad([300.0, -100.0, -s2], [300.0, -100.0, s2], [300.0, 600.0, s2], [300.0, 600.0, -s2], SURF_WALL);
    for t in &mut wall {
        t.group = 7;
    }
    tris.extend(wall);
    let mut w = world_from_surfaces(&tris);
    let mut s = sora(&w);
    for _ in 0..80 {
        s.tick(&w, &run_x());
    }
    assert!(s.motor.pos.x < 300.0, "group 7 on: the wall stops him at {}", s.motor.pos.x);
    w.set_group(7, false);
    let mut s = sora(&w);
    for _ in 0..80 {
        s.tick(&w, &run_x());
    }
    assert!(s.motor.pos.x > 400.0, "{}", s.motor.pos.x);
}

/// The scripted recording on the synthetic data against the committed trace (no game values in it).
/// `KH2_BLESS=1` rewrites the golden file.
#[test]
fn the_scripted_replay_matches_the_golden_trace() {
    let w = room(Some(800.0));
    let mut s = sora(&w);
    let steps = kh2::replay::parse(include_str!("../../../tests/scripts/basic.txt")).unwrap();
    let trace = kh2::replay::run(&mut s, &w, &steps);
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/synthetic_basic.trace");
    if std::env::var_os("KH2_BLESS").is_some() {
        std::fs::write(path, &trace).unwrap();
    }
    let golden = std::fs::read_to_string(path).expect("golden trace (run with KH2_BLESS=1 once)").replace("\r\n", "\n");
    if golden != trace {
        let first = golden.lines().zip(trace.lines()).position(|(a, b)| a != b);
        panic!("trace differs from the golden file at line {first:?}");
    }
}
