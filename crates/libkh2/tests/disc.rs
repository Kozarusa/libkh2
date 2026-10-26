//! Tests against the user's own disc. They run only with `KH2_ISO` set to the USA disc image and
//! print what they compare; without it they return at once.

use std::sync::Arc;
use std::time::Instant;

use glam::Vec3;
use kh2::data::Disc;
use kh2::sora::{Input, Sora, State};
use kh2::{world_from_surfaces, Surface, World};

/// The compiled kh2-rs script hosts keep a few process-wide tables (the shared memory blocks, the
/// group edits, the area mask, the group enemy sum), as the game's globals are; tests that run
/// scripts at the same time disturb each other through them. Each disc test holds this lock for
/// its whole run (the guard lives in the test thread's local storage and goes with the thread).
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

thread_local! {
    static HELD: std::cell::RefCell<Option<std::sync::MutexGuard<'static, ()>>> = const { std::cell::RefCell::new(None) };
}

fn disc() -> Option<Disc> {
    let iso = std::env::var_os("KH2_ISO")?;
    HELD.with(|h| {
        if h.borrow().is_none() {
            *h.borrow_mut() = Some(SERIAL.lock().unwrap_or_else(|e| e.into_inner()));
        }
    });
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
        let mut load = |f: u8| d.form_assets(&data, f, 0, 41).ok().map(Arc::new);
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

#[test]
fn valor_master_final_carry_a_second_keyblade_on_the_left_hand() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = game.drive.clone().unwrap();
    assert!(data.keyblades().contains(&41));
    let w = floor();
    for form in 1..=4u8 {
        let a = d.form_assets(&data, form, 0, 41).expect("form assets");
        eprintln!("form {form}: hands {:?}", a.files.hands);
        assert_eq!(a.weapon2.is_some(), form != 2, "the second blade of form {form}");
        let mut s = standing(&game, &w);
        s.drive.points.whole = 9;
        s.drive.points.max = 9;
        s.drive.owned[form as usize] = true;
        let arc = Arc::new(a);
        s.drive_enter(form, &mut |_| Some(arc.clone())).unwrap();
        for _ in 0..400 {
            s.tick(&w, &Input { attack: s.timer as i32 % 4 == 0, ..Default::default() });
        }
        if let Some(w2) = &s.weapon2 {
            let hand = s.weapon2_matrix().w_axis.truncate();
            eprintln!("form {form}: second blade {} bones, motion {}, left hand at {hand:?}", w2.pose.len(), w2.tag());
            assert!(w2.pose.len() > 1 && hand.is_finite());
        }
    }
}

#[test]
fn the_loadout_sets_combo_lengths_growth_and_survival() {
    use kh2_game_core::abilities::id;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let rules = game.ability_rules.clone().expect("ability tables");
    let w = floor();
    let block = |ids: &[u16]| {
        let mut s = standing(&game, &w);
        assert!(s.set_loadout(ids));
        (s.abilities.max_ground, s.abilities.max_air, s.abilities.max_finishers, s.growth)
    };
    let base = block(&[id::GUARD]);
    let plus = block(&[id::GUARD, id::COMBO_PLUS, id::AIR_COMBO_PLUS, id::FINISHING_PLUS]);
    let neg = block(&[id::GUARD, id::NEGATIVE_COMBO]);
    eprintln!("base {base:?}, plus {plus:?}, negative {neg:?}");
    assert_eq!((plus.0, plus.1, plus.2), (base.0 + 1, base.1 + 1, base.2 + 1));
    assert!(neg.0 < base.0);
    // Growth abilities in the list set the growth levels (High Jump level 1 = the first growth ability).
    let hj = rules.growth_ability(0, 1).expect("High Jump LV1 item");
    let g = block(&[hj]).3;
    eprintln!("High Jump item {hj}: growth {g:?}");
    assert_eq!(g.high_jump, 1);
    // Guard is needed for the guard.
    let mut s = standing(&game, &w);
    s.set_loadout(&[]);
    s.tick(&w, &Input { guard: true, ..Default::default() });
    assert_ne!(s.state, State::Guard, "no Guard ability: no guard");
    // Second Chance keeps 1 HP on a lethal hit.
    let sc = rules.id_of_bit(kh2_game_core::abilities::BIT_SECOND_CHANCE).expect("Second Chance item");
    let mut s = standing(&game, &w);
    s.set_loadout(&[id::GUARD, sc]);
    let row = game.params.atkp.iter().find(|a| a.sub_id >> 8 == 0).unwrap().id as i16;
    s.hit(row, 10_000, Vec3::new(0.0, 0.0, -1.0));
    assert_eq!(s.hp, 1);
    // Berserk Charge raises strength rows' damage while the MP charge runs.
    let mut s = standing(&game, &w);
    s.set_loadout(&[id::GUARD, id::BERSERK_CHARGE]);
    let strength_row = game.params.atkp.iter().filter(|a| a.critical_adjust == 0 && a.r#type == 0).max_by_key(|a| a.power).unwrap();
    let before = s.damage(strength_row.id, Some(strength_row.sub_id), 0, 100, 100, 100).unwrap();
    s.charge = kh2_game_core::magic_rules::Charge { left: 100.0, total: 100.0 };
    let during = s.damage(strength_row.id, Some(strength_row.sub_id), 0, 100, 100, 100).unwrap();
    eprintln!("Berserk Charge: atkp {} damage {before} -> {during} while charging", strength_row.id);
    assert!(during > before);
}

#[test]
fn items_heal_through_the_use_motion_and_defeats_raise_the_level() {
    use kh2::sora::Event;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let w = floor();
    let potion = game.params.items.iter().find(|i| i.id == 1).expect("item 1");
    let amount = potion.flag1 as i32 | (potion.rank as i32) << 8;
    let mut s = standing(&game, &w);
    s.hp = 2;
    s.tick(&w, &Input::default());
    assert!(s.use_item(1));
    let mut used = None;
    for t in 0..200 {
        s.tick(&w, &Input::default());
        for e in s.events.drain(..) {
            if let Event::ItemUsed { hp, .. } = e {
                used = Some((t, hp));
            }
        }
    }
    let expect = ((s.max_hp * amount * 100 + 9999) / 10000).min(s.max_hp - 2);
    eprintln!("Potion (amount {amount}): used {used:?}, expected +{expect}, HP {}/{}", s.hp, s.max_hp);
    assert_eq!(used.map(|u| u.1), Some(expect));
    assert!(used.unwrap().0 > 30, "the effect waits for the trigger and 30 frames");
    // Ether during the MP charge shortens the charge.
    let mut s = standing(&game, &w);
    s.charge = kh2_game_core::magic_rules::Charge { left: 3000.0, total: 3000.0 };
    s.mp = 0;
    assert!(s.apply_item(3));
    assert!(s.charge.left < 3000.0 || s.mp == s.max_mp);
    // Defeats: EXP and level-ups from the lvup table.
    let mut s = standing(&game, &w);
    let need = game.params.levels[0].exp;
    let mut paid = 0;
    while s.level == 1 && paid < 100_000 {
        paid += s.report_defeat(100, 10);
    }
    let up = s.events.iter().any(|e| matches!(e, Event::LevelUp { level: 2, .. }));
    eprintln!("level 2 after {paid} EXP (lvup row 1 needs {need}): strength {} magic {} defense {}", s.strength, s.magic, s.defense);
    assert!(up && paid >= need && s.level == 2);
}

#[test]
fn donald_and_goofy_follow_sora() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let party = d.party().expect("party data");
    let w = floor();
    let mut s = standing(&game, &w);
    let mut rng = kh2::party::PartyRng::default();
    let mut f = vec![kh2::party::Follower::new(&party, 0, Vec3::new(-200.0, 0.0, -100.0)), kh2::party::Follower::new(&party, 1, Vec3::new(200.0, 0.0, -100.0))];
    for m in &party.members {
        eprintln!("{}: walk {} run {} turn {} radius {}", m.model, m.walk, m.run, m.turn, m.radius);
    }
    let run = Input { stick: [0.0, 1.0], cam_look: [0.0, 1.0], ..Default::default() };
    for t in 0..400 {
        s.tick(&w, &if t < 300 { run } else { Input::default() });
        kh2::party::tick(&party, &mut f, &mut rng, &mut s, &w);
    }
    for x in &f {
        let dist = x.motor.pos.distance(s.motor.pos);
        eprintln!("member {}: at {:?}, {dist:.0} from Sora ({:?}), motion {}", x.member, x.motor.pos, s.motor.pos, x.body.tag());
        assert!(dist < 600.0, "follows");
        assert!(x.body.pose.len() > 1);
    }
}

#[test]
fn twilight_town_enemies_run_their_scripts_fight_and_die() {
    use kh2::enemy::{EnemyEvent, Enemies};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    eprintln!("{} enemy kinds loadable", data.load_all().len());
    let w = floor();
    for kind in ["M_EX020", "M_EX010", "M_EX990"] {
        let k = data.kind(kind).expect(kind);
        let mut s = standing(&game, &w);
        s.max_hp = 999;
        s.hp = 999;
        let mut en = Enemies::new(data.clone());
        let id = en.spawn(kind, Vec3::new(0.0, 0.0, 400.0), std::f32::consts::PI, &w).unwrap();
        let start = en.list[0].pos();
        let mut hit_sora = 0;
        for _ in 0..900 {
            s.tick(&w, &Input::default());
            en.tick(&mut s, &w, game.params.sstm.fall_max);
            for e in en.events.drain(..) {
                if matches!(e, EnemyEvent::HitSora { .. }) {
                    hit_sora += 1;
                }
            }
        }
        let moved = en.list.first().map_or(0.0, |e| e.pos().distance(start));
        eprintln!("{kind} ({}, enmp {}, hp {}): script {}, moved {moved:.0}, hit Sora {hit_sora} times, Sora HP {}, unserved host calls {:?}", k.kind.name, k.kind.enmp, en.list[0].stats.max_hp, k.prog.is_some(), s.hp, en.unserved_calls());
        assert!(k.prog.is_some());
        assert!(moved > 10.0 || hit_sora > 0, "{kind} acts");
        // Sora fights back: face it and attack until it falls.
        let mut defeated = None;
        let mut hits = 0;
        for t in 0..3000 {
            let Some(e) = en.list.iter().find(|e| e.id == id) else { break };
            let to = e.pos() - s.motor.pos;
            s.target = Some(kh2::sora::Target { pos: e.pos() + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
            let stick = if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] };
            s.tick(&w, &Input { stick: [stick[0], stick[1]], cam_look: [0.0, 1.0], attack: t % 4 == 0, ..Default::default() });
            en.tick(&mut s, &w, game.params.sstm.fall_max);
            for ev in en.events.drain(..) {
                match ev {
                    EnemyEvent::Hit { .. } => hits += 1,
                    EnemyEvent::Defeated { paid, .. } => defeated = Some((t, paid)),
                    _ => {}
                }
            }
            if defeated.is_some() {
                break;
            }
        }
        eprintln!("{kind}: Sora landed {hits} hits, defeated {defeated:?}, Sora EXP {}", s.exp);
        assert!(hits > 0, "{kind}: Sora's attacks reach it");
        assert!(defeated.is_some(), "{kind} is defeated");
    }
}

#[test]
fn spells_fly_and_hit_enemies() {
    use kh2::enemy::{EnemyEvent, Enemies};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let w = floor();
    // (slot, enemy places): Fire around Sora, Blizzard at one far off to the side (homing), Thunder on a target.
    for (slot, places) in [(0u8, vec![Vec3::new(0.0, 0.0, 120.0)]), (1, vec![Vec3::new(300.0, 0.0, 700.0)]), (2, vec![Vec3::new(0.0, 0.0, 600.0)])] {
        let mut s = standing(&game, &w);
        s.max_hp = 999;
        s.hp = 999;
        let mut en = Enemies::new(data.clone());
        for p in &places {
            en.spawn("M_EX990", *p, std::f32::consts::PI, &w);
        }
        // Freeze the Dusks' scripts so they stay where they are put.
        for e in en.list.iter_mut() {
            e.rt = None;
        }
        let first_hp = en.list[0].hp;
        s.target = Some(kh2::sora::Target { pos: places[0] + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
        s.tick(&w, &Input::default());
        s.cast_request = Some((slot, 30));
        let mut hits = Vec::new();
        for _ in 0..240 {
            s.tick(&w, &Input::default());
            en.tick(&mut s, &w, game.params.sstm.fall_max);
            for ev in en.events.drain(..) {
                if let EnemyEvent::Hit { atkp, damage, .. } = ev {
                    hits.push((atkp, damage));
                }
            }
        }
        eprintln!("slot {slot}: hits {hits:?}, first enemy HP {first_hp} -> {:?}", en.list.first().map(|e| e.hp));
        let want = match slot {
            0 => 502,
            1 => 248,
            _ => 250,
        };
        assert!(hits.iter().any(|h| h.0 == want), "slot {slot}: atkp {want} hits");
    }
}

#[test]
fn donald_and_goofy_fight_an_enemy() {
    use kh2::enemy::{EnemyEvent, Enemies};
    use kh2::party::{tick_battle, Follower};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let party = d.party().expect("party data");
    let w = floor();
    for (k, m) in party.members.iter().enumerate() {
        eprintln!("member {k} {}: class {}, weapon {}, records {:?}", m.model, m.class, m.weapon.is_some(), party.ai.get(&m.class).map(|a| a.records.len()));
    }
    let mut s = standing(&game, &w);
    s.max_hp = 999;
    s.hp = 999;
    let mut en = Enemies::new(data.clone());
    en.spawn("M_EX990", Vec3::new(0.0, 0.0, 500.0), std::f32::consts::PI, &w);
    for e in en.list.iter_mut() {
        e.rt = None;
        e.hp = 100000;
    }
    let mut rng = kh2::party::PartyRng::default();
    let mut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, -50.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, -50.0))];
    let mut hits = Vec::new();
    for _ in 0..1200 {
        s.tick(&w, &Input::default());
        tick_battle(&party, &mut fs, &mut rng, &mut s, &w, Some(&mut en));
        en.tick(&mut s, &w, game.params.sstm.fall_max);
        for ev in en.events.drain(..) {
            if let EnemyEvent::Hit { atkp, damage, .. } = ev {
                hits.push((atkp, damage));
            }
        }
    }
    eprintln!("party hits {hits:?}; members at {:?}", fs.iter().map(|f| f.motor.pos).collect::<Vec<_>>());
    assert!(!hits.is_empty(), "the members hit the enemy");
}

#[test]
fn a_reaction_command_is_offered_accepted_and_delivered() {
    use kh2::enemy::{EnemyEvent, Enemies, Status};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let w = floor();
    // The motions of the kinds the library spawns offer nothing (as kh2-rs's scan found).
    let offering: usize = data.load_all().iter().filter_map(|k| (0..k.rig.mset.slots.len()).filter_map(|s| k.rig.mset.get(s)).map(|s| s.triggers.ranges.iter().filter(|r| r.id == 20).count()).sum::<usize>().checked_sub(0)).sum();
    eprintln!("{} cmd rows; trigger 20 ranges in the loaded kinds: {offering}", data.cmds.len());
    // A generic reaction row (Execute equal to its id, a DistRange, a use counter).
    for c in data.cmds.iter().filter(|c| c.reaction_count != 0).take(8) {
        eprintln!("cmd {} execute {} flags {:#x} dist {} counter {} action {}", c.id, c.execute, c.flags, c.dist_range, c.reaction_count, c.cmd_action);
    }
    let row = data.cmds.iter().find(|c| c.id == c.execute && c.reaction_count != 0 && c.flags >> 16 & 1 == 0 && c.flags >> 5 & 1 == 0 && c.execute != 0x51 && c.execute != 0x7f && ![1, 2, 4, 5, 0x37, 0x48, 0x67, 0x68, 0x13c, 0x14a, 0x1c4].contains(&c.execute)).expect("a reaction row").clone();
    let mut s = standing(&game, &w);
    let mut en = Enemies::new(data.clone());
    let id = en.spawn("M_EX990", Vec3::new(0.0, 0.0, 150.0), std::f32::consts::PI, &w).unwrap();
    en.fake_offer = Some(row.id);
    s.tick(&w, &Input::default());
    en.tick(&mut s, &w, game.params.sstm.fall_max);
    let o = en.offer.expect("offered");
    eprintln!("row {} (execute {}, dist {}, counter {}): offer {o:?}", row.id, row.execute, row.dist_range, row.reaction_count);
    assert_eq!((o.cmd, o.from, o.status), (row.id, id, Status::Available));
    assert_eq!(en.accept(&mut s), Some(row.id));
    assert_eq!(en.uses.get(&row.reaction_count), Some(&1));
    s.tick(&w, &Input::default());
    en.tick(&mut s, &w, game.params.sstm.fall_max);
    let got: Vec<_> = en.events.drain(..).filter(|e| matches!(e, EnemyEvent::Reaction { .. })).collect();
    eprintln!("delivered {got:?}");
    assert!(matches!(got.as_slice(), [EnemyEvent::Reaction { id: i, cmd, .. }] if *i == id && *cmd == row.id));
    // A row with a DistRange, offered from beyond it, is refused.
    if let Some(far) = data.cmds.iter().find(|c| c.id == c.execute && c.dist_range > 0 && c.flags >> 5 & 1 == 0 && c.flags >> 16 & 1 == 0 && c.execute != 0x51) {
        en.fake_offer = Some(far.id);
        en.list[0].obj.motor.pos.z = far.dist_range as f32 + 400.0;
        s.tick(&w, &Input::default());
        en.tick(&mut s, &w, game.params.sstm.fall_max);
        eprintln!("far row {}: {:?}", far.id, en.offer);
        assert_eq!(en.offer.map(|o| o.status), Some(Status::TooFar));
        assert_eq!(en.accept(&mut s), None);
    }
}

#[test]
fn level_ups_give_the_lvup_rows_abilities_and_the_record_saves() {
    use kh2::sora::Event;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let w = floor();
    for set in 0..3u8 {
        let mut s = standing(&game, &w);
        s.weapon_set = set;
        // Enough for many levels.
        for _ in 0..40 {
            s.report_defeat(1000, 1000);
        }
        let ups: Vec<_> = s.events.drain(..).filter_map(|e| if let Event::LevelUp { level, strength, magic, defense, ap, ability } = e { Some((level, strength, magic, defense, ap, ability)) } else { None }).collect();
        eprintln!("set {set}: level {} exp {}, {} level-ups, abilities {:?}", s.level, s.exp, ups.len(), s.granted);
        assert!(ups.len() >= 2);
        assert_eq!(ups.last().unwrap().0, s.level);
        let rows = &game.params.levels;
        for &(level, _, _, _, _, ability) in &ups {
            let r = &rows[level as usize - 1];
            assert_eq!(ability, [r.sword_ability, r.shield_ability, r.staff_ability][set as usize]);
        }
        assert_eq!(s.granted, ups.iter().filter(|u| u.5 != 0).map(|u| u.5).collect::<Vec<_>>());
        // The record round-trips.
        s.hp = 7;
        s.motor.pos = Vec3::new(10.0, 0.0, -20.0);
        let rec = s.save();
        let mut t = standing(&game, &w);
        t.load(&rec).expect("loads");
        assert_eq!((t.level, t.exp, t.hp, t.max_hp, t.weapon_set, t.strength, t.ap), (s.level, s.exp, s.hp, s.max_hp, s.weapon_set, s.strength, s.ap));
        assert_eq!(t.granted, s.granted);
        assert_eq!(t.motor.pos, s.motor.pos);
        assert_eq!(t.save(), rec);
        // Broken records change nothing.
        let before = t.save();
        for cut in [0, 3, 6, rec.len() - 1] {
            assert!(t.load(&rec[..cut]).is_err());
        }
        let mut bad = rec.clone();
        bad[6] = 200; // level 200
        assert!(t.load(&bad).is_err());
        assert_eq!(t.save(), before);
    }
}

#[test]
fn members_take_hits_fall_revive_use_items_and_fight_in_every_style() {
    use kh2::enemy::Enemies;
    use kh2::party::{tick_battle, Follower, MemberEvent, PartyRng};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let party = d.party().expect("party data");
    let w = floor();
    for m in &party.members {
        eprintln!("{}: neo {} HP {} MP {} base {:?} lvup rows {} stun {} status at 1 {:?} at 20 {:?}", m.model, m.neo, m.hp_max, m.mp_max, m.base, m.lvup.len(), m.stun, m.status(1), m.status(20));
        assert!(m.hp_max > 0 && !m.lvup.is_empty());
        if let Some(a) = party.ai.get(&m.class) {
            eprintln!("  class {}: battle {} range {} player_range {} player_near {} retarget {} half {}", m.class, a.params.battle, a.params.range, a.params.player_range, a.params.player_near, a.params.retarget, party.half);
        }
    }
    // Hits, KO and revive: Soldiers around the members, who have little HP.
    let mut s = standing(&game, &w);
    s.max_hp = 999;
    s.hp = 999;
    let mut en = Enemies::new(data.clone());
    for k in 0..4 {
        en.spawn("M_EX010", Vec3::new(-150.0 + 100.0 * k as f32, 0.0, 250.0), std::f32::consts::PI, &w);
    }
    for e in en.list.iter_mut() {
        e.hp = 100000;
    }
    let mut rng = PartyRng::default();
    let mut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 150.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, 150.0))];
    for f in fs.iter_mut() {
        f.hp = 3;
    }
    let (mut hits, mut kos, mut revives) = (0, 0, 0);
    for _ in 0..4000 {
        s.hp = 999;
        s.tick(&w, &Input::default());
        tick_battle(&party, &mut fs, &mut rng, &mut s, &w, Some(&mut en));
        en.tick(&mut s, &w, game.params.sstm.fall_max);
        en.events.clear();
        for f in fs.iter_mut() {
            for e in f.events.drain(..) {
                match e {
                    MemberEvent::Hit { .. } => hits += 1,
                    MemberEvent::KnockedOut => kos += 1,
                    MemberEvent::Revived { .. } => revives += 1,
                    _ => {}
                }
            }
        }
    }
    eprintln!("members: {hits} hits taken, {kos} knock-outs, {revives} revives; HP {:?}", fs.iter().map(|f| (f.hp, f.hp_max, f.ko)).collect::<Vec<_>>());
    assert!(hits > 0 && kos > 0 && revives > 0);
    // Items: Sora in danger, Donald holds Potions.
    let mut s = standing(&game, &w);
    s.hp = 2;
    let mut en = Enemies::new(data.clone());
    en.spawn("M_EX990", Vec3::new(0.0, 0.0, 400.0), std::f32::consts::PI, &w);
    for e in en.list.iter_mut() {
        e.rt = None;
    }
    let mut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0))];
    fs[0].items = vec![(1, 3)];
    // Cure (ability 168) not equipped: the item handler answers.
    fs[0].abilities.retain(|&a| a != 168);
    let mut used = Vec::new();
    for _ in 0..600 {
        s.tick(&w, &Input::default());
        tick_battle(&party, &mut fs, &mut rng, &mut s, &w, Some(&mut en));
        en.tick(&mut s, &w, game.params.sstm.fall_max);
        used.extend(fs[0].events.drain(..).filter(|e| matches!(e, MemberEvent::ItemUsed { .. })));
    }
    eprintln!("items used {used:?}; Sora HP {} / {}; stock {:?}", s.hp, s.max_hp, fs[0].items);
    assert!(used.contains(&MemberEvent::ItemUsed { item: 1, who: 0 }) && s.hp > 2);
    // Every style fights.
    for style in 0..4u8 {
        let mut s = standing(&game, &w);
        s.max_hp = 999;
        s.hp = 999;
        let mut en = Enemies::new(data.clone());
        // Style 2 takes only enemies whose surface is within the class's +0x84 (100) of Sora.
        let z = if style == 2 { 110.0 } else { 250.0 };
        en.spawn("M_EX990", Vec3::new(0.0, 0.0, z), std::f32::consts::PI, &w);
        for e in en.list.iter_mut() {
            e.rt = None;
            e.hp = 100000;
        }
        s.target = Some(kh2::sora::Target { pos: Vec3::new(0.0, 80.0, z), radius: 40.0, half_height: 80.0 });
        let mut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, 0.0))];
        for f in fs.iter_mut() {
            f.style = style;
        }
        let mut n = 0;
        for _ in 0..1200 {
            s.tick(&w, &Input::default());
            tick_battle(&party, &mut fs, &mut rng, &mut s, &w, Some(&mut en));
            en.tick(&mut s, &w, game.params.sstm.fall_max);
            n += en.events.drain(..).filter(|e| matches!(e, kh2::enemy::EnemyEvent::Hit { .. })).count();
        }
        eprintln!("style {style}: {n} hits");
        assert!(n > 0, "style {style} fights");
    }
}

#[test]
fn a_limit_with_donald_runs_its_script_and_counts_down() {
    use kh2::limit::{LimitEvent, Limits};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = d.limits().expect("limit tables");
    let party = d.party().expect("party data");
    let w = floor();
    for r in data.rows.iter().take(12) {
        eprintln!("limt {}: members {:?} command {} item {} world {} cost {} time {} file {}", r.id, r.members, r.command, r.item, r.world, r.cost, r.time, r.file);
    }
    let mut s = standing(&game, &w);
    let fs = vec![kh2::party::Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0)), kh2::party::Follower::new(&party, 1, Vec3::new(100.0, 0.0, 0.0))];
    let present = kh2::limit::present(&fs, &party);
    let mut lim = Limits::default();
    let st = lim.status(&data, &s, &present);
    eprintln!("present {present:?}; status {st:?}");
    // Without a party Donald's limit (282, Comet) is refused; with him it is available.
    let alone = lim.status(&data, &s, &[]);
    assert!(alone.iter().any(|(c, r)| *c == 282 && r.is_err()));
    let cmd = 282;
    assert!(st.iter().any(|(c, r)| *c == cmd && r.is_ok()));
    let mp = s.mp;
    lim.start(&data, &mut d, &mut s, &present, cmd).expect("starts");
    assert!(s.mp < mp, "MP spent");
    assert!(lim.status(&data, &s, &present).iter().all(|(_, r)| r.is_err()), "busy while it runs");
    let mut events = Vec::new();
    let mut ticks = 0;
    while lim.running() && ticks < 20000 {
        s.tick(&w, &Input::default());
        lim.tick(&s);
        events.append(&mut lim.events);
        ticks += 1;
    }
    for _ in 0..40 {
        lim.tick(&s);
        events.append(&mut lim.events);
    }
    let motions = events.iter().filter(|e| matches!(e, LimitEvent::Motion { .. })).count();
    let effects = events.iter().filter(|e| matches!(e, LimitEvent::Effect { .. } | LimitEvent::Named(_))).count();
    eprintln!("limit {cmd}: {ticks} ticks, {motions} motions, {effects} effects; events {:?}", events.iter().filter(|e| !matches!(e, LimitEvent::Motion { .. } | LimitEvent::Effect { .. })).collect::<Vec<_>>());
    assert!(matches!(events.first(), Some(LimitEvent::Started { .. })));
    assert!(events.iter().any(|e| matches!(e, LimitEvent::Ended { .. })));
    assert!(motions + effects > 0, "the limit's script asks for motions or effects");
}

#[test]
fn a_soldier_offers_its_reaction_command_and_its_script_moves_sora() {
    use kh2::enemy::{EnemyEvent, Enemies, Status};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let w = floor();
    let with_parts: Vec<&str> = data.load_all().iter().filter(|k| k.parts.as_ref().is_some_and(|t| t.reaction_parts().next().is_some())).map(|k| k.kind.model).collect();
    let with_paired: Vec<&str> = data.load_all().iter().filter(|k| !k.paired.is_empty()).map(|k| k.kind.model).collect();
    eprintln!("kinds with reaction parts {with_parts:?}; with paired motions {with_paired:?}; Sora's body part {}", data.player_parts.is_some());
    let mut s = standing(&game, &w);
    s.max_hp = 999;
    s.hp = 999;
    let mut en = Enemies::new(data.clone());
    let id = en.spawn("M_EX010", Vec3::new(0.0, 0.0, 120.0), std::f32::consts::PI, &w).unwrap();
    let (mut offered, mut accepted, mut delivered, mut reaction_ticks, mut moved) = (None, false, None, 0, 0.0f32);
    for t in 0..3000 {
        // Keep facing the Soldier, attack now and then so it gets stunned.
        let e = en.list.iter().find(|e| e.id == id).map(|e| e.pos());
        if let Some(p) = e {
            s.target = Some(kh2::sora::Target { pos: p + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
        }
        let before = s.motor.pos;
        let accept_now = en.offer.is_some_and(|o| o.status == Status::Available) && !accepted;
        if accept_now {
            accepted = en.accept(&mut s).is_some();
        }
        s.tick(&w, &Input { attack: !accepted && t % 8 == 0, cam_look: [0.0, 1.0], ..Default::default() });
        en.tick(&mut s, &w, game.params.sstm.fall_max);
        if s.state == kh2::sora::State::Reaction {
            reaction_ticks += 1;
            moved += (s.motor.pos - before).length();
        }
        if offered.is_none()
            && let Some(o) = en.offer
        {
            offered = Some((t, o));
        }
        for ev in en.events.drain(..) {
            if let EnemyEvent::Reaction { cmd, handled, .. } = ev {
                delivered = Some((cmd, handled));
            }
        }
        if accepted && reaction_ticks > 0 && s.state != kh2::sora::State::Reaction {
            break;
        }
    }
    eprintln!("first offer {offered:?}; accepted {accepted}; delivered {delivered:?}; Sora in the paired motion {reaction_ticks} ticks, moved {moved:.0}");
    assert!(offered.is_some_and(|(_, o)| o.cmd == 34 && o.from == id), "the Soldier's parts offer command 34");
    assert!(accepted && delivered.is_some_and(|(c, h)| c == 34 && h));
    assert!(reaction_ticks > 0, "the script's paired motion plays on Sora");
}

#[test]
fn bonus_levels_member_levels_and_form_ability_lists() {
    use kh2::party::{raise_levels, Follower, MemberEvent};
    use kh2::sora::Event;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let party = d.party().expect("party data");
    let w = floor();
    let bons = &game.params.bons;
    eprintln!("{} bons rows; first {:?}", bons.len(), bons.iter().take(4).map(|b| (b.reward_id, b.character_id, b.hp_increase, b.mp_increase, b.drive_gauge_upgrade, b.bonus_item1, b.bonus_item2)).collect::<Vec<_>>());
    let row = bons.iter().find(|b| b.character_id == 1 && (b.hp_increase > 0 || b.mp_increase > 0)).expect("a Sora row with HP or MP");
    let mut s = standing(&game, &w);
    let (hp0, mp0) = (s.max_hp, s.max_mp);
    let others = s.give_bonus(u16::from(row.reward_id)).expect("given");
    assert_eq!((s.max_hp, s.max_mp), (hp0 + i32::from(row.hp_increase), mp0 + i32::from(row.mp_increase)));
    assert!(s.events.iter().any(|e| matches!(e, Event::Bonus { .. })));
    assert!(s.give_bonus(u16::from(row.reward_id)).is_none(), "once per id");
    eprintln!("bonus {}: Sora HP {hp0}->{} MP {mp0}->{}; other rows {others:?}", row.reward_id, s.max_hp, s.max_mp);
    let rec = s.save();
    let mut t = standing(&game, &w);
    t.load(&rec).unwrap();
    assert_eq!(t.bonus_given, s.bonus_given);
    // The members' levels follow the shared experience.
    let mut fs = vec![Follower::new(&party, 0, Vec3::ZERO), Follower::new(&party, 1, Vec3::ZERO)];
    raise_levels(&party, &mut fs, 5000);
    let ups: Vec<_> = fs.iter().map(|f| (f.level, f.events.iter().filter(|e| matches!(e, MemberEvent::LevelUp { .. })).count())).collect();
    eprintln!("members at 5000 EXP: {ups:?}");
    assert!(fs.iter().all(|f| f.level > 1));
    // Form ability lists.
    for form in 1..=5u8 {
        eprintln!("form {form} abilities {:?}", s.form_ability_list(form));
    }
    assert!(!s.form_ability_list(1).is_empty());
}

/// Every enemy kind kh2-rs has a kind file for: spawn it, let it act for 10 s while Sora stands by,
/// then let Sora fight it; print one line per kind (the table of docs/ENEMY-KINDS.md). Fails only on
/// a kind that loads but neither acts nor can be hit.
#[test]
fn every_enemy_kind_runs() {
    use kh2::enemy::{EnemyEvent, Enemies};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let w = floor();
    let mut models: Vec<&str> = kh2_enemy_host::KINDS.iter().map(|k| k.model).collect();
    models.sort();
    models.dedup();
    let mut table = Vec::new();
    let mut dead = Vec::new();
    for model in models {
        let Some(k) = data.kind(model) else {
            table.push(format!("| {model} | - | not loadable | | | | | |"));
            continue;
        };
        let mut s = standing(&game, &w);
        s.max_hp = 9999;
        s.hp = 9999;
        let mut en = Enemies::new(data.clone());
        let Some(id) = en.spawn(model, Vec3::new(0.0, 0.0, 300.0), std::f32::consts::PI, &w) else { continue };
        let start = en.list[0].pos();
        let (mut hit_sora, mut moved, mut hits, mut defeated, mut motions) = (0, 0.0f32, 0, false, std::collections::HashSet::new());
        for t in 0..1800 {
            let Some(e) = en.list.iter().find(|e| e.id == id) else { break };
            moved = moved.max(e.pos().distance(start));
            motions.insert(e.anim.slot);
            let to = e.pos() - s.motor.pos;
            let fight = t >= 600;
            if fight {
                s.target = Some(kh2::sora::Target { pos: e.pos() + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
            }
            let stick = if fight && to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] };
            s.hp = 9999;
            s.tick(&w, &Input { stick, cam_look: [0.0, 1.0], attack: fight && t % 4 == 0, ..Default::default() });
            en.tick(&mut s, &w, game.params.sstm.fall_max);
            for ev in en.events.drain(..) {
                match ev {
                    EnemyEvent::HitSora { .. } => hit_sora += 1,
                    EnemyEvent::Hit { .. } => hits += 1,
                    EnemyEvent::Defeated { .. } => defeated = true,
                    _ => {}
                }
            }
            if defeated {
                break;
            }
        }
        let unserved: u64 = en.unserved_calls().iter().map(|c| c.2).sum();
        let acts = moved > 10.0 || hit_sora > 0 || motions.len() > 2;
        if !acts && hits == 0 {
            dead.push(model);
        }
        table.push(format!("| {model} | {} | {} | {:.0} | {} | {} | {} | {} |", k.kind.name, if k.prog.is_some() { "yes" } else { "no" }, moved, motions.len(), hit_sora, if defeated { "yes".to_string() } else { format!("no ({hits} hits)") }, unserved));
    }
    eprintln!("| model | kind | script | moved cm | motions | hit Sora | defeated by Sora | unserved host calls |");
    eprintln!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for l in &table {
        eprintln!("{l}");
    }
    eprintln!("inert: {dead:?}");
}

/// The areas' own collision: Sora is put on each area's spawn point and walks.
#[test]
fn areas_load_and_sora_walks_on_them() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let mut ok = 0;
    for area in ["tt00", "tt01", "tt07", "hb00", "hb05", "al00", "bb00", "mu00", "lk00", "po00", "eh00", "nm00", "ca00", "tr00", "lm00", "dc00", "wi00", "es00", "zz00"] {
        let Ok(c) = d.coct(area) else {
            eprintln!("{area}: no collision");
            continue;
        };
        let w = World::from_coct(&c);
        let at = w.spawn_point();
        let mut s = Sora::new(game.clone(), at);
        for _ in 0..240 {
            s.tick(&w, &Input { stick: [0.0, 1.0], cam_look: [0.0, 1.0], ..Default::default() });
        }
        let walked = s.motor.pos.with_y(0.0).distance(at.with_y(0.0));
        eprintln!("{area}: {} triangles, spawn {at:?}, walked {walked:.0} cm, grounded {}", w.tris.len(), s.motor.grounded);
        if walked > 50.0 {
            ok += 1;
        }
    }
    assert!(ok >= 3);
}

#[test]
fn member_scripts_cast_cure_and_specials() {
    use kh2::enemy::{EnemyEvent, Enemies};
    use kh2::party::{tick_battle, Follower, MemberEvent, PartyRng};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let party = d.party().expect("party data");
    let w = floor();
    for m in &party.members {
        let ai = party.ai.get(&m.class).unwrap();
        let specials: Vec<String> = ai.records.iter().filter(|r| !r.mode.is_empty()).map(|r| format!("{:?} {} item {}", r.handler, r.mode, r.item)).collect();
        eprintln!("{}: script {}, equipped {:?}; mode records {specials:?}", m.model, m.script.is_some(), m.abilities);
        assert!(m.script.is_some());
    }
    let mut s = standing(&game, &w);
    let mut en = Enemies::new(data.clone());
    en.spawn("M_EX990", Vec3::new(0.0, 0.0, 300.0), std::f32::consts::PI, &w);
    for e in en.list.iter_mut() {
        e.rt = None;
        e.hp = 100000;
    }
    let mut rng = PartyRng::default();
    let mut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, 0.0))];
    let (mut modes, mut heals, mut hits, mut effects) = (Vec::new(), Vec::new(), 0, 0);
    for t in 0..3000 {
        // Sora in danger now and then: Cure is wanted.
        if t % 600 == 0 {
            s.hp = 3;
        }
        s.tick(&w, &Input::default());
        tick_battle(&party, &mut fs, &mut rng, &mut s, &w, Some(&mut en));
        en.tick(&mut s, &w, game.params.sstm.fall_max);
        hits += en.events.drain(..).filter(|e| matches!(e, EnemyEvent::Hit { .. })).count();
        for (k, f) in fs.iter_mut().enumerate() {
            for e in f.events.drain(..) {
                match e {
                    MemberEvent::ScriptMode { mode } => modes.push((k, mode)),
                    MemberEvent::Healed { who, amount } => heals.push((k, who, amount)),
                    MemberEvent::Effect { .. } => effects += 1,
                    _ => {}
                }
            }
        }
    }
    let unserved: Vec<_> = fs.iter().map(|f| f.trace.missing()).collect();
    modes.dedup();
    eprintln!("modes {:?}", modes.iter().take(30).collect::<Vec<_>>());
    eprintln!("heals {heals:?}; enemy hits {hits}; effects {effects}; unserved host calls {unserved:?}; MP {:?}", fs.iter().map(|f| f.mp).collect::<Vec<_>>());
    assert!(modes.iter().any(|(_, m)| m != "idle"), "a special's mode runs");
    assert!(heals.iter().any(|h| h.1 == 0 && h.2 > 0), "Donald's Cure heals Sora");
}

#[test]
fn limit_motions_play_on_the_bodies_and_auto_limit_offers() {
    use kh2::limit::{LimitEvent, Limits};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = d.limits().expect("limit tables");
    let party = d.party().expect("party data");
    let w = floor();
    let mut s = standing(&game, &w);
    let mut fs = vec![kh2::party::Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0)), kh2::party::Follower::new(&party, 1, Vec3::new(100.0, 0.0, 0.0))];
    let present = kh2::limit::present(&fs, &party);
    // Auto Limit: nothing without the ability; Donald with it offers his limit.
    let mut lim = Limits::default();
    assert!(lim.auto_offer(&data, &s, &fs, &party, None).is_none());
    fs[0].abilities.push(417);
    let auto = lim.auto_offer(&data, &s, &fs, &party, None);
    eprintln!("auto limit offer with Donald's Auto Limit: {auto:?}");
    assert!(auto.is_some_and(|(c, _)| data.rows.iter().any(|r| r.command == c && r.members[0] == 2)));
    let mut best = 0;
    for cmd in [auto.unwrap().0, 344, 255, 338] {
    s.mp = s.max_mp;
    lim.start(&data, &mut d, &mut s, &present, cmd).expect("starts");
    eprintln!("{} LIMI sets", lim.sets.len());
    let (mut sora_held, mut member_held, mut motions) = (0, 0, 0);
    while lim.running() {
        s.tick(&w, &Input::default());
        lim.tick(&s);
        lim.play_motions(&mut s, &mut fs, &party);
        motions += lim.events.drain(..).filter(|e| matches!(e, LimitEvent::Motion { .. })).count();
        if s.state == kh2::sora::State::Reaction {
            sora_held += 1;
        }
        if fs.iter().any(|f| f.limit_hold) {
            member_held += 1;
        }
    }
    lim.play_motions(&mut s, &mut fs, &party);
    eprintln!("limit {cmd}: {motions} motion requests; Sora held {sora_held} ticks, a member held {member_held} ticks; after: Sora {:?}, held {:?}", s.state, fs.iter().map(|f| f.limit_hold).collect::<Vec<_>>());
    assert!(s.state != kh2::sora::State::Reaction && fs.iter().all(|f| !f.limit_hold), "released at the end");
    best = best.max(sora_held + member_held);
    }
    eprintln!("most ticks a body was held: {best}");
    assert!(best > 0, "a limit's motions play on Sora and a member");
}

#[test]
fn summons_arrive_run_their_scripts_and_leave() {
    use kh2::summon::{SummonEvent, SummonRefusal, Summons};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = d.summons().expect("summon tables");
    let edata = Arc::new(d.enemies().expect("enemy data"));
    let w = floor();
    eprintln!("classes: stitch {} genie {} follow {}", data.stitch.is_some(), data.genie.is_some(), data.follow.is_some());
    for r in &data.rows {
        eprintln!("  neo {:?}", data.objs.iter().find(|o| o.object_id == r.entity1).map(|o| (o.model_name.clone(), o.neo_status)));
        eprintln!("sumn: command {} item {} entity1 {} limit {}", r.command, r.item, r.entity1, r.limit_command);
    }
    for (k, row) in data.rows.iter().enumerate().take(4) {
        let mut s = standing(&game, &w);
        s.drive.points = kh2_game_core::drive_rules::Points { whole: 9, fraction: 0, max: 9 };
        let mut sm = Summons::default();
        assert_eq!(sm.summon(&data, &mut s, true, row.command), Err(SummonRefusal::CharmNotOwned));
        sm.owned = data.rows.iter().map(|r| r.item).collect();
        assert_eq!(sm.summon(&data, &mut s, false, row.command), Err(SummonRefusal::PartyMissing));
        sm.summon(&data, &mut s, true, row.command).expect("summoned");
        let mut en = kh2::enemy::Enemies::new(edata.clone());
        for k in 0..3 {
            en.spawn("M_EX990", Vec3::new(-200.0 + 200.0 * k as f32, 0.0, 400.0), 3.0, &w);
        }
        for e in en.list.iter_mut() {
            e.rt = None;
            e.hp = 100000;
        }
        let (mut arrived_at, mut modes, mut dismissed, mut gone_at) = (None, Vec::new(), false, None);
        let (mut records, mut blasts, mut heals) = (std::collections::BTreeSet::new(), 0, 0);
        for t in 0..4000 {
            // Sora low now and then: the classes' heal / blast / potion records want it.
            if t % 300 == 0 {
                s.hp = 3;
            }
            s.tick(&w, &Input::default());
            sm.tick(&data, &mut d, &mut s, &w, true, Some(&mut en));
            en.tick(&mut s, &w, game.params.sstm.fall_max);
            en.events.clear();
            for e in sm.events.drain(..) {
                match e {
                    SummonEvent::Arrived { .. } => arrived_at = Some(t),
                    SummonEvent::Mode(m) => modes.push(m),
                    SummonEvent::Gone => gone_at = Some(t),
                    SummonEvent::Record { index } => {
                        records.insert(index);
                    }
                    SummonEvent::Blast { .. } => blasts += 1,
                    SummonEvent::Heal { .. } | SummonEvent::Potion | SummonEvent::Refill => heals += 1,
                    _ => {}
                }
            }
            if arrived_at.is_some_and(|a| t == a + 1500) {
                dismissed = sm.dismiss();
            }
            if gone_at.is_some() {
                break;
            }
        }
        let unserved: Vec<_> = sm.object.as_ref().map(|o| o.trace.missing()).unwrap_or_default();
        eprintln!("summon {k} (command {}): arrived at {arrived_at:?}, modes {modes:?}, class records {records:?}, blast hits {blasts}, heals {heals}, dismissed {dismissed}, gone at {gone_at:?}, points left {}, unserved {unserved:?}", row.command, s.drive.points.whole);
        // Rows: Peter Pan, Genie, Stitch, Chicken Little; Genie and Stitch run their classes.
        if matches!(k, 1 | 2) {
            assert!(!records.is_empty(), "Stitch / Genie choose their records");
        }
        assert!(arrived_at.is_some(), "the object appears");
        assert!(!modes.is_empty(), "its script runs");
        assert!(gone_at.is_some(), "it leaves and the summon ends");
    }
}

/// The areas' own battle groups: which areas place which kinds; tt04's Dusks spawn on tt04's collision
/// and act; M_EX850 / M_EX870 spawned from their own groups (with their spawn records) act.
#[test]
fn area_battle_groups_spawn_and_fight() {
    use kh2::enemy::Enemies;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let mut areas = Vec::new();
    for world in ["tt", "hb", "al", "bb", "mu", "lk", "po", "eh", "nm", "ca", "tr", "lm", "dc", "wi", "he"] {
        for n in 0..45 {
            let a = format!("{world}{n:02}");
            let b = d.area_battle(&a, None);
            if !b.enemies.is_empty() {
                areas.push((a, b));
            }
        }
    }
    for (a, b) in &areas {
        let mut kinds: Vec<&str> = b.enemies.iter().map(|e| e.model.as_str()).collect();
        kinds.sort();
        kinds.dedup();
        eprintln!("{a}: programs {:?}, {} enemies {kinds:?}{}", b.programs, b.enemies.len(), if b.skipped.is_empty() { String::new() } else { format!(", no kind for {:?}", b.skipped) });
    }
    assert!(areas.len() > 5);
    let mut tested = 0;
    for want in ["M_EX990", "M_EX850", "M_EX870"] {
        let Some((a, b)) = areas.iter().find(|(_, b)| b.enemies.iter().any(|e| e.model == want)) else { continue };
        let program = b.enemies.iter().find(|e| e.model == want).unwrap().program;
        let w = World::from_coct(&d.coct(a).expect("collision"));
        let at = b.player.map_or(w.spawn_point(), |p| p.0);
        let mut s = Sora::new(game.clone(), at);
        s.max_hp = 9999;
        s.hp = 9999;
        let mut en = Enemies::new(data.clone());
        for e in b.enemies.iter().filter(|e| e.program == program && !e.disabled) {
            en.spawn_with(&e.model, e.at, e.yaw, &w, e.spawn_kind, Some(e.spawn_params));
        }
        let starts: Vec<(String, Vec3, Option<usize>)> = en.list.iter().map(|e| (e.data.kind.model.to_string(), e.pos(), e.anim.slot)).collect();
        let mut moved = vec![0.0f32; en.list.len()];
        let mut slots: Vec<std::collections::HashSet<Option<usize>>> = vec![Default::default(); en.list.len()];
        for _ in 0..900 {
            s.hp = 9999;
            s.tick(&w, &Input::default());
            en.tick(&mut s, &w, game.params.sstm.fall_max);
            en.events.clear();
            // Host 1.0's created objects join the list after the first ones.
            for (k, e) in en.list.iter().enumerate().take(starts.len()) {
                moved[k] = moved[k].max(e.pos().distance(starts[k].1));
                slots[k].insert(e.anim.slot);
            }
        }
        let report: Vec<String> = starts.iter().enumerate().map(|(k, s)| format!("{} moved {:.0} motions {}", s.0, moved[k], slots[k].len())).collect();
        eprintln!("{a} program {program}: {report:?}; unserved {:?}", en.unserved_calls());
        let acted = starts.iter().enumerate().filter(|(k, s)| s.0 == want && (moved[*k] > 10.0 || slots[*k].len() > 1)).count();
        eprintln!("{want} in {a}: {acted} of {} act", starts.iter().filter(|s| s.0 == want).count());
        tested += 1;
    }
    assert!(tested > 0);
}

/// A Struggle-style fight: tt04's program with Seifer (N_EX570_BTL) and the other Struggle
/// opponents, on tt04's collision, the mission counters kept by the test host (orbs: each hit Sora
/// lands takes one from the opponent, each hit he takes loses one, as the Struggle's rule).
#[test]
fn struggle_opponents_fight_with_mission_counters() {
    use kh2::enemy::{EnemyEvent, Enemies};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let b = d.area_battle("tt04", None);
    let w = World::from_coct(&d.coct("tt04").expect("collision"));
    let mut fights = 0;
    for opponent in ["N_EX570_BTL", "N_EX500_BTL", "N_EX600_BTL"] {
        let Some(e0) = b.enemies.iter().find(|e| e.model == opponent) else { continue };
        let mut s = Sora::new(game.clone(), e0.at + Vec3::new(0.0, 0.0, -300.0));
        s.max_hp = 999;
        s.hp = 999;
        let mut en = Enemies::new(data.clone());
        en.mission = Some([100, 100, 0]);
        let id = en.spawn_with(&e0.model, e0.at, e0.yaw, &w, e0.spawn_kind, Some(e0.spawn_params)).unwrap();
        let (mut hits, mut taken, mut lowest, mut moved) = (0, 0, i32::MAX, 0.0f32);
        let start = en.list[0].pos();
        for t in 0..3600 {
            let Some(e) = en.list.iter().find(|e| e.id == id) else { break };
            let to = e.pos() - s.motor.pos;
            s.target = Some(kh2::sora::Target { pos: e.pos() + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
            let stick = if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] };
            s.hp = 999;
            s.tick(&w, &Input { stick, cam_look: [0.0, 1.0], attack: t % 4 == 0, ..Default::default() });
            en.tick(&mut s, &w, game.params.sstm.fall_max);
            for ev in en.events.drain(..) {
                match ev {
                    EnemyEvent::Hit { .. } => {
                        hits += 1;
                        if let Some(m) = en.mission.as_mut() {
                            m[1] = (m[1] - 1).max(0);
                            m[0] += 1;
                        }
                    }
                    EnemyEvent::HitSora { .. } => {
                        taken += 1;
                        if let Some(m) = en.mission.as_mut() {
                            m[0] = (m[0] - 1).max(0);
                            m[1] += 1;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(e) = en.list.iter().find(|e| e.id == id) {
                lowest = lowest.min(e.hp);
                moved = moved.max(e.pos().distance(start));
            }
        }
        eprintln!("{opponent} (program {}): Sora hit it {hits} times, it hit Sora {taken} times, its lowest HP {lowest}, moved {moved:.0}, counters {:?}, unserved {:?}", e0.program, en.mission, en.unserved_calls());
        assert!(hits > 0 && moved > 10.0, "{opponent} fights");
        fights += 1;
    }
    assert!(fights > 0);
}

#[test]
fn equipment_builds_the_status_and_items_are_described() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let party = d.party().expect("party data");
    let w = floor();
    let p = &game.params;
    let info = |id| kh2::equipment::info(&p.items, &p.item_stats, id).unwrap();
    let kk = info(41);
    eprintln!("Kingdom Key: {kk:?}");
    let blade = p.items.iter().filter(|i| i.r#type == 2).map(|i| info(i.id)).max_by_key(|i| i.attack).unwrap();
    for i in p.items.iter().map(|i| info(i.id)).filter(|i| i.defense > 0 || i.resist.iter().any(|&r| r != 100 && r != 0)).take(6) {
        eprintln!("gear {i:?}");
    }
    let armor = p.items.iter().map(|i| info(i.id)).filter(|i| i.kind != 0 && i.kind != 2).max_by_key(|i| (i.defense, i.resist.iter().filter(|&&r| r != 100).count())).unwrap();
    eprintln!("strongest keyblade {blade:?}; an armor {armor:?}");
    let mut s = standing(&game, &w);
    let (str0, def0, res0) = (s.strength, s.defense, s.resist);
    s.set_equipment(&[blade.id, armor.id]);
    eprintln!("Sora: strength {str0} -> {}, defense {def0} -> {}, resistances {res0:?} -> {:?}, AP {}", s.strength, s.defense, s.resist, s.ap);
    assert_eq!(s.strength - str0, i32::from(blade.attack) - i32::from(kk.attack));
    assert_eq!(s.defense - def0, i32::from(armor.defense) + i32::from(blade.defense) - i32::from(kk.defense));
    if armor.resist.iter().any(|&r| r != 100) {
        assert_ne!(s.resist, res0);
    }
    // The record keeps the equipment.
    let rec = s.save();
    let mut t = standing(&game, &w);
    t.load(&rec).unwrap();
    assert_eq!((t.equipment.clone(), t.strength, t.resist), (s.equipment.clone(), s.strength, s.resist));
    // A member's equipment adds to his status.
    let mut f = kh2::party::Follower::new(&party, 0, Vec3::ZERO);
    let before = f.stats(&party);
    f.equipment.push(armor.id);
    let after = f.stats(&party);
    eprintln!("Donald: defense {} -> {}, weakness {:?} -> {:?}", before.defense, after.defense, before.weakness, after.weakness);
    assert_eq!(after.defense - before.defense, i32::from(armor.defense));
}

/// During a limit the members are in the limit state (kh2-rs `LimitMembers { away: true }`): they do
/// not fight on their own. The LIMI motions carry no attack windows (measured: at most the hit of a
/// swing already running at the start), so the limits' damage needs the limit's own attack objects,
/// which kh2-rs does not port (docs/DEVIATIONS.md).
#[test]
fn limits_hold_the_members_and_their_motions_carry_no_attacks() {
    use kh2::enemy::{EnemyEvent, Enemies};
    use kh2::limit::{LimitEvent, Limits};
    use kh2::party::{tick_battle, Follower, PartyRng};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let ldata = d.limits().expect("limit tables");
    let edata = Arc::new(d.enemies().expect("enemy data"));
    let party = d.party().expect("party data");
    let w = floor();
    let mut results = Vec::new();
    for cmd in [255u16, 338, 282, 344] {
        let mut s = standing(&game, &w);
        s.max_hp = 999;
        s.hp = 999;
        let mut en = Enemies::new(edata.clone());
        for k in 0..6 {
            let a = k as f32;
            en.spawn("M_EX990", Vec3::new(a.sin() * 200.0, 0.0, 150.0 + a.cos() * 150.0), 3.0, &w);
        }
        for e in en.list.iter_mut() {
            e.rt = None;
            e.hp = 100000;
        }
        let mut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, 0.0))];
        let present = kh2::limit::present(&fs, &party);
        let mut rng = PartyRng::default();
        let mut lim = Limits::default();
        lim.start(&ldata, &mut d, &mut s, &present, cmd).expect("starts");
        let (mut hits, mut rows) = (0, std::collections::BTreeSet::new());
        let mut ticks = 0;
        while lim.running() && ticks < 3000 {
            s.tick(&w, &Input::default());
            tick_battle(&party, &mut fs, &mut rng, &mut s, &w, Some(&mut en));
            en.tick(&mut s, &w, game.params.sstm.fall_max);
            lim.tick(&s);
            lim.play_motions(&mut s, &mut fs, &party);
            lim.events.retain(|e| !matches!(e, LimitEvent::Motion { .. }));
            for ev in en.events.drain(..) {
                if let EnemyEvent::Hit { atkp, .. } = ev {
                    hits += 1;
                    rows.insert(atkp);
                    lim.count_hit();
                }
            }
            ticks += 1;
        }
        eprintln!("limit {cmd}: {ticks} ticks, {hits} hits on the enemies, rows {rows:?}, limit count {}", lim.hits());
        results.push((cmd, hits));
    }
    assert!(results.iter().all(|r| r.1 <= 2), "the members do not fight during a limit");
}

/// The areas' own set-up and waves (kh2-rs `spawn_waves`): tt04's programs and eh06's (M_EX870),
/// Sora at the group's player entry; the kind-2 groups create their waves as their enemies fall.
#[test]
fn area_runs_set_up_groups_and_waves() {
    use kh2::area::AreaRun;
    use kh2::enemy::Enemies;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let mut any_wave = false;
    for (area, program) in [("tt04", None), ("eh06", Some(1u16)), ("hb09", None)] {
        let w = World::from_coct(&d.coct(area).expect("collision"));
        let b = d.area_battle(area, None);
        let programs: Vec<u16> = match program {
            Some(p) => vec![p],
            None => {
                let mut v: Vec<u16> = b.enemies.iter().map(|e| e.program).collect();
                v.dedup();
                v.into_iter().take(4).collect()
            }
        };
        for p in programs {
            let mut en = Enemies::new(data.clone());
            let mut run = AreaRun::set_up(&mut d, area, p, &mut en, &w);
            let group = run.groups.iter().find(|g| g.program == p);
            let start = group.and_then(|g| g.player).map(|v| v / kh2_enemy_host::SCALE).unwrap_or(w.spawn_point());
            let mut s = Sora::new(game.clone(), start);
            s.max_hp = 9999;
            s.hp = 9999;
            let at_load = en.list.len();
            let (mut most, mut made) = (at_load, 0);
            let mut moved: std::collections::HashMap<u32, f32> = Default::default();
            let mut kinds_made = std::collections::BTreeSet::new();
            let starts: std::collections::HashMap<u32, Vec3> = en.list.iter().map(|e| (e.id, e.pos())).collect();
            for t in 0..2400 {
                let before = en.next_id;
                s.hp = 9999;
                // Sora attacks the nearest enemy, so the groups' members fall and the waves come.
                let near = en.list.iter().filter(|e| e.alive() && !e.removed).min_by(|a, b| a.pos().distance(s.motor.pos).total_cmp(&b.pos().distance(s.motor.pos))).map(|e| e.pos());
                let stick = near.map_or([0.0, 0.0], |n| {
                    let to = n - s.motor.pos;
                    if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] }
                });
                if let Some(n) = near {
                    s.target = Some(kh2::sora::Target { pos: n + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
                }
                s.tick(&w, &Input { stick, cam_look: [0.0, 1.0], attack: t % 4 == 0, ..Default::default() });
                en.tick(&mut s, &w, game.params.sstm.fall_max);
                en.events.clear();
                run.tick(&mut en, &w, s.motor.pos, &[]);
                made += (en.next_id - before) as usize;
                for e in en.list.iter().filter(|e| e.id >= before) {
                    kinds_made.insert(e.data.kind.model);
                }
                en.prune();
                most = most.max(en.list.len());
                for e in &en.list {
                    if let Some(st) = starts.get(&e.id) {
                        let m = moved.entry(e.id).or_default();
                        *m = m.max(e.pos().distance(*st));
                    }
                }
            }
            let acted = moved.values().filter(|&&m| m > 10.0).count();
            eprintln!("{area} program {p}: {at_load} at load, {made} created by waves, at most {most} at once, {acted} of the first ones moved, kinds made {kinds_made:?}, kind-2 groups {}, raised {:?}, off {:?}", run.live.len(), run.raised, run.off);
            any_wave |= made > 0;
        }
    }
    assert!(any_wave, "some kind-2 group made a wave");
}

/// The Struggle (kh2-rs WP-7E): tt04's programs whose groups carry a mission command start the
/// mission manager; its clock runs, the hits knock orbs out, the orbs are taken back, the script
/// sends the result. Sora fights the opponent.
#[test]
fn the_struggle_mission_runs_its_clock_orbs_and_result() {
    use kh2::area::AreaRun;
    use kh2::enemy::{EnemyEvent, Enemies};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let w = World::from_coct(&d.coct("tt04").expect("collision"));
    let b = d.area_battle("tt04", None);
    let _ = b;
    let mut probe = Enemies::new(data.clone());
    let all = AreaRun::set_up(&mut d, "tt04", 0, &mut probe, &w).groups;
    // The Struggle matches: the missions with the orb pool (TT04_MS00..02; Seifer is program 184).
    let mut programs: Vec<u16> = all.iter().filter(|g| g.mission.is_some() && g.orb_pool).map(|g| g.program).collect();
    programs.sort();
    programs.dedup();
    eprintln!("tt04 missions: {:?}", all.iter().filter_map(|g| g.mission.as_ref().map(|m| (g.program, m.1.clone(), g.orb_pool))).collect::<Vec<_>>());
    let mut ran = 0;
    for p in programs {
        let mut en = Enemies::new(data.clone());
        let mut run = AreaRun::set_up(&mut d, "tt04", p, &mut en, &w);
        let Some(m) = run.mission.as_ref() else {
            if let Some(e) = &run.mission_error {
                eprintln!("tt04 program {p}: {e}");
            }
            continue;
        };
        let (name, start) = (m.a.name.clone(), m.counters());
        eprintln!("tt04 program {p}: opponents {:?}", en.list.iter().map(|e| e.data.kind.model).collect::<Vec<_>>());
        let group = run.groups.iter().find(|g| g.program == p && g.mission.is_some());
        let at = group.and_then(|g| g.player).map(|v| v / kh2_enemy_host::SCALE).unwrap_or(w.spawn_point());
        let mut s = Sora::new(game.clone(), at);
        let (mut hits, mut taken, mut most_orbs, mut knocked) = (0, 0, 0, 0);
        let mut ticks = 0;
        for t in 0..60 * 200 {
            ticks = t;
            let near = en.list.iter().filter(|e| e.alive() && !e.removed).min_by(|a, b| a.pos().distance(s.motor.pos).total_cmp(&b.pos().distance(s.motor.pos))).map(|e| e.pos());
            let stick = near.map_or([0.0, 0.0], |n| {
                let to = n - s.motor.pos;
                if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] }
            });
            if let Some(n) = near {
                s.target = Some(kh2::sora::Target { pos: n + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
            }
            s.tick(&w, &Input { stick, cam_look: [0.0, 1.0], attack: t % 4 == 0, ..Default::default() });
            en.tick(&mut s, &w, game.params.sstm.fall_max);
            for ev in en.events.drain(..) {
                match ev {
                    EnemyEvent::Hit { .. } => hits += 1,
                    EnemyEvent::HitSora { .. } => taken += 1,
                    _ => {}
                }
            }
            let before = run.mission.as_ref().map_or(0, |m| m.a.orbs.len());
            run.tick_mission(&s, &mut en, &w);
            run.tick(&mut en, &w, s.motor.pos, &[]);
            en.prune();
            let Some(m) = run.mission.as_ref() else { break };
            knocked += m.a.orbs.len().saturating_sub(before);
            most_orbs = most_orbs.max(m.a.orbs.len());
            if m.ended || m.outcome().is_some() {
                break;
            }
        }
        let m = run.mission.as_ref().unwrap();
        eprintln!(
            "tt04 program {p}: mission {name} counters {start:?} -> {:?}, clock {} frames shown, state {}, end {:?}, result {:?}, ended {}, game over {}, {knocked} orbs knocked out (at most {most_orbs} at once), Sora hit {hits}, hit Sora {taken}, {ticks} ticks, raised {:?}",
            m.counters(), m.a.clock.shown, m.a.state, m.a.end_cause, m.outcome(), m.ended, m.game_over, run.raised
        );
        assert!(m.a.state >= 1, "the HUD came up");
        assert!(knocked > 0, "hits knock orbs out");
        assert!(m.a.end_cause.is_some() && m.outcome().is_some(), "the match ends and the script sends the result");
        assert!(m.counters()[1] < start[1], "the opponent lost orbs");
        ran += 1;
    }
    assert!(ran > 0, "a tt04 program starts a mission");
}

/// A talk (kh2-rs `npc::talk_step`): townspeople models with a talk camera record (type 0x13);
/// Sora faces the target and is held, the target faces him, the camera cuts to mode 4 at kh2-rs
/// `talk_pose` of the shot `talk_shot` reads, the end releases him and the camera warps back; a
/// lock-on target reports mode 2.
#[test]
fn a_talk_locks_sora_and_cuts_the_camera_to_the_talk_view() {
    use kh2::camera::{CamInput, Camera};
    use kh2_tables::tables as t;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let p = game.params.camera.clone().expect("camera constants");
    let objs = kh2_tables::load::<t::Objentry>(&mut d.0, &t::OBJENTRY).expect("objentry").rows;
    let _ = d.0.apply_names(["000tt.idx"]);
    let w = floor();
    let mut tested = 0;
    // Twilight Town's townspeople (tt02's a72_ / b72_ / c72_ / b50_ in kh2-rs's own test).
    for row in objs.iter().filter(|r| r.model_name.starts_with("H_") || r.model_name.starts_with("N_")) {
        if tested >= 3 {
            break;
        }
        let Ok(file) = d.read(&format!("obj/{}.mdlx", row.model_name)) else { continue };
        let Some(parts) = kh2_game_core::reaction_parts::Table::from_mdlx(&file) else { continue };
        let Some(cam_rec) = parts.recs.iter().find(|r| r.kind == kh2_enemy_host::npc::TYPE_TALK_CAMERA) else { continue };
        let rest = kh2_model::rest_pose(&kh2_model::mdlx(&file).expect("model").bones);
        let mut s = standing(&game, &w);
        let mut cam = Camera::new(0);
        let inp = CamInput { auto: true, step: 1.0, ..Default::default() };
        cam.tick(&p, &mut s, &w, &inp);
        let target = kh2::talk::Target { pos: s.motor.pos + Vec3::new(150.0, 0.0, 150.0), yaw: 0.0, part: cam_rec.part, talk_id: 7, ..Default::default() };
        let talk = kh2::talk::begin(&mut s, Some(&parts), &rest, &target);
        let shot = talk.req.shot.expect("talk shot");
        cam.start_talk(&p, shot);
        let v = cam.tick(&p, &mut s, &w, &inp);
        let want = kh2_game_core::camera_rules::talk_pose(&shot, s.motor.pos);
        eprintln!("{}: part {}, shot {shot:?}, view {:?} (mode {}), want {want:?}, Sora yaw {:.2}, target yaw {:.2}", row.model_name, cam_rec.part, v.pos, v.mode, talk.req.sora_yaw, talk.req.target_yaw);
        assert_eq!(v.mode, 4, "the talk camera");
        assert!(v.pos.distance(want) < 1.0 && v.look.distance(shot.look) < 1.0, "the cut to talk_pose");
        assert!((talk.req.sora_yaw - std::f32::consts::FRAC_PI_4).abs() < 1e-3, "Sora faces the target");
        assert!((talk.req.target_yaw - (-3.0 * std::f32::consts::FRAC_PI_4)).abs() < 1e-3, "the target faces Sora");
        assert_eq!(talk.req.target_motion, Some(kh2_enemy_host::npc::MOTION_TALK));
        let at = s.motor.pos;
        for _ in 0..30 {
            s.tick(&w, &Input { stick: [0.0, 1.0], cam_look: [0.0, 1.0], ..Default::default() });
            cam.tick(&p, &mut s, &w, &inp);
        }
        assert!(s.motor.pos.distance(at) < 1.0, "held during the talk");
        kh2::talk::end(&mut s);
        cam.end_talk();
        let v = cam.tick(&p, &mut s, &w, &inp);
        assert_eq!(v.mode, 0, "back to the follow camera");
        for _ in 0..30 {
            s.tick(&w, &Input { stick: [0.0, 1.0], cam_look: [0.0, 1.0], ..Default::default() });
        }
        assert!(s.motor.pos.distance(at) > 10.0, "released");
        s.target = Some(kh2::sora::Target { pos: s.motor.pos + Vec3::new(300.0, 100.0, 0.0), radius: 30.0, half_height: 50.0 });
        assert_eq!(cam.tick(&p, &mut s, &w, &inp).mode, kh2::camera::MODE_LOCK_ON);
        tested += 1;
    }
    assert!(tested > 0, "a townsperson with a talk camera record");
}

/// Glue against kh2-rs values (docs/FAITHFULNESS.md class c): the Struggle glue's orb loss per
/// reaction equals kh2-rs `orbs_knocked_out` (FUN_001caeb0) for Sora's and the opponent's side,
/// and the orbs launched equal the counter's drop.
#[test]
fn the_struggle_glue_knocks_out_what_kh2_rs_orbs_knocked_out_says() {
    use kh2::area::AreaRun;
    use kh2::enemy::Enemies;
    use kh2_enemy_host::struggle::orbs_knocked_out;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let w = World::from_coct(&d.coct("tt04").expect("collision"));
    let mut checked = 0;
    for reaction in 0u8..20 {
        let mut en = Enemies::new(data.clone());
        let mut run = AreaRun::set_up(&mut d, "tt04", 184, &mut en, &w);
        let mut s = Sora::new(game.clone(), w.spawn_point());
        // Up to the running state (the start layout lets the player act).
        for _ in 0..400 {
            run.tick_mission(&s, &mut en, &w);
        }
        let m = run.mission.as_ref().expect("Seifer's mission");
        let before = m.counters();
        let orbs = m.a.orbs.len();
        s.reactions_taken = vec![reaction];
        if let Some(e) = en.list.first_mut() {
            e.struck = vec![reaction];
        }
        run.tick_mission(&s, &mut en, &w);
        let m = run.mission.as_ref().unwrap();
        let after = m.counters();
        let want = orbs_knocked_out(reaction, before[0]) + if en.list.is_empty() { 0 } else { orbs_knocked_out(reaction, before[1]) };
        let lost = (before[0] - after[0]) + (before[1] - after[1]);
        assert_eq!(lost, want, "reaction {reaction}: counters {before:?} -> {after:?}");
        assert_eq!(m.a.orbs.len() - orbs, want as usize, "reaction {reaction}: orbs launched");
        checked += 1;
    }
    assert_eq!(checked, 20);
}

/// Seifer's bat hits Sora (round 8): his script swings (atkp 1043 / 1298 / 1535 on his own attack
/// shapes, group 1 / 2) when Sora does not keep him in hit reactions; every hit on Sora knocks
/// Sora's orbs out. (The round-7 test pressed attack every 4 ticks: Seifer never left his
/// reactions, so he never swung.)
#[test]
fn seifers_bat_hits_sora_and_knocks_his_orbs_out() {
    use kh2::enemy::{EnemyEvent, Enemies};
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let w = World::from_coct(&d.coct("tt04").expect("collision"));
    let mut en = Enemies::new(data.clone());
    let mut run = kh2::area::AreaRun::set_up(&mut d, "tt04", 184, &mut en, &w);
    let e0 = en.list[0].pos();
    let mut s = Sora::new(game.clone(), e0 + Vec3::new(0.0, 0.0, -200.0));
    let mut hits = std::collections::BTreeMap::new();
    for t in 0..3600 {
        s.hp = 999;
        let ep = en.list[0].pos();
        let to = ep - s.motor.pos;
        let stick = if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] };
        s.target = Some(kh2::sora::Target { pos: ep + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
        s.tick(&w, &Input { stick, cam_look: [0.0, 1.0], attack: t % 90 == 0, ..Default::default() });
        en.tick(&mut s, &w, game.params.sstm.fall_max);
        for ev in en.events.drain(..) {
            if let EnemyEvent::HitSora { atkp, .. } = ev {
                *hits.entry(atkp).or_insert(0) += 1;
            }
        }
        run.tick_mission(&s, &mut en, &w);
    }
    let m = run.mission.as_ref().expect("mission");
    eprintln!("Seifer's hits on Sora by atkp {hits:?}, counters {:?}", m.counters());
    assert!(hits.values().sum::<i32>() > 0, "the bat reaches Sora");
    assert!(m.counters()[0] < 100, "Sora lost orbs");
}

/// A hit the opponent took before the enemies' tick (a member's or a summon's, in
/// `kh2_party_tick` / the summon tick) still knocks its orbs out at the mission frame.
#[test]
fn hits_from_members_before_the_enemy_tick_knock_orbs_out() {
    use kh2::area::AreaRun;
    use kh2::enemy::Enemies;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let w = World::from_coct(&d.coct("tt04").expect("collision"));
    let mut en = Enemies::new(data.clone());
    let mut run = AreaRun::set_up(&mut d, "tt04", 184, &mut en, &w);
    let mut s = Sora::new(game.clone(), en.list[0].pos() + Vec3::new(0.0, 0.0, -2000.0));
    for _ in 0..400 {
        s.tick(&w, &Input::default());
        en.tick(&mut s, &w, game.params.sstm.fall_max);
        run.tick_mission(&s, &mut en, &w);
    }
    let before = run.mission.as_ref().unwrap().counters()[1];
    // A class-0 reaction (3 orbs) landed by a member's hit, then the library's enemy tick.
    let r = (0u8..40).find(|&r| kh2_enemy_host::struggle::orbs_knocked_out(r, 100) == 3).expect("a 3-orb reaction");
    en.list[0].struck.push(r);
    s.tick(&w, &Input::default());
    en.tick(&mut s, &w, game.params.sstm.fall_max);
    run.tick_mission(&s, &mut en, &w);
    let after = run.mission.as_ref().unwrap().counters()[1];
    assert_eq!(before - after, 3, "{before} -> {after}");
}

/// Round 9 evidence for the limit damage dead end: the effect files (type 18) of every limit file
/// decode with kh2-rs's kh2-fx and use no particle program of the hit family (pppKeHitBall,
/// pppKeHitChk, pppKeHitChkPxB, pppKeHitBorn, pppKeThHitBorn); the limit's own script (Comet)
/// makes only the timer, effect and query calls listed in docs/DEVIATIONS.md, also with its
/// sub-commands sent.
#[test]
fn limit_effect_files_carry_no_hit_programs() {
    let Some(mut d) = disc() else { return };
    let ld = d.limits().expect("limits");
    let mut files: Vec<String> = ld.rows.iter().map(|r| r.file.clone()).collect();
    files.sort();
    files.dedup();
    let mut checked = 0;
    for f in &files {
        let Ok(b) = d.read(&format!("limit/us/{f}")) else { continue };
        for e in kh2_bar::parse(&b).unwrap().iter().filter(|e| e.kind == 18) {
            let p = kh2_fx::Pax::parse(e.data).expect("pax");
            let hits: Vec<&str> = p.dpds.iter().flat_map(|d| &d.particles).flat_map(|pd| &pd.groups).flat_map(|g| &g.programs).map(|pr| kh2_fx::ppp::name(pr.kind)).filter(|n| n.contains("Hit")).collect();
            assert!(hits.is_empty(), "{f}: {hits:?}");
            checked += 1;
        }
    }
    eprintln!("{checked} limit effect files, no hit programs");
    assert!(checked > 10);
}

/// Sound and effect cues (round 9): Sora's run and combo motions carry footstep, sound and effect
/// frame triggers, the enemies' motions theirs; each cue names its owner's model and place.
#[test]
fn motions_give_sound_and_effect_cues() {
    use kh2::cue::{of_anim, Kind, Owner};
    use kh2::enemy::Enemies;
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let data = Arc::new(d.enemies().expect("enemy data"));
    let w = floor();
    let mut s = standing(&game, &w);
    let mut en = Enemies::new(data);
    en.spawn("M_EX010", Vec3::new(0.0, 0.0, 250.0), 0.0, &w);
    let mut kinds = std::collections::BTreeMap::new();
    for t in 0..900 {
        let attack = t > 300 && t % 8 == 0;
        s.tick(&w, &Input { stick: if t < 200 { [0.0, 1.0] } else { [0.0, 0.0] }, cam_look: [0.0, 1.0], attack, ..Default::default() });
        s.collect_cues();
        s.hp = s.max_hp;
        en.tick(&mut s, &w, game.params.sstm.fall_max);
        for e in &en.list {
            s.cues.extend(of_anim(&e.anim, Owner::Enemy(e.id), e.data.kind.model, e.pos()));
        }
        for c in s.cues.drain(..) {
            assert!(!c.model.is_empty());
            *kinds.entry((format!("{:?}", c.kind), format!("{:?}", c.owner).chars().take(5).collect::<String>())).or_insert(0) += 1;
            if matches!(c.kind, Kind::Sound) && kinds.len() < 12 {
                eprintln!("sound cue {c:?}");
            }
        }
    }
    eprintln!("cues by (kind, owner): {kinds:?}");
    assert!(kinds.keys().any(|(k, o)| k == "Footstep" && o == "Sora"), "Sora's run has footsteps");
    assert!(kinds.keys().any(|(_, o)| o == "Enemy"), "the enemy's motions have cues");
}

/// An area's set-up for the host (round 9, kh2-rs `run_area`): tt02's map programs place its
/// field objects with talk ids and examine commands, its evt programs the townspeople who offer
/// talk (0x1f), and the player's start.
#[test]
fn an_area_setup_lists_townspeople_with_their_talks() {
    let Some(mut d) = disc() else { return };
    let mut best = None;
    for map in 0u8..12 {
        let s = d.area_setup("tt02", [map, 0, 0], 0);
        let people = s.objects.iter().filter(|o| o.talk_id != 0).count();
        eprintln!("tt02 map {map}: {} groups, {} objects, {people} townspeople, mask {:#x}, camera {}, music {:?}, player {:?}", s.groups.len(), s.objects.len(), s.collision_mask, s.camera_type, s.music, s.player);
        if people > 0 && best.is_none() {
            best = Some(s);
        }
    }
    let s = best.expect("a tt02 map program with talk objects");
    let talkers: Vec<_> = s.objects.iter().filter(|o| o.talk_id != 0).collect();
    for o in talkers.iter().take(4) {
        eprintln!("  {} ({:#x}) in {} at {:?} talk {:#x} command {:#x}", o.model, o.object, o.group, o.at, o.talk_id, o.part_command);
    }
    assert!(!talkers.is_empty(), "objects with talk ids");
    assert!(talkers.iter().any(|o| o.part_command == 0x1e || o.part_command == 0x1f), "an examine / talk command");
    // The townspeople come with the evt programs (kh2-rs npc test: groups a72_ / b72_ / c72_ / b50_).
    let people: usize = (0u8..64).map(|e| d.area_setup("tt02", [0, 0, e], 0).objects.iter().filter(|o| o.part_command == 0x1f).count()).sum();
    eprintln!("tt02 evt programs 0..63: {people} objects offering talk (0x1f)");
    assert!(people > 0, "townspeople offering talk");
    assert!(s.player.is_some(), "the player's start");
}

/// Runtime meshes (round 10): Sora's body skinned by his current bones stands about his height,
/// every submesh names a decoded texture, the posed mesh follows the motion.
#[test]
fn sora_mesh_skins_with_his_bones() {
    let Some(mut d) = disc() else { return };
    let game = Arc::new(d.game().expect("Sora loads"));
    let w = floor();
    let mut s = standing(&game, &w);
    let m = kh2::mesh::skin(&s.body.rig, &s.body.pose, s.body_matrix());
    let (lo, hi) = m.positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(Vec3::from(*p)), b.max(Vec3::from(*p))));
    eprintln!("Sora: {} vertices, {} triangles, {} submeshes, {} textures, bounds {lo:?}..{hi:?}", m.positions.len(), m.indices.len() / 3, m.subs.len(), s.body.rig.model.textures.len());
    assert!(m.positions.len() > 500 && m.indices.len().is_multiple_of(3));
    assert!((100.0..250.0).contains(&(hi.y - lo.y)), "about Sora's height");
    for sub in &m.subs {
        let t = &s.body.rig.model.textures[sub.texture as usize];
        assert_eq!(t.rgba.len() as u32, t.width * t.height * 4);
    }
    // Run: the posed mesh moves with him.
    for _ in 0..30 {
        s.tick(&w, &Input { stick: [0.0, 1.0], cam_look: [0.0, 1.0], ..Default::default() });
    }
    let m2 = kh2::mesh::skin(&s.body.rig, &s.body.pose, s.body_matrix());
    let c = |m: &kh2::mesh::Skinned| m.positions.iter().fold(Vec3::ZERO, |a, p| a + Vec3::from(*p)) / m.positions.len() as f32;
    assert!(c(&m2).z - c(&m).z > 50.0, "the mesh runs with him");
}
