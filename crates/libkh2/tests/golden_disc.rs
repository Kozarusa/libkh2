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
