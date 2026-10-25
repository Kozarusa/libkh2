//! Motion playback for one skeleton, without Bevy.
//!
//! A transcription of kh2-rs `kh2-game-core/src/skel.rs` (`Animator::play`, `restart`, `start`,
//! `tick`, `motion`, `triggers`, `gframe`, `trigger`, and the pose step `step`): the same frame
//! stepping (motion fps / 60 per tick), loop return, root delta, the motion controller's
//! cross-fade (`kh2_anim::blend`, the game's FUN_0017dd38 family) and pose evaluation
//! (`Motion::pose_with`). Only the Bevy joint entities and the skinned meshes are left out; the
//! drawn pose stays in `pose` for the host.

use std::sync::Arc;

use glam::{Mat4, Vec3};
use kh2_anim::{Mset, Triggers};
use kh2_model::Model;

/// Ticks per second of the game's logic (kh2-rs `data::LOGIC_HZ`).
pub const LOGIC_HZ: f32 = 60.0;

/// A skeleton and its motion set (kh2-rs `skel::Rig` without the effect file name).
pub struct Rig {
    pub model: Model,
    pub mset: Mset,
}

/// One playing motion (kh2-rs `skel::Animator`).
pub struct Anim {
    pub rig: Arc<Rig>,
    pub slot: Option<usize>,
    /// Current frame in the motion's local frame units.
    pub frame: f32,
    pub prev_frame: f32,
    fresh: bool,
    pub speed: f32,
    pub looping: bool,
    pub finished: bool,
    /// Model-space bone matrices of the current (blended) pose, relative to the motion's root.
    pub pose: Vec<Mat4>,
    blend: kh2_anim::blend::Blend,
    posed: bool,
    pub root: kh2_anim::motion::RootPose,
    /// Root movement over the last tick: model-space translation and yaw.
    pub root_delta: (Vec3, f32),
    rig_state: kh2_anim::rig::RigState,
}

struct Meta {
    fps: f32,
    end: f32,
    ret: f32,
}

fn meta(m: Option<&kh2_anim::Motion>) -> Option<Meta> {
    m.map(|m| Meta { fps: m.fps.max(1.0), end: m.frame_end, ret: m.frame_return.clamp(m.frame_start, m.frame_end) })
}

fn wrap_angle(a: f32) -> f32 {
    kh2_game_core::motor::wrap_angle(a)
}

impl Anim {
    pub fn new(rig: Arc<Rig>) -> Anim {
        Anim {
            pose: kh2_model::rest_pose(&rig.model.bones),
            slot: None,
            frame: 0.0,
            prev_frame: 0.0,
            fresh: false,
            speed: 1.0,
            looping: true,
            finished: false,
            blend: Default::default(),
            posed: false,
            root: Default::default(),
            root_delta: (Vec3::ZERO, 0.0),
            rig_state: Default::default(),
            rig,
        }
    }

    /// Starts a motion slot unless it already plays. `blend` is the cross-fade length in ticks.
    pub fn play(&mut self, slot: Option<usize>, looping: bool, blend: f32) {
        if slot == self.slot && !self.finished && looping == self.looping {
            return;
        }
        self.restart(slot, looping, blend);
    }

    /// Always restarts (the same attack twice).
    pub fn restart(&mut self, slot: Option<usize>, looping: bool, blend: f32) {
        self.blend.start(self.posed.then_some(self.pose.as_slice()), blend);
        self.slot = slot;
        self.looping = looping;
        self.finished = false;
        let start = self.motion().map_or(0.0, |m| m.frame_start);
        self.frame = start;
        self.prev_frame = start;
        self.fresh = true;
        self.root = self.root_at(start);
        self.root_delta = (Vec3::ZERO, 0.0);
    }

    fn root_at(&self, frame: f32) -> kh2_anim::motion::RootPose {
        match self.motion().map(|m| (m, &m.data)) {
            Some((m, kh2_anim::motion::Data::Interpolated(ip))) => ip.root_at(m.tick(frame)).unwrap_or_default(),
            _ => Default::default(),
        }
    }

    /// Advances one 60 Hz tick: frame, looping, and root movement.
    pub fn tick(&mut self) {
        let mut step = self.speed;
        if self.blend.active() {
            match self.blend.advance(self.speed) {
                None => {
                    self.prev_frame = self.frame;
                    self.root_delta = (Vec3::ZERO, 0.0);
                    return;
                }
                Some(over) => step = over,
            }
        }
        self.prev_frame = if std::mem::take(&mut self.fresh) { self.frame - 1.0 } else { self.frame };
        let Some(m) = meta(self.motion()) else {
            self.root_delta = (Vec3::ZERO, 0.0);
            return;
        };
        let before = self.root;
        let mut moved = (Vec3::ZERO, 0.0);
        self.frame += m.fps / LOGIC_HZ * step;
        if self.frame >= m.end {
            if self.looping {
                let end = self.root_at(m.end);
                moved.0 += end.translation - before.translation;
                moved.1 += wrap_angle(end.yaw - before.yaw);
                let span = (m.end - m.ret).max(1.0 / 64.0);
                self.frame = m.ret + (self.frame - m.end) % span;
                self.prev_frame = m.ret.min(self.prev_frame);
                self.root = self.root_at(m.ret);
            } else {
                self.frame = m.end;
                self.finished = true;
            }
        }
        let now = self.root_at(self.frame);
        moved.0 += now.translation - self.root.translation;
        moved.1 += wrap_angle(now.yaw - self.root.yaw);
        self.root = now;
        self.root_delta = moved;
    }

    /// `skel::step`: one tick and, with `pose`, the drawn pose (relative to the motion's root, mixed
    /// by the running cross-fade).
    pub fn step(&mut self, pose: bool) {
        self.tick();
        if !pose {
            return;
        }
        let rig = self.rig.clone();
        let Some(motion) = self.slot.and_then(|s| rig.mset.get(s)).and_then(|s| s.motion.as_ref()) else { return };
        let mut p = motion.pose_with(&rig.model.bones, self.frame, &mut self.rig_state);
        if self.root != Default::default() {
            let undo = (Mat4::from_translation(self.root.translation) * Mat4::from_rotation_y(self.root.yaw)).inverse();
            for m in &mut p {
                *m = undo * *m;
            }
        }
        self.blend.apply(&mut p);
        self.pose = p;
        self.posed = true;
    }

    pub fn motion(&self) -> Option<&kh2_anim::Motion> {
        self.rig.mset.get(self.slot?)?.motion.as_ref()
    }

    pub fn triggers(&self) -> Option<&Triggers> {
        Some(&self.rig.mset.get(self.slot?)?.triggers)
    }

    /// Current time in global frames (60 Hz), the unit motion triggers are authored in.
    pub fn gframe(&self) -> f32 {
        self.motion().map_or(self.frame, |m| self.frame * LOGIC_HZ / m.fps.max(1.0))
    }

    /// True while range trigger `id` covers the current frame.
    pub fn trigger(&self, id: u8) -> bool {
        self.triggers().is_some_and(|t| t.is_active(self.gframe(), id))
    }

    pub fn tag(&self) -> &str {
        self.slot.and_then(|s| self.rig.mset.get(s)).map_or("-", |s| s.tag.as_str())
    }

    pub fn blending(&self) -> bool {
        self.blend.active()
    }

    /// Follows another animator's slot and frame (kh2-rs `sora::sync_weapon`: the weapons' motion
    /// sets mirror the body's slot for slot; a weapon without the slot plays nothing).
    pub fn follow(&mut self, body: &Anim) {
        let slot = body.slot.filter(|&x| self.rig.mset.has(x));
        if self.slot != slot {
            self.restart(slot, body.looping, 0.0);
        }
        self.frame = body.frame;
        self.prev_frame = body.prev_frame;
        self.looping = body.looping;
        self.speed = body.speed;
    }

    /// The pose at the current frame without advancing (the weapon after `follow`).
    pub fn pose_now(&mut self) {
        let rig = self.rig.clone();
        let Some(motion) = self.slot.and_then(|s| rig.mset.get(s)).and_then(|s| s.motion.as_ref()) else { return };
        let mut p = motion.pose_with(&rig.model.bones, self.frame, &mut self.rig_state);
        let root = self.root_at(self.frame);
        if root != Default::default() {
            let undo = (Mat4::from_translation(root.translation) * Mat4::from_rotation_y(root.yaw)).inverse();
            for m in &mut p {
                *m = undo * *m;
            }
        }
        self.pose = p;
        self.posed = true;
    }
}

/// A motion slot sampled at a frame (no blending, a fresh rig state): the pose relative to the
/// motion's root (as the animator draws it) and the root itself (model-space translation and yaw;
/// zero for motions without root motion).
pub fn sample(rig: &Rig, slot: usize, frame: f32) -> Option<(Vec<Mat4>, kh2_anim::motion::RootPose)> {
    let m = rig.mset.get(slot)?.motion.as_ref()?;
    let mut st = kh2_anim::rig::RigState::default();
    let mut pose = m.pose_with(&rig.model.bones, frame, &mut st);
    let root = match &m.data {
        kh2_anim::motion::Data::Interpolated(ip) => ip.root_at(m.tick(frame)).unwrap_or_default(),
        _ => Default::default(),
    };
    if root != Default::default() {
        let undo = (Mat4::from_translation(root.translation) * Mat4::from_rotation_y(root.yaw)).inverse();
        for p in &mut pose {
            *p = undo * *p;
        }
    }
    Some((pose, root))
}
