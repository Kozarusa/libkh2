//! libkh2 viewer: a window where Sora runs on a test room, drawn as his skeleton (lines between
//! bone origins) with the keyblades, Donald and Goofy, and enemies under their BDX scripts, through
//! libkh2's field camera. Donald and Goofy fight the enemies with the battle subset; Sora's swings
//! and spells hit them.
//!
//! `cargo run --release -p libkh2-viewer -- <disc image>` (or set KH2_ISO). Optional second
//! argument: an area name (`tt00`) to walk on that area's collision instead of the test room.
//!
//! Keys: WASD move (Shift walks), Space jump, J attack, K guard / Quick Run / Glide, arrows camera,
//! Q lock onto the post, 1-4 Fire / Blizzard / Thunder / Cure, P Potion, F Valor, M Master,
//! R revert, H take a hit, G toggle the growth abilities, E spawn a Shadow, a Soldier and a Dusk
//! in front of Sora (lock-on with Q then takes the nearest enemy), T accept a reaction command,
//! Esc quit.

use std::sync::Arc;

use glam::{Mat4, Vec3, Vec4};
use kh2::camera::{CamInput, Camera};
use kh2::sora::{Input, Sora, Target};
use kh2::{world_from_surfaces, Surface, World};
use minifb::{Key, KeyRepeat, Window, WindowOptions};

const W: usize = 960;
const H: usize = 640;

struct Canvas {
    px: Vec<u32>,
    vp: Mat4,
}

impl Canvas {
    fn project(&self, p: Vec3) -> Option<(f32, f32)> {
        let c = self.vp * Vec4::new(p.x, p.y, p.z, 1.0);
        if c.w <= 1.0 {
            return None;
        }
        Some(((c.x / c.w * 0.5 + 0.5) * W as f32, (0.5 - c.y / c.w * 0.5) * H as f32))
    }

    fn line(&mut self, a: Vec3, b: Vec3, color: u32) {
        let (Some((x0, y0)), Some((x1, y1))) = (self.project(a), self.project(b)) else { return };
        let n = (x1 - x0).abs().max((y1 - y0).abs()).ceil().min(4000.0) as i32;
        for i in 0..=n {
            let t = if n == 0 { 0.0 } else { i as f32 / n as f32 };
            let (x, y) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t);
            if x >= 0.0 && y >= 0.0 && (x as usize) < W && (y as usize) < H {
                self.px[y as usize * W + x as usize] = color;
            }
        }
    }

    fn skeleton(&mut self, root: Mat4, pose: &[Mat4], parents: &[i32], color: u32) {
        for (i, m) in pose.iter().enumerate() {
            let p = parents.get(i).copied().unwrap_or(-1);
            if p >= 0 && (p as usize) < pose.len() {
                let a = (root * pose[p as usize]).w_axis.truncate();
                let b = (root * *m).w_axis.truncate();
                self.line(a, b, color);
            }
        }
    }
}

fn room() -> World {
    let s = 3000.0;
    let t = |v, flags| Surface { v, flags, ..Default::default() };
    let mut tris = vec![t([[-s, 0.0, -s], [-s, 0.0, s], [s, 0.0, s]], 1), t([[-s, 0.0, -s], [s, 0.0, s], [s, 0.0, -s]], 1)];
    // Four walls facing inward, and a block to jump on.
    for (a, b) in [([-s, -s], [s, -s]), ([s, -s], [s, s]), ([s, s], [-s, s]), ([-s, s], [-s, -s])] {
        tris.push(t([[a[0], 0.0, a[1]], [b[0], 0.0, b[1]], [b[0], 400.0, b[1]]], kh2::SURF_WALL));
        tris.push(t([[a[0], 0.0, a[1]], [b[0], 400.0, b[1]], [a[0], 400.0, a[1]]], kh2::SURF_WALL));
    }
    let (x0, x1, z0, z1, h) = (600.0, 1000.0, -200.0, 200.0, 120.0);
    tris.push(t([[x0, h, z0], [x0, h, z1], [x1, h, z1]], 1));
    tris.push(t([[x0, h, z0], [x1, h, z1], [x1, h, z0]], 1));
    world_from_surfaces(&tris)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let iso = args.next().or_else(|| std::env::var("KH2_ISO").ok()).expect("pass the disc image or set KH2_ISO");
    let area = args.next();
    let mut disc = kh2::data::Disc::open(std::path::Path::new(&iso)).unwrap_or_else(|e| panic!("{e}"));
    let game = Arc::new(disc.game().unwrap_or_else(|e| panic!("{e}")));
    let world = match &area {
        Some(a) => World::from_coct(&disc.coct(a).unwrap_or_else(|e| panic!("{e}"))),
        None => room(),
    };
    let party = disc.party().ok();
    let enemy_data = disc.enemies().ok().map(Arc::new);
    let mut enemies = enemy_data.clone().map(kh2::enemy::Enemies::new);
    let drive = game.drive.clone();
    let mut forms = std::collections::HashMap::new();
    let start = if area.is_some() { world.spawn_point() } else { Vec3::ZERO };
    let mut sora = Sora::new(game.clone(), start);
    sora.motor.grounded = true;
    sora.drive.points = kh2_game_core::drive_rules::Points { whole: 9, fraction: 0, max: 9 };
    sora.drive.owned = [false, true, true, true, true, false];
    let mut party_rng = kh2::party::PartyRng::default();
    let mut followers: Vec<kh2::party::Follower> = party.as_ref().map(|p| (0..2).map(|m| kh2::party::Follower::new(p, m, start + Vec3::new(if m == 0 { -150.0 } else { 150.0 }, 0.0, -150.0))).collect()).unwrap_or_default();
    let mut camera = Camera::new(0);
    let post = start + Vec3::new(-800.0, 0.0, 600.0);
    let parents = |r: &kh2::anim::Rig| r.model.bones.iter().map(|b| b.parent).collect::<Vec<i32>>();
    let mut look = [0.0f32, 1.0];
    let mut window = Window::new("libkh2 viewer", W, H, WindowOptions::default()).expect("window");
    window.set_target_fps(60);
    let mut canvas = Canvas { px: vec![0; W * H], vp: Mat4::IDENTITY };
    // KH2_VIEWER_FRAMES=n: run n frames with a scripted walk and attack, write the last frame to
    // KH2_VIEWER_SHOT (a PPM file) if set, then quit (a smoke test of the viewer).
    let frames: Option<u32> = std::env::var("KH2_VIEWER_FRAMES").ok().and_then(|v| v.parse().ok());
    let mut frame = 0u32;
    while window.is_open() && !window.is_key_down(Key::Escape) && frames.is_none_or(|n| frame < n) {
        frame += 1;
        let scripted = frames.is_some();
        let down = |k: Key| window.is_key_down(k);
        let pressed = |k: Key| window.is_key_pressed(k, KeyRepeat::No);
        let axis = |p: Key, n: Key| down(p) as i32 as f32 - down(n) as i32 as f32;
        let mut stick = [axis(Key::D, Key::A), axis(Key::W, Key::S)];
        if down(Key::LeftShift) {
            stick = [stick[0] * 0.5, stick[1] * 0.5];
        }
        if scripted {
            stick = if frame < 90 { [0.3, 1.0] } else { [0.0, 0.0] };
        }
        let input = Input { stick, cam_look: look, attack: down(Key::J) || (scripted && frame > 100 && frame.is_multiple_of(6)), jump: down(Key::Space), guard: down(Key::K) };
        for (k, slot) in [(Key::Key1, 0u8), (Key::Key2, 1), (Key::Key3, 2), (Key::Key4, 3)] {
            if pressed(k) {
                sora.cast_request = Some((slot, 30));
            }
        }
        if pressed(Key::P) {
            sora.use_item(1);
        }
        if pressed(Key::G) {
            let l = if sora.growth.high_jump == 0 { 3 } else { 0 };
            sora.growth = kh2::sora::Growth { high_jump: l, quick_run: l, aerial_dodge: l, glide: l };
        }
        if (pressed(Key::E) || (scripted && frame == 1))
            && let Some(en) = enemies.as_mut()
        {
            let fwd = sora.motor.forward();
            let side = Vec3::new(fwd.z, 0.0, -fwd.x);
            for (k, kind) in ["M_EX020", "M_EX010", "M_EX990"].iter().enumerate() {
                let at = sora.motor.pos + fwd * 500.0 + side * (k as f32 - 1.0) * 250.0;
                en.spawn(kind, at, (-fwd.x).atan2(-fwd.z), &world);
            }
        }
        let nearest = enemies.as_ref().and_then(|en| en.list.iter().filter(|e| e.alive() && !e.removed).min_by(|a, b| a.pos().distance(sora.motor.pos).total_cmp(&b.pos().distance(sora.motor.pos))).map(|e| e.pos()));
        if pressed(Key::Q) || (scripted && frame == 2) {
            sora.target = if sora.target.is_some() { None } else { Some(Target { pos: nearest.unwrap_or(post) + Vec3::Y * 100.0, radius: 40.0, half_height: 100.0 }) };
        } else if sora.target.is_some()
            && let Some(p) = nearest
        {
            sora.target = Some(Target { pos: p + Vec3::Y * 100.0, radius: 40.0, half_height: 100.0 });
        }
        if pressed(Key::T)
            && let Some(en) = enemies.as_mut()
            && let Some(c) = en.accept(&mut sora)
        {
            eprintln!("reaction command {c} accepted");
        }
        if pressed(Key::H) {
            let row = game.params.atkp.iter().find(|a| kh2_game_core::reaction_rules::reaction_class(a.enemy_reaction) == 3).map_or(0, |a| a.id as i16);
            sora.hit(row, 1, -sora.motor.forward());
        }
        if pressed(Key::R) {
            sora.drive_revert(true);
        }
        for (k, form) in [(Key::F, 1u8), (Key::M, 3)] {
            if pressed(k)
                && let Some(d) = drive.as_ref()
            {
                let mut load = |f: u8| -> Option<Arc<kh2::drive::FormAssets>> {
                    if let std::collections::hash_map::Entry::Vacant(e) = forms.entry(f) {
                        e.insert(Arc::new(disc.form_assets(d, f, 0, 41).ok()?));
                    }
                    forms.get(&f).cloned()
                };
                if let Err(e) = sora.drive_enter(form, &mut load) {
                    eprintln!("drive: {e}");
                }
            }
        }
        sora.tick(&world, &input);
        if !sora.attack_triggers().is_empty() && sora.target.is_some_and(|t| t.pos.distance(sora.motor.pos) < 250.0) {
            sora.connected = true;
        }
        for e in sora.events.drain(..) {
            eprintln!("event: {e:?}");
        }
        if let Some(p) = party.as_ref() {
            let fighting = enemies.as_mut().filter(|e| !e.list.is_empty());
            kh2::party::tick_battle(p, &mut followers, &mut party_rng, &mut sora, &world, fighting);
        }
        if let Some(en) = enemies.as_mut() {
            en.tick(&mut sora, &world, game.params.sstm.fall_max);
            for e in en.events.drain(..) {
                eprintln!("enemy event: {e:?}");
            }
            en.list.retain(|e| !e.removed);
        }
        // The camera.
        let cam_in = CamInput { stick: [axis(Key::Right, Key::Left), axis(Key::Up, Key::Down)], auto: true, step: 1.0, ..Default::default() };
        let (eye, at, fov) = match game.params.camera.as_ref() {
            Some(p) => {
                let v = camera.tick(p, &mut sora, &world, &cam_in);
                (v.pos, v.look, v.fov)
            }
            None => (sora.motor.pos - sora.motor.forward() * 450.0 + Vec3::Y * 250.0, sora.motor.pos + Vec3::Y * 120.0, 1.2),
        };
        let d = (at - eye).with_y(0.0);
        if d.length_squared() > 1.0 {
            look = [d.x, d.z];
        }
        let aspect = W as f32 / H as f32;
        let fov_v = 2.0 * ((fov * 0.5).tan() / (4.0 / 3.0)).atan();
        canvas.vp = Mat4::perspective_rh(fov_v, aspect, 10.0, 100_000.0) * Mat4::look_at_rh(eye, at, Vec3::Y);
        // Draw.
        canvas.px.fill(0x101820);
        for tr in &world.tris {
            let c = if tr.n.y.abs() >= 0.5 { 0x2a4a2a } else { 0x5a5a7a };
            canvas.line(tr.v[0], tr.v[1], c);
            canvas.line(tr.v[1], tr.v[2], c);
            canvas.line(tr.v[2], tr.v[0], c);
        }
        for i in -15..=15 {
            let f = i as f32 * 200.0;
            canvas.line(start + Vec3::new(f, 1.0, -3000.0), start + Vec3::new(f, 1.0, 3000.0), 0x1c2c1c);
            canvas.line(start + Vec3::new(-3000.0, 1.0, f), start + Vec3::new(3000.0, 1.0, f), 0x1c2c1c);
        }
        canvas.line(post, post + Vec3::Y * 220.0, if sora.target.is_some() { 0xff4040 } else { 0x806060 });
        let body = sora.body_matrix();
        canvas.skeleton(body, &sora.body.pose, &parents(&sora.body.rig), if sora.invuln > 0.0 { 0xffff80 } else { 0xf0f0f0 });
        if let Some(w) = &sora.weapon {
            canvas.skeleton(sora.weapon_matrix(), &w.pose, &parents(&w.rig), 0xffd040);
        }
        if let Some(w) = &sora.weapon2 {
            canvas.skeleton(sora.weapon2_matrix(), &w.pose, &parents(&w.rig), 0xffd040);
        }
        for v in sora.attack_volumes() {
            canvas.line(v.a, v.b + Vec3::Y * v.r, 0xff3030);
        }
        for f in &followers {
            let color = if f.member == 0 { 0x60a0ff } else { 0x60ff90 };
            canvas.skeleton(f.matrix(), &f.body.pose, &parents(&f.body.rig), if f.attack.is_some() { 0xc0e0ff } else { color });
            if let (Some(w), Some(p)) = (&f.weapon, party.as_ref()) {
                canvas.skeleton(f.weapon_matrix(p), &w.pose, &parents(&w.rig), 0xd0a040);
            }
        }
        let mut offer = String::new();
        if let Some(en) = enemies.as_ref() {
            for e in &en.list {
                let hurt = e.obj.reacting;
                let color = if !e.alive() { 0x603030 } else if hurt { 0xffa060 } else { 0xe04040 };
                canvas.skeleton(e.matrix(), &e.anim.pose, &parents(&e.data.rig), color);
            }
            if let Some(o) = en.offer {
                offer = format!("  reaction {} ({:?}, T)", o.cmd, o.status);
            }
        }
        let foes = enemies.as_ref().map_or(0, |e| e.list.iter().filter(|e| e.alive()).count());
        window.set_title(&format!("libkh2 viewer  state {:?}  motion {}  HP {}/{}  MP {}/{}  form {}  combo {}  LV {}  enemies {foes}{offer}", sora.state, sora.body.tag(), sora.hp, sora.max_hp, sora.mp, sora.max_mp, sora.drive.form, sora.hits, sora.level));
        window.update_with_buffer(&canvas.px, W, H).expect("draw");
    }
    if let Ok(path) = std::env::var("KH2_VIEWER_SHOT") {
        let mut out = format!("P6 {W} {H} 255\n").into_bytes();
        for p in &canvas.px {
            out.extend([(p >> 16) as u8, (p >> 8) as u8, *p as u8]);
        }
        std::fs::write(path, out).expect("shot");
    }
}
