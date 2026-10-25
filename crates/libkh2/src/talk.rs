//! Talks with townspeople (kh2-rs `kh2-event/src/npc.rs` `talk_step`, the accept of a talk or
//! examine command, Execute 0x1f / 0x1e): the host owns its NPCs and their dialogue; when it
//! reports that Sora interacts with one, the library does the player's side of `FUN_001a5728` /
//! `FUN_001a5510` and hands the host a talk request.
//!
//! Compiled from kh2-rs source: `talk_shot` (`FUN_0018f108`: the target's type-0x13 record, else
//! its type-3 record), `MOTION_TALK`, `TALK_END_TIMER`, the camera's `start_talk` / `end_talk` and
//! `talk_pose`. This file is the glue of `talk_step` without the Bevy queries: the velocities
//! cleared (`FUN_00177d28`), Sora facing the target, the target facing Sora unless its array bit 4
//! is set (`FUN_00189f40`), the talk camera from its record (frame: the target's own with array
//! bit 4 or 6, else around the player), the control lock (`FUN_001b7d50`) until the end.
//!
//! decided: a record on a bone is read through the bone matrices the host gives (its NPC's pose,
//! model space under the root), else the model's rest pose [approx: kh2-rs reads the last pose].

use glam::{Mat4, Quat, Vec3};
use kh2_game_core::camera_rules::{TalkFrame, TalkShot};
use kh2_game_core::reaction_parts::Table;
use kh2_enemy_host::npc;

use crate::sora::Sora;

/// What the host says about the townsperson Sora interacts with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Target {
    /// Feet (cm) and facing.
    pub pos: Vec3,
    pub yaw: f32,
    /// The talk part of the offer (the reaction part index).
    pub part: u8,
    /// The object's script array bit 4 (keeps its facing, camera in its own frame) and bit 6
    /// (camera in its own frame).
    pub keep_facing: bool,
    pub own_frame: bool,
    /// Execute 0x1e (Examine): no camera, no turn of the target, no talk motion.
    pub examine: bool,
    /// The spawn record's talk id (kh2-rs `AreaRecord::talk_id`), handed back.
    pub talk_id: u32,
}

/// What the library did, for the host to run the dialogue (`FUN_001a53f8`: the talk program of
/// `talk_id`) and play the target's motion.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Request {
    pub talk_id: u32,
    pub examine: bool,
    /// The target's new facing (it turned to Sora unless it keeps its facing).
    pub target_yaw: f32,
    pub sora_yaw: f32,
    /// The motion the target plays (`MOTION_TALK`, 0x32), none for an examine.
    pub target_motion: Option<usize>,
    /// The talk camera's shot, when the target has the record (`FUN_0018f108`).
    pub shot: Option<TalkShot>,
}

/// One running talk of a Sora.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Talk {
    pub req: Request,
    /// The target's facing before the talk (it comes back at the end, `FUN_0016a828`).
    pub saved_yaw: f32,
}

/// `talk_step`'s start for an accepted talk / examine.
pub fn begin(sora: &mut Sora, parts: Option<&Table>, bones: &[Mat4], t: &Target) -> Talk {
    sora.motor.vel = Vec3::ZERO;
    let to = (t.pos - sora.motor.pos).with_y(0.0);
    if to.length_squared() > 1e-6 {
        sora.motor.yaw = to.x.atan2(to.z);
    }
    let mut req = Request { talk_id: t.talk_id, examine: t.examine, target_yaw: t.yaw, sora_yaw: sora.motor.yaw, target_motion: None, shot: None };
    if !t.examine {
        if !t.keep_facing && to.length_squared() > 1e-6 {
            req.target_yaw = (-to.x).atan2(-to.z);
        }
        let frame = if t.keep_facing || t.own_frame { TalkFrame::Object { forward: Vec3::new(req.target_yaw.sin(), 0.0, req.target_yaw.cos()) } } else { TalkFrame::AroundPlayer };
        let rot = Quat::from_rotation_y(req.target_yaw);
        let bone = |j: usize, off: Vec3| bones.get(j).map(|m| t.pos + rot * m.transform_point3(off));
        req.shot = parts.and_then(|p| npc::talk_shot(p, t.part, (t.pos, req.target_yaw), frame, &bone)).map(|(s, _)| s);
        req.target_motion = Some(npc::MOTION_TALK);
    }
    sora.control_locked = true;
    Talk { req, saved_yaw: t.yaw }
}

/// The end (`FUN_00189ff0`, `FUN_00159768`, `FUN_001b7d18`): the lock is released; the target's
/// facing comes back and its follow timer gains `TALK_END_TIMER` (both the host's).
pub fn end(sora: &mut Sora) {
    sora.control_locked = false;
}
