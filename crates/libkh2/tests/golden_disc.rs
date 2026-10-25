//! Golden traces on the disc: the main scenarios run with fixed inputs and every tick's state
//! digest is compared with `local/golden/<scenario>.txt` (outside git: written on the first run, or
//! with `KH2_BLESS=1`). A change in the ported layer, in kh2-rs's compiled files or in the disc
//! readers shows up as the first tick that differs.

use std::sync::Arc;

use glam::Vec3;
use kh2::data::{Disc, Game};
use kh2::replay::{check_golden, snapshot};
use kh2::sora::{Input, Sora};
use kh2::{world_from_surfaces, Surface, World};

fn room() -> World {
    let s = 3000.0;
    let t = |v, flags| Surface { v, flags, ..Default::default() };
    world_from_surfaces(&[
        t([[-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s]], 1),
        t([[-s, 0.0, -s], [s, 0.0, s], [s, 0.0, -s]], 1),
        t([[900.0, -100.0, -s], [900.0, -100.0, s], [900.0, 600.0, s]], kh2::SURF_WALL),
        t([[900.0, -100.0, -s], [900.0, 600.0, s], [900.0, 600.0, -s]], kh2::SURF_WALL),
        // A raised block to jump on and fall off: top at 120 over x -900..-600.
        t([[-900.0, 120.0, -300.0], [-900.0, 120.0, 300.0], [-600.0, 120.0, 300.0]], 1),
        t([[-900.0, 120.0, -300.0], [-600.0, 120.0, 300.0], [-600.0, 120.0, -300.0]], 1),
    ])
}

fn setup() -> Option<(Disc, Arc<Game>, World)> {
    let iso = std::env::var_os("KH2_ISO")?;
    let mut d = Disc::open(std::path::Path::new(&iso)).ok()?;
    let g = Arc::new(d.game().ok()?);
    Some((d, g, room()))
}

fn sora(g: &Arc<Game>) -> Sora {
    let mut s = Sora::new(g.clone(), Vec3::ZERO);
    s.motor.grounded = true;
    s
}

fn input(stick: [f32; 2], attack: bool, jump: bool, guard: bool) -> Input {
    Input { stick, cam_look: [0.0, 1.0], attack, jump, guard }
}

fn run(name: &str, s: &mut Sora, w: &World, ticks: usize, mut f: impl FnMut(usize, &mut Sora) -> Input) {
    let mut lines = Vec::new();
    for t in 0..ticks {
        let i = f(t, s);
        s.tick(w, &i);
        if !s.attack_triggers().is_empty() {
            s.connected = true;
        }
        lines.push(snapshot(s));
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/golden").join(format!("{name}.txt"));
    match check_golden(&path, &lines) {
        Ok(true) => {}
        Ok(false) => eprintln!("{name}: golden written"),
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn scenarios() {
    let Some((mut d, g, w)) = setup() else { return };
    // Movement: walk, run, turn, the wall, a jump onto the block and off it.
    let mut s = sora(&g);
    run("movement", &mut s, &w, 420, |t, _| match t {
        0..=29 => input([0.0, 0.5], false, false, false),
        30..=139 => input([1.0, 0.0], false, false, false),
        140..=239 => input([-1.0, 0.0], false, t >= 200, false),
        _ => input([-1.0, 0.0], false, (300..340).contains(&t), false),
    });
    // Ground and air combos with hits.
    let mut s = sora(&g);
    run("combos", &mut s, &w, 400, |t, _| input([0.0, 0.0], t % 4 == 0, (200..230).contains(&t), false));
    // Growth abilities at MAX.
    let mut s = sora(&g);
    s.growth = kh2::sora::Growth { high_jump: 5, quick_run: 5, aerial_dodge: 5, glide: 5 };
    run("growth", &mut s, &w, 300, |t, _| match t {
        0..=59 => input([0.0, 1.0], false, false, t >= 10),
        60..=89 => input([0.0, 1.0], false, true, false),
        90..=94 => input([0.0, 1.0], false, false, false),
        95..=99 => input([0.0, 1.0], false, true, false),
        _ => input([0.0, 1.0], false, false, true),
    });
    // Hit reactions: a flinch row and a blow row, guard and counter.
    let mut s = sora(&g);
    let rows: Vec<i16> = [3u8, 4]
        .iter()
        .map(|&c| g.params.atkp.iter().find(|a| kh2_game_core::reaction_rules::reaction_class(a.enemy_reaction) == c && a.sub_id >> 8 == 0).unwrap().id as i16)
        .collect();
    run("hurt", &mut s, &w, 400, |t, s| {
        match t {
            10 => {
                s.hit(rows[0], 1, Vec3::new(0.0, 0.0, -1.0));
            }
            120 => {
                s.hit(rows[1], 1, Vec3::new(0.0, 0.0, -1.0));
            }
            301 => {
                s.hit(rows[1], 1, Vec3::new(0.0, 0.0, -1.0));
            }
            _ => {}
        }
        input([0.0, 0.0], t == 310, false, t == 300)
    });
    // Magic: the four spells in turn.
    let mut s = sora(&g);
    run("magic", &mut s, &w, 400, |t, s| {
        if t % 100 == 5 {
            s.cast_request = Some(((t / 100) as u8, 30));
        }
        input([0.0, 0.0], false, false, false)
    });
    // Drive: Valor in and out, then Master.
    let data = g.drive.clone().unwrap();
    let valor = Arc::new(d.form_assets(&data, 1, 0, 41).unwrap());
    let master = Arc::new(d.form_assets(&data, 3, 0, 41).unwrap());
    let mut s = sora(&g);
    s.drive.points = kh2_game_core::drive_rules::Points { whole: 9, fraction: 0, max: 9 };
    s.drive.owned = [false, true, false, true, false, false];
    run("drive", &mut s, &w, 700, |t, s| {
        match t {
            5 => {
                let _ = s.drive_enter(1, &mut |_| Some(valor.clone()));
            }
            300 => {
                s.drive_revert(true);
            }
            320 => {
                let _ = s.drive_enter(3, &mut |_| Some(master.clone()));
            }
            _ => {}
        }
        input([0.0, 0.0], t > 200 && t % 5 == 0, false, false)
    });
    // Items and growth: a Potion, an Ether, a defeat.
    let mut s = sora(&g);
    s.hp = 3;
    run("items", &mut s, &w, 200, |t, s| {
        match t {
            5 => {
                s.use_item(1);
            }
            120 => {
                s.mp = 10;
                s.use_item(3);
            }
            199 => {
                s.report_defeat(100, 25);
            }
            _ => {}
        }
        input([0.0, 0.0], false, false, false)
    });
}

/// Sora, Donald and Goofy against a Shadow, a Soldier and a Dusk: every tick's Sora, enemy and member
/// lines. Run twice in-process (the two runs must agree tick by tick: no hidden state, no hash
/// order, no time) and checked against `local/golden/battle.txt`.
#[test]
fn battle_replays_the_same() {
    let Some((mut d, g, w)) = setup() else { return };
    let data = Arc::new(d.enemies().expect("enemy data"));
    let party = d.party().expect("party data");
    let run_once = || {
        let mut s = sora(&g);
        s.max_hp = 999;
        s.hp = 999;
        let mut en = kh2::enemy::Enemies::new(data.clone());
        for (k, kind) in ["M_EX020", "M_EX010", "M_EX990"].iter().enumerate() {
            en.spawn(kind, Vec3::new(-300.0 + 300.0 * k as f32, 0.0, 500.0), std::f32::consts::PI, &w);
        }
        let mut rng = kh2::party::PartyRng::default();
        let mut fs = vec![kh2::party::Follower::new(&party, 0, Vec3::new(-100.0, 0.0, -80.0)), kh2::party::Follower::new(&party, 1, Vec3::new(100.0, 0.0, -80.0))];
        let mut lines = Vec::new();
        for t in 0..900 {
            if let Some(e) = en.list.iter().find(|e| e.alive() && !e.removed) {
                s.target = Some(kh2::sora::Target { pos: e.pos() + Vec3::Y * 80.0, radius: 40.0, half_height: 80.0 });
            }
            let to = s.target.map_or(Vec3::ZERO, |t| (t.pos - s.motor.pos).with_y(0.0));
            let stick = if to.length() > 150.0 { [to.x / to.length(), to.z / to.length()] } else { [0.0, 0.0] };
            s.tick(&w, &input(stick, t % 6 == 0, false, false));
            kh2::party::tick_battle(&party, &mut fs, &mut rng, &mut s, &w, Some(&mut en));
            en.tick(&mut s, &w, g.params.sstm.fall_max);
            let ev: Vec<String> = en.events.drain(..).map(|e| format!("{e:?}")).collect();
            lines.push(format!("{} | {} | {}", snapshot(&s), kh2::replay::battle_snapshot(&en, &fs), ev.join(",")));
        }
        lines
    };
    let a = run_once();
    let b = run_once();
    if let Some(t) = a.iter().zip(&b).position(|(x, y)| x != y) {
        panic!("the two runs differ at tick {t}:\n{}\n{}", a[t], b[t]);
    }
    let hits = a.iter().filter(|l| l.contains("Hit {")).count();
    eprintln!("battle: {} ticks, {hits} ticks with hits", a.len());
    assert!(hits > 0);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../local/golden/battle.txt");
    match check_golden(&path, &a) {
        Ok(true) => {}
        Ok(false) => eprintln!("battle: golden written"),
        Err(e) => panic!("{e}"),
    }
}
