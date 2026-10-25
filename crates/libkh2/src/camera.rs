//! The field camera as a helper: kh2-rs's `camera_rules` (compiled in place, oracle-tested in
//! kh2-rs) driven the way kh2-rs's `camera::tick` drives it, without Bevy. The host gives the
//! right stick and buttons; the subject is the Sora instance, the collision the library's world.

use glam::{Vec2, Vec3, Vec3Swizzles};
use kh2_game_core::camera_rules::{self as rules, Blocker, Hit, Mode, Params, Rig, Sticks, Subject, TickIn};
use kh2_game_core::world::World;

use crate::sora::{Sora, State};

/// kh2-rs `camera::cast_world`: the nearest front face on the segment (COCT worlds), point,
/// normal and attribute word.
pub fn cast_world(w: &World, a: Vec3, b: Vec3) -> Option<Hit> {
    let d = b - a;
    let len = d.length();
    if len < 1e-3 {
        return None;
    }
    let dir = d / len;
    let (lo, hi) = (a.min(b), a.max(b));
    let mut best: Option<(f32, Hit)> = None;
    for (i, t) in w.tris.iter().enumerate() {
        if t.hi.x < lo.x || t.lo.x > hi.x || t.hi.y < lo.y || t.lo.y > hi.y || t.hi.z < lo.z || t.lo.z > hi.z || !w.group_enabled(i) {
            continue;
        }
        if w.coct && t.n.dot(dir) >= 0.0 {
            continue;
        }
        let Some(s) = ray_tri(a, dir, &t.v) else { continue };
        if s <= len && best.is_none_or(|(b, _)| s < b) {
            best = Some((s, Hit { point: a + dir * s, normal: t.n, attr: t.flags }));
        }
    }
    best.map(|(_, h)| h)
}

fn ray_tri(o: Vec3, d: Vec3, v: &[Vec3; 3]) -> Option<f32> {
    let e1 = v[1] - v[0];
    let e2 = v[2] - v[0];
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-8 {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - v[0];
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let vv = d.dot(q) * inv;
    if vv < 0.0 || u + vv > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 0.0).then_some(t)
}

/// One tick of camera input.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CamInput {
    /// Right stick: x right, y up (pushed forward), length 0..1.
    pub stick: [f32; 2],
    /// R3 (reset behind Sora) and Select (look-around) pressed this tick.
    pub reset: bool,
    pub select: bool,
    /// Zoom added directly (mouse wheel), units.
    pub zoom: f32,
    /// The auto-camera is allowed (the game turns it off while R2 is held).
    pub auto: bool,
    /// Frame step: 1 per 60 Hz tick, 2 per 30 Hz field frame.
    pub step: f32,
}

/// What the host draws.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CamView {
    pub pos: Vec3,
    pub look: Vec3,
    pub fov: f32,
    pub roll: f32,
    pub yaw: f32,
    pub mode: i32,
}

/// The camera of one Sora.
pub struct Camera {
    pub rig: Option<Rig>,
    pub row: usize,
    prev_pos: Vec3,
}

fn stick(x: f32, y_down: f32) -> (Vec2, f32) {
    let v = Vec2::new(x, y_down);
    let m = v.length().min(1.0);
    (if m > 0.0 { v / v.length() } else { Vec2::ZERO }, m)
}

impl Camera {
    pub fn new(row: usize) -> Camera {
        Camera { rig: None, row: row.min(3), prev_pos: Vec3::ZERO }
    }

    /// One camera tick after Sora's (kh2-rs `camera::tick`, field modes).
    pub fn tick(&mut self, p: &Params, sora: &mut Sora, world: &World, input: &CamInput) -> CamView {
        let cast = |a: Vec3, b: Vec3| cast_world(world, a, b);
        let pos = sora.motor.pos;
        let warp = pos.distance(self.prev_pos) > 500.0;
        let subject = Subject {
            pos,
            facing: sora.motor.forward(),
            moved: pos.xz() != self.prev_pos.xz(),
            run_speed: sora.game.params.prty.run_speed,
            eye_height: sora.game.params.prty.subjective_offset,
            can_look: sora.state == State::Ground,
        };
        self.prev_pos = pos;
        let row = p.rows[self.row];
        let rig = self.rig.get_or_insert_with(|| Rig::new(p, row, &subject, &cast));
        rig.row = row;
        let li = sora.last_input();
        let (left, left_mag) = stick(li.stick[0], -li.stick[1]);
        let (right, right_mag) = stick(input.stick[0], -input.stick[1]);
        let sticks = Sticks { left, left_mag, right, right_mag };
        let step = if input.step > 0.0 { input.step } else { 1.0 };
        let mut subject = subject;
        if rig.mode == Mode::Look && subject.can_look {
            let d = rules::look_turn(p, &sticks) * step;
            if d != 0.0 {
                sora.motor.yaw = rules::wrap(sora.motor.yaw + d);
                subject.facing = sora.motor.forward();
            }
        }
        let lock = sora.target.map(|t| t.pos);
        let blockers: [Blocker; 0] = [];
        rig.tick(p, &TickIn { sticks, reset: input.reset, select: input.select, subject, lock, zoom: input.zoom, auto: input.auto, warp, step, cast: &cast, blockers: &blockers });
        CamView { pos: rig.pos, look: rig.look, fov: rig.fov, roll: rig.roll, yaw: rig.yaw, mode: rig.mode as i32 }
    }
}

/// The lock-on choice (kh2-rs `sora::tick`'s `nearest`): the nearest of `points` within the
/// `sstm` LockOnRange of Sora; `None` when none is.
pub fn lock_on_pick(sora: &Sora, points: &[Vec3]) -> Option<usize> {
    let range = sora.game.params.sstm.lock_on_range;
    let at = sora.motor.pos;
    points.iter().enumerate().filter(|(_, p)| p.distance(at) <= range).min_by(|a, b| a.1.distance(at).total_cmp(&b.1.distance(at))).map(|(i, _)| i)
}
