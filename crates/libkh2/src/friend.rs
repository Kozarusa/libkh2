//! The party members' own BDX scripts.
//!
//! A transcription of kh2-rs `kh2-party/src/friend_ai.rs` (`friend_attach`, `friend_update`,
//! `trigger_bits`, `step_attacks`, `sweep_points`, `attack_size`, `attack_volume`) around kh2-rs's
//! `friend_host.rs` (the enemy host plus the members' host group 10), compiled in place in
//! `libkh2-enemy-host`. Each member's model file carries a script (`obj/P_EX020.mdlx` type 3): it
//! starts in `idle`; the battle AI switches it to a record's mode for a Special or Cure
//! (FUN_00113d48); its motion requests go through the member's slot remap and play on the body; its
//! attack objects (host 2.10) are stepped and tested against the enemies; its hits on the target
//! record (host 2.17) become damage on the enemy or, for heal rows (type 5 / 6), a heal on Sora or a
//! member (kh2-rs `party_script_hits`).
//!
//! decided: the scripts' effects (hosts 1.22 / 1.87 / 1.149) are reported as events, not played; the
//! hand object's throw (Goofy's `blade`) moves the script's object but the weapon is drawn on the
//! hand; target searches (host 6.5) walk the enemies' TARGET shapes.

use std::collections::HashMap;

use glam::Vec3;
use kh2_bdx::{Program, Runtime};
use kh2_enemy_host::enemy_ai::AiTrace;
use kh2_enemy_host::enemy_host::{BevyHost, Env};
use kh2_enemy_host::enemy_obj::Obj;
use kh2_enemy_host::friend_host::{AttackObj, FriendFields, FriendHost, FxReq, SearchPart, TARGET_ENEMY};
use kh2_game_core::magic_rules::HitMemory;

use crate::anim::Anim;

/// kh2-rs `friend_ai::HANDLE_BASE`: the members' script handles, past the enemies'.
const HANDLE_BASE: u32 = 0x1001;
/// The mode a new member's script starts in.
pub const START_MODE: &str = "idle";

/// A member's script runtime (kh2-rs `FriendBrain`).
pub struct FriendBrain {
    pub rt: Runtime,
    pub obj: Obj,
    pub tag: String,
    pub flags124: bool,
    pub started: bool,
    pub fields: FriendFields,
    /// The victims' hit memory for this member's attack objects, by enemy id.
    pub memory: HashMap<u32, HitMemory>,
    pub errors_seen: usize,
}

impl FriendBrain {
    /// kh2-rs `friend_attach`.
    pub fn new(prog: Program, flags124: bool, table: Option<&kh2_bdx::HostTable>, slot: usize, model: &str) -> FriendBrain {
        let mut rt = Runtime::new(prog);
        if let Some(t) = table {
            rt.set_host_table(t.clone());
        }
        rt.shared.rng = 0x51ed_270b ^ (slot as u32).wrapping_mul(0x85eb_ca6b);
        let handle = kh2_enemy_host::shared::enemy_obj::SELF_BASE + (HANDLE_BASE + slot as u32) * kh2_enemy_host::shared::enemy_obj::OBJ_SIZE as u32;
        FriendBrain { rt, obj: Obj::new(handle), tag: format!("{model}#{slot}"), flags124, started: false, fields: FriendFields::default(), memory: HashMap::new(), errors_seen: 0 }
    }
}

/// kh2-rs `friend_ai::trigger_bits`: range triggers 16..19 active, frame triggers 9..12 crossed
/// (counted from the motion's first frame).
pub fn trigger_bits(anim: &Anim) -> [u8; 2] {
    let (Some(t), Some(m)) = (anim.triggers(), anim.motion()) else { return [0; 2] };
    let g = |f: f32| (f - m.frame_start) * crate::anim::LOGIC_HZ / m.fps.max(1.0);
    let mut bits = [0u8; 2];
    for k in 0..4u8 {
        if t.is_active(anim.gframe(), 16 + k) {
            bits[0] |= 1 << k;
        }
    }
    if anim.frame != anim.prev_frame {
        for ft in t.crossed(g(anim.prev_frame), g(anim.frame)).filter(|f| (9..=12).contains(&f.id)) {
            bits[1] |= 1 << (ft.id - 9);
        }
    }
    bits
}

/// kh2-rs `friend_ai::sweep_points` (FUN_001a2cc0's sweep).
fn sweep_points(pos: Vec3, prev: Option<Vec3>, step: f32) -> Vec<Vec3> {
    let d = prev.map_or(Vec3::ZERO, |p| p - pos);
    let mut left = if d.length() > 2.0 { d.length() } else { 0.0 };
    let dir = d.normalize_or_zero();
    let (mut p, mut out) = (pos, Vec::new());
    loop {
        out.push(p);
        p += dir * step;
        left -= step;
        if step * 0.5 >= left {
            return out;
        }
    }
}

/// kh2-rs `friend_ai::attack_size`.
fn attack_size(a: &AttackObj) -> (f32, f32) {
    if a.grow > 0.0 {
        let s = (a.t / a.grow * std::f32::consts::PI * 0.5).sin();
        (a.from.0 * (1.0 - s) + a.radius * s, a.from.1 * (1.0 - s) + a.height * s)
    } else {
        (a.radius, a.height)
    }
}

/// kh2-rs `friend_ai::attack_volume`, in centimetres.
fn attack_volume(c: Vec3, radius: f32, height: f32) -> crate::enemy::Volume {
    let half = (height - radius).max(0.0);
    crate::enemy::Volume { a: c - Vec3::Y * half, b: c + Vec3::Y * half, r: radius }
}

/// One hit of a member's attack object: (enemy id, atkp row index).
pub struct ObjectHit {
    pub enemy: u32,
    pub row: usize,
    pub handle: u32,
}

/// kh2-rs `friend_ai::step_attacks` (FUN_0018c2d8 over the member's attack objects) against the
/// enemies' hurt volumes (by id).
pub fn step_attacks(fields: &mut FriendFields, memory: &mut HashMap<u32, HitMemory>, atkp: &[kh2_tables::tables::Atkp], victims: &[(u32, Vec<crate::enemy::Volume>)]) -> Vec<ObjectHit> {
    let mut out = Vec::new();
    fields.attacks.retain(|a| !a.released);
    for a in fields.attacks.iter_mut() {
        a.steps += 1;
        if a.grow > 0.0 {
            a.t += 1.0;
            if a.grow < a.t {
                a.released = true;
                continue;
            }
        }
        let Some(pos) = a.pos else { continue };
        let (r, h) = attack_size(a);
        let step = r.min(h);
        if step <= 0.0 {
            continue;
        }
        let Some(base) = kh2_game_core::magic_rules::attack_row(atkp, a.atkp, a.kind) else { continue };
        let points = sweep_points(pos, a.prev, step);
        for (id, shapes) in victims {
            let mem = memory.entry(*id).or_default();
            if !mem.may_hit(atkp[base].r#type, a.group) {
                continue;
            }
            if !points.iter().any(|&c| {
                let v = attack_volume(c, r, h);
                shapes.iter().any(|hv| crate::enemy::overlap(&v, hv))
            }) {
                continue;
            }
            let n = mem.count_hit(a.group);
            let vi = kh2_game_core::magic_rules::variant(atkp, base, n);
            mem.mark(a.group, atkp[vi].interval);
            a.hit = true;
            out.push(ObjectHit { enemy: *id, row: vi, handle: a.handle });
        }
    }
    out
}

/// What the script asked of the member this tick (kh2-rs `friend_update`'s hand-over to `party`).
#[derive(Default)]
pub struct ScriptOut {
    pub slot: Option<usize>,
    pub queued: Option<usize>,
    pub moved: Option<Vec3>,
    pub yaw: Option<f32>,
    pub invuln: Option<f32>,
    pub paid: Option<u8>,
    pub target_hits: Vec<i16>,
    pub object_hits: Vec<ObjectHit>,
    pub mode: Option<String>,
    pub effects: Vec<u16>,
    /// Host 6.5 wrote the target record: Some(enemy id, shape) or a clear.
    pub searched: Option<Option<(u32, usize)>>,
}

/// The world the script reads this tick.
pub struct ScriptWorld<'a> {
    pub player: Vec3,
    pub player_yaw: f32,
    pub world: &'a kh2_game_core::world::World,
    pub in_battle: bool,
    pub victims: &'a [(u32, Vec<crate::enemy::Volume>)],
    pub targets: &'a [SearchPart],
    pub atkp: &'a kh2_enemy_host::data::GameData,
    pub free: Option<&'a kh2_game_core::party_rules::FreePointConsts>,
}

/// kh2-rs `friend_update` for one member.
#[allow(clippy::too_many_arguments)]
pub fn update(brain: &mut FriendBrain, body: &Anim, pos: Vec3, yaw: f32, order: [u8; 4], ai: Option<&kh2_game_core::party_battle::ClassAi>, mode_request: Option<(String, u8)>, trace: &mut AiTrace, w: &ScriptWorld) -> ScriptOut {
    let FriendBrain { rt, obj, tag, flags124, started, fields, memory, errors_seen } = brain;
    fields.order = order;
    if let Some(ai) = ai {
        fields.entries_at = ai.entries_at;
        fields.entries = ai.entries.clone();
    }
    fields.in_battle = w.in_battle;
    for m in memory.values_mut() {
        m.tick();
    }
    obj.motor.pos = pos;
    obj.motor.yaw = yaw;
    obj.motor.grounded = true;
    obj.slot = body.slot;
    obj.frame = body.frame;
    obj.trigger_bits = trigger_bits(body);
    obj.done = body.slot.is_some() && (body.finished || body.looping && body.frame < body.prev_frame);
    let yaw_before = obj.motor.yaw;
    let paired = HashMap::new();
    let env = Env { player: w.player, player_yaw: w.player_yaw, world: Some(w.world), rig: &body.rig, flags124: *flags124, enmp: 0, player_serial: 0, player_motion_done: true, paired: &paired, player_model: 0, mission_counters: None, event_active: false };
    let handle = obj.handle;
    let mut out = ScriptOut::default();
    {
        let mut host = FriendHost { inner: BevyHost { obj, env: &env, trace, tag: tag.as_str() }, fields, game: w.atkp, free: w.free, targets: w.targets };
        if !*started {
            *started = true;
            rt.spawn(&mut host, handle);
            rt.set_mode_named(&mut host, START_MODE);
        }
        if let Some((want, mp)) = mode_request {
            host.fields.pending_mp = mp;
            rt.set_mode_named(&mut host, &want);
        }
        rt.update(&mut host);
    }
    if let Some((found, _)) = fields.searched.take() {
        out.searched = Some(found.map(|(e, s)| (e.0 as u32, s)));
    }
    out.paid = fields.paid.take();
    out.target_hits = std::mem::take(&mut fields.target_hits);
    out.object_hits = step_attacks(fields, memory, &w.atkp.atkp, w.victims);
    // An attack object's routine (2.38) runs for every hit (FUN_0018cf50).
    let calls: Vec<(u32, u32, u32, u32)> = out.object_hits.iter().filter_map(|h| fields.attacks.iter().find(|a| a.handle == h.handle)?.callback.map(|(pc, arg)| (pc, arg, h.handle, h.enemy))).collect();
    for (pc, arg, handle, victim) in calls {
        fields.callback_victim = Some(kh2_enemy_host::Entity(u64::from(victim)));
        let mut host = FriendHost { inner: BevyHost { obj, env: &env, trace, tag: tag.as_str() }, fields, game: w.atkp, free: w.free, targets: w.targets };
        rt.call_routine(&mut host, pc, &[arg, TARGET_ENEMY, handle]);
    }
    fields.callback_victim = None;
    if let Some(e) = fields.retarget.take() {
        out.searched = Some(Some((e.0 as u32, 0)));
    }
    for f in fields.fx.iter_mut().filter(|f| !f.started) {
        f.started = true;
        f.gone = true;
        out.effects.push(match f.req {
            FxReq::Bound { id, .. } | FxReq::At { id, .. } => id,
        });
    }
    fields.fx.retain(|f| f.gone_ticks < 600);
    for f in fields.fx.iter_mut() {
        f.gone_ticks += 1;
    }
    if let Some(w) = fields.weapon.as_mut() {
        if !w.linked {
            w.pos += w.vel;
        }
        w.vel = Vec3::ZERO;
    }
    out.mode = rt.current_mode();
    out.slot = obj.motion_req.take().map(|r| r.slot);
    out.queued = fields.queued.take();
    out.moved = (obj.move_vel != Vec3::ZERO).then_some(obj.move_vel);
    out.yaw = (obj.motor.yaw != yaw_before).then_some(obj.motor.yaw);
    out.invuln = fields.invuln.take();
    obj.move_vel = Vec3::ZERO;
    obj.effects.clear();
    obj.outbox.clear();
    obj.player_cmds.clear();
    obj.drops.clear();
    if let Some(l) = rt.core.mode_log.as_mut() {
        l.clear();
    }
    *errors_seen = rt.core.errors.len();
    out
}
