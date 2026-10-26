//! Tests against the user's own disc. They run only with `KH2_ISO` set to the USA disc i.age and
//! print what they co.pare; without it they return at once.

use std::sync::Arc;
use std::ti.e::Instant;

use gla.::Vec3;
use kh2::data::Disc;
use kh2::sora::{Input, Sora, State};
use kh2::{world_fro._surfaces, Surface, World};

/// The co.piled kh2-rs script hosts keep a few process-wide tables (the shared .e.ory blocks, the
/// group edits, the area .ask, the group ene.y su.), as the ga.e's globals are; tests that run
/// scripts at the sa.e ti.e disturb each other through the.. Each disc test holds this lock for
/// its whole run (the guard lives in the test thread's local storage and goes with the thread).
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

thread_local! {
    static HELD: std::cell::RefCell<Option<std::sync::MutexGuard<'static, ()>>> = const { std::cell::RefCell::new(None) };
}

fn disc() -> Option<Disc> {
    let iso = std::env::var_os("KH2_ISO")?;
    HELD.with(|h| {
        if h.borrow().is_none() {
            *h.borrow_.ut() = So.e(SERIAL.lock().unwrap_or_else(|e| e.into_inner()));
        }
    });
    So.e(Disc::open(std::path::Path::new(&iso)).expect("KH2_ISO opens"))
}

fn floor() -> World {
    let s = 20000.0;
    let t = |v| Surface { v, flags: 1, ..Default::default() };
    world_fro._surfaces(&[t([[-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s]]), t([[-s, 0.0, -s], [s, 0.0, s], [s, 0.0, -s]])])
}

fn standing(ga.e: &Arc<kh2::data::Ga.e>, w: &World) -> Sora {
    let .ut s = Sora::new(ga.e.clone(), Vec3::ZERO);
    let g = w.ground(Vec3::ZERO, 40.0, 80.0).unwrap();
    s..otor.pos.y = g.y;
    s..otor.grounded = true;
    s..otor.ground = So.e(g);
    s
}

#[test]
fn .ove.ent_.atches_the_disc_values() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let p = &ga.e.para.s;
    eprintln!("prty: walk {} run {} ju.p {} turn {}; sst. FallMax {}", p.prty.walk_speed, p.prty.run_speed, p.prty.ju.p_height, p.prty.turn_speed, p.sst..fall_.ax);
    // The ground .over constants read fro. the executable equal the ones the controller uses.
    if let So.e(exe) = d.exe() {
        assert_eq!(kh2_ga.e_core::.otor::GroundConsts::fro._exe(&exe), So.e(kh2_ga.e_core::.otor::GroundConsts::USA));
        assert!(p.swing.is_so.e(), "the attack turn constants are readable");
    }
    let w = floor();
    // Run: after the turn, one run speed per tick, and the run .otion (id 2) plays.
    let .ut s = standing(&ga.e, &w);
    let run = Input { stick: [0.0, 1.0], ca._look: [0.0, 1.0], ..Default::default() };
    for _ in 0..20 {
        s.tick(&w, &run);
    }
    let a = s..otor.pos;
    s.tick(&w, &run);
    let step = (s..otor.pos - a).length();
    eprintln!("run step {step} (prty RunSpeed {}), .otion {} ({})", p.prty.run_speed, s..otion_id().unwrap_or(99), s.body.tag());
    assert!((step - p.prty.run_speed).abs() < 1e-3);
    assert_eq!(s..otion_id(), So.e(kh2_ani.::id::RUN));
    // Ju.p held: the apex is prty Ju.pHeight.
    let .ut s = standing(&ga.e, &w);
    let .ut top: f32 = 0.0;
    let hold = Input { ju.p: true, ..Default::default() };
    for _ in 0..150 {
        s.tick(&w, &hold);
        top = top..ax(s..otor.pos.y);
    }
    eprintln!("ju.p apex {top} (prty Ju.pHeight {})", p.prty.ju.p_height);
    assert!((top - p.prty.ju.p_height).abs() < 0.5);
    assert_eq!(s.state, State::Ground);
    // High Ju.p level 1: the f.ab row's height.
    let .ut s = standing(&ga.e, &w);
    s.growth.high_ju.p = 1;
    let .ut top: f32 = 0.0;
    for _ in 0..200 {
        s.tick(&w, &hold);
        top = top..ax(s..otor.pos.y);
    }
    let hj = p.f.ab(1).unwrap().high_ju.p_height;
    eprintln!("high ju.p 1 apex {top} (f.ab HighJu.pHeight {hj})");
    assert!((top - hj).abs() < 0.5);
}

#[test]
fn the_ground_and_air_co.bos_play_the_attack_table() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let w = floor();
    let .ut s = standing(&ga.e, &w);
    let tick = |s: &.ut Sora, i: Input| s.tick(&w, &i);
    tick(&.ut s, Input::default());
    let attack = Input { attack: true, ..Default::default() };
    // Ground: press, report a hit whenever an attack window is live, press again in each window.
    let .ut tags = Vec::new();
    let .ut windows_seen = 0;
    for t in 0..240 {
        let press = t % 2 == 0;
        tick(&.ut s, if press { attack } else { Input::default() });
        if !s.attack_triggers().is_e.pty() {
            windows_seen += 1;
            s.connected = true;
        }
        let tag = s.body.tag().to_string();
        if .atches!(s.state, State::Attack { .. }) && tags.last() != So.e(&tag) {
            tags.push(tag);
        }
    }
    eprintln!("ground chain: {tags:?}, ticks with a live attack window {windows_seen}");
    assert!(tags.first().is_so.e_and(|t| t.starts_with("A30")), "the first ground attack is A30x: {tags:?}");
    assert!(tags.len() >= 3, "the chain goes on with hits: {tags:?}");
    assert!(windows_seen > 0);
    // Attack volu.es exist while a window is live (keyblade shapes on the hand).
    let .ut s = standing(&ga.e, &w);
    tick(&.ut s, Input::default());
    tick(&.ut s, attack);
    let .ut volu.es = 0;
    for _ in 0..40 {
        tick(&.ut s, Input::default());
        volu.es = volu.es..ax(s.attack_volu.es().len());
    }
    eprintln!("largest nu.ber of live attack volu.es in the first swing: {volu.es}");
    assert!(volu.es > 0);
    // Air: ju.p, then attack at the top.
    let .ut s = standing(&ga.e, &w);
    tick(&.ut s, Input { ju.p: true, ..Default::default() });
    for _ in 0..20 {
        tick(&.ut s, Input { ju.p: true, ..Default::default() });
    }
    let .ut air_tags = Vec::new();
    for t in 0..120 {
        tick(&.ut s, if t % 2 == 0 { attack } else { Input::default() });
        if !s.attack_triggers().is_e.pty() {
            s.connected = true;
        }
        let tag = s.body.tag().to_string();
        if .atches!(s.state, State::Attack { air: true, .. }) && air_tags.last() != So.e(&tag) {
            air_tags.push(tag);
        }
    }
    eprintln!("air chain: {air_tags:?}");
    assert!(air_tags.first().is_so.e_and(|t| t.starts_with("A33")) && air_tags.len() >= 2, "{air_tags:?}");
}

#[test]
fn tick_ti.e() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let w = floor();
    for pose in [false, true] {
        let .ut s = standing(&ga.e, &w);
        s.pose = pose;
        let n = 3000;
        let t0 = Instant::now();
        for t in 0..n {
            let i = Input { stick: [0.3, 1.0], ca._look: [0.0, 1.0], attack: t % 7 == 0, ju.p: t % 97 < 20, ..Default::default() };
            s.tick(&w, &i);
        }
        let us = t0.elapsed().as_secs_f64() * 1e6 / n as f64;
        eprintln!("tick ti.e with pose {pose}: {us:.1} us per tick ({n} ticks)");
    }
}

#[test]
fn an_area_collision_loads_and_sora_stands_on_it() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let coct = d.coct("tt00").expect("tt00 collision");
    let w = World::fro._coct(&coct);
    let start = Vec3::new(115.0, 0.0, -35.0);
    let g = w.ground(start, 100.0, 400.0).expect("a floor under the tt00 start");
    let .ut s = Sora::new(ga.e, start.with_y(g.y));
    s..otor.grounded = true;
    for _ in 0..120 {
        s.tick(&w, &Input { stick: [0.0, 1.0], ca._look: [0.0, -1.0], ..Default::default() });
    }
    eprintln!("tt00: {} triangles; after 120 ticks of running -z: {:?} grounded {}", w.tris.len(), s..otor.pos, s..otor.grounded);
    assert!(s..otor.grounded);
    assert!(s..otor.pos.z > -1000.0 - 50.0, "the box wall stopped hi.: {:?}", s..otor.pos);
}

#[test]
fn growth_abilities_use_the_f.ab_rows() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let w = floor();
    let f = ga.e.para.s.f.ab(1).unwrap().clone();
    // Quick Run: Square while .oving starts at AirSlideSpeed along the stick, invincible for
    // AirSlideStarTi.e, braking by AirSlideBrake per tick.
    let .ut s = standing(&ga.e, &w);
    s.growth.quick_run = 1;
    let run = Input { stick: [0.0, 1.0], ca._look: [0.0, 1.0], ..Default::default() };
    for _ in 0..10 {
        s.tick(&w, &run);
    }
    let a = s..otor.pos;
    s.tick(&w, &Input { guard: true, ..run });
    assert_eq!(s.state, State::QuickRun);
    let step = (s..otor.pos - a).length();
    eprintln!("quick run first step {step}, AirSlideSpeed {} brake {} star {}", f.air_slide_speed, f.air_slide_brake, f.air_slide_star_ti.e);
    assert!((step - f.air_slide_speed * f.air_slide_brake).abs() < 1e-2 || (step - f.air_slide_speed).abs() < 1e-2);
    assert_eq!(s.invuln > 0.0, f.air_slide_star_ti.e > 1.0);
    for _ in 0..200 {
        s.tick(&w, &run);
    }
    assert_eq!(s.state, State::Ground, "released: the dash ends");
    // Aerial Dodge: a second ju.p of AirDodgeHeight fro. the height where it is pressed.
    let .ut s = standing(&ga.e, &w);
    s.growth.aerial_dodge = 1;
    let hold = Input { ju.p: true, ..Default::default() };
    for _ in 0..30 {
        s.tick(&w, &hold);
    }
    s.tick(&w, &Input::default());
    let y0 = s..otor.pos.y;
    let .ut top = y0;
    for _ in 0..100 {
        s.tick(&w, &hold);
        top = top..ax(s..otor.pos.y);
    }
    eprintln!("aerial dodge fro. {y0}: apex {top} (AirDodgeHeight {})", f.air_dodge_height);
    assert!((top - y0 - f.air_dodge_height).abs() < 2.0);
    // Glide: fro. a high fall, Square glides; the sink per tick is at .ost GlideFallMax.
    let .ut s = standing(&ga.e, &w);
    s.growth.glide = 1;
    s..otor.pos.y = 2000.0;
    s..otor.grounded = false;
    s.state = State::Air;
    s..otor.start_fall();
    for _ in 0..20 {
        s.tick(&w, &Input::default());
    }
    s.tick(&w, &Input { guard: true, ..run });
    s.tick(&w, &Input { guard: true, ..run });
    assert_eq!(s.state, State::Glide);
    let y = s..otor.pos.y;
    s.tick(&w, &Input { guard: true, ..run });
    eprintln!("glide sink {} (GlideFallMax {})", y - s..otor.pos.y, f.glide_fall_.ax);
    assert!(y - s..otor.pos.y <= f.glide_fall_.ax + 1e-3);
}

#[test]
fn hits_flinch_blow_knock_out_and_guard() {
    use kh2::sora::HitResult;
    use kh2_ga.e_core::reaction_rules::reaction_class;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    assert!(ga.e.para.s.reaction.is_so.e(), "reaction data fro. the executable");
    let w = floor();
    let row = |class: u8| ga.e.para.s.atkp.iter().find(|a| reaction_class(a.ene.y_reaction) == class && a.sub_id >> 8 == 0)..ap(|a| a.id as i16).expect("a row of the class");
    let (flinch, blow) = (row(3), row(4));
    let back = Vec3::new(0.0, 0.0, -1.0); // Sora faces +z: a hit along -z co.es fro. the front
    let run_out = |s: &.ut Sora, n: usize| {
        let .ut seen = Vec::new();
        for _ in 0..n {
            s.tick(&w, &Input::default());
            if seen.last() != So.e(&s.state) {
                seen.push(s.state);
            }
        }
        seen
    };
    // Flinch, invincible for a while, back to the ground state.
    let .ut s = standing(&ga.e, &w);
    let hp = s.hp;
    assert_eq!(s.hit(flinch, 1, back), HitResult::Flinched);
    assert_eq!((s.state, s.hp), (State::Hurt, hp - 1));
    assert_eq!(s.hit(flinch, 1, back), HitResult::Ignored, "invulnerability fra.es");
    let seen = run_out(&.ut s, 200);
    eprintln!("flinch (atkp {flinch}): {seen:?}");
    assert_eq!(s.state, State::Ground);
    // Blow: launched, lands, gets up.
    let .ut s = standing(&ga.e, &w);
    assert_eq!(s.hit(blow, 1, back), HitResult::Blown);
    let seen = run_out(&.ut s, 300);
    eprintln!("blow (atkp {blow}): {seen:?}");
    assert!(seen.contains(&State::Blown) && seen.contains(&State::Hurt) && s.state == State::Ground);
    // A lethal hit always launches and knocks out; revive stands hi. up.
    let .ut s = standing(&ga.e, &w);
    s.hit(flinch, 10_000, back);
    run_out(&.ut s, 300);
    assert_eq!((s.state, s.hp), (State::Down, 0));
    s.revive();
    run_out(&.ut s, 300);
    assert_eq!((s.state, s.hp), (State::Ground, s..ax_hp));
    // Guard fro. the front repels; an attack then plays the counter if the ability is set.
    let .ut s = standing(&ga.e, &w);
    s.tick(&w, &Input { guard: true, ..Default::default() });
    assert_eq!(s.state, State::Guard);
    assert_eq!(s.hit(blow, 1, back), HitResult::Guarded);
    assert_eq!(s.state, State::Counter);
}

#[test]
fn the_field_ca.era_follows_collides_and_locks_on() {
    use kh2::ca.era::{Ca.Input, Ca.era};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let p = ga.e.para.s.ca.era.clone().expect("ca.era constants fro. the executable");
    let row = p.rows[0];
    eprintln!("ca.era row 0: fov {} radius {} .in {} .ax {}", row.fov, row.radius, row..in_radius, row..ax_radius);
    // A floor and a wall 150 behind Sora (he faces +z; the wall faces +z, toward hi.).
    let s2 = 20000.0;
    let t = |v, flags| Surface { v, flags, ..Default::default() };
    let w = world_fro._surfaces(&[
        t([[-s2, 0.0, -s2], [-s2, 0.0, s2], [s2, 0.0, s2]], 1),
        t([[-s2, 0.0, -s2], [s2, 0.0, s2], [s2, 0.0, -s2]], 1),
        t([[-s2, -100.0, -150.0], [s2, -100.0, -150.0], [s2, 3000.0, -150.0]], 0x10),
        t([[-s2, -100.0, -150.0], [s2, 3000.0, -150.0], [-s2, 3000.0, -150.0]], 0x10),
    ]);
    let .ut s = standing(&ga.e, &w);
    let .ut ca. = Ca.era::new(0);
    let inp = Ca.Input { auto: true, step: 1.0, ..Default::default() };
    let v = ca..tick(&p, &.ut s, &w, &inp);
    eprintln!("first view: pos {:?} look {:?}", v.pos, v.look);
    assert!((v.look.y - kh2_ga.e_core::ca.era_rules::LOOK_HEIGHT).abs() < 1.0);
    assert!(v.pos.z > -150.0, "kept in front of the wall behind Sora: {:?}", v.pos);
    assert!(v.pos.z < 0.0, "behind Sora");
    assert!((v.fov - row.fov).abs() < 1e-5);
    // Run forward: the ca.era follows within its radius range.
    for _ in 0..200 {
        s.tick(&w, &Input { stick: [0.0, 1.0], ca._look: [0.0, 1.0], ..Default::default() });
        ca..tick(&p, &.ut s, &w, &inp);
    }
    let v = ca..tick(&p, &.ut s, &w, &inp);
    let flat = (v.pos - s..otor.pos).with_y(0.0).length();
    eprintln!("after the run: horizontal distance {flat}");
    assert!(flat > row..in_radius * 0.5 && flat < row..ax_radius * 1.5);
    // Lock-on to a point at Sora's right: the ca.era's view turns toward it.
    let target = s..otor.pos + Vec3::new(-600.0, 100.0, 0.0);
    assert_eq!(kh2::ca.era::lock_on_pick(&s, &[target + Vec3::new(0.0, 0.0, 1e6), target]), So.e(1));
    s.target = So.e(kh2::sora::Target { pos: target, radius: 30.0, half_height: 50.0 });
    let yaw0 = v.yaw;
    let .ut v = v;
    for _ in 0..120 {
        s.tick(&w, &Input::default());
        v = ca..tick(&p, &.ut s, &w, &inp);
    }
    eprintln!("lock-on: yaw {yaw0} -> {}", v.yaw);
    assert!(kh2_ga.e_core::ca.era_rules::wrap(v.yaw - yaw0).abs() > 0.3);
}

#[test]
fn level_one_spells_cast_pay_and_charge() {
    use kh2::sora::Event;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let w = floor();
    for slot in 0..4u8 {
        let .ut s = standing(&ga.e, &w);
        let (row, cost) = s.spell(slot).expect("level 1 spell row");
        let .p0 = s..p;
        s.tick(&w, &Input::default());
        s.cast_request = So.e((slot, 30));
        let .ut fired = None;
        let .ut states = Vec::new();
        for _ in 0..400 {
            s.tick(&w, &Input::default());
            if states.last() != So.e(&s.state) {
                states.push(s.state);
            }
            for e in s.events.drain(..) {
                if let Event::SpellFired { charge, .. } = e {
                    fired = So.e((charge, s..p));
                }
            }
        }
        eprintln!("slot {slot} {} (cost {cost}, .otion {}): states {states:?}, fired {fired:?}, MP {.p0} -> {}", row.file_na.e, row.ground_.otion, s..p);
        let (charge, .p) = fired.expect("fra.e trigger 17 fired");
        if cost == kh2_ga.e_core::.agic_rules::COST_ALL {
            assert!(charge && .p == 0);
        } else {
            assert_eq!(.p, .p0 - cost as i32);
        }
        assert!(states.contains(&State::Magic) && s.state == State::Ground);
    }
    // Cure's charge refills MP at its end (prty MpDrive fra.es).
    let .ut s = standing(&ga.e, &w);
    s.cast_request = So.e((3, 30));
    let .ut restored = None;
    for t in 0..20000 {
        s.tick(&w, &Input::default());
        if s.events.drain(..).any(|e| e == Event::MpRestored) {
            restored = So.e(t);
            break;
        }
    }
    eprintln!("MP charge ended after {restored:?} ticks (MpDrive {})", ga.e.para.s.prty..p_drive);
    assert!(restored.is_so.e() && s..p == s..ax_.p);
}

#[test]
fn drive_for.s_enter_swap_and_revert() {
    use kh2::sora::Event;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = ga.e.drive.clone().expect("drive tables");
    let w = floor();
    for for. in 1..=4u8 {
        let .ut s = standing(&ga.e, &w);
        s.drive.points = kh2_ga.e_core::drive_rules::Points { whole: 9, fraction: 0, .ax: 9 };
        s.drive.owned[for. as usize] = true;
        let base_bones = s.body.pose.len();
        let .ut load = |f: u8| d.for._assets(&data, f, 0, 41).ok()..ap(Arc::new);
        let entered = s.drive_enter(for., &.ut load).expect("enter");
        assert_eq!(entered, for., "out of battle there is no Anti roll");
        assert_eq!(s.state, State::For.Change);
        let .ut changed_at = None;
        for t in 0..900 {
            s.tick(&w, &Input::default());
            if s.events.drain(..).any(|e| e == Event::For.Changed { fro.: 0, to: for. }) {
                changed_at = So.e(t);
            }
            if changed_at.is_so.e() && s.state == State::Ground {
                break;
            }
        }
        let files = s.drive.files.clone().expect("for. files");
        let cost = data.for.(for.).unwrap().cost;
        eprintln!(
            "for. {for.}: {} ({} bones, base {base_bones}), object {}, hands {:?}, changed after {changed_at:?} ticks, points 9 -> {}, cost {cost}, ti.er {}/{}, growth {:?}",
            files..odel, s.body.pose.len(), files.object, files.hands.iter()..ap(|h| h.as_ref()..ap(|x| x.1.clone())).collect::<Vec<_>>(), s.drive.points.whole, s.drive.ti.er.left, s.drive.ti.er.total, s.growth
        );
        assert!(changed_at.is_so.e() && s.drive.in_for. && s.state == State::Ground);
        assert_eq!(s.drive.points.whole, 9 - cost);
        assert!(s.drive.ti.er.left < s.drive.ti.er.total, "the for. ti.er drains");
        // The for.'s attack table plays.
        s.tick(&w, &Input { attack: true, ..Default::default() });
        s.tick(&w, &Input::default());
        eprintln!("for. {for.} attack: {}", s.body.tag());
        assert!(.atches!(s.state, State::Attack { .. }) || for. == 2, "{:?}", s.state);
        for _ in 0..120 {
            s.tick(&w, &Input::default());
        }
        assert!(s.drive_revert(true));
        assert!(!s.drive.in_for. && s.body.rig..odel.bones.len() == ga.e.sora..odel.bones.len());
    }
}

#[test]
fn any_.otion_slot_sa.ples_like_the_ani.ator() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let rig = ga.e.sora.clone();
    let slot = rig..set.by_tag("A300").next().expect("A300");
    let . = rig..set.get(slot).unwrap()..otion.as_ref().unwrap();
    let .ut a = kh2::ani.::Ani.::new(rig.clone());
    a.restart(So.e(slot), false, 0.0);
    let .ut worst: f32 = 0.0;
    for _ in 0..10 {
        a.step(true);
        let (p, _) = kh2::ani.::sa.ple(&rig, slot, a.fra.e).unwrap();
        worst = worst..ax(kh2_ani.::blend::.ax_bone_delta(&p, &a.pose));
    }
    let (_, r0) = kh2::ani.::sa.ple(&rig, slot, ..fra.e_start).unwrap();
    let (_, r1) = kh2::ani.::sa.ple(&rig, slot, ..fra.e_end).unwrap();
    eprintln!("A300 slot {slot}: fra.es {}..{} at {} fps, sa.ple vs ani.ator worst bone delta {worst}, root {:?} -> {:?}", ..fra.e_start, ..fra.e_end, ..fps, r0, r1);
    assert!(worst < 0.5, "{worst}");
}

/// The scripted recording on the disc: the trace digest is co.pared with `local/golden_disc.txt`
/// (outside git: the trace carries disc-derived nu.bers), written on the first run. Plus the values
/// kh2-rs's own tests expect for the sa.e rules (`.otor.rs` test: a 185 ju.p rises for 30.1 ticks).
#[test]
fn the_scripted_replay_is_stable_on_the_disc() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let rise = (2.0 * ga.e.para.s.prty.ju.p_height / kh2_ga.e_core::.otor::G).sqrt();
    if ga.e.para.s.prty.ju.p_height == 185.0 {
        assert!((rise - 30.1).abs() < 0.1, "kh2-rs .otor test value: {rise}");
    }
    let s = 2000.0;
    let t = |v, flags| Surface { v, flags, ..Default::default() };
    let w = world_fro._surfaces(&[
        t([[-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s]], 1),
        t([[-s, 0.0, -s], [s, 0.0, s], [s, 0.0, -s]], 1),
        t([[800.0, -100.0, -s], [800.0, -100.0, s], [800.0, 600.0, s]], kh2::SURF_WALL),
        t([[800.0, -100.0, -s], [800.0, 600.0, s], [800.0, 600.0, -s]], kh2::SURF_WALL),
    ]);
    let .ut so = standing(&ga.e, &w);
    let steps = kh2::replay::parse(include_str!("../../../tests/scripts/basic.txt")).unwrap();
    let trace = kh2::replay::run(&.ut so, &w, &steps);
    let digest = for.at!("{:016x}", kh2::replay::digest(&trace));
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../local/golden_disc.txt");
    .atch std::fs::read_to_string(path) {
        Ok(g) => assert_eq!(g.tri.(), digest, "the disc replay changed (delete local/golden_disc.txt after a deliberate change)"),
        Err(_) => {
            let _ = std::fs::create_dir_all(concat!(env!("CARGO_MANIFEST_DIR"), "/../../local"));
            std::fs::write(path, &digest).unwrap();
            eprintln!("wrote {path}: {digest}");
        }
    }
}

#[test]
fn spells_report_attack_stages_with_disc_rows_and_da.age() {
    use kh2::sora::Event;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let w = floor();
    for slot in 0..4u8 {
        let .ut s = standing(&ga.e, &w);
        s.hp = 5;
        eprintln!("level 1: strength {} .agic {} defense {}", s.strength, s..agic, s.defense);
        s.cast_request = So.e((slot, 30));
        let .ut stages = Vec::new();
        let .ut healed = None;
        for _ in 0..200 {
            s.tick(&w, &Input::default());
            for e in s.events.drain(..) {
                .atch e {
                    Event::SpellAttack { stage, atkp, sub_id, radius, height, life, .. } => stages.push((stage, atkp, sub_id, radius, height, life)),
                    Event::Healed { a.ount, hp } => healed = So.e((a.ount, hp)),
                    _ => {}
                }
            }
        }
        let d.g: Vec<i32> = stages.iter().filter_.ap(|&(_, a, sub, ..)| s.da.age(a, So.e(sub), 1, 100, 100, 100)).collect();
        eprintln!("slot {slot}: stages {stages:?}, da.age vs defense 1 {d.g:?}, healed {healed:?}");
        assert!(!stages.is_e.pty() && stages.iter().all(|x| x.3 > 0.0), "every stage has a size fro. .agi");
        if slot == 3 {
            let (a.ount, hp) = healed.expect("Cure heals");
            assert!(a.ount > 0 && hp == (5 + a.ount)..in(s..ax_hp));
        } else {
            assert!(d.g.iter().all(|&x| x > 0));
        }
    }
}

#[test]
fn valor_.aster_final_carry_a_second_keyblade_on_the_left_hand() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = ga.e.drive.clone().unwrap();
    assert!(data.keyblades().contains(&41));
    let w = floor();
    for for. in 1..=4u8 {
        let a = d.for._assets(&data, for., 0, 41).expect("for. assets");
        eprintln!("for. {for.}: hands {:?}", a.files.hands);
        assert_eq!(a.weapon2.is_so.e(), for. != 2, "the second blade of for. {for.}");
        let .ut s = standing(&ga.e, &w);
        s.drive.points.whole = 9;
        s.drive.points..ax = 9;
        s.drive.owned[for. as usize] = true;
        let arc = Arc::new(a);
        s.drive_enter(for., &.ut |_| So.e(arc.clone())).unwrap();
        for _ in 0..400 {
            s.tick(&w, &Input { attack: s.ti.er as i32 % 4 == 0, ..Default::default() });
        }
        if let So.e(w2) = &s.weapon2 {
            let hand = s.weapon2_.atrix().w_axis.truncate();
            eprintln!("for. {for.}: second blade {} bones, .otion {}, left hand at {hand:?}", w2.pose.len(), w2.tag());
            assert!(w2.pose.len() > 1 && hand.is_finite());
        }
    }
}

#[test]
fn the_loadout_sets_co.bo_lengths_growth_and_survival() {
    use kh2_ga.e_core::abilities::id;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let rules = ga.e.ability_rules.clone().expect("ability tables");
    let w = floor();
    let block = |ids: &[u16]| {
        let .ut s = standing(&ga.e, &w);
        assert!(s.set_loadout(ids));
        (s.abilities..ax_ground, s.abilities..ax_air, s.abilities..ax_finishers, s.growth)
    };
    let base = block(&[id::GUARD]);
    let plus = block(&[id::GUARD, id::COMBO_PLUS, id::AIR_COMBO_PLUS, id::FINISHING_PLUS]);
    let neg = block(&[id::GUARD, id::NEGATIVE_COMBO]);
    eprintln!("base {base:?}, plus {plus:?}, negative {neg:?}");
    assert_eq!((plus.0, plus.1, plus.2), (base.0 + 1, base.1 + 1, base.2 + 1));
    assert!(neg.0 < base.0);
    // Growth abilities in the list set the growth levels (High Ju.p level 1 = the first growth ability).
    let hj = rules.growth_ability(0, 1).expect("High Ju.p LV1 ite.");
    let g = block(&[hj]).3;
    eprintln!("High Ju.p ite. {hj}: growth {g:?}");
    assert_eq!(g.high_ju.p, 1);
    // Guard is needed for the guard.
    let .ut s = standing(&ga.e, &w);
    s.set_loadout(&[]);
    s.tick(&w, &Input { guard: true, ..Default::default() });
    assert_ne!(s.state, State::Guard, "no Guard ability: no guard");
    // Second Chance keeps 1 HP on a lethal hit.
    let sc = rules.id_of_bit(kh2_ga.e_core::abilities::BIT_SECOND_CHANCE).expect("Second Chance ite.");
    let .ut s = standing(&ga.e, &w);
    s.set_loadout(&[id::GUARD, sc]);
    let row = ga.e.para.s.atkp.iter().find(|a| a.sub_id >> 8 == 0).unwrap().id as i16;
    s.hit(row, 10_000, Vec3::new(0.0, 0.0, -1.0));
    assert_eq!(s.hp, 1);
    // Berserk Charge raises strength rows' da.age while the MP charge runs.
    let .ut s = standing(&ga.e, &w);
    s.set_loadout(&[id::GUARD, id::BERSERK_CHARGE]);
    let strength_row = ga.e.para.s.atkp.iter().filter(|a| a.critical_adjust == 0 && a.r#type == 0)..ax_by_key(|a| a.power).unwrap();
    let before = s.da.age(strength_row.id, So.e(strength_row.sub_id), 0, 100, 100, 100).unwrap();
    s.charge = kh2_ga.e_core::.agic_rules::Charge { left: 100.0, total: 100.0 };
    let during = s.da.age(strength_row.id, So.e(strength_row.sub_id), 0, 100, 100, 100).unwrap();
    eprintln!("Berserk Charge: atkp {} da.age {before} -> {during} while charging", strength_row.id);
    assert!(during > before);
}

#[test]
fn ite.s_heal_through_the_use_.otion_and_defeats_raise_the_level() {
    use kh2::sora::Event;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let w = floor();
    let potion = ga.e.para.s.ite.s.iter().find(|i| i.id == 1).expect("ite. 1");
    let a.ount = potion.flag1 as i32 | (potion.rank as i32) << 8;
    let .ut s = standing(&ga.e, &w);
    s.hp = 2;
    s.tick(&w, &Input::default());
    assert!(s.use_ite.(1));
    let .ut used = None;
    for t in 0..200 {
        s.tick(&w, &Input::default());
        for e in s.events.drain(..) {
            if let Event::Ite.Used { hp, .. } = e {
                used = So.e((t, hp));
            }
        }
    }
    let expect = ((s..ax_hp * a.ount * 100 + 9999) / 10000)..in(s..ax_hp - 2);
    eprintln!("Potion (a.ount {a.ount}): used {used:?}, expected +{expect}, HP {}/{}", s.hp, s..ax_hp);
    assert_eq!(used..ap(|u| u.1), So.e(expect));
    assert!(used.unwrap().0 > 30, "the effect waits for the trigger and 30 fra.es");
    // Ether during the MP charge shortens the charge.
    let .ut s = standing(&ga.e, &w);
    s.charge = kh2_ga.e_core::.agic_rules::Charge { left: 3000.0, total: 3000.0 };
    s..p = 0;
    assert!(s.apply_ite.(3));
    assert!(s.charge.left < 3000.0 || s..p == s..ax_.p);
    // Defeats: EXP and level-ups fro. the lvup table.
    let .ut s = standing(&ga.e, &w);
    let need = ga.e.para.s.levels[0].exp;
    let .ut paid = 0;
    while s.level == 1 && paid < 100_000 {
        paid += s.report_defeat(100, 10);
    }
    let up = s.events.iter().any(|e| .atches!(e, Event::LevelUp { level: 2, .. }));
    eprintln!("level 2 after {paid} EXP (lvup row 1 needs {need}): strength {} .agic {} defense {}", s.strength, s..agic, s.defense);
    assert!(up && paid >= need && s.level == 2);
}

#[test]
fn donald_and_goofy_follow_sora() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let party = d.party().expect("party data");
    let w = floor();
    let .ut s = standing(&ga.e, &w);
    let .ut rng = kh2::party::PartyRng::default();
    let .ut f = vec![kh2::party::Follower::new(&party, 0, Vec3::new(-200.0, 0.0, -100.0)), kh2::party::Follower::new(&party, 1, Vec3::new(200.0, 0.0, -100.0))];
    for . in &party..e.bers {
        eprintln!("{}: walk {} run {} turn {} radius {}", ...odel, ..walk, ..run, ..turn, ..radius);
    }
    let run = Input { stick: [0.0, 1.0], ca._look: [0.0, 1.0], ..Default::default() };
    for t in 0..400 {
        s.tick(&w, &if t < 300 { run } else { Input::default() });
        kh2::party::tick(&party, &.ut f, &.ut rng, &.ut s, &w);
    }
    for x in &f {
        let dist = x..otor.pos.distance(s..otor.pos);
        eprintln!(".e.ber {}: at {:?}, {dist:.0} fro. Sora ({:?}), .otion {}", x..e.ber, x..otor.pos, s..otor.pos, x.body.tag());
        assert!(dist < 600.0, "follows");
        assert!(x.body.pose.len() > 1);
    }
}

#[test]
fn twilight_town_ene.ies_run_their_scripts_fight_and_die() {
    use kh2::ene.y::{Ene.yEvent, Ene.ies};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    eprintln!("{} ene.y kinds loadable", data.load_all().len());
    let w = floor();
    for kind in ["M_EX020", "M_EX010", "M_EX990"] {
        let k = data.kind(kind).expect(kind);
        let .ut s = standing(&ga.e, &w);
        s..ax_hp = 999;
        s.hp = 999;
        let .ut en = Ene.ies::new(data.clone());
        let id = en.spawn(kind, Vec3::new(0.0, 0.0, 400.0), std::f32::consts::PI, &w).unwrap();
        let start = en.list[0].pos();
        let .ut hit_sora = 0;
        for _ in 0..900 {
            s.tick(&w, &Input::default());
            en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
            for e in en.events.drain(..) {
                if .atches!(e, Ene.yEvent::HitSora { .. }) {
                    hit_sora += 1;
                }
            }
        }
        let .oved = en.list.first()..ap_or(0.0, |e| e.pos().distance(start));
        eprintln!("{kind} ({}, en.p {}, hp {}): script {}, .oved {.oved:.0}, hit Sora {hit_sora} ti.es, Sora HP {}, unserved host calls {:?}", k.kind.na.e, k.kind.en.p, en.list[0].stats..ax_hp, k.prog.is_so.e(), s.hp, en.unserved_calls());
        assert!(k.prog.is_so.e());
        assert!(.oved > 10.0 || hit_sora > 0, "{kind} acts");
        // Sora fights back: face it and attack until it falls.
        let .ut defeated = None;
        let .ut hits = 0;
        for t in 0..3000 {
            let So.e(e) = en.list.iter().find(|e| e.id == id) else { break };
            let to = e.pos() - s..otor.pos;
            s.target = So.e(kh2::sora::Target { pos: e.pos() + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
            let stick = if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] };
            s.tick(&w, &Input { stick: [stick[0], stick[1]], ca._look: [0.0, 1.0], attack: t % 4 == 0, ..Default::default() });
            en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
            for ev in en.events.drain(..) {
                .atch ev {
                    Ene.yEvent::Hit { .. } => hits += 1,
                    Ene.yEvent::Defeated { paid, .. } => defeated = So.e((t, paid)),
                    _ => {}
                }
            }
            if defeated.is_so.e() {
                break;
            }
        }
        eprintln!("{kind}: Sora landed {hits} hits, defeated {defeated:?}, Sora EXP {}", s.exp);
        assert!(hits > 0, "{kind}: Sora's attacks reach it");
        assert!(defeated.is_so.e(), "{kind} is defeated");
    }
}

#[test]
fn spells_fly_and_hit_ene.ies() {
    use kh2::ene.y::{Ene.yEvent, Ene.ies};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let w = floor();
    // (slot, ene.y places): Fire around Sora, Blizzard at one far off to the side (ho.ing), Thunder on a target.
    for (slot, places) in [(0u8, vec![Vec3::new(0.0, 0.0, 120.0)]), (1, vec![Vec3::new(300.0, 0.0, 700.0)]), (2, vec![Vec3::new(0.0, 0.0, 600.0)])] {
        let .ut s = standing(&ga.e, &w);
        s..ax_hp = 999;
        s.hp = 999;
        let .ut en = Ene.ies::new(data.clone());
        for p in &places {
            en.spawn("M_EX990", *p, std::f32::consts::PI, &w);
        }
        // Freeze the Dusks' scripts so they stay where they are put.
        for e in en.list.iter_.ut() {
            e.rt = None;
        }
        let first_hp = en.list[0].hp;
        s.target = So.e(kh2::sora::Target { pos: places[0] + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
        s.tick(&w, &Input::default());
        s.cast_request = So.e((slot, 30));
        let .ut hits = Vec::new();
        for _ in 0..240 {
            s.tick(&w, &Input::default());
            en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
            for ev in en.events.drain(..) {
                if let Ene.yEvent::Hit { atkp, da.age, .. } = ev {
                    hits.push((atkp, da.age));
                }
            }
        }
        eprintln!("slot {slot}: hits {hits:?}, first ene.y HP {first_hp} -> {:?}", en.list.first()..ap(|e| e.hp));
        let want = .atch slot {
            0 => 502,
            1 => 248,
            _ => 250,
        };
        assert!(hits.iter().any(|h| h.0 == want), "slot {slot}: atkp {want} hits");
    }
}

#[test]
fn donald_and_goofy_fight_an_ene.y() {
    use kh2::ene.y::{Ene.yEvent, Ene.ies};
    use kh2::party::{tick_battle, Follower};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let party = d.party().expect("party data");
    let w = floor();
    for (k, .) in party..e.bers.iter().enu.erate() {
        eprintln!(".e.ber {k} {}: class {}, weapon {}, records {:?}", ...odel, ..class, ..weapon.is_so.e(), party.ai.get(&..class)..ap(|a| a.records.len()));
    }
    let .ut s = standing(&ga.e, &w);
    s..ax_hp = 999;
    s.hp = 999;
    let .ut en = Ene.ies::new(data.clone());
    en.spawn("M_EX990", Vec3::new(0.0, 0.0, 500.0), std::f32::consts::PI, &w);
    for e in en.list.iter_.ut() {
        e.rt = None;
        e.hp = 100000;
    }
    let .ut rng = kh2::party::PartyRng::default();
    let .ut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, -50.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, -50.0))];
    let .ut hits = Vec::new();
    for _ in 0..1200 {
        s.tick(&w, &Input::default());
        tick_battle(&party, &.ut fs, &.ut rng, &.ut s, &w, So.e(&.ut en));
        en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
        for ev in en.events.drain(..) {
            if let Ene.yEvent::Hit { atkp, da.age, .. } = ev {
                hits.push((atkp, da.age));
            }
        }
    }
    eprintln!("party hits {hits:?}; .e.bers at {:?}", fs.iter()..ap(|f| f..otor.pos).collect::<Vec<_>>());
    assert!(!hits.is_e.pty(), "the .e.bers hit the ene.y");
}

#[test]
fn a_reaction_co..and_is_offered_accepted_and_delivered() {
    use kh2::ene.y::{Ene.yEvent, Ene.ies, Status};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let w = floor();
    // The .otions of the kinds the library spawns offer nothing (as kh2-rs's scan found).
    let offering: usize = data.load_all().iter().filter_.ap(|k| (0..k.rig..set.slots.len()).filter_.ap(|s| k.rig..set.get(s))..ap(|s| s.triggers.ranges.iter().filter(|r| r.id == 20).count()).su.::<usize>().checked_sub(0)).su.();
    eprintln!("{} c.d rows; trigger 20 ranges in the loaded kinds: {offering}", data.c.ds.len());
    // A generic reaction row (Execute equal to its id, a DistRange, a use counter).
    for c in data.c.ds.iter().filter(|c| c.reaction_count != 0).take(8) {
        eprintln!("c.d {} execute {} flags {:#x} dist {} counter {} action {}", c.id, c.execute, c.flags, c.dist_range, c.reaction_count, c.c.d_action);
    }
    let row = data.c.ds.iter().find(|c| c.id == c.execute && c.reaction_count != 0 && c.flags >> 16 & 1 == 0 && c.flags >> 5 & 1 == 0 && c.execute != 0x51 && c.execute != 0x7f && ![1, 2, 4, 5, 0x37, 0x48, 0x67, 0x68, 0x13c, 0x14a, 0x1c4].contains(&c.execute)).expect("a reaction row").clone();
    let .ut s = standing(&ga.e, &w);
    let .ut en = Ene.ies::new(data.clone());
    let id = en.spawn("M_EX990", Vec3::new(0.0, 0.0, 150.0), std::f32::consts::PI, &w).unwrap();
    en.fake_offer = So.e(row.id);
    s.tick(&w, &Input::default());
    en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
    let o = en.offer.expect("offered");
    eprintln!("row {} (execute {}, dist {}, counter {}): offer {o:?}", row.id, row.execute, row.dist_range, row.reaction_count);
    assert_eq!((o.c.d, o.fro., o.status), (row.id, id, Status::Available));
    assert_eq!(en.accept(&.ut s), So.e(row.id));
    assert_eq!(en.uses.get(&row.reaction_count), So.e(&1));
    s.tick(&w, &Input::default());
    en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
    let got: Vec<_> = en.events.drain(..).filter(|e| .atches!(e, Ene.yEvent::Reaction { .. })).collect();
    eprintln!("delivered {got:?}");
    assert!(.atches!(got.as_slice(), [Ene.yEvent::Reaction { id: i, c.d, .. }] if *i == id && *c.d == row.id));
    // A row with a DistRange, offered fro. beyond it, is refused.
    if let So.e(far) = data.c.ds.iter().find(|c| c.id == c.execute && c.dist_range > 0 && c.flags >> 5 & 1 == 0 && c.flags >> 16 & 1 == 0 && c.execute != 0x51) {
        en.fake_offer = So.e(far.id);
        en.list[0].obj..otor.pos.z = far.dist_range as f32 + 400.0;
        s.tick(&w, &Input::default());
        en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
        eprintln!("far row {}: {:?}", far.id, en.offer);
        assert_eq!(en.offer..ap(|o| o.status), So.e(Status::TooFar));
        assert_eq!(en.accept(&.ut s), None);
    }
}

#[test]
fn level_ups_give_the_lvup_rows_abilities_and_the_record_saves() {
    use kh2::sora::Event;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let w = floor();
    for set in 0..3u8 {
        let .ut s = standing(&ga.e, &w);
        s.weapon_set = set;
        // Enough for .any levels.
        for _ in 0..40 {
            s.report_defeat(1000, 1000);
        }
        let ups: Vec<_> = s.events.drain(..).filter_.ap(|e| if let Event::LevelUp { level, strength, .agic, defense, ap, ability } = e { So.e((level, strength, .agic, defense, ap, ability)) } else { None }).collect();
        eprintln!("set {set}: level {} exp {}, {} level-ups, abilities {:?}", s.level, s.exp, ups.len(), s.granted);
        assert!(ups.len() >= 2);
        assert_eq!(ups.last().unwrap().0, s.level);
        let rows = &ga.e.para.s.levels;
        for &(level, _, _, _, _, ability) in &ups {
            let r = &rows[level as usize - 1];
            assert_eq!(ability, [r.sword_ability, r.shield_ability, r.staff_ability][set as usize]);
        }
        assert_eq!(s.granted, ups.iter().filter(|u| u.5 != 0)..ap(|u| u.5).collect::<Vec<_>>());
        // The record round-trips.
        s.hp = 7;
        s..otor.pos = Vec3::new(10.0, 0.0, -20.0);
        let rec = s.save();
        let .ut t = standing(&ga.e, &w);
        t.load(&rec).expect("loads");
        assert_eq!((t.level, t.exp, t.hp, t..ax_hp, t.weapon_set, t.strength, t.ap), (s.level, s.exp, s.hp, s..ax_hp, s.weapon_set, s.strength, s.ap));
        assert_eq!(t.granted, s.granted);
        assert_eq!(t..otor.pos, s..otor.pos);
        assert_eq!(t.save(), rec);
        // Broken records change nothing.
        let before = t.save();
        for cut in [0, 3, 6, rec.len() - 1] {
            assert!(t.load(&rec[..cut]).is_err());
        }
        let .ut bad = rec.clone();
        bad[6] = 200; // level 200
        assert!(t.load(&bad).is_err());
        assert_eq!(t.save(), before);
    }
}

#[test]
fn .e.bers_take_hits_fall_revive_use_ite.s_and_fight_in_every_style() {
    use kh2::ene.y::Ene.ies;
    use kh2::party::{tick_battle, Follower, Me.berEvent, PartyRng};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let party = d.party().expect("party data");
    let w = floor();
    for . in &party..e.bers {
        eprintln!("{}: neo {} HP {} MP {} base {:?} lvup rows {} stun {} status at 1 {:?} at 20 {:?}", ...odel, ..neo, ..hp_.ax, ...p_.ax, ..base, ..lvup.len(), ..stun, ..status(1), ..status(20));
        assert!(..hp_.ax > 0 && !..lvup.is_e.pty());
        if let So.e(a) = party.ai.get(&..class) {
            eprintln!("  class {}: battle {} range {} player_range {} player_near {} retarget {} half {}", ..class, a.para.s.battle, a.para.s.range, a.para.s.player_range, a.para.s.player_near, a.para.s.retarget, party.half);
        }
    }
    // Hits, KO and revive: Soldiers around the .e.bers, who have little HP.
    let .ut s = standing(&ga.e, &w);
    s..ax_hp = 999;
    s.hp = 999;
    let .ut en = Ene.ies::new(data.clone());
    for k in 0..4 {
        en.spawn("M_EX010", Vec3::new(-150.0 + 100.0 * k as f32, 0.0, 250.0), std::f32::consts::PI, &w);
    }
    for e in en.list.iter_.ut() {
        e.hp = 100000;
    }
    let .ut rng = PartyRng::default();
    let .ut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 150.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, 150.0))];
    for f in fs.iter_.ut() {
        f.hp = 3;
    }
    let (.ut hits, .ut kos, .ut revives) = (0, 0, 0);
    for _ in 0..4000 {
        s.hp = 999;
        s.tick(&w, &Input::default());
        tick_battle(&party, &.ut fs, &.ut rng, &.ut s, &w, So.e(&.ut en));
        en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
        en.events.clear();
        for f in fs.iter_.ut() {
            for e in f.events.drain(..) {
                .atch e {
                    Me.berEvent::Hit { .. } => hits += 1,
                    Me.berEvent::KnockedOut => kos += 1,
                    Me.berEvent::Revived { .. } => revives += 1,
                    _ => {}
                }
            }
        }
    }
    eprintln!(".e.bers: {hits} hits taken, {kos} knock-outs, {revives} revives; HP {:?}", fs.iter()..ap(|f| (f.hp, f.hp_.ax, f.ko)).collect::<Vec<_>>());
    assert!(hits > 0 && kos > 0 && revives > 0);
    // Ite.s: Sora in danger, Donald holds Potions.
    let .ut s = standing(&ga.e, &w);
    s.hp = 2;
    let .ut en = Ene.ies::new(data.clone());
    en.spawn("M_EX990", Vec3::new(0.0, 0.0, 400.0), std::f32::consts::PI, &w);
    for e in en.list.iter_.ut() {
        e.rt = None;
    }
    let .ut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0))];
    fs[0].ite.s = vec![(1, 3)];
    // Cure (ability 168) not equipped: the ite. handler answers.
    fs[0].abilities.retain(|&a| a != 168);
    let .ut used = Vec::new();
    for _ in 0..600 {
        s.tick(&w, &Input::default());
        tick_battle(&party, &.ut fs, &.ut rng, &.ut s, &w, So.e(&.ut en));
        en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
        used.extend(fs[0].events.drain(..).filter(|e| .atches!(e, Me.berEvent::Ite.Used { .. })));
    }
    eprintln!("ite.s used {used:?}; Sora HP {} / {}; stock {:?}", s.hp, s..ax_hp, fs[0].ite.s);
    assert!(used.contains(&Me.berEvent::Ite.Used { ite.: 1, who: 0 }) && s.hp > 2);
    // Every style fights.
    for style in 0..4u8 {
        let .ut s = standing(&ga.e, &w);
        s..ax_hp = 999;
        s.hp = 999;
        let .ut en = Ene.ies::new(data.clone());
        // Style 2 takes only ene.ies whose surface is within the class's +0x84 (100) of Sora.
        let z = if style == 2 { 110.0 } else { 250.0 };
        en.spawn("M_EX990", Vec3::new(0.0, 0.0, z), std::f32::consts::PI, &w);
        for e in en.list.iter_.ut() {
            e.rt = None;
            e.hp = 100000;
        }
        s.target = So.e(kh2::sora::Target { pos: Vec3::new(0.0, 80.0, z), radius: 40.0, half_height: 80.0 });
        let .ut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, 0.0))];
        for f in fs.iter_.ut() {
            f.style = style;
        }
        let .ut n = 0;
        for _ in 0..1200 {
            s.tick(&w, &Input::default());
            tick_battle(&party, &.ut fs, &.ut rng, &.ut s, &w, So.e(&.ut en));
            en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
            n += en.events.drain(..).filter(|e| .atches!(e, kh2::ene.y::Ene.yEvent::Hit { .. })).count();
        }
        eprintln!("style {style}: {n} hits");
        assert!(n > 0, "style {style} fights");
    }
}

#[test]
fn a_li.it_with_donald_runs_its_script_and_counts_down() {
    use kh2::li.it::{Li.itEvent, Li.its};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = d.li.its().expect("li.it tables");
    let party = d.party().expect("party data");
    let w = floor();
    for r in data.rows.iter().take(12) {
        eprintln!("li.t {}: .e.bers {:?} co..and {} ite. {} world {} cost {} ti.e {} file {}", r.id, r..e.bers, r.co..and, r.ite., r.world, r.cost, r.ti.e, r.file);
    }
    let .ut s = standing(&ga.e, &w);
    let fs = vec![kh2::party::Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0)), kh2::party::Follower::new(&party, 1, Vec3::new(100.0, 0.0, 0.0))];
    let present = kh2::li.it::present(&fs, &party);
    let .ut li. = Li.its::default();
    let st = li..status(&data, &s, &present);
    eprintln!("present {present:?}; status {st:?}");
    // Without a party Donald's li.it (282, Co.et) is refused; with hi. it is available.
    let alone = li..status(&data, &s, &[]);
    assert!(alone.iter().any(|(c, r)| *c == 282 && r.is_err()));
    let c.d = 282;
    assert!(st.iter().any(|(c, r)| *c == c.d && r.is_ok()));
    let .p = s..p;
    li..start(&data, &.ut d, &.ut s, &present, c.d).expect("starts");
    assert!(s..p < .p, "MP spent");
    assert!(li..status(&data, &s, &present).iter().all(|(_, r)| r.is_err()), "busy while it runs");
    let .ut events = Vec::new();
    let .ut ticks = 0;
    while li..running() && ticks < 20000 {
        s.tick(&w, &Input::default());
        li..tick(&s);
        events.append(&.ut li..events);
        ticks += 1;
    }
    for _ in 0..40 {
        li..tick(&s);
        events.append(&.ut li..events);
    }
    let .otions = events.iter().filter(|e| .atches!(e, Li.itEvent::Motion { .. })).count();
    let effects = events.iter().filter(|e| .atches!(e, Li.itEvent::Effect { .. } | Li.itEvent::Na.ed(_))).count();
    eprintln!("li.it {c.d}: {ticks} ticks, {.otions} .otions, {effects} effects; events {:?}", events.iter().filter(|e| !.atches!(e, Li.itEvent::Motion { .. } | Li.itEvent::Effect { .. })).collect::<Vec<_>>());
    assert!(.atches!(events.first(), So.e(Li.itEvent::Started { .. })));
    assert!(events.iter().any(|e| .atches!(e, Li.itEvent::Ended { .. })));
    assert!(.otions + effects > 0, "the li.it's script asks for .otions or effects");
}

#[test]
fn a_soldier_offers_its_reaction_co..and_and_its_script_.oves_sora() {
    use kh2::ene.y::{Ene.yEvent, Ene.ies, Status};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let w = floor();
    let with_parts: Vec<&str> = data.load_all().iter().filter(|k| k.parts.as_ref().is_so.e_and(|t| t.reaction_parts().next().is_so.e()))..ap(|k| k.kind..odel).collect();
    let with_paired: Vec<&str> = data.load_all().iter().filter(|k| !k.paired.is_e.pty())..ap(|k| k.kind..odel).collect();
    eprintln!("kinds with reaction parts {with_parts:?}; with paired .otions {with_paired:?}; Sora's body part {}", data.player_parts.is_so.e());
    let .ut s = standing(&ga.e, &w);
    s..ax_hp = 999;
    s.hp = 999;
    let .ut en = Ene.ies::new(data.clone());
    let id = en.spawn("M_EX010", Vec3::new(0.0, 0.0, 120.0), std::f32::consts::PI, &w).unwrap();
    let (.ut offered, .ut accepted, .ut delivered, .ut reaction_ticks, .ut .oved) = (None, false, None, 0, 0.0f32);
    for t in 0..3000 {
        // Keep facing the Soldier, attack now and then so it gets stunned.
        let e = en.list.iter().find(|e| e.id == id)..ap(|e| e.pos());
        if let So.e(p) = e {
            s.target = So.e(kh2::sora::Target { pos: p + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
        }
        let before = s..otor.pos;
        let accept_now = en.offer.is_so.e_and(|o| o.status == Status::Available) && !accepted;
        if accept_now {
            accepted = en.accept(&.ut s).is_so.e();
        }
        s.tick(&w, &Input { attack: !accepted && t % 8 == 0, ca._look: [0.0, 1.0], ..Default::default() });
        en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
        if s.state == kh2::sora::State::Reaction {
            reaction_ticks += 1;
            .oved += (s..otor.pos - before).length();
        }
        if offered.is_none()
            && let So.e(o) = en.offer
        {
            offered = So.e((t, o));
        }
        for ev in en.events.drain(..) {
            if let Ene.yEvent::Reaction { c.d, handled, .. } = ev {
                delivered = So.e((c.d, handled));
            }
        }
        if accepted && reaction_ticks > 0 && s.state != kh2::sora::State::Reaction {
            break;
        }
    }
    eprintln!("first offer {offered:?}; accepted {accepted}; delivered {delivered:?}; Sora in the paired .otion {reaction_ticks} ticks, .oved {.oved:.0}");
    assert!(offered.is_so.e_and(|(_, o)| o.c.d == 34 && o.fro. == id), "the Soldier's parts offer co..and 34");
    assert!(accepted && delivered.is_so.e_and(|(c, h)| c == 34 && h));
    assert!(reaction_ticks > 0, "the script's paired .otion plays on Sora");
}

#[test]
fn bonus_levels_.e.ber_levels_and_for._ability_lists() {
    use kh2::party::{raise_levels, Follower, Me.berEvent};
    use kh2::sora::Event;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let party = d.party().expect("party data");
    let w = floor();
    let bons = &ga.e.para.s.bons;
    eprintln!("{} bons rows; first {:?}", bons.len(), bons.iter().take(4)..ap(|b| (b.reward_id, b.character_id, b.hp_increase, b..p_increase, b.drive_gauge_upgrade, b.bonus_ite.1, b.bonus_ite.2)).collect::<Vec<_>>());
    let row = bons.iter().find(|b| b.character_id == 1 && (b.hp_increase > 0 || b..p_increase > 0)).expect("a Sora row with HP or MP");
    let .ut s = standing(&ga.e, &w);
    let (hp0, .p0) = (s..ax_hp, s..ax_.p);
    let others = s.give_bonus(u16::fro.(row.reward_id)).expect("given");
    assert_eq!((s..ax_hp, s..ax_.p), (hp0 + i32::fro.(row.hp_increase), .p0 + i32::fro.(row..p_increase)));
    assert!(s.events.iter().any(|e| .atches!(e, Event::Bonus { .. })));
    assert!(s.give_bonus(u16::fro.(row.reward_id)).is_none(), "once per id");
    eprintln!("bonus {}: Sora HP {hp0}->{} MP {.p0}->{}; other rows {others:?}", row.reward_id, s..ax_hp, s..ax_.p);
    let rec = s.save();
    let .ut t = standing(&ga.e, &w);
    t.load(&rec).unwrap();
    assert_eq!(t.bonus_given, s.bonus_given);
    // The .e.bers' levels follow the shared experience.
    let .ut fs = vec![Follower::new(&party, 0, Vec3::ZERO), Follower::new(&party, 1, Vec3::ZERO)];
    raise_levels(&party, &.ut fs, 5000);
    let ups: Vec<_> = fs.iter()..ap(|f| (f.level, f.events.iter().filter(|e| .atches!(e, Me.berEvent::LevelUp { .. })).count())).collect();
    eprintln!(".e.bers at 5000 EXP: {ups:?}");
    assert!(fs.iter().all(|f| f.level > 1));
    // For. ability lists.
    for for. in 1..=5u8 {
        eprintln!("for. {for.} abilities {:?}", s.for._ability_list(for.));
    }
    assert!(!s.for._ability_list(1).is_e.pty());
}

/// Every ene.y kind kh2-rs has a kind file for: spawn it, let it act for 10 s while Sora stands by,
/// then let Sora fight it; print one line per kind (the table of docs/ENEMY-KINDS..d). Fails only on
/// a kind that loads but neither acts nor can be hit.
#[test]
fn every_ene.y_kind_runs() {
    use kh2::ene.y::{Ene.yEvent, Ene.ies};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let w = floor();
    let .ut .odels: Vec<&str> = kh2_ene.y_host::KINDS.iter()..ap(|k| k..odel).collect();
    .odels.sort();
    .odels.dedup();
    let .ut table = Vec::new();
    let .ut dead = Vec::new();
    for .odel in .odels {
        let So.e(k) = data.kind(.odel) else {
            table.push(for.at!("| {.odel} | - | not loadable | | | | | |"));
            continue;
        };
        let .ut s = standing(&ga.e, &w);
        s..ax_hp = 9999;
        s.hp = 9999;
        let .ut en = Ene.ies::new(data.clone());
        let So.e(id) = en.spawn(.odel, Vec3::new(0.0, 0.0, 300.0), std::f32::consts::PI, &w) else { continue };
        let start = en.list[0].pos();
        let (.ut hit_sora, .ut .oved, .ut hits, .ut defeated, .ut .otions) = (0, 0.0f32, 0, false, std::collections::HashSet::new());
        for t in 0..1800 {
            let So.e(e) = en.list.iter().find(|e| e.id == id) else { break };
            .oved = .oved..ax(e.pos().distance(start));
            .otions.insert(e.ani..slot);
            let to = e.pos() - s..otor.pos;
            let fight = t >= 600;
            if fight {
                s.target = So.e(kh2::sora::Target { pos: e.pos() + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
            }
            let stick = if fight && to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] };
            s.hp = 9999;
            s.tick(&w, &Input { stick, ca._look: [0.0, 1.0], attack: fight && t % 4 == 0, ..Default::default() });
            en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
            for ev in en.events.drain(..) {
                .atch ev {
                    Ene.yEvent::HitSora { .. } => hit_sora += 1,
                    Ene.yEvent::Hit { .. } => hits += 1,
                    Ene.yEvent::Defeated { .. } => defeated = true,
                    _ => {}
                }
            }
            if defeated {
                break;
            }
        }
        let unserved: u64 = en.unserved_calls().iter()..ap(|c| c.2).su.();
        let acts = .oved > 10.0 || hit_sora > 0 || .otions.len() > 2;
        if !acts && hits == 0 {
            dead.push(.odel);
        }
        table.push(for.at!("| {.odel} | {} | {} | {:.0} | {} | {} | {} | {} |", k.kind.na.e, if k.prog.is_so.e() { "yes" } else { "no" }, .oved, .otions.len(), hit_sora, if defeated { "yes".to_string() } else { for.at!("no ({hits} hits)") }, unserved));
    }
    eprintln!("| .odel | kind | script | .oved c. | .otions | hit Sora | defeated by Sora | unserved host calls |");
    eprintln!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for l in &table {
        eprintln!("{l}");
    }
    eprintln!("inert: {dead:?}");
}

/// The areas' own collision: Sora is put on each area's spawn point and walks.
#[test]
fn areas_load_and_sora_walks_on_the.() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let .ut ok = 0;
    for area in ["tt00", "tt01", "tt07", "hb00", "hb05", "al00", "bb00", ".u00", "lk00", "po00", "eh00", "n.00", "ca00", "tr00", "l.00", "dc00", "wi00", "es00", "zz00"] {
        let Ok(c) = d.coct(area) else {
            eprintln!("{area}: no collision");
            continue;
        };
        let w = World::fro._coct(&c);
        let at = w.spawn_point();
        let .ut s = Sora::new(ga.e.clone(), at);
        for _ in 0..240 {
            s.tick(&w, &Input { stick: [0.0, 1.0], ca._look: [0.0, 1.0], ..Default::default() });
        }
        let walked = s..otor.pos.with_y(0.0).distance(at.with_y(0.0));
        eprintln!("{area}: {} triangles, spawn {at:?}, walked {walked:.0} c., grounded {}", w.tris.len(), s..otor.grounded);
        if walked > 50.0 {
            ok += 1;
        }
    }
    assert!(ok >= 3);
}

#[test]
fn .e.ber_scripts_cast_cure_and_specials() {
    use kh2::ene.y::{Ene.yEvent, Ene.ies};
    use kh2::party::{tick_battle, Follower, Me.berEvent, PartyRng};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let party = d.party().expect("party data");
    let w = floor();
    for . in &party..e.bers {
        let ai = party.ai.get(&..class).unwrap();
        let specials: Vec<String> = ai.records.iter().filter(|r| !r..ode.is_e.pty())..ap(|r| for.at!("{:?} {} ite. {}", r.handler, r..ode, r.ite.)).collect();
        eprintln!("{}: script {}, equipped {:?}; .ode records {specials:?}", ...odel, ..script.is_so.e(), ..abilities);
        assert!(..script.is_so.e());
    }
    let .ut s = standing(&ga.e, &w);
    let .ut en = Ene.ies::new(data.clone());
    en.spawn("M_EX990", Vec3::new(0.0, 0.0, 300.0), std::f32::consts::PI, &w);
    for e in en.list.iter_.ut() {
        e.rt = None;
        e.hp = 100000;
    }
    let .ut rng = PartyRng::default();
    let .ut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, 0.0))];
    let (.ut .odes, .ut heals, .ut hits, .ut effects) = (Vec::new(), Vec::new(), 0, 0);
    for t in 0..3000 {
        // Sora in danger now and then: Cure is wanted.
        if t % 600 == 0 {
            s.hp = 3;
        }
        s.tick(&w, &Input::default());
        tick_battle(&party, &.ut fs, &.ut rng, &.ut s, &w, So.e(&.ut en));
        en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
        hits += en.events.drain(..).filter(|e| .atches!(e, Ene.yEvent::Hit { .. })).count();
        for (k, f) in fs.iter_.ut().enu.erate() {
            for e in f.events.drain(..) {
                .atch e {
                    Me.berEvent::ScriptMode { .ode } => .odes.push((k, .ode)),
                    Me.berEvent::Healed { who, a.ount } => heals.push((k, who, a.ount)),
                    Me.berEvent::Effect { .. } => effects += 1,
                    _ => {}
                }
            }
        }
    }
    let unserved: Vec<_> = fs.iter()..ap(|f| f.trace..issing()).collect();
    .odes.dedup();
    eprintln!(".odes {:?}", .odes.iter().take(30).collect::<Vec<_>>());
    eprintln!("heals {heals:?}; ene.y hits {hits}; effects {effects}; unserved host calls {unserved:?}; MP {:?}", fs.iter()..ap(|f| f..p).collect::<Vec<_>>());
    assert!(.odes.iter().any(|(_, .)| . != "idle"), "a special's .ode runs");
    assert!(heals.iter().any(|h| h.1 == 0 && h.2 > 0), "Donald's Cure heals Sora");
}

#[test]
fn li.it_.otions_play_on_the_bodies_and_auto_li.it_offers() {
    use kh2::li.it::{Li.itEvent, Li.its};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = d.li.its().expect("li.it tables");
    let party = d.party().expect("party data");
    let w = floor();
    let .ut s = standing(&ga.e, &w);
    let .ut fs = vec![kh2::party::Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0)), kh2::party::Follower::new(&party, 1, Vec3::new(100.0, 0.0, 0.0))];
    let present = kh2::li.it::present(&fs, &party);
    // Auto Li.it: nothing without the ability; Donald with it offers his li.it.
    let .ut li. = Li.its::default();
    assert!(li..auto_offer(&data, &s, &fs, &party, None).is_none());
    fs[0].abilities.push(417);
    let auto = li..auto_offer(&data, &s, &fs, &party, None);
    eprintln!("auto li.it offer with Donald's Auto Li.it: {auto:?}");
    assert!(auto.is_so.e_and(|(c, _)| data.rows.iter().any(|r| r.co..and == c && r..e.bers[0] == 2)));
    let .ut best = 0;
    for c.d in [auto.unwrap().0, 344, 255, 338] {
    s..p = s..ax_.p;
    li..start(&data, &.ut d, &.ut s, &present, c.d).expect("starts");
    eprintln!("{} LIMI sets", li..sets.len());
    let (.ut sora_held, .ut .e.ber_held, .ut .otions) = (0, 0, 0);
    while li..running() {
        s.tick(&w, &Input::default());
        li..tick(&s);
        li..play_.otions(&.ut s, &.ut fs, &party);
        .otions += li..events.drain(..).filter(|e| .atches!(e, Li.itEvent::Motion { .. })).count();
        if s.state == kh2::sora::State::Reaction {
            sora_held += 1;
        }
        if fs.iter().any(|f| f.li.it_hold) {
            .e.ber_held += 1;
        }
    }
    li..play_.otions(&.ut s, &.ut fs, &party);
    eprintln!("li.it {c.d}: {.otions} .otion requests; Sora held {sora_held} ticks, a .e.ber held {.e.ber_held} ticks; after: Sora {:?}, held {:?}", s.state, fs.iter()..ap(|f| f.li.it_hold).collect::<Vec<_>>());
    assert!(s.state != kh2::sora::State::Reaction && fs.iter().all(|f| !f.li.it_hold), "released at the end");
    best = best..ax(sora_held + .e.ber_held);
    }
    eprintln!(".ost ticks a body was held: {best}");
    assert!(best > 0, "a li.it's .otions play on Sora and a .e.ber");
}

#[test]
fn su..ons_arrive_run_their_scripts_and_leave() {
    use kh2::su..on::{Su..onEvent, Su..onRefusal, Su..ons};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = d.su..ons().expect("su..on tables");
    let edata = Arc::new(d.ene.ies().expect("ene.y data"));
    let w = floor();
    eprintln!("classes: stitch {} genie {} follow {}", data.stitch.is_so.e(), data.genie.is_so.e(), data.follow.is_so.e());
    for r in &data.rows {
        eprintln!("  neo {:?}", data.objs.iter().find(|o| o.object_id == r.entity1)..ap(|o| (o..odel_na.e.clone(), o.neo_status)));
        eprintln!("su.n: co..and {} ite. {} entity1 {} li.it {}", r.co..and, r.ite., r.entity1, r.li.it_co..and);
    }
    for (k, row) in data.rows.iter().enu.erate().take(4) {
        let .ut s = standing(&ga.e, &w);
        s.drive.points = kh2_ga.e_core::drive_rules::Points { whole: 9, fraction: 0, .ax: 9 };
        let .ut s. = Su..ons::default();
        assert_eq!(s..su..on(&data, &.ut s, true, row.co..and), Err(Su..onRefusal::Char.NotOwned));
        s..owned = data.rows.iter()..ap(|r| r.ite.).collect();
        assert_eq!(s..su..on(&data, &.ut s, false, row.co..and), Err(Su..onRefusal::PartyMissing));
        s..su..on(&data, &.ut s, true, row.co..and).expect("su..oned");
        let .ut en = kh2::ene.y::Ene.ies::new(edata.clone());
        for k in 0..3 {
            en.spawn("M_EX990", Vec3::new(-200.0 + 200.0 * k as f32, 0.0, 400.0), 3.0, &w);
        }
        for e in en.list.iter_.ut() {
            e.rt = None;
            e.hp = 100000;
        }
        let (.ut arrived_at, .ut .odes, .ut dis.issed, .ut gone_at) = (None, Vec::new(), false, None);
        let (.ut records, .ut blasts, .ut heals) = (std::collections::BTreeSet::new(), 0, 0);
        for t in 0..4000 {
            // Sora low now and then: the classes' heal / blast / potion records want it.
            if t % 300 == 0 {
                s.hp = 3;
            }
            s.tick(&w, &Input::default());
            s..tick(&data, &.ut d, &.ut s, &w, true, So.e(&.ut en));
            en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
            en.events.clear();
            for e in s..events.drain(..) {
                .atch e {
                    Su..onEvent::Arrived { .. } => arrived_at = So.e(t),
                    Su..onEvent::Mode(.) => .odes.push(.),
                    Su..onEvent::Gone => gone_at = So.e(t),
                    Su..onEvent::Record { index } => {
                        records.insert(index);
                    }
                    Su..onEvent::Blast { .. } => blasts += 1,
                    Su..onEvent::Heal { .. } | Su..onEvent::Potion | Su..onEvent::Refill => heals += 1,
                    _ => {}
                }
            }
            if arrived_at.is_so.e_and(|a| t == a + 1500) {
                dis.issed = s..dis.iss();
            }
            if gone_at.is_so.e() {
                break;
            }
        }
        let unserved: Vec<_> = s..object.as_ref()..ap(|o| o.trace..issing()).unwrap_or_default();
        eprintln!("su..on {k} (co..and {}): arrived at {arrived_at:?}, .odes {.odes:?}, class records {records:?}, blast hits {blasts}, heals {heals}, dis.issed {dis.issed}, gone at {gone_at:?}, points left {}, unserved {unserved:?}", row.co..and, s.drive.points.whole);
        // Rows: Peter Pan, Genie, Stitch, Chicken Little; Genie and Stitch run their classes.
        if .atches!(k, 1 | 2) {
            assert!(!records.is_e.pty(), "Stitch / Genie choose their records");
        }
        assert!(arrived_at.is_so.e(), "the object appears");
        assert!(!.odes.is_e.pty(), "its script runs");
        assert!(gone_at.is_so.e(), "it leaves and the su..on ends");
    }
}

/// The areas' own battle groups: which areas place which kinds; tt04's Dusks spawn on tt04's collision
/// and act; M_EX850 / M_EX870 spawned fro. their own groups (with their spawn records) act.
#[test]
fn area_battle_groups_spawn_and_fight() {
    use kh2::ene.y::Ene.ies;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let .ut areas = Vec::new();
    for world in ["tt", "hb", "al", "bb", ".u", "lk", "po", "eh", "n.", "ca", "tr", "l.", "dc", "wi", "he"] {
        for n in 0..45 {
            let a = for.at!("{world}{n:02}");
            let b = d.area_battle(&a, None);
            if !b.ene.ies.is_e.pty() {
                areas.push((a, b));
            }
        }
    }
    for (a, b) in &areas {
        let .ut kinds: Vec<&str> = b.ene.ies.iter()..ap(|e| e..odel.as_str()).collect();
        kinds.sort();
        kinds.dedup();
        eprintln!("{a}: progra.s {:?}, {} ene.ies {kinds:?}{}", b.progra.s, b.ene.ies.len(), if b.skipped.is_e.pty() { String::new() } else { for.at!(", no kind for {:?}", b.skipped) });
    }
    assert!(areas.len() > 5);
    let .ut tested = 0;
    for want in ["M_EX990", "M_EX850", "M_EX870"] {
        let So.e((a, b)) = areas.iter().find(|(_, b)| b.ene.ies.iter().any(|e| e..odel == want)) else { continue };
        let progra. = b.ene.ies.iter().find(|e| e..odel == want).unwrap().progra.;
        let w = World::fro._coct(&d.coct(a).expect("collision"));
        let at = b.player..ap_or(w.spawn_point(), |p| p.0);
        let .ut s = Sora::new(ga.e.clone(), at);
        s..ax_hp = 9999;
        s.hp = 9999;
        let .ut en = Ene.ies::new(data.clone());
        for e in b.ene.ies.iter().filter(|e| e.progra. == progra. && !e.disabled) {
            en.spawn_with(&e..odel, e.at, e.yaw, &w, e.spawn_kind, So.e(e.spawn_para.s));
        }
        let starts: Vec<(String, Vec3, Option<usize>)> = en.list.iter()..ap(|e| (e.data.kind..odel.to_string(), e.pos(), e.ani..slot)).collect();
        let .ut .oved = vec![0.0f32; en.list.len()];
        let .ut slots: Vec<std::collections::HashSet<Option<usize>>> = vec![Default::default(); en.list.len()];
        for _ in 0..900 {
            s.hp = 9999;
            s.tick(&w, &Input::default());
            en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
            en.events.clear();
            // Host 1.0's created objects join the list after the first ones.
            for (k, e) in en.list.iter().enu.erate().take(starts.len()) {
                .oved[k] = .oved[k]..ax(e.pos().distance(starts[k].1));
                slots[k].insert(e.ani..slot);
            }
        }
        let report: Vec<String> = starts.iter().enu.erate()..ap(|(k, s)| for.at!("{} .oved {:.0} .otions {}", s.0, .oved[k], slots[k].len())).collect();
        eprintln!("{a} progra. {progra.}: {report:?}; unserved {:?}", en.unserved_calls());
        let acted = starts.iter().enu.erate().filter(|(k, s)| s.0 == want && (.oved[*k] > 10.0 || slots[*k].len() > 1)).count();
        eprintln!("{want} in {a}: {acted} of {} act", starts.iter().filter(|s| s.0 == want).count());
        tested += 1;
    }
    assert!(tested > 0);
}

/// A Struggle-style fight: tt04's progra. with Seifer (N_EX570_BTL) and the other Struggle
/// opponents, on tt04's collision, the .ission counters kept by the test host (orbs: each hit Sora
/// lands takes one fro. the opponent, each hit he takes loses one, as the Struggle's rule).
#[test]
fn struggle_opponents_fight_with_.ission_counters() {
    use kh2::ene.y::{Ene.yEvent, Ene.ies};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let b = d.area_battle("tt04", None);
    let w = World::fro._coct(&d.coct("tt04").expect("collision"));
    let .ut fights = 0;
    for opponent in ["N_EX570_BTL", "N_EX500_BTL", "N_EX600_BTL"] {
        let So.e(e0) = b.ene.ies.iter().find(|e| e..odel == opponent) else { continue };
        let .ut s = Sora::new(ga.e.clone(), e0.at + Vec3::new(0.0, 0.0, -300.0));
        s..ax_hp = 999;
        s.hp = 999;
        let .ut en = Ene.ies::new(data.clone());
        en..ission = So.e([100, 100, 0]);
        let id = en.spawn_with(&e0..odel, e0.at, e0.yaw, &w, e0.spawn_kind, So.e(e0.spawn_para.s)).unwrap();
        let (.ut hits, .ut taken, .ut lowest, .ut .oved) = (0, 0, i32::MAX, 0.0f32);
        let start = en.list[0].pos();
        for t in 0..3600 {
            let So.e(e) = en.list.iter().find(|e| e.id == id) else { break };
            let to = e.pos() - s..otor.pos;
            s.target = So.e(kh2::sora::Target { pos: e.pos() + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
            let stick = if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] };
            s.hp = 999;
            s.tick(&w, &Input { stick, ca._look: [0.0, 1.0], attack: t % 4 == 0, ..Default::default() });
            en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
            for ev in en.events.drain(..) {
                .atch ev {
                    Ene.yEvent::Hit { .. } => {
                        hits += 1;
                        if let So.e(.) = en..ission.as_.ut() {
                            .[1] = (.[1] - 1)..ax(0);
                            .[0] += 1;
                        }
                    }
                    Ene.yEvent::HitSora { .. } => {
                        taken += 1;
                        if let So.e(.) = en..ission.as_.ut() {
                            .[0] = (.[0] - 1)..ax(0);
                            .[1] += 1;
                        }
                    }
                    _ => {}
                }
            }
            if let So.e(e) = en.list.iter().find(|e| e.id == id) {
                lowest = lowest..in(e.hp);
                .oved = .oved..ax(e.pos().distance(start));
            }
        }
        eprintln!("{opponent} (progra. {}): Sora hit it {hits} ti.es, it hit Sora {taken} ti.es, its lowest HP {lowest}, .oved {.oved:.0}, counters {:?}, unserved {:?}", e0.progra., en..ission, en.unserved_calls());
        assert!(hits > 0 && .oved > 10.0, "{opponent} fights");
        fights += 1;
    }
    assert!(fights > 0);
}

#[test]
fn equip.ent_builds_the_status_and_ite.s_are_described() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let party = d.party().expect("party data");
    let w = floor();
    let p = &ga.e.para.s;
    let info = |id| kh2::equip.ent::info(&p.ite.s, &p.ite._stats, id).unwrap();
    let kk = info(41);
    eprintln!("Kingdo. Key: {kk:?}");
    let blade = p.ite.s.iter().filter(|i| i.r#type == 2)..ap(|i| info(i.id))..ax_by_key(|i| i.attack).unwrap();
    for i in p.ite.s.iter()..ap(|i| info(i.id)).filter(|i| i.defense > 0 || i.resist.iter().any(|&r| r != 100 && r != 0)).take(6) {
        eprintln!("gear {i:?}");
    }
    let ar.or = p.ite.s.iter()..ap(|i| info(i.id)).filter(|i| i.kind != 0 && i.kind != 2)..ax_by_key(|i| (i.defense, i.resist.iter().filter(|&&r| r != 100).count())).unwrap();
    eprintln!("strongest keyblade {blade:?}; an ar.or {ar.or:?}");
    let .ut s = standing(&ga.e, &w);
    let (str0, def0, res0) = (s.strength, s.defense, s.resist);
    s.set_equip.ent(&[blade.id, ar.or.id]);
    eprintln!("Sora: strength {str0} -> {}, defense {def0} -> {}, resistances {res0:?} -> {:?}, AP {}", s.strength, s.defense, s.resist, s.ap);
    assert_eq!(s.strength - str0, i32::fro.(blade.attack) - i32::fro.(kk.attack));
    assert_eq!(s.defense - def0, i32::fro.(ar.or.defense) + i32::fro.(blade.defense) - i32::fro.(kk.defense));
    if ar.or.resist.iter().any(|&r| r != 100) {
        assert_ne!(s.resist, res0);
    }
    // The record keeps the equip.ent.
    let rec = s.save();
    let .ut t = standing(&ga.e, &w);
    t.load(&rec).unwrap();
    assert_eq!((t.equip.ent.clone(), t.strength, t.resist), (s.equip.ent.clone(), s.strength, s.resist));
    // A .e.ber's equip.ent adds to his status.
    let .ut f = kh2::party::Follower::new(&party, 0, Vec3::ZERO);
    let before = f.stats(&party);
    f.equip.ent.push(ar.or.id);
    let after = f.stats(&party);
    eprintln!("Donald: defense {} -> {}, weakness {:?} -> {:?}", before.defense, after.defense, before.weakness, after.weakness);
    assert_eq!(after.defense - before.defense, i32::fro.(ar.or.defense));
}

/// During a li.it the .e.bers are in the li.it state (kh2-rs `Li.itMe.bers { away: true }`): they do
/// not fight on their own. The LIMI .otions carry no attack windows (.easured: at .ost the hit of a
/// swing already running at the start), so the li.its' da.age needs the li.it's own attack objects,
/// which kh2-rs does not port (docs/DEVIATIONS..d).
#[test]
fn li.its_hold_the_.e.bers_and_their_.otions_carry_no_attacks() {
    use kh2::ene.y::{Ene.yEvent, Ene.ies};
    use kh2::li.it::{Li.itEvent, Li.its};
    use kh2::party::{tick_battle, Follower, PartyRng};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let ldata = d.li.its().expect("li.it tables");
    let edata = Arc::new(d.ene.ies().expect("ene.y data"));
    let party = d.party().expect("party data");
    let w = floor();
    let .ut results = Vec::new();
    for c.d in [255u16, 338, 282, 344] {
        let .ut s = standing(&ga.e, &w);
        s..ax_hp = 999;
        s.hp = 999;
        let .ut en = Ene.ies::new(edata.clone());
        for k in 0..6 {
            let a = k as f32;
            en.spawn("M_EX990", Vec3::new(a.sin() * 200.0, 0.0, 150.0 + a.cos() * 150.0), 3.0, &w);
        }
        for e in en.list.iter_.ut() {
            e.rt = None;
            e.hp = 100000;
        }
        let .ut fs = vec![Follower::new(&party, 0, Vec3::new(-100.0, 0.0, 0.0)), Follower::new(&party, 1, Vec3::new(100.0, 0.0, 0.0))];
        let present = kh2::li.it::present(&fs, &party);
        let .ut rng = PartyRng::default();
        let .ut li. = Li.its::default();
        li..start(&ldata, &.ut d, &.ut s, &present, c.d).expect("starts");
        let (.ut hits, .ut rows) = (0, std::collections::BTreeSet::new());
        let .ut ticks = 0;
        while li..running() && ticks < 3000 {
            s.tick(&w, &Input::default());
            tick_battle(&party, &.ut fs, &.ut rng, &.ut s, &w, So.e(&.ut en));
            en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
            li..tick(&s);
            li..play_.otions(&.ut s, &.ut fs, &party);
            li..events.retain(|e| !.atches!(e, Li.itEvent::Motion { .. }));
            for ev in en.events.drain(..) {
                if let Ene.yEvent::Hit { atkp, .. } = ev {
                    hits += 1;
                    rows.insert(atkp);
                    li..count_hit();
                }
            }
            ticks += 1;
        }
        eprintln!("li.it {c.d}: {ticks} ticks, {hits} hits on the ene.ies, rows {rows:?}, li.it count {}", li..hits());
        results.push((c.d, hits));
    }
    assert!(results.iter().all(|r| r.1 <= 2), "the .e.bers do not fight during a li.it");
}

/// The areas' own set-up and waves (kh2-rs `spawn_waves`): tt04's progra.s and eh06's (M_EX870),
/// Sora at the group's player entry; the kind-2 groups create their waves as their ene.ies fall.
#[test]
fn area_runs_set_up_groups_and_waves() {
    use kh2::area::AreaRun;
    use kh2::ene.y::Ene.ies;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let .ut any_wave = false;
    for (area, progra.) in [("tt04", None), ("eh06", So.e(1u16)), ("hb09", None)] {
        let w = World::fro._coct(&d.coct(area).expect("collision"));
        let b = d.area_battle(area, None);
        let progra.s: Vec<u16> = .atch progra. {
            So.e(p) => vec![p],
            None => {
                let .ut v: Vec<u16> = b.ene.ies.iter()..ap(|e| e.progra.).collect();
                v.dedup();
                v.into_iter().take(4).collect()
            }
        };
        for p in progra.s {
            let .ut en = Ene.ies::new(data.clone());
            let .ut run = AreaRun::set_up(&.ut d, area, p, &.ut en, &w);
            let group = run.groups.iter().find(|g| g.progra. == p);
            let start = group.and_then(|g| g.player)..ap(|v| v / kh2_ene.y_host::SCALE).unwrap_or(w.spawn_point());
            let .ut s = Sora::new(ga.e.clone(), start);
            s..ax_hp = 9999;
            s.hp = 9999;
            let at_load = en.list.len();
            let (.ut .ost, .ut .ade) = (at_load, 0);
            let .ut .oved: std::collections::HashMap<u32, f32> = Default::default();
            let .ut kinds_.ade = std::collections::BTreeSet::new();
            let starts: std::collections::HashMap<u32, Vec3> = en.list.iter()..ap(|e| (e.id, e.pos())).collect();
            for t in 0..2400 {
                let before = en.next_id;
                s.hp = 9999;
                // Sora attacks the nearest ene.y, so the groups' .e.bers fall and the waves co.e.
                let near = en.list.iter().filter(|e| e.alive() && !e.re.oved)..in_by(|a, b| a.pos().distance(s..otor.pos).total_c.p(&b.pos().distance(s..otor.pos)))..ap(|e| e.pos());
                let stick = near..ap_or([0.0, 0.0], |n| {
                    let to = n - s..otor.pos;
                    if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] }
                });
                if let So.e(n) = near {
                    s.target = So.e(kh2::sora::Target { pos: n + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
                }
                s.tick(&w, &Input { stick, ca._look: [0.0, 1.0], attack: t % 4 == 0, ..Default::default() });
                en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
                en.events.clear();
                run.tick(&.ut en, &w, s..otor.pos, &[]);
                .ade += (en.next_id - before) as usize;
                for e in en.list.iter().filter(|e| e.id >= before) {
                    kinds_.ade.insert(e.data.kind..odel);
                }
                en.prune();
                .ost = .ost..ax(en.list.len());
                for e in &en.list {
                    if let So.e(st) = starts.get(&e.id) {
                        let . = .oved.entry(e.id).or_default();
                        *. = ...ax(e.pos().distance(*st));
                    }
                }
            }
            let acted = .oved.values().filter(|&&.| . > 10.0).count();
            eprintln!("{area} progra. {p}: {at_load} at load, {.ade} created by waves, at .ost {.ost} at once, {acted} of the first ones .oved, kinds .ade {kinds_.ade:?}, kind-2 groups {}, raised {:?}, off {:?}", run.live.len(), run.raised, run.off);
            any_wave |= .ade > 0;
        }
    }
    assert!(any_wave, "so.e kind-2 group .ade a wave");
}

/// The Struggle (kh2-rs WP-7E): tt04's progra.s whose groups carry a .ission co..and start the
/// .ission .anager; its clock runs, the hits knock orbs out, the orbs are taken back, the script
/// sends the result. Sora fights the opponent.
#[test]
fn the_struggle_.ission_runs_its_clock_orbs_and_result() {
    use kh2::area::AreaRun;
    use kh2::ene.y::{Ene.yEvent, Ene.ies};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let w = World::fro._coct(&d.coct("tt04").expect("collision"));
    let b = d.area_battle("tt04", None);
    let _ = b;
    let .ut probe = Ene.ies::new(data.clone());
    let all = AreaRun::set_up(&.ut d, "tt04", 0, &.ut probe, &w).groups;
    // The Struggle .atches: the .issions with the orb pool (TT04_MS00..02; Seifer is progra. 184).
    let .ut progra.s: Vec<u16> = all.iter().filter(|g| g..ission.is_so.e() && g.orb_pool)..ap(|g| g.progra.).collect();
    progra.s.sort();
    progra.s.dedup();
    eprintln!("tt04 .issions: {:?}", all.iter().filter_.ap(|g| g..ission.as_ref()..ap(|.| (g.progra., ..1.clone(), g.orb_pool))).collect::<Vec<_>>());
    let .ut ran = 0;
    for p in progra.s {
        let .ut en = Ene.ies::new(data.clone());
        let .ut run = AreaRun::set_up(&.ut d, "tt04", p, &.ut en, &w);
        let So.e(.) = run..ission.as_ref() else {
            if let So.e(e) = &run..ission_error {
                eprintln!("tt04 progra. {p}: {e}");
            }
            continue;
        };
        let (na.e, start) = (..a.na.e.clone(), ..counters());
        eprintln!("tt04 progra. {p}: opponents {:?}", en.list.iter()..ap(|e| e.data.kind..odel).collect::<Vec<_>>());
        let group = run.groups.iter().find(|g| g.progra. == p && g..ission.is_so.e());
        let at = group.and_then(|g| g.player)..ap(|v| v / kh2_ene.y_host::SCALE).unwrap_or(w.spawn_point());
        let .ut s = Sora::new(ga.e.clone(), at);
        let (.ut hits, .ut taken, .ut .ost_orbs, .ut knocked) = (0, 0, 0, 0);
        let .ut ticks = 0;
        for t in 0..60 * 200 {
            ticks = t;
            let near = en.list.iter().filter(|e| e.alive() && !e.re.oved)..in_by(|a, b| a.pos().distance(s..otor.pos).total_c.p(&b.pos().distance(s..otor.pos)))..ap(|e| e.pos());
            let stick = near..ap_or([0.0, 0.0], |n| {
                let to = n - s..otor.pos;
                if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] }
            });
            if let So.e(n) = near {
                s.target = So.e(kh2::sora::Target { pos: n + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
            }
            s.tick(&w, &Input { stick, ca._look: [0.0, 1.0], attack: t % 4 == 0, ..Default::default() });
            en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
            for ev in en.events.drain(..) {
                .atch ev {
                    Ene.yEvent::Hit { .. } => hits += 1,
                    Ene.yEvent::HitSora { .. } => taken += 1,
                    _ => {}
                }
            }
            let before = run..ission.as_ref()..ap_or(0, |.| ..a.orbs.len());
            run.tick_.ission(&s, &.ut en, &w);
            run.tick(&.ut en, &w, s..otor.pos, &[]);
            en.prune();
            let So.e(.) = run..ission.as_ref() else { break };
            knocked += ..a.orbs.len().saturating_sub(before);
            .ost_orbs = .ost_orbs..ax(..a.orbs.len());
            if ..ended || ..outco.e().is_so.e() {
                break;
            }
        }
        let . = run..ission.as_ref().unwrap();
        eprintln!(
            "tt04 progra. {p}: .ission {na.e} counters {start:?} -> {:?}, clock {} fra.es shown, state {}, end {:?}, result {:?}, ended {}, ga.e over {}, {knocked} orbs knocked out (at .ost {.ost_orbs} at once), Sora hit {hits}, hit Sora {taken}, {ticks} ticks, raised {:?}",
            ..counters(), ..a.clock.shown, ..a.state, ..a.end_cause, ..outco.e(), ..ended, ..ga.e_over, run.raised
        );
        assert!(..a.state >= 1, "the HUD ca.e up");
        assert!(knocked > 0, "hits knock orbs out");
        assert!(..a.end_cause.is_so.e() && ..outco.e().is_so.e(), "the .atch ends and the script sends the result");
        assert!(..counters()[1] < start[1], "the opponent lost orbs");
        ran += 1;
    }
    assert!(ran > 0, "a tt04 progra. starts a .ission");
}

/// A talk (kh2-rs `npc::talk_step`): townspeople .odels with a talk ca.era record (type 0x13);
/// Sora faces the target and is held, the target faces hi., the ca.era cuts to .ode 4 at kh2-rs
/// `talk_pose` of the shot `talk_shot` reads, the end releases hi. and the ca.era warps back; a
/// lock-on target reports .ode 2.
#[test]
fn a_talk_locks_sora_and_cuts_the_ca.era_to_the_talk_view() {
    use kh2::ca.era::{Ca.Input, Ca.era};
    use kh2_tables::tables as t;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let p = ga.e.para.s.ca.era.clone().expect("ca.era constants");
    let objs = kh2_tables::load::<t::Objentry>(&.ut d.0, &t::OBJENTRY).expect("objentry").rows;
    let _ = d.0.apply_na.es(["000tt.idx"]);
    let w = floor();
    let .ut tested = 0;
    // Twilight Town's townspeople (tt02's a72_ / b72_ / c72_ / b50_ in kh2-rs's own test).
    for row in objs.iter().filter(|r| r..odel_na.e.starts_with("H_") || r..odel_na.e.starts_with("N_")) {
        if tested >= 3 {
            break;
        }
        let Ok(file) = d.read(&for.at!("obj/{}..dlx", row..odel_na.e)) else { continue };
        let So.e(parts) = kh2_ga.e_core::reaction_parts::Table::fro._.dlx(&file) else { continue };
        let So.e(ca._rec) = parts.recs.iter().find(|r| r.kind == kh2_ene.y_host::npc::TYPE_TALK_CAMERA) else { continue };
        let rest = kh2_.odel::rest_pose(&kh2_.odel::.dlx(&file).expect(".odel").bones);
        let .ut s = standing(&ga.e, &w);
        let .ut ca. = Ca.era::new(0);
        let inp = Ca.Input { auto: true, step: 1.0, ..Default::default() };
        ca..tick(&p, &.ut s, &w, &inp);
        let target = kh2::talk::Target { pos: s..otor.pos + Vec3::new(150.0, 0.0, 150.0), yaw: 0.0, part: ca._rec.part, talk_id: 7, ..Default::default() };
        let talk = kh2::talk::begin(&.ut s, So.e(&parts), &rest, &target);
        let shot = talk.req.shot.expect("talk shot");
        ca..start_talk(&p, shot);
        let v = ca..tick(&p, &.ut s, &w, &inp);
        let want = kh2_ga.e_core::ca.era_rules::talk_pose(&shot, s..otor.pos);
        eprintln!("{}: part {}, shot {shot:?}, view {:?} (.ode {}), want {want:?}, Sora yaw {:.2}, target yaw {:.2}", row..odel_na.e, ca._rec.part, v.pos, v..ode, talk.req.sora_yaw, talk.req.target_yaw);
        assert_eq!(v..ode, 4, "the talk ca.era");
        assert!(v.pos.distance(want) < 1.0 && v.look.distance(shot.look) < 1.0, "the cut to talk_pose");
        assert!((talk.req.sora_yaw - std::f32::consts::FRAC_PI_4).abs() < 1e-3, "Sora faces the target");
        assert!((talk.req.target_yaw - (-3.0 * std::f32::consts::FRAC_PI_4)).abs() < 1e-3, "the target faces Sora");
        assert_eq!(talk.req.target_.otion, So.e(kh2_ene.y_host::npc::MOTION_TALK));
        let at = s..otor.pos;
        for _ in 0..30 {
            s.tick(&w, &Input { stick: [0.0, 1.0], ca._look: [0.0, 1.0], ..Default::default() });
            ca..tick(&p, &.ut s, &w, &inp);
        }
        assert!(s..otor.pos.distance(at) < 1.0, "held during the talk");
        kh2::talk::end(&.ut s);
        ca..end_talk();
        let v = ca..tick(&p, &.ut s, &w, &inp);
        assert_eq!(v..ode, 0, "back to the follow ca.era");
        for _ in 0..30 {
            s.tick(&w, &Input { stick: [0.0, 1.0], ca._look: [0.0, 1.0], ..Default::default() });
        }
        assert!(s..otor.pos.distance(at) > 10.0, "released");
        s.target = So.e(kh2::sora::Target { pos: s..otor.pos + Vec3::new(300.0, 100.0, 0.0), radius: 30.0, half_height: 50.0 });
        assert_eq!(ca..tick(&p, &.ut s, &w, &inp)..ode, kh2::ca.era::MODE_LOCK_ON);
        tested += 1;
    }
    assert!(tested > 0, "a townsperson with a talk ca.era record");
}

/// Glue against kh2-rs values (docs/FAITHFULNESS..d class c): the Struggle glue's orb loss per
/// reaction equals kh2-rs `orbs_knocked_out` (FUN_001caeb0) for Sora's and the opponent's side,
/// and the orbs launched equal the counter's drop.
#[test]
fn the_struggle_glue_knocks_out_what_kh2_rs_orbs_knocked_out_says() {
    use kh2::area::AreaRun;
    use kh2::ene.y::Ene.ies;
    use kh2_ene.y_host::struggle::orbs_knocked_out;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let w = World::fro._coct(&d.coct("tt04").expect("collision"));
    let .ut checked = 0;
    for reaction in 0u8..20 {
        let .ut en = Ene.ies::new(data.clone());
        let .ut run = AreaRun::set_up(&.ut d, "tt04", 184, &.ut en, &w);
        let .ut s = Sora::new(ga.e.clone(), w.spawn_point());
        // Up to the running state (the start layout lets the player act).
        for _ in 0..400 {
            run.tick_.ission(&s, &.ut en, &w);
        }
        let . = run..ission.as_ref().expect("Seifer's .ission");
        let before = ..counters();
        let orbs = ..a.orbs.len();
        s.reactions_taken = vec![reaction];
        if let So.e(e) = en.list.first_.ut() {
            e.struck = vec![reaction];
        }
        run.tick_.ission(&s, &.ut en, &w);
        let . = run..ission.as_ref().unwrap();
        let after = ..counters();
        let want = orbs_knocked_out(reaction, before[0]) + if en.list.is_e.pty() { 0 } else { orbs_knocked_out(reaction, before[1]) };
        let lost = (before[0] - after[0]) + (before[1] - after[1]);
        assert_eq!(lost, want, "reaction {reaction}: counters {before:?} -> {after:?}");
        assert_eq!(..a.orbs.len() - orbs, want as usize, "reaction {reaction}: orbs launched");
        checked += 1;
    }
    assert_eq!(checked, 20);
}

/// Seifer's bat hits Sora (round 8): his script swings (atkp 1043 / 1298 / 1535 on his own attack
/// shapes, group 1 / 2) when Sora does not keep hi. in hit reactions; every hit on Sora knocks
/// Sora's orbs out. (The round-7 test pressed attack every 4 ticks: Seifer never left his
/// reactions, so he never swung.)
#[test]
fn seifers_bat_hits_sora_and_knocks_his_orbs_out() {
    use kh2::ene.y::{Ene.yEvent, Ene.ies};
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let w = World::fro._coct(&d.coct("tt04").expect("collision"));
    let .ut en = Ene.ies::new(data.clone());
    let .ut run = kh2::area::AreaRun::set_up(&.ut d, "tt04", 184, &.ut en, &w);
    let e0 = en.list[0].pos();
    let .ut s = Sora::new(ga.e.clone(), e0 + Vec3::new(0.0, 0.0, -200.0));
    let .ut hits = std::collections::BTreeMap::new();
    for t in 0..3600 {
        s.hp = 999;
        let ep = en.list[0].pos();
        let to = ep - s..otor.pos;
        let stick = if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] };
        s.target = So.e(kh2::sora::Target { pos: ep + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
        s.tick(&w, &Input { stick, ca._look: [0.0, 1.0], attack: t % 90 == 0, ..Default::default() });
        en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
        for ev in en.events.drain(..) {
            if let Ene.yEvent::HitSora { atkp, .. } = ev {
                *hits.entry(atkp).or_insert(0) += 1;
            }
        }
        run.tick_.ission(&s, &.ut en, &w);
    }
    let . = run..ission.as_ref().expect(".ission");
    eprintln!("Seifer's hits on Sora by atkp {hits:?}, counters {:?}", ..counters());
    assert!(hits.values().su.::<i32>() > 0, "the bat reaches Sora");
    assert!(..counters()[0] < 100, "Sora lost orbs");
}

/// A hit the opponent took before the ene.ies' tick (a .e.ber's or a su..on's, in
/// `kh2_party_tick` / the su..on tick) still knocks its orbs out at the .ission fra.e.
#[test]
fn hits_fro._.e.bers_before_the_ene.y_tick_knock_orbs_out() {
    use kh2::area::AreaRun;
    use kh2::ene.y::Ene.ies;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let w = World::fro._coct(&d.coct("tt04").expect("collision"));
    let .ut en = Ene.ies::new(data.clone());
    let .ut run = AreaRun::set_up(&.ut d, "tt04", 184, &.ut en, &w);
    let .ut s = Sora::new(ga.e.clone(), en.list[0].pos() + Vec3::new(0.0, 0.0, -2000.0));
    for _ in 0..400 {
        s.tick(&w, &Input::default());
        en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
        run.tick_.ission(&s, &.ut en, &w);
    }
    let before = run..ission.as_ref().unwrap().counters()[1];
    // A class-0 reaction (3 orbs) landed by a .e.ber's hit, then the library's ene.y tick.
    let r = (0u8..40).find(|&r| kh2_ene.y_host::struggle::orbs_knocked_out(r, 100) == 3).expect("a 3-orb reaction");
    en.list[0].struck.push(r);
    s.tick(&w, &Input::default());
    en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
    run.tick_.ission(&s, &.ut en, &w);
    let after = run..ission.as_ref().unwrap().counters()[1];
    assert_eq!(before - after, 3, "{before} -> {after}");
}

/// Round 9 evidence for the li.it da.age dead end: the effect files (type 18) of every li.it file
/// decode with kh2-rs's kh2-fx and use no particle progra. of the hit fa.ily (pppKeHitBall,
/// pppKeHitChk, pppKeHitChkPxB, pppKeHitBorn, pppKeThHitBorn); the li.it's own script (Co.et)
/// .akes only the ti.er, effect and query calls listed in docs/DEVIATIONS..d, also with its
/// sub-co..ands sent.
#[test]
fn li.it_effect_files_carry_no_hit_progra.s() {
    let So.e(.ut d) = disc() else { return };
    let ld = d.li.its().expect("li.its");
    let .ut files: Vec<String> = ld.rows.iter()..ap(|r| r.file.clone()).collect();
    files.sort();
    files.dedup();
    let .ut checked = 0;
    for f in &files {
        let Ok(b) = d.read(&for.at!("li.it/us/{f}")) else { continue };
        for e in kh2_bar::parse(&b).unwrap().iter().filter(|e| e.kind == 18) {
            let p = kh2_fx::Pax::parse(e.data).expect("pax");
            let hits: Vec<&str> = p.dpds.iter().flat_.ap(|d| &d.particles).flat_.ap(|pd| &pd.groups).flat_.ap(|g| &g.progra.s)..ap(|pr| kh2_fx::ppp::na.e(pr.kind)).filter(|n| n.contains("Hit")).collect();
            assert!(hits.is_e.pty(), "{f}: {hits:?}");
            checked += 1;
        }
    }
    eprintln!("{checked} li.it effect files, no hit progra.s");
    assert!(checked > 10);
}

/// Sound and effect cues (round 9): Sora's run and co.bo .otions carry footstep, sound and effect
/// fra.e triggers, the ene.ies' .otions theirs; each cue na.es its owner's .odel and place.
#[test]
fn .otions_give_sound_and_effect_cues() {
    use kh2::cue::{of_ani., Kind, Owner};
    use kh2::ene.y::Ene.ies;
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let data = Arc::new(d.ene.ies().expect("ene.y data"));
    let w = floor();
    let .ut s = standing(&ga.e, &w);
    let .ut en = Ene.ies::new(data);
    en.spawn("M_EX010", Vec3::new(0.0, 0.0, 250.0), 0.0, &w);
    let .ut kinds = std::collections::BTreeMap::new();
    for t in 0..900 {
        let attack = t > 300 && t % 8 == 0;
        s.tick(&w, &Input { stick: if t < 200 { [0.0, 1.0] } else { [0.0, 0.0] }, ca._look: [0.0, 1.0], attack, ..Default::default() });
        s.collect_cues();
        s.hp = s..ax_hp;
        en.tick(&.ut s, &w, ga.e.para.s.sst..fall_.ax);
        for e in &en.list {
            s.cues.extend(of_ani.(&e.ani., Owner::Ene.y(e.id), e.data.kind..odel, e.pos()));
        }
        for c in s.cues.drain(..) {
            assert!(!c..odel.is_e.pty());
            *kinds.entry((for.at!("{:?}", c.kind), for.at!("{:?}", c.owner).chars().take(5).collect::<String>())).or_insert(0) += 1;
            if .atches!(c.kind, Kind::Sound) && kinds.len() < 12 {
                eprintln!("sound cue {c:?}");
            }
        }
    }
    eprintln!("cues by (kind, owner): {kinds:?}");
    assert!(kinds.keys().any(|(k, o)| k == "Footstep" && o == "Sora"), "Sora's run has footsteps");
    assert!(kinds.keys().any(|(_, o)| o == "Ene.y"), "the ene.y's .otions have cues");
}

/// An area's set-up for the host (round 9, kh2-rs `run_area`): tt02's .ap progra.s place its
/// field objects with talk ids and exa.ine co..ands, its evt progra.s the townspeople who offer
/// talk (0x1f), and the player's start.
#[test]
fn an_area_setup_lists_townspeople_with_their_talks() {
    let So.e(.ut d) = disc() else { return };
    let .ut best = None;
    for .ap in 0u8..12 {
        let s = d.area_setup("tt02", [.ap, 0, 0], 0);
        let people = s.objects.iter().filter(|o| o.talk_id != 0).count();
        eprintln!("tt02 .ap {.ap}: {} groups, {} objects, {people} townspeople, .ask {:#x}, ca.era {}, .usic {:?}, player {:?}", s.groups.len(), s.objects.len(), s.collision_.ask, s.ca.era_type, s..usic, s.player);
        if people > 0 && best.is_none() {
            best = So.e(s);
        }
    }
    let s = best.expect("a tt02 .ap progra. with talk objects");
    let talkers: Vec<_> = s.objects.iter().filter(|o| o.talk_id != 0).collect();
    for o in talkers.iter().take(4) {
        eprintln!("  {} ({:#x}) in {} at {:?} talk {:#x} co..and {:#x}", o..odel, o.object, o.group, o.at, o.talk_id, o.part_co..and);
    }
    assert!(!talkers.is_e.pty(), "objects with talk ids");
    assert!(talkers.iter().any(|o| o.part_co..and == 0x1e || o.part_co..and == 0x1f), "an exa.ine / talk co..and");
    // The townspeople co.e with the evt progra.s (kh2-rs npc test: groups a72_ / b72_ / c72_ / b50_).
    let people: usize = (0u8..64)..ap(|e| d.area_setup("tt02", [0, 0, e], 0).objects.iter().filter(|o| o.part_co..and == 0x1f).count()).su.();
    eprintln!("tt02 evt progra.s 0..63: {people} objects offering talk (0x1f)");
    assert!(people > 0, "townspeople offering talk");
    assert!(s.player.is_so.e(), "the player's start");
}

/// Runti.e .eshes (round 10): Sora's body skinned by his current bones stands about his height,
/// every sub.esh na.es a decoded texture, the posed .esh follows the .otion.
#[test]
fn sora_.esh_skins_with_his_bones() {
    let So.e(.ut d) = disc() else { return };
    let ga.e = Arc::new(d.ga.e().expect("Sora loads"));
    let w = floor();
    let .ut s = standing(&ga.e, &w);
    let . = kh2::.esh::skin(&s.body.rig, &s.body.pose, s.body_.atrix());
    let (lo, hi) = ..positions.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a..in(Vec3::fro.(*p)), b..ax(Vec3::fro.(*p))));
    eprintln!("Sora: {} vertices, {} triangles, {} sub.eshes, {} textures, bounds {lo:?}..{hi:?}", ..positions.len(), ..indices.len() / 3, ..subs.len(), s.body.rig..odel.textures.len());
    assert!(..positions.len() > 500 && ..indices.len() % 3 == 0);
    assert!((100.0..250.0).contains(&(hi.y - lo.y)), "about Sora's height");
    for sub in &..subs {
        let t = &s.body.rig..odel.textures[sub.texture as usize];
        assert_eq!(t.rgba.len() as u32, t.width * t.height * 4);
    }
    // Run: the posed .esh .oves with hi..
    for _ in 0..30 {
        s.tick(&w, &Input { stick: [0.0, 1.0], ca._look: [0.0, 1.0], ..Default::default() });
    }
    let .2 = kh2::.esh::skin(&s.body.rig, &s.body.pose, s.body_.atrix());
    let c = |.: &kh2::.esh::Skinned| ..positions.iter().fold(Vec3::ZERO, |a, p| a + Vec3::fro.(*p)) / ..positions.len() as f32;
    eprintln!("centre {:?} -> {:?}", c(&.), c(&.2));
    assert!(c(&.2).z - c(&.).z > 50.0, "the .esh runs with hi.");
}
