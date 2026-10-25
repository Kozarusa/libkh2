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

#[test]
fn the_field_camera_follows_collides_and_locks_on() {
    use kh2::camera::{CamInput, Camera};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let p = game.params.camera.clone().expect("camera constants from the executable");
    let row = p.rows[0];
    eprintln!("camera row 0: fov {} radius {} min {} max {}", row.fov, row.radius, row.min_radius, row.max_radius);
    // A floor and a wall 150 behind Sora (he faces +z; the wall faces +z, toward him).
    let s2 = 20000.0;
    let t = |v, flags| Surface { v, flags, ..Default::default() };
    let w = world_from_surfaces(&[
        t([[-s2, 0.0, -s2], [-s2, 0.0, s2], [s2, 0.0, s2]], 1),
        t([[-s2, 0.0, -s2], [s2, 0.0, s2], [s2, 0.0, -s2]], 1),
        t([[-s2, -100.0, -150.0], [s2, -100.0, -150.0], [s2, 3000.0, -150.0]], 0x10),
        t([[-s2, -100.0, -150.0], [s2, 3000.0, -150.0], [-s2, 3000.0, -150.0]], 0x10),
    ]);
    let mut s = standing(&game, &w);
    let mut cam = Camera::new(0);
    let inp = CamInput { auto: true, step: 1.0, ..Default::default() };
    let v = cam.tick(&p, &mut s, &w, &inp);
    eprintln!("first view: pos {:?} look {:?}", v.pos, v.look);
    assert!((v.look.y - kh2_game_core::camera_rules::LOOK_HEIGHT).abs() < 1.0);
    assert!(v.pos.z > -150.0, "kept in front of the wall behind Sora: {:?}", v.pos);
    assert!(v.pos.z < 0.0, "behind Sora");
    assert!((v.fov - row.fov).abs() < 1e-5);
    // Run forward: the camera follows within its radius range.
    for _ in 0..200 {
        s.tick(&w, &Input { stick: [0.0, 1.0], cam_look: [0.0, 1.0], ..Default::default() });
        cam.tick(&p, &mut s, &w, &inp);
    }
    let v = cam.tick(&p, &mut s, &w, &inp);
    let flat = (v.pos - s.motor.pos).with_y(0.0).length();
    eprintln!("after the run: horizontal distance {flat}");
    assert!(flat > row.min_radius * 0.5 && flat < row.max_radius * 1.5);
    // Lock-on to a point at Sora's right: the camera's view turns toward it.
    let target = s.motor.pos + Vec3::new(-600.0, 100.0, 0.0);
    assert_eq!(kh2::camera::lock_on_pick(&s, &[target + Vec3::new(0.0, 0.0, 1e6), target]), Some(1));
    s.target = Some(kh2::sora::Target { pos: target, radius: 30.0, half_height: 50.0 });
    let yaw0 = v.yaw;
    let mut v = v;
    for _ in 0..120 {
        s.tick(&w, &Input::default());
        v = cam.tick(&p, &mut s, &w, &inp);
    }
    eprintln!("lock-on: yaw {yaw0} -> {}", v.yaw);
    assert!(kh2_game_core::camera_rules::wrap(v.yaw - yaw0).abs() > 0.3);
}

#[test]
fn level_one_spells_cast_pay_and_charge() {
    use kh2::sora::Event;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let w = floor();
    for slot in 0..4u8 {
        let mut s = standing(&game, &w);
        let (row, cost) = s.spell(slot).expect("level 1 spell row");
        let mp0 = s.mp;
        s.tick(&w, &Input::default());
        s.cast_request = Some((slot, 30));
        let mut fired = None;
        let mut states = Vec::new();
        for _ in 0..400 {
            s.tick(&w, &Input::default());
            if states.last() != Some(&s.state) {
                states.push(s.state);
            }
            for e in s.events.drain(..) {
                if let Event::SpellFired { charge, .. } = e {
                    fired = Some((charge, s.mp));
                }
            }
        }
        eprintln!("slot {slot} {} (cost {cost}, motion {}): states {states:?}, fired {fired:?}, MP {mp0} -> {}", row.file_name, row.ground_motion, s.mp);
        let (charge, mp) = fired.expect("frame trigger 17 fired");
        if cost == kh2_game_core::magic_rules::COST_ALL {
            assert!(charge && mp == 0);
        } else {
            assert_eq!(mp, mp0 - cost as i32);
        }
        assert!(states.contains(&State::Magic) && s.state == State::Ground);
    }
    // Cure's charge refills MP at its end (prty MpDrive frames).
    let mut s = standing(&game, &w);
    s.cast_request = Some((3, 30));
    let mut restored = None;
    for t in 0..20000 {
        s.tick(&w, &Input::default());
        if s.events.drain(..).any(|e| e == Event::MpRestored) {
            restored = Some(t);
            break;
        }
    }
    eprintln!("MP charge ended after {restored:?} ticks (MpDrive {})", game.params.prty.mp_drive);
    assert!(restored.is_some() && s.mp == s.max_mp);
}

#[test]
fn drive_forms_enter_swap_and_revert() {
    use kh2::sora::Event;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = game.drive.clone().expect("drive tables");
    let w = floor();
    for form in 1..=4u8 {
        let mut s = standing(&game, &w);
        s.drive.points = kh2_game_core::drive_rules::Points { whole: 9, fraction: 0, max: 9 };
        s.drive.owned[form as usize] = true;
        let base_bones = s.body.pose.len();
        let mut load = |f: u8| d.form_assets(&data, f, 0).ok().map(Arc::new);
        let entered = s.drive_enter(form, &mut load).expect("enter");
        assert_eq!(entered, form, "out of battle there is no Anti roll");
        assert_eq!(s.state, State::FormChange);
        let mut changed_at = None;
        for t in 0..900 {
            s.tick(&w, &Input::default());
            if s.events.drain(..).any(|e| e == Event::FormChanged { from: 0, to: form }) {
                changed_at = Some(t);
            }
            if changed_at.is_some() && s.state == State::Ground {
                break;
            }
        }
        let files = s.drive.files.clone().expect("form files");
        let cost = data.form(form).unwrap().cost;
        eprintln!(
            "form {form}: {} ({} bones, base {base_bones}), object {}, hands {:?}, changed after {changed_at:?} ticks, points 9 -> {}, cost {cost}, timer {}/{}, growth {:?}",
            files.model, s.body.pose.len(), files.object, files.hands.iter().map(|h| h.as_ref().map(|x| x.1.clone())).collect::<Vec<_>>(), s.drive.points.whole, s.drive.timer.left, s.drive.timer.total, s.growth
        );
        assert!(changed_at.is_some() && s.drive.in_form && s.state == State::Ground);
        assert_eq!(s.drive.points.whole, 9 - cost);
        assert!(s.drive.timer.left < s.drive.timer.total, "the form timer drains");
        // The form's attack table plays.
        s.tick(&w, &Input { attack: true, ..Default::default() });
        s.tick(&w, &Input::default());
        eprintln!("form {form} attack: {}", s.body.tag());
        assert!(matches!(s.state, State::Attack { .. }) || form == 2, "{:?}", s.state);
        for _ in 0..120 {
            s.tick(&w, &Input::default());
        }
        assert!(s.drive_revert(true));
        assert!(!s.drive.in_form && s.body.rig.model.bones.len() == game.sora.model.bones.len());
    }
}

#[test]
fn any_motion_slot_samples_like_the_animator() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let rig = game.sora.clone();
    let slot = rig.mset.by_tag("A300").next().expect("A300");
    let m = rig.mset.get(slot).unwrap().motion.as_ref().unwrap();
    let mut a = kh2::anim::Anim::new(rig.clone());
    a.restart(Some(slot), false, 0.0);
    let mut worst: f32 = 0.0;
    for _ in 0..10 {
        a.step(true);
        let (p, _) = kh2::anim::sample(&rig, slot, a.frame).unwrap();
        worst = worst.max(kh2_anim::blend::max_bone_delta(&p, &a.pose));
    }
    let (_, r0) = kh2::anim::sample(&rig, slot, m.frame_start).unwrap();
    let (_, r1) = kh2::anim::sample(&rig, slot, m.frame_end).unwrap();
    eprintln!("A300 slot {slot}: frames {}..{} at {} fps, sample vs animator worst bone delta {worst}, root {:?} -> {:?}", m.frame_start, m.frame_end, m.fps, r0, r1);
    assert!(worst < 0.5, "{worst}");
}

/// The scripted recording on the disc: the trace digest is compared with `local/golden_disc.txt`
/// (outside git: the trace carries disc-derived numbers), written on the first run. Plus the values
/// kh2-rs's own tests expect for the same rules (`motor.rs` test: a 185 jump rises for 30.1 ticks).
#[test]
fn the_scripted_replay_is_stable_on_the_disc() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let rise = (2.0 * game.params.prty.jump_height / kh2_game_core::motor::G).sqrt();
    if game.params.prty.jump_height == 185.0 {
        assert!((rise - 30.1).abs() < 0.1, "kh2-rs motor test value: {rise}");
    }
    let s = 2000.0;
    let t = |v, flags| Surface { v, flags, ..Default::default() };
    let w = world_from_surfaces(&[
        t([[-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s]], 1),
        t([[-s, 0.0, -s], [s, 0.0, s], [s, 0.0, -s]], 1),
        t([[800.0, -100.0, -s], [800.0, -100.0, s], [800.0, 600.0, s]], kh2::SURF_WALL),
        t([[800.0, -100.0, -s], [800.0, 600.0, s], [800.0, 600.0, -s]], kh2::SURF_WALL),
    ]);
    let mut so = standing(&game, &w);
    let steps = kh2::replay::parse(include_str!("../../../tests/scripts/basic.txt")).unwrap();
    let trace = kh2::replay::run(&mut so, &w, &steps);
    let digest = format!("{:016x}", kh2::replay::digest(&trace));
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/golden_disc.txt");
    match std::fs::read_to_string(path) {
        Ok(g) => assert_eq!(g.trim(), digest, "the disc replay changed (delete local/golden_disc.txt after a deliberate change)"),
        Err(_) => {
            let _ = std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/../../local"));
            std::fs::write(path, &digest).unwrap();
            eprintln!("wrote {path}: {digest}");
        }
    }
}

#[test]
fn spells_report_attack_stages_with_disc_rows_and_damage() {
    use kh2::sora::Event;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let w = floor();
    for slot in 0..4u8 {
        let mut s = standing(&game, &w);
        s.hp = 5;
        eprintln!("level 1: strength {} magic {} defense {}", s.strength, s.magic, s.defense);
        s.cast_request = Some((slot, 30));
        let mut stages = Vec::new();
        let mut healed = None;
        for _ in 0..200 {
            s.tick(&w, &Input::default());
            for e in s.events.drain(..) {
                match e {
                    Event::SpellAttack { stage, atkp, sub_id, radius, height, life, .. } => stages.push((stage, atkp, sub_id, radius, height, life)),
                    Event::Healed { amount, hp } => healed = Some((amount, hp)),
                    _ => {}
                }
            }
        }
        let dmg: Vec<i32> = stages.iter().filter_map(|&(_, a, sub, ..)| s.damage(a, Some(sub), 1, 100, 100, 100)).collect();
        eprintln!("slot {slot}: stages {stages:?}, damage vs defense 1 {dmg:?}, healed {healed:?}");
        assert!(!stages.is_empty() && stages.iter().all(|x| x.3 > 0.0), "every stage has a size from magi");
        if slot == 3 {
            let (amount, hp) = healed.expect("Cure heals");
            assert!(amount > 0 && hp == (5 + amount).min(s.max_hp));
        } else {
            assert!(dmg.iter().all(|&x| x > 0));
        }
    }
}
