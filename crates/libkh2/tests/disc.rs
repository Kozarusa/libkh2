//! Tests against the user's own disc. They run only with `KH2_ISO` set to the USA disc image and
//! print what they compare; without it they return at once.

use std::sync::Arc;
use std::time::Instant;

use glam::Vec3;
use kh2::data::Disc;
use kh2::sora::{Input, Sora, State};
use kh2::{world_from_surfaces, Surface, World};

fn disc() -> Option<Disc> {
    let iso = std::env::var_os("KH2_ISO")?;
    Some(Disc::open(std::path::Path::new(&iso)).expect("KH2_ISO opens"))
}

fn floor() -> World {
    let s = 20000.0;
    let t = |v| Surface { v, flags: 1, ..Default::default() };
    world_from_surfaces(&[t([[-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s]]), t([[-s, 0.0, -s], [s, 0.0, s], [s, 0.0, -s]])])
}

fn standing(game: &Arc<kh2::data::Game>, w: &World) -> Sora {
    let mut s = Sora::new(game.clone(), Vec3::ZERO);
    let g = w.ground(Vec3::ZERO, 40.0, 80.0).unwrap();
    s.motor.pos.y = g.y;
    s.motor.grounded = true;
    s.motor.ground = Some(g);
    s
}

#[test]
fn movement_matches_the_disc_values() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let p = &game.params;
    eprintln!("prty: walk {} run {} jump {} turn {}; sstm FallMax {}", p.prty.walk_speed, p.prty.run_speed, p.prty.jump_height, p.prty.turn_speed, p.sstm.fall_max);
    // The ground mover constants read from the executable equal the ones the controller uses.
    if let Some(exe) = d.exe() {
        assert_eq!(kh2_game_core::motor::GroundConsts::from_exe(&exe), Some(kh2_game_core::motor::GroundConsts::USA));
        assert!(p.swing.is_some(), "the attack turn constants are readable");
    }
    let w = floor();
    // Run: after the turn, one run speed per tick, and the run motion (id 2) plays.
    let mut s = standing(&game, &w);
    let run = Input { stick: [0.0, 1.0], cam_look: [0.0, 1.0], ..Default::default() };
    for _ in 0..20 {
        s.tick(&w, &run);
    }
    let a = s.motor.pos;
    s.tick(&w, &run);
    let step = (s.motor.pos - a).length();
    eprintln!("run step {step} (prty RunSpeed {}), motion {} ({})", p.prty.run_speed, s.motion_id().unwrap_or(99), s.body.tag());
    assert!((step - p.prty.run_speed).abs() < 1e-3);
    assert_eq!(s.motion_id(), Some(kh2_anim::id::RUN));
    // Jump held: the apex is prty JumpHeight.
    let mut s = standing(&game, &w);
    let mut top: f32 = 0.0;
    let hold = Input { jump: true, ..Default::default() };
    for _ in 0..150 {
        s.tick(&w, &hold);
        top = top.max(s.motor.pos.y);
    }
    eprintln!("jump apex {top} (prty JumpHeight {})", p.prty.jump_height);
    assert!((top - p.prty.jump_height).abs() < 0.5);
    assert_eq!(s.state, State::Ground);
    // High Jump level 1: the fmab row's height.
    let mut s = standing(&game, &w);
    s.growth.high_jump = 1;
    let mut top: f32 = 0.0;
    for _ in 0..200 {
        s.tick(&w, &hold);
        top = top.max(s.motor.pos.y);
    }
    let hj = p.fmab(1).unwrap().high_jump_height;
    eprintln!("high jump 1 apex {top} (fmab HighJumpHeight {hj})");
    assert!((top - hj).abs() < 0.5);
}

#[test]
fn the_ground_and_air_combos_play_the_attack_table() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let w = floor();
    let mut s = standing(&game, &w);
    let tick = |s: &mut Sora, i: Input| s.tick(&w, &i);
    tick(&mut s, Input::default());
    let attack = Input { attack: true, ..Default::default() };
    // Ground: press, report a hit whenever an attack window is live, press again in each window.
    let mut tags = Vec::new();
    let mut windows_seen = 0;
    for t in 0..240 {
        let press = t % 2 == 0;
        tick(&mut s, if press { attack } else { Input::default() });
        if !s.attack_triggers().is_empty() {
            windows_seen += 1;
            s.connected = true;
        }
        let tag = s.body.tag().to_string();
        if matches!(s.state, State::Attack { .. }) && tags.last() != Some(&tag) {
            tags.push(tag);
        }
    }
    eprintln!("ground chain: {tags:?}, ticks with a live attack window {windows_seen}");
    assert!(tags.first().is_some_and(|t| t.starts_with("A30")), "the first ground attack is A30x: {tags:?}");
    assert!(tags.len() >= 3, "the chain goes on with hits: {tags:?}");
    assert!(windows_seen > 0);
    // Attack volumes exist while a window is live (keyblade shapes on the hand).
    let mut s = standing(&game, &w);
    tick(&mut s, Input::default());
    tick(&mut s, attack);
    let mut volumes = 0;
    for _ in 0..40 {
        tick(&mut s, Input::default());
        volumes = volumes.max(s.attack_volumes().len());
    }
    eprintln!("largest number of live attack volumes in the first swing: {volumes}");
    assert!(volumes > 0);
    // Air: jump, then attack at the top.
    let mut s = standing(&game, &w);
    tick(&mut s, Input { jump: true, ..Default::default() });
    for _ in 0..20 {
        tick(&mut s, Input { jump: true, ..Default::default() });
    }
    let mut air_tags = Vec::new();
    for t in 0..120 {
        tick(&mut s, if t % 2 == 0 { attack } else { Input::default() });
        if !s.attack_triggers().is_empty() {
            s.connected = true;
        }
        let tag = s.body.tag().to_string();
        if matches!(s.state, State::Attack { air: true, .. }) && air_tags.last() != Some(&tag) {
            air_tags.push(tag);
        }
    }
    eprintln!("air chain: {air_tags:?}");
    assert!(air_tags.first().is_some_and(|t| t.starts_with("A33")) && air_tags.len() >= 2, "{air_tags:?}");
}

#[test]
fn tick_time() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let w = floor();
    for pose in [false, true] {
        let mut s = standing(&game, &w);
        s.pose = pose;
        let n = 3000;
        let t0 = Instant::now();
        for t in 0..n {
            let i = Input { stick: [0.3, 1.0], cam_look: [0.0, 1.0], attack: t % 7 == 0, jump: t % 97 < 20, ..Default::default() };
            s.tick(&w, &i);
        }
        let us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
        eprintln!("tick time with pose {pose}: {us:.1} us per tick ({n} ticks)");
    }
}

#[test]
fn an_area_collision_loads_and_sora_stands_on_it() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let coct = d.coct("tt00").expect("tt00 collision");
    let w = World::from_coct(&coct);
    let start = Vec3::new(115.0, 0.0, -35.0);
    let g = w.ground(start, 100.0, 400.0).expect("a floor under the tt00 start");
    let mut s = Sora::new(game, start.with_y(g.y));
    s.motor.grounded = true;
    for _ in 0..120 {
        s.tick(&w, &Input { stick: [0.0, 1.0], cam_look: [0.0, -1.0], ..Default::default() });
    }
    eprintln!("tt00: {} triangles; after 120 ticks of running -z: {:?} grounded {}", w.tris.len(), s.motor.pos, s.motor.grounded);
    assert!(s.motor.grounded);
    assert!(s.motor.pos.z > -1000.0 - 50.0, "the box wall stopped him: {:?}", s.motor.pos);
}

#[test]
fn growth_abilities_use_the_fmab_rows() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let w = floor();
    let f = game.params.fmab(1).unwrap().clone();
    // Quick Run: Square while moving starts at AirSlideSpeed along the stick, invincible for
    // AirSlideStarTime, braking by AirSlideBrake per tick.
    let mut s = standing(&game, &w);
    s.growth.quick_run = 1;
    let run = Input { stick: [0.0, 1.0], cam_look: [0.0, 1.0], ..Default::default() };
    for _ in 0..10 {
        s.tick(&w, &run);
    }
    let a = s.motor.pos;
    s.tick(&w, &Input { guard: true, ..run });
    assert_eq!(s.state, State::QuickRun);
    let step = (s.motor.pos - a).length();
    eprintln!("quick run first step {step}, AirSlideSpeed {} brake {} star {}", f.air_slide_speed, f.air_slide_brake, f.air_slide_star_time);
    assert!((step - f.air_slide_speed * f.air_slide_brake).abs() < 1e-2 || (step - f.air_slide_speed).abs() < 1e-2);
    assert_eq!(s.invuln > 0.0, f.air_slide_star_time > 1.0);
    for _ in 0..200 {
        s.tick(&w, &run);
    }
    assert_eq!(s.state, State::Ground, "released: the dash ends");
    // Aerial Dodge: a second jump of AirDodgeHeight from the height where it is pressed.
    let mut s = standing(&game, &w);
    s.growth.aerial_dodge = 1;
    let hold = Input { jump: true, ..Default::default() };
    for _ in 0..30 {
        s.tick(&w, &hold);
    }
    s.tick(&w, &Input::default());
    let y0 = s.motor.pos.y;
    let mut top = y0;
    for _ in 0..100 {
        s.tick(&w, &hold);
        top = top.max(s.motor.pos.y);
    }
    eprintln!("aerial dodge from {y0}: apex {top} (AirDodgeHeight {})", f.air_dodge_height);
    assert!((top - y0 - f.air_dodge_height).abs() < 2.0);
    // Glide: from a high fall, Square glides; the sink per tick is at most GlideFallMax.
    let mut s = standing(&game, &w);
    s.growth.glide = 1;
    s.motor.pos.y = 2000.0;
    s.motor.grounded = false;
    s.state = State::Air;
    s.motor.start_fall();
    for _ in 0..20 {
        s.tick(&w, &Input::default());
    }
    s.tick(&w, &Input { guard: true, ..run });
    s.tick(&w, &Input { guard: true, ..run });
    assert_eq!(s.state, State::Glide);
    let y = s.motor.pos.y;
    s.tick(&w, &Input { guard: true, ..run });
    eprintln!("glide sink {} (GlideFallMax {})", y - s.motor.pos.y, f.glide_fall_max);
    assert!(y - s.motor.pos.y <= f.glide_fall_max + 1e-3);
}

#[test]
fn hits_flinch_blow_knock_out_and_guard() {
    use kh2::sora::HitResult;
    use kh2_game_core::reaction_rules::reaction_class;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    assert!(game.params.reaction.is_some(), "reaction data from the executable");
    let w = floor();
    let row = |class: u8| game.params.atkp.iter().find(|a| reaction_class(a.enemy_reaction) == class && a.sub_id >> 8 == 0).map(|a| a.id as i16).expect("a row of the class");
    let (flinch, blow) = (row(3), row(4));
    let back = Vec3::new(0.0, 0.0, -1.0); // Sora faces +z: a hit along -z comes from the front
    let run_out = |s: &mut Sora, n: usize| {
        let mut seen = Vec::new();
        for _ in 0..n {
            s.tick(&w, &Input::default());
            if seen.last() != Some(&s.state) {
                seen.push(s.state);
            }
        }
        seen
    };
    // Flinch, invincible for a while, back to the ground state.
    let mut s = standing(&game, &w);
    let hp = s.hp;
    assert_eq!(s.hit(flinch, 1, back), HitResult::Flinched);
    assert_eq!((s.state, s.hp), (State::Hurt, hp - 1));
    assert_eq!(s.hit(flinch, 1, back), HitResult::Ignored, "invulnerability frames");
    let seen = run_out(&mut s, 200);
    eprintln!("flinch (atkp {flinch}): {seen:?}");
    assert_eq!(s.state, State::Ground);
    // Blow: launched, lands, gets up.
    let mut s = standing(&game, &w);
    assert_eq!(s.hit(blow, 1, back), HitResult::Blown);
    let seen = run_out(&mut s, 300);
    eprintln!("blow (atkp {blow}): {seen:?}");
    assert!(seen.contains(&State::Blown) && seen.contains(&State::Hurt) && s.state == State::Ground);
    // A lethal hit always launches and knocks out; revive stands him up.
    let mut s = standing(&game, &w);
    s.hit(flinch, 10_000, back);
    run_out(&mut s, 300);
    assert_eq!((s.state, s.hp), (State::Down, 0));
    s.revive();
    run_out(&mut s, 300);
    assert_eq!((s.state, s.hp), (State::Ground, s.max_hp));
    // Guard from the front repels; an attack then plays the counter if the ability is set.
    let mut s = standing(&game, &w);
    s.tick(&w, &Input { guard: true, ..Default::default() });
    assert_eq!(s.state, State::Guard);
    assert_eq!(s.hit(blow, 1, back), HitResult::Guarded);
    assert_eq!(s.state, State::Counter);
}
