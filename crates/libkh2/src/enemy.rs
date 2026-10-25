//! Enemies under their own BDX scripts.
//!
//! The script runtime is `kh2_bdx` (kh2-rs's interpreter, checked against the original there); the
//! host that answers the script's calls is kh2-rs's `enemy_host.rs` compiled in place
//! (`kh2_enemy_host`). This module is the plain-struct transcription of the Bevy systems around
//! them in kh2-rs:
//!
//! * `enemy_ai.rs`: `load_programs` (the BDX entry of the model, the objentry flag, the host table),
//!   `ai_attach` (a runtime per enemy), `ai_update` (events 6 / 0x11, messages, the script tick, then
//!   carrying out what it asked: motion, root motion, the dash mover, movement through `Motor`,
//!   jump follow-up motions, status rebuilds, HP changes, fades, hide), `dispatch_messages`,
//!   `motion_events`, `trigger_bits`;
//! * `enemy.rs`: `spawn_enemy`, `think` (the hit reaction states), `receive_hits` (damage and the
//!   victim reaction of the attack row);
//! * `data.rs`: `GameData::enemy_stats` and `GameData::damage`;
//! * `combat.rs`: the attack and hurt volumes and their overlap test.
//!
//! Reaction commands (round 4/5): kh2-rs `reaction_cmd.rs`'s offer from the motions' range trigger
//! 20 and from the objects' reaction parts (`reaction_parts.rs` + `part_offer`, FUN_0019cb70: the
//! Soldier's five parts offer command 34), the accept, event 11 to the script, and what the script
//! then asks of Sora (`PlayerReactionCmd`: the paired motion from the enemy's set, the velocity,
//! the queued motion, the unhittable bit) handed to `Sora::reaction`.
//!
//! decided: no clashes, no spawn groups from the area scripts (the host spawns), no camera (every
//! enemy is on screen), prize drops reported as counts (no prize objects).

pub use kh2_game_core::reaction_cmd::Status;
use kh2_game_core::reaction_cmd::{offers_at, status, CmdRow, Ctx, Offer};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use kh2_bdx::{Program, Runtime};
use kh2_enemy_host::data::Stats;
use kh2_enemy_host::enemy_ai::AiTrace;
use kh2_enemy_host::enemy_host::{compute_drop, parse_przt, prize_row, BevyHost, Env, PrizeRow};
use kh2_enemy_host::enemy_obj::{handle_of_serial, MotionReq, Obj, HIT, PLAYER};
use kh2_enemy_host::shared::enemy::{EState, Kind};
use kh2_game_core::motor::wrap_angle;
use kh2_game_core::reaction_rules::{self as rules, Blow, Push, VictimAction, VictimCtx};
use kh2_game_core::world::World;
use kh2_tables::tables::{self as t, Atkp};

use crate::anim::{Anim, Rig};
use crate::data::{Disc, LoadError};
use crate::sora::Sora;

/// Motion ids of the reaction code (kh2-rs `enemy.rs`).
const HURT_FRONT: usize = 10;
const BLOW: usize = 16;
/// Size of the game's message queue (FUN_00187170).
const MAILBOX_LEN: usize = 0x40;
/// An airborne enemy this far below Sora has fallen out of the map (kh2-rs `FALL_OUT_UNITS`).
const FALL_OUT_UNITS: f32 = 3000.0;

/// Everything an enemy kind needs from the disc.
pub struct KindData {
    pub kind: &'static Kind,
    pub rig: Arc<Rig>,
    pub prog: Option<Program>,
    pub flags124: bool,
    pub object_id: u32,
    /// The paired player motions of the kind's motion set (kh2-rs `player_reaction::paired_entries`).
    pub paired: HashMap<usize, Vec<kh2_enemy_host::player_reaction::Paired>>,
    /// The reaction parts of the kind's object file (kh2-rs `reaction_parts::Table`).
    pub parts: Option<kh2_game_core::reaction_parts::Table>,
}

impl KindData {
    /// Host 1.29's view of the paired entries: (sub-motion name, length in 60 Hz ticks).
    fn paired_ticks(&self) -> HashMap<usize, Vec<(String, f32)>> {
        self.paired
            .iter()
            .map(|(k, v)| (*k, v.iter().map(|(n, s)| (n.clone(), s.motion.as_ref().map_or(0.0, |m| (m.frame_end - m.frame_start).max(0.0) * crate::anim::LOGIC_HZ / m.fps.max(1.0)))).collect()))
            .collect()
    }
}

/// The enemy tables (kh2-rs `AiPrograms` plus `GameData`'s enemy rows).
pub struct EnemyData {
    /// By kind name and by model name.
    pub kinds: HashMap<&'static str, Arc<KindData>>,
    pub table: Option<kh2_bdx::HostTable>,
    pub prizes: Vec<PrizeRow>,
    pub enmp: Vec<t::Enmp>,
    pub lvpm: Vec<t::Lvpm>,
    pub player_model: u8,
    /// The `cmd` rows a reaction offer reads (kh2-rs `ReactionRules`).
    pub cmds: Vec<CmdRow>,
    /// Sora's own collision records (P_EX100): his type 1 body volume touches the reaction parts.
    pub player_parts: Option<kh2_game_core::reaction_parts::Table>,
}

impl EnemyData {
    /// kh2-rs `GameData::enemy_stats`: HP per part = ceil(lvpm.HpMultiplier x enmp.Health / 100),
    /// damage caps per ten-thousand of HP, strength caps from lvpm.
    pub fn stats(&self, enmp_id: u16, world_level: i32) -> Stats {
        let Some(e) = self.enmp.iter().find(|e| e.id == enmp_id).or_else(|| self.enmp.first()) else { return Stats { max_hp: 1, weakness: [100; 7], ..Default::default() } };
        let level = if e.level == 0 { world_level } else { e.level as i32 };
        let Some(lv) = self.lvpm.get(((level - 1).max(0) as usize).min(self.lvpm.len().saturating_sub(1))) else { return Stats { max_hp: 1, weakness: [100; 7], ..Default::default() } };
        let hp = (lv.hp_multiplier as i32 * e.health[0] as i32 + 99) / 100;
        Stats {
            level,
            max_hp: hp.max(1),
            strength: lv.strength as i32,
            magic: lv.strength as i32,
            defense: lv.defense as i32,
            ap: 0,
            max_mp: 0,
            max_taken: (hp * e.max_damage as i32 + 9999) / 10000,
            min_taken: (hp * e.min_damage as i32 + 9999) / 10000,
            max_dealt: lv.max_strength as i32,
            min_dealt: lv.min_strength as i32,
            weakness: [e.physical_weakness, e.fire_weakness, e.ice_weakness, e.thunder_weakness, e.dark_weakness, e.light_weakness, e.general_weakness].map(|w| w as i32),
        }
    }

    /// The lvpm Experience of the world level and the enmp Experience of a row (for the payout).
    pub fn experience(&self, enmp_id: u16, world_level: i32) -> (u16, u16) {
        let e = self.enmp.iter().find(|e| e.id == enmp_id);
        let level = e.map_or(world_level, |e| if e.level == 0 { world_level } else { e.level as i32 });
        let lv = self.lvpm.get(((level - 1).max(0) as usize).min(self.lvpm.len().saturating_sub(1)));
        (lv.map_or(0, |l| l.experience), e.map_or(0, |e| e.experience))
    }
}

/// kh2-rs `GameData::damage`: the game's per-hit damage.
pub fn damage(atk: &Atkp, from: &Stats, to: &Stats) -> i32 {
    let stat = if atk.critical_adjust == 1 { from.magic } else { from.strength };
    let power = atk.power as i32;
    let dmg = match atk.r#type {
        5 => (to.max_hp * stat * power + 9999) / 10000,
        6 => (stat * power + 99) / 100,
        _ => {
            let cap_hi = to.max_taken.min(if from.max_dealt > 0 { from.max_dealt } else { i32::MAX });
            let cap_lo = to.min_taken.max(from.min_dealt);
            let base = (stat - to.defense).min(cap_hi).max(cap_lo);
            let elem = to.weakness[(atk.element as usize).min(6)] as i64;
            ((base as i64 * power as i64 * elem * to.weakness[6] as i64 + 999_999) / 1_000_000) as i32
        }
    };
    dmg.max(0)
}

impl Disc {
    /// kh2-rs `enemy_ai::load_programs` for every kind of `kh2_enemy_host::KINDS`, plus the tables.
    /// Kinds whose files are missing are skipped.
    pub fn enemies(&mut self) -> Result<EnemyData, LoadError> {
        let objs = kh2_tables::load::<t::Objentry>(&mut self.0, &t::OBJENTRY).map_err(|e| LoadError(format!("objentry: {e}")))?.rows;
        let enmp = kh2_tables::load::<t::Enmp>(&mut self.0, &t::ENMP).map_err(|e| LoadError(format!("enmp: {e}")))?.rows;
        let lvpm = kh2_tables::load::<t::Lvpm>(&mut self.0, &t::LVPM).map_err(|e| LoadError(format!("lvpm: {e}")))?.rows;
        let table = self.exe().and_then(|e| kh2_bdx::HostTable::from_exe(&e));
        let prizes = self.read("00battle.bin").ok().and_then(|b| kh2_bar::parse(&b).ok().and_then(|e| e.iter().find(|x| x.tag == "przt").map(|x| parse_przt(x.data)))).unwrap_or_default();
        let player_model = objs.iter().find(|r| r.model_name == "P_EX100").map_or(1, |r| r.weapon_joint);
        let mut kinds = HashMap::new();
        for &kind in kh2_enemy_host::KINDS {
            let row = objs.iter().find(|r| r.model_name == kind.model);
            let mset = kh2_enemy_host::shared::enemy::mset_name(row.map(|r| r.animation_name.as_str()), kind.model);
            let Ok(rig) = self.rig_named(kind.model, &mset) else { continue };
            let mdlx = self.read(&format!("obj/{}.mdlx", kind.model)).ok();
            let prog = mdlx.as_deref().and_then(|f| kh2_bar::parse(f).ok().and_then(|b| b.into_iter().find(|e| e.kind == 3).map(|e| e.data.to_vec()))).and_then(|e| Program::parse(&e).ok());
            let parts = mdlx.as_deref().and_then(kh2_game_core::reaction_parts::Table::from_mdlx);
            let paired = self.read(&format!("obj/{mset}.mset")).map(|m| kh2_enemy_host::player_reaction::paired_entries(&m)).unwrap_or_default();
            let kd = Arc::new(KindData { kind, rig: Arc::new(rig), prog, flags124: row.is_some_and(|r| r.flags & 8 != 0), object_id: row.map_or(0, |r| r.object_id), paired, parts });
            // By the kind's name (`Shadow`, `Soldier`, ...) and by its model (`M_EX020`).
            kinds.insert(kind.model, kd.clone());
            kinds.insert(kind.name, kd);
        }
        let cmds = kh2_tables::load::<t::Cmd>(&mut self.0, &t::CMD).map(|t| t.rows.iter().map(CmdRow::from).collect()).unwrap_or_default();
        let player_parts = self.read("obj/P_EX100.mdlx").ok().and_then(|f| kh2_game_core::reaction_parts::Table::from_mdlx(&f));
        Ok(EnemyData { kinds, table, prizes, enmp, lvpm, player_model, cmds, player_parts })
    }
}

/// A capsule in world units (kh2-rs `combat::Volume`, centimetres here).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    pub a: Vec3,
    pub b: Vec3,
    pub r: f32,
}

/// kh2-rs `combat::shape_volume`.
fn shape_volume(s: &kh2_model::collision::Shape, pose: &[Mat4], root: Mat4) -> Volume {
    let m = match s.bone {
        Some(b) => root * pose.get(b).copied().unwrap_or(Mat4::IDENTITY),
        None => root,
    };
    let c = m.transform_point3(Vec3::from(s.offset));
    let h = if matches!(s.shape, 0 | 1) { s.height.max(0.0) } else { 0.0 };
    Volume { a: c, b: c + Vec3::Y * h, r: s.radius.abs() }
}

/// kh2-rs `combat::seg_dist`: the distance between two segments.
fn seg_dist(p1: Vec3, q1: Vec3, p2: Vec3, q2: Vec3) -> f32 {
    let (d1, d2, r) = (q1 - p1, q2 - p2, p1 - p2);
    let (a, e, f) = (d1.dot(d1), d2.dot(d2), d2.dot(r));
    let (s, t) = if a <= 1e-8 && e <= 1e-8 {
        (0.0, 0.0)
    } else if a <= 1e-8 {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= 1e-8 {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let den = a * e - b * b;
            let mut s = if den > 1e-8 { ((b * f - c * e) / den).clamp(0.0, 1.0) } else { 0.0 };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };
    ((p1 + d1 * s) - (p2 + d2 * t)).length()
}

pub fn overlap(x: &Volume, y: &Volume) -> bool {
    seg_dist(x.a, x.b, y.a, y.b) <= x.r + y.r
}

/// The hurt volumes of a body (kh2-rs `combat::hurt_volumes`: shapes of kind 2 and 3).
pub fn hurt_volumes(rig: &Rig, pose: &[Mat4], root: Mat4) -> Vec<Volume> {
    rig.model.shapes.iter().filter(|s| s.kind == kh2_model::collision::kind::BATTLE || s.kind == kh2_model::collision::kind::TARGET).map(|s| shape_volume(s, pose, root)).collect()
}

/// The live attack volumes of a body: (atkp, trigger start, volume) of range triggers 10 / 33 that
/// cover the frame (kh2-rs `combat::live_attacks`).
pub fn live_attacks(anim: &Anim, root: Mat4) -> Vec<(i16, i16, Volume)> {
    let Some(tr) = anim.triggers() else { return Vec::new() };
    let mut out = Vec::new();
    for r in tr.active(anim.gframe()).filter(|r| r.id == kh2_anim::triggers::range::ATTACK || r.id == kh2_anim::triggers::range::ATTACK_COMBO) {
        let (Some(&atkp), Some(&group)) = (r.params.first(), r.params.get(1)) else { continue };
        for s in anim.rig.model.shapes.iter().filter(|s| s.kind == kh2_model::collision::kind::ATTACK && s.group as i16 == group) {
            out.push((atkp, r.start, shape_volume(s, &anim.pose, root)));
        }
    }
    out
}

/// Something the enemies did that the host may show or react to.
#[derive(Clone, Debug, PartialEq)]
pub enum EnemyEvent {
    /// Sora's attack hit enemy `id` (atkp row, damage, HP left).
    Hit { id: u32, atkp: i16, damage: i32, hp: i32 },
    /// An enemy's attack hit Sora (atkp row, damage, `Sora::hit`'s result code).
    HitSora { id: u32, atkp: i16, damage: i32, result: i32 },
    /// HP reached 0: the enmp row, the lvpm and enmp Experience values (already paid to Sora).
    Defeated { id: u32, enmp: u16, lvpm_experience: u16, enmp_experience: u16, paid: i32 },
    /// The script started element `effect` of the enemy's effect file.
    Effect { id: u32, effect: u16 },
    /// A prize drop (host 1.92): row and the prize counts per kind, the items.
    Drop { id: u32, row: u16, prizes: [u32; 9], items: Vec<u16> },
    /// The enemy is gone (hidden by its script, or fell out of the map).
    Removed { id: u32 },
    /// An accepted reaction command reached enemy `id`'s script as event 11 (`handled`: the script has a handler).
    Reaction { id: u32, cmd: u16, handled: bool },
}

/// One enemy (kh2-rs `Enemy` + `AiBrain` + its `Animator`).
pub struct Enemy {
    pub id: u32,
    pub data: Arc<KindData>,
    pub rt: Option<Runtime>,
    pub obj: Obj,
    pub anim: Anim,
    pub tag: String,
    pub enmp: u16,
    pub stats: Stats,
    pub hp: i32,
    pub min_hp: i32,
    pub state: EState,
    pub timer: f32,
    pub push: Option<Push>,
    pub blow: Option<Blow>,
    pub blown_back: bool,
    pub started: bool,
    pub dead_sent: bool,
    pub defeated: bool,
    pub last_frame: f32,
    pub in_air: bool,
    pub hits: Vec<(bool, u8, i16)>,
    /// Hits landed by this enemy's attacks: (trigger start) of the motion slot that landed them.
    pub landed: HashSet<(Option<usize>, i16)>,
    pub removed: bool,
}

impl Enemy {
    pub fn pos(&self) -> Vec3 {
        self.obj.motor.pos
    }

    pub fn alive(&self) -> bool {
        self.hp > 0
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(Quat::from_rotation_y(self.obj.motor.yaw), self.obj.motor.pos)
    }
}

/// Every enemy of one battle, the mailbox and the trace.
pub struct Enemies {
    pub data: Arc<EnemyData>,
    pub list: Vec<Enemy>,
    pub serial: u32,
    pub next_id: u32,
    pub trace: AiTrace,
    pub mail: Vec<(u32, String, f32)>,
    pub seed: u32,
    pub events: Vec<EnemyEvent>,
    /// Sora's hits already landed: (enemy id, Sora's motion slot, trigger start).
    pub sora_landed: HashSet<(u32, Option<usize>, i16)>,
    pub world_level: i32,
    /// Sora's spells in flight (`spell.rs`).
    pub spells: crate::spell::Spells,
    /// The command in the player's slot 1 this tick (kh2-rs `ReactionOffer`).
    pub offer: Option<ReactionOffer>,
    /// kh2-rs's harness `FakeOffer`: every living enemy offers this command (no enemy the library
    /// can spawn offers one from its motions).
    pub fake_offer: Option<u16>,
    /// Accepted commands waiting for their target's tick: (enemy id, command, position index).
    pub reactions: Vec<(u32, u16, i16)>,
    /// Use counters by `ReactionCount` slot (kh2-rs `ReactionUses`, capped at 999).
    pub uses: HashMap<u16, u16>,
}

/// An offered reaction command (kh2-rs `reaction_cmd::Active`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReactionOffer {
    pub cmd: u16,
    pub pos: i16,
    pub from: u32,
    pub status: Status,
    pub message: u32,
}

impl Enemies {
    pub fn new(data: Arc<EnemyData>) -> Enemies {
        Enemies { data, list: Vec::new(), serial: 0, next_id: 1, trace: AiTrace::default(), mail: Vec::new(), seed: 1, events: Vec::new(), sora_landed: HashSet::new(), world_level: 1, spells: Default::default(), offer: None, fake_offer: None, reactions: Vec::new(), uses: HashMap::new() }
    }

    /// kh2-rs `enemy::spawn_enemy` + `enemy_ai::ai_attach`: the kind by its name (`M_EX020` Shadow,
    /// `M_EX010` Soldier, `M_EX990` Dusk, ...) at `at`, facing `yaw`. Returns its id.
    pub fn spawn(&mut self, kind: &str, at: Vec3, yaw: f32, world: &World) -> Option<u32> {
        let k = self.data.kinds.get(kind)?.clone();
        let stats = self.data.stats(k.kind.enmp, self.world_level);
        self.serial += 1;
        let serial = self.serial;
        let rt = k.prog.clone().map(|p| {
            let mut rt = Runtime::new(p);
            if let Some(t) = &self.data.table {
                rt.set_host_table(t.clone());
            }
            rt.shared.rng = 0x9E37_79B9u32 ^ serial.wrapping_mul(0x85EB_CA6B);
            rt
        });
        let mut obj = Obj::new(handle_of_serial(serial));
        obj.motor.pos = at;
        obj.motor.yaw = yaw;
        if let Some(g) = world.ground(at, at.y + 40.0, 200.0) {
            obj.motor.pos.y = g.y;
            obj.motor.grounded = true;
        }
        let id = self.next_id;
        self.next_id += 1;
        let mut anim = Anim::new(k.rig.clone());
        let idle = (0..4).map(|v| v as usize).find(|&s| k.rig.mset.has(s));
        anim.play(idle, true, 0.0);
        self.list.push(Enemy {
            id,
            tag: format!("{}#{serial}", k.kind.name),
            enmp: k.kind.enmp,
            hp: stats.max_hp,
            stats,
            min_hp: 0,
            state: EState::Idle,
            timer: 0.0,
            push: None,
            blow: None,
            blown_back: false,
            started: false,
            dead_sent: false,
            defeated: false,
            last_frame: 0.0,
            in_air: false,
            hits: Vec::new(),
            landed: HashSet::new(),
            removed: false,
            data: k,
            rt,
            obj,
            anim,
        });
        Some(id)
    }

    /// One 60 Hz tick of every enemy after Sora's (kh2-rs `ai_update`, `think`, the combat tests and
    /// `receive_hits`), against Sora: his attacks hit them, theirs hit him.
    pub fn tick(&mut self, sora: &mut Sora, world: &World, fall_max: f32) {
        self.trace.tick += 1;
        self.offer(sora);
        self.combat(sora);
        self.spells_tick(sora, world);
        // Last tick's messages go to every scripted object, each with its own delay (FUN_00187250).
        let mut mail: HashMap<u32, Vec<(u32, String, f32)>> = HashMap::new();
        for (sender, name, delay) in std::mem::take(&mut self.mail) {
            for e in &self.list {
                let d = kh2_bdx::message_delay(&mut self.seed, delay);
                mail.entry(e.id).or_default().push((sender, name.clone(), d));
            }
        }
        let others: Arc<HashMap<u32, kh2_enemy_host::enemy_obj::Other>> = Arc::new(self.list.iter().map(|e| (e.obj.handle, kh2_enemy_host::enemy_obj::Other { pos: e.obj.motor.pos, slot: e.anim.slot })).collect());
        let player = sora.motor.pos;
        let player_yaw = sora.motor.yaw;
        let reaction = sora.game.params.reaction.clone();
        let (player_serial, player_motion_done) = (sora.reaction.serial, sora.reaction.paired_done);
        for e in self.list.iter_mut().filter(|e| !e.removed) {
            Self::think(e, reaction.as_ref(), world, fall_max);
            let rig = e.data.rig.clone();
            let obj = &mut e.obj;
            obj.hp = e.hp;
            obj.reacting = matches!(e.state, EState::Hurt | EState::Blown);
            obj.on_screen = true;
            if !obj.motor.grounded && !e.in_air {
                obj.motor.start_fall();
            }
            let wrapped = e.anim.looping && e.anim.frame < e.last_frame;
            obj.done = e.anim.slot.is_some() && (e.anim.finished || wrapped);
            obj.slot = e.anim.slot;
            obj.frame = e.anim.frame;
            if obj.done && obj.motion_req.is_none() && !obj.motion_queue.is_empty() {
                let (slot, blend) = obj.motion_queue.remove(0);
                obj.motion_req = Some(MotionReq { slot, blend, start: None });
                obj.slot = Some(slot);
                obj.done = false;
                obj.blend_left = blend.max(0.0);
            }
            obj.blend_left = (obj.blend_left - 1.0).max(0.0);
            obj.trigger_bits = trigger_bits(&e.anim);
            let motion_events = motion_events(&e.anim);
            obj.hp_max = e.stats.max_hp;
            obj.hp_min = e.min_hp;
            obj.others = others.clone();
            let paired = e.data.paired_ticks();
            let env = Env {
                player,
                player_yaw,
                world: Some(world),
                rig: &rig,
                flags124: e.data.flags124,
                enmp: e.enmp,
                player_serial,
                player_motion_done,
                paired: &paired,
                player_model: self.data.player_model,
                mission_counters: None,
                event_active: false,
            };
            let handle = obj.handle;
            if let Some(rt) = e.rt.as_mut() {
                let mut host = BevyHost { obj, env: &env, trace: &mut self.trace, tag: e.tag.as_str() };
                if !e.started {
                    e.started = true;
                    rt.spawn(&mut host, handle);
                }
                for m in std::mem::take(&mut host.obj.modes_asked) {
                    rt.set_mode_named(&mut host, &m);
                }
                if e.hp <= 0 && !e.dead_sent {
                    e.dead_sent = true;
                    rt.set_mode_named(&mut host, "dead");
                }
                for (real, reaction, atkp) in std::mem::take(&mut e.hits) {
                    host.obj.next_hit += 1;
                    let h = HIT + host.obj.next_hit;
                    host.obj.hits.insert(h, (real, reaction, atkp));
                    rt.event(&mut host, 6, &[h]);
                }
                for (code, arg) in motion_events {
                    rt.event(&mut host, 17, &[handle, u32::from(code), u32::from(arg)]);
                }
                // An accepted reaction command: event 0xb with (player, command id, target record)
                // (kh2-rs `enemy_ai`, FUN_00177c20 -> FUN_0016b420).
                for k in (0..self.reactions.len()).rev() {
                    if self.reactions[k].0 != e.id {
                        continue;
                    }
                    let (_, cmd, pos) = self.reactions.remove(k);
                    let rec = host.obj.new_record(handle, pos);
                    let handled = rt.event(&mut host, 11, &[kh2_enemy_host::enemy_obj::PLAYER, cmd as u32, rec]);
                    self.events.push(EnemyEvent::Reaction { id: e.id, cmd, handled });
                }
                for (sender, name, delay) in mail.remove(&e.id).unwrap_or_default() {
                    rt.receive_message(&mut host, &name, sender, delay);
                }
                rt.update(&mut host);
            }
            let obj = &mut e.obj;
            for m in obj.outbox.drain(..) {
                if self.mail.len() < MAILBOX_LEN {
                    self.mail.push(m);
                }
            }
            for id in obj.drops.drain(..) {
                if let Some(row) = prize_row(&self.data.prizes, id) {
                    let d = compute_drop(row, 1.0, 1.0, &mut self.seed, 0);
                    self.events.push(EnemyEvent::Drop { id: e.id, row: d.row, prizes: d.prizes, items: d.items });
                }
            }
            if let Some(row) = obj.param_row.take() {
                e.stats = self.data.stats(row, self.world_level);
                e.hp = e.stats.max_hp;
                e.enmp = row;
            }
            for (i, pct) in std::mem::take(&mut obj.weak_scale) {
                let base = self.data.stats(e.enmp, self.world_level).weakness[i];
                e.stats.weakness[i] = (base * pct + 99) / 100;
            }
            e.min_hp = obj.hp_min;
            if obj.hp_add != 0 {
                e.hp = (e.hp + std::mem::take(&mut obj.hp_add)).max(e.min_hp).min(e.stats.max_hp);
            }
            // What the script asked of the player (kh2-rs `player_reaction::consume`, one tick later).
            for c in obj.player_cmds.drain(..) {
                use kh2_enemy_host::shared::reaction::PlayerReactionCmd as P;
                let name = kh2_enemy_host::player_reaction::sub_name(self.data.player_model);
                sora.reaction.pending.push(match c {
                    P::PairedMotion { slot, blend, serial, .. } => crate::sora::ReactionCmd::Paired { motion: e.data.paired.get(&slot).and_then(|l| l.iter().find(|(n, _)| *n == name)).map(|(_, m)| m.clone()), blend, serial },
                    P::Unhittable(on) => crate::sora::ReactionCmd::Unhittable(on),
                    P::Velocity { dir, speed } => crate::sora::ReactionCmd::Velocity(dir * speed),
                    P::QueueMotion { slot, blend } => crate::sora::ReactionCmd::Queue(slot, blend),
                    P::Charge { .. } => continue,
                });
            }
            if let Some(s) = obj.player_serial.take() {
                sora.reaction.serial = s;
            }
            obj.creates.clear();
            obj.remote.clear();
            // ---- carry out the requests
            if !matches!(e.state, EState::Hurt | EState::Blown) {
                e.state = EState::Idle;
            }
            if let Some(req) = obj.motion_req.take() {
                let looping = rig.mset.get(req.slot).and_then(|s| s.motion.as_ref()).is_some_and(|m| m.frame_return < m.frame_end);
                e.anim.restart(Some(req.slot), looping, req.blend);
                if let Some(s) = req.start {
                    e.anim.frame = s;
                    e.anim.prev_frame = s;
                }
            }
            let (root_t, root_yaw) = e.anim.root_delta;
            obj.motor.yaw = wrap_angle(obj.motor.yaw + root_yaw);
            let root_move = Quat::from_rotation_y(obj.motor.yaw - e.anim.root.yaw) * root_t;
            let was_air = !obj.motor.grounded;
            let step = match obj.dash.as_mut() {
                Some((v, k)) => {
                    *v = if obj.move_vel != Vec3::ZERO { *v * *k + obj.move_vel * (1.0 - *k) } else { *v * *k };
                    *v
                }
                None => obj.move_vel,
            };
            let mut delta = step + root_move.with_y(0.0);
            if was_air {
                delta += obj.air_vel;
            }
            obj.move_vel = Vec3::ZERO;
            let before = obj.motor.pos;
            let dy = was_air.then(|| obj.motor.jump_dy(fall_max) + root_move.y);
            let mut asked = delta;
            asked.y = dy.unwrap_or(root_move.y - kh2_game_core::motor::G);
            let landed = obj.motor.move_by(world, delta, dy);
            if !was_air && obj.motor.grounded {
                obj.motor.pos.y += e.anim.root.translation.y.max(0.0);
            }
            obj.want_vel = asked;
            obj.moved = obj.motor.pos - before;
            let mut follow = None;
            if landed {
                obj.air_vel = Vec3::ZERO;
                obj.fall_pending = false;
                follow = (obj.land_motion >= 0).then_some(obj.land_motion);
            } else if !obj.motor.grounded && obj.fall_pending && !obj.motor.jump.rising() {
                obj.fall_pending = false;
                follow = (obj.fall_motion >= 0).then_some(obj.fall_motion);
            }
            if let Some(m) = follow
                && rig.mset.has(m as usize)
            {
                let looping = rig.mset.get(m as usize).and_then(|s| s.motion.as_ref()).is_some_and(|x| x.frame_return < x.frame_end);
                e.anim.restart(Some(m as usize), looping, 4.0);
                obj.slot = Some(m as usize);
            }
            e.in_air = !obj.motor.grounded;
            if !obj.motor.grounded && obj.motor.pos.y < player.y - FALL_OUT_UNITS {
                obj.hidden = true;
            }
            e.last_frame = e.anim.frame;
            for id in obj.effects.drain(..) {
                self.events.push(EnemyEvent::Effect { id: e.id, effect: id });
            }
            obj.advance_fade();
            obj.appear.tick(1.0);
            if obj.hidden {
                e.removed = true;
                self.events.push(EnemyEvent::Removed { id: e.id });
            }
            e.anim.step(true);
        }
        self.list.retain(|e| !e.removed);
    }

    /// kh2-rs `enemy::think` for the hit reaction states (the script does the rest).
    fn think(e: &mut Enemy, reaction: Option<&crate::data::Reaction>, world: &World, fall_max: f32) {
        e.timer += 1.0;
        let mut vel = Vec3::ZERO;
        let mut dy = None;
        match e.state {
            EState::Idle => {}
            EState::Hurt => {
                if e.anim.finished || e.timer > 72.0 {
                    e.state = EState::Idle;
                    e.timer = 0.0;
                }
            }
            EState::Blown => {
                if let (Some(b), Some(r)) = (e.blow.as_mut(), reaction) {
                    let (h, vy) = b.step(&r.defaults, &r.consts);
                    vel = b.dir * h;
                    dy = Some(vy);
                }
                if e.obj.motor.grounded && e.timer > 15.0 {
                    e.blow = None;
                    e.state = EState::Hurt;
                    e.timer = 0.0;
                    let rig = e.data.rig.clone();
                    let has = |s: usize| rig.mset.slot(s, kh2_anim::PlayerState::BattleWeapon).or_else(|| rig.mset.has(s).then_some(s));
                    e.anim.restart(has(BLOW + if e.blown_back { 3 } else { 1 }).or(has(BLOW + 1)), false, 0.0);
                }
            }
        }
        if let (Some(p), Some(r)) = (e.push.as_mut(), reaction) {
            vel += p.step(&r.consts);
            if p.finished() {
                e.push = None;
            }
        }
        if vel != Vec3::ZERO || dy.is_some() {
            if dy.is_some() {
                e.obj.motor.grounded = false;
                if dy.is_some_and(|y| y < -fall_max) {
                    dy = Some(-fall_max);
                }
            }
            e.obj.motor.move_by(world, vel, dy);
        }
    }

    /// The hit tests (kh2-rs `combat.rs`) both ways, then the victims' damage and reactions.
    fn combat(&mut self, sora: &mut Sora) {
        let params = sora.game.params.clone();
        let Some(reaction) = params.reaction.clone() else { return };
        // Sora's live attacks (keyblades and body) against every living enemy's hurt volumes.
        let sora_attacks: Vec<(i16, i16, Volume)> = {
            let mut v = Vec::new();
            if let Some(w) = &sora.weapon {
                v.extend(live_attacks(w, sora.weapon_matrix()));
            }
            if let Some(w) = &sora.weapon2 {
                v.extend(live_attacks(w, sora.weapon2_matrix()));
            }
            v.extend(live_attacks(&sora.body, sora.body_matrix()));
            v
        };
        let sora_slot = sora.body.slot;
        self.sora_landed.retain(|k| k.1 == sora_slot);
        let from = Stats {
            level: sora.level,
            max_hp: sora.max_hp,
            strength: sora.strength + sora.loadout.as_ref().map_or(0, |l| kh2_game_core::abilities::berserk_attack(l.block(), sora.charge.active())),
            magic: sora.magic,
            defense: sora.defense,
            max_taken: i32::MAX,
            min_taken: 1,
            max_dealt: i32::MAX,
            min_dealt: 0,
            weakness: [100; 7],
            ..Default::default()
        };
        for e in self.list.iter_mut().filter(|e| e.alive() && !e.removed && e.data.kind.status && e.obj.flags & (1 << kh2_enemy_host::enemy_obj::FLAG_UNHITTABLE) == 0) {
            let hurt = hurt_volumes(&e.data.rig, &e.anim.pose, e.matrix());
            for &(atkp, start, v) in &sora_attacks {
                let key = (e.id, sora_slot, start);
                if self.sora_landed.contains(&key) || !hurt.iter().any(|h| overlap(&v, h)) {
                    continue;
                }
                let Some(atk) = params.atkp(atkp).cloned() else { continue };
                self.sora_landed.insert(key);
                sora.connected = true;
                sora.drive_hit(atkp);
                let mut dmg = damage(&atk, &from, &e.stats);
                if let Some(d) = sora.game.drive.as_ref() {
                    dmg = kh2_game_core::drive_rules::outgoing_damage(&d.tuning, if sora.drive.in_form { sora.drive.form } else { 0 }, atk.r#type, dmg);
                }
                if let Some(l) = &sora.loadout {
                    dmg = kh2_game_core::abilities::attacker_bonuses(l.block(), sora.hits, &atk, dmg);
                }
                let dir = (e.obj.motor.pos - sora.motor.pos).with_y(0.0).normalize_or_zero();
                Self::receive(e, &atk, dmg, dir, &reaction);
                self.events.push(EnemyEvent::Hit { id: e.id, atkp, damage: dmg, hp: e.hp });
                if e.hp <= 0 && !e.defeated {
                    e.defeated = true;
                    let (table, row) = self.data.experience(e.enmp, self.world_level);
                    let paid = sora.report_defeat(table, row);
                    self.events.push(EnemyEvent::Defeated { id: e.id, enmp: e.enmp, lvpm_experience: table, enmp_experience: row, paid });
                }
            }
        }
        // The enemies' live attacks against Sora's hurt volumes.
        let sora_hurt = if sora.pose { hurt_volumes(&sora.body.rig, &sora.body.pose, sora.body_matrix()) } else { Vec::new() };
        let sora_hurt = if sora_hurt.is_empty() { vec![Volume { a: sora.motor.pos + Vec3::Y * 35.0, b: sora.motor.pos + Vec3::Y * 115.0, r: 35.0 }] } else { sora_hurt };
        let to = Stats { level: sora.level, max_hp: sora.max_hp, strength: sora.strength, magic: sora.magic, defense: sora.defense, max_taken: i32::MAX, min_taken: 1, max_dealt: i32::MAX, min_dealt: 0, weakness: [100; 7], ..Default::default() };
        // Bit 3 of the player's flags (host 1.70 / 1.71): no hit reaches him (FUN_0019a4f0).
        let unhittable = sora.reaction.unhittable;
        for e in self.list.iter_mut().filter(|e| e.alive() && !e.removed && !unhittable) {
            let slot = e.anim.slot;
            e.landed.retain(|k| k.0 == slot);
            for (atkp, start, v) in live_attacks(&e.anim, e.matrix()) {
                if e.landed.contains(&(slot, start)) || !sora_hurt.iter().any(|h| overlap(&v, h)) {
                    continue;
                }
                e.landed.insert((slot, start));
                let Some(atk) = params.atkp(atkp) else { continue };
                let dmg = damage(atk, &e.stats, &to);
                let dir = (sora.motor.pos - e.obj.motor.pos).with_y(0.0).normalize_or_zero();
                let result = sora.hit(atkp, dmg, dir) as i32;
                self.events.push(EnemyEvent::HitSora { id: e.id, atkp, damage: dmg, result });
            }
        }
    }

    /// kh2-rs `enemy::receive_hits` for one hit: HP, then the victim reaction of the row.
    pub(crate) fn receive(e: &mut Enemy, atk: &Atkp, dmg: i32, dir: Vec3, reaction: &crate::data::Reaction) {
        let useless = rules::is_ineffective(e.stats.weakness[(atk.element as usize).min(6)], e.stats.weakness[6]);
        kh2_enemy_host::enemy_obj::note_hit_row(atk.id as i16, atk.enemy_reaction, atk.element);
        e.hp = (e.hp - dmg).max(e.min_hp);
        let facing = e.obj.motor.forward();
        let from_front = facing.dot(dir) < 0.0;
        let ctx = VictimCtx { status: 0, grounded: e.obj.motor.grounded, attacker_flag_4000: false, from_front, hit_part_plain: true };
        let out = rules::victim_outcome(&rules::HitRow::from_atkp(atk), useless, &ctx, &reaction.patn, &reaction.defaults, &reaction.consts);
        e.hits.push((rules::reaction_class(out.effective) >= 2, out.effective, atk.id as i16));
        let d = &reaction.defaults;
        let action = match out.action {
            x @ VictimAction::Blow { .. } => x,
            _ if e.hp == 0 && !useless => VictimAction::Blow { motion: rules::blow_motion(from_front), up: d.blow_up, horiz: d.blow_speed, face_attacker: from_front },
            x => x,
        };
        let away = dir.with_y(0.0).normalize_or_zero();
        let rig = e.data.rig.clone();
        let has = |s: usize| rig.mset.has(s).then_some(s);
        let first = |ids: &[usize]| ids.iter().find_map(|&i| has(i));
        match action {
            VictimAction::None | VictimAction::NotDecoded { .. } => {}
            VictimAction::Push { strength } => e.push = Push::new(away, strength, 0.0, &reaction.consts),
            VictimAction::Flinch { motion, fallback } | VictimAction::FlinchPush { motion, fallback, .. } => {
                if let VictimAction::FlinchPush { strength, .. } = action {
                    e.push = Push::new(away, strength, 0.0, &reaction.consts);
                }
                e.timer = 0.0;
                e.state = EState::Hurt;
                e.anim.restart(first(&[motion as usize, fallback as usize]), false, 0.0);
            }
            VictimAction::Class14 { motion, strength, face_attacker } => {
                let to = if face_attacker { -away } else { away };
                e.obj.motor.yaw = to.x.atan2(to.z);
                e.push = Push::new(away, strength, 0.0, &reaction.consts);
                e.timer = 0.0;
                e.state = EState::Hurt;
                e.anim.restart(first(&[motion as usize, HURT_FRONT]), false, 0.0);
            }
            VictimAction::Blow { motion, up, horiz, face_attacker } => {
                let to = if face_attacker { -away } else { away };
                e.obj.motor.yaw = to.x.atan2(to.z);
                e.blow = Some(Blow::new(away, up, horiz, d));
                e.push = None;
                e.blown_back = !from_front;
                e.obj.motor.grounded = false;
                e.timer = 0.0;
                e.state = EState::Blown;
                e.anim.restart(first(&[motion as usize, BLOW, HURT_FRONT]), false, 0.0);
            }
        }
        let _ = PLAYER;
    }

    /// Sora's spells after their trigger (`spell.rs`) against the enemies: damage with his magic and
    /// the hit's row variant, the victim reaction, defeats.
    fn spells_tick(&mut self, sora: &mut Sora, world: &World) {
        let params = sora.game.params.clone();
        for (slot, kind) in std::mem::take(&mut sora.fired) {
            let magi = rules_magi(&params, kind);
            self.spells.fire(slot, kind, magi);
        }
        if !self.spells.active() {
            return;
        }
        let victims: Vec<(u32, Vec<Volume>, Vec3, Vec3)> = self
            .list
            .iter()
            .filter(|e| e.alive() && !e.removed && e.data.kind.status)
            .map(|e| {
                let hurt = hurt_volumes(&e.data.rig, &e.anim.pose, e.matrix());
                let target = hurt.first().map_or(e.pos() + Vec3::Y * 80.0, |v| v.a);
                (e.id, hurt, target, e.pos())
            })
            .collect();
        let target = sora.target.map(|t| t.pos).or_else(|| victims.iter().min_by(|a, b| a.3.distance(sora.motor.pos).total_cmp(&b.3.distance(sora.motor.pos))).map(|v| v.2));
        let caster = crate::spell::Caster { pos: sora.motor.pos, facing: sora.motor.forward(), target, grounded: sora.motor.grounded };
        let (hits, push) = self.spells.tick(&caster, &params.atkp, world, &victims);
        if let (Some((dir, power)), Some(r)) = (push, params.reaction.as_ref()) {
            sora.push = Push::new(dir, power, 0.0, &r.consts);
        }
        let Some(reaction) = params.reaction.clone() else { return };
        let from = Stats { level: sora.level, max_hp: sora.max_hp, strength: sora.strength, magic: sora.magic, defense: sora.defense, max_taken: i32::MAX, min_taken: 1, max_dealt: i32::MAX, min_dealt: 0, weakness: [100; 7], ..Default::default() };
        for h in hits {
            let id = victims[h.victim].0;
            let Some(e) = self.list.iter_mut().find(|e| e.id == id) else { continue };
            let atk = params.atkp[h.row].clone();
            let mut dmg = damage(&atk, &from, &e.stats);
            if let Some(l) = &sora.loadout {
                dmg = kh2_game_core::abilities::attacker_bonuses(l.block(), sora.hits, &atk, dmg);
            }
            Self::receive(e, &atk, dmg, h.dir, &reaction);
            self.events.push(EnemyEvent::Hit { id, atkp: atk.id as i16, damage: dmg, hp: e.hp });
            if e.hp <= 0 && !e.defeated {
                e.defeated = true;
                let (table, row) = self.data.experience(e.enmp, self.world_level);
                let paid = sora.report_defeat(table, row);
                self.events.push(EnemyEvent::Defeated { id, enmp: e.enmp, lvpm_experience: table, enmp_experience: row, paid });
            }
        }
    }

    /// kh2-rs `reaction_cmd::offer` for the enemies: the range trigger 20 offers of their playing
    /// motions (and the harness offer), each row's status for Sora; the last one found holds slot 1.
    fn offer(&mut self, sora: &Sora) {
        self.offer = None;
        if sora.hp <= 0 || matches!(sora.state, crate::sora::State::Down) {
            return;
        }
        // decided: the form field is the drive form while one is on (kh2-rs `DriveState::object_form`).
        let form = if sora.drive.in_form { sora.drive.form } else { 0 };
        let ctx = Ctx { form, battle: !self.list.is_empty(), mp: sora.mp, ..Default::default() };
        // The native offers (FUN_0019cad0 -> FUN_0019cb70): the parts touching Sora's body volume,
        // best by priority then distance; the player's window is range trigger 26 of his motion.
        let rules = kh2_game_core::reaction_cmd::ReactionRules { cmds: self.data.cmds.clone() };
        let window = sora.body.triggers().and_then(|tg| kh2_game_core::reaction_cmd::window_at(tg, sora.body.gframe()));
        let player_shape = self.data.player_parts.as_ref().and_then(|t| t.first(kh2_game_core::reaction_parts::TYPE_BODY)).and_then(|r| kh2_game_core::reaction_parts::shape_of(r, sora.motor.pos, sora.motor.yaw));
        let mut best: Option<(u8, f32)> = None;
        for e in self.list.iter().filter(|e| !e.removed) {
            if let (Some(table), Some(player)) = (e.data.parts.as_ref(), player_shape.as_ref())
                && e.alive()
            {
                let (mut on, mut off) = (Vec::new(), Vec::new());
                if let Some(tg) = e.anim.triggers() {
                    for r in tg.active(e.anim.gframe()) {
                        match (r.id, r.params.first()) {
                            (4, Some(&p)) => on.push(p),
                            (5, Some(&p)) => off.push(p),
                            _ => {}
                        }
                    }
                }
                if let Some(o) = kh2_game_core::reaction_cmd::part_offer(&rules, table, u16::MAX, (&on, &off), (e.pos(), e.obj.motor.yaw), player, (sora.motor.pos, sora.motor.yaw), (&ctx, window))
                    && !best.is_some_and(|(p, d)| o.priority < p || (o.priority == p && d <= o.dist))
                {
                    best = Some((o.priority, o.dist));
                    let message = rules.cmd(o.cmd).map_or(0, |c| c.message_id as u32);
                    self.offer = Some(ReactionOffer { cmd: o.cmd, pos: o.part as i16, from: e.id, status: o.status, message });
                }
            }
            let mut found = e.anim.triggers().map(|tg| offers_at(tg, e.anim.gframe())).unwrap_or_default();
            if let Some(c) = self.fake_offer.filter(|_| e.alive()) {
                found.push(Offer { cmd: c, pos: 0 });
            }
            for o in found {
                let Some(c) = self.data.cmds.iter().find(|c| c.id == o.cmd) else { continue };
                let d = (e.pos() - sora.motor.pos).with_y(0.0).length();
                let status = status(&self.data.cmds, c, &Ctx { distance: d, ..ctx });
                self.offer = Some(ReactionOffer { cmd: o.cmd, pos: o.pos, from: e.id, status, message: c.message_id as u32 });
            }
        }
    }

    /// kh2-rs `reaction_cmd::accept` (FUN_00177c20's generic path): refused unless available; bump
    /// the use counter; the target must still exist; CmdAction 1 / 2 stop Sora; the command goes to
    /// the target's script as event 11 on its next tick. Returns the command, or None when refused.
    /// decided: limit commands (Execute 81) and Mickey's class handler are not taken (refused here).
    pub fn accept(&mut self, sora: &mut Sora) -> Option<u16> {
        let a = self.offer?;
        if a.status != Status::Available {
            return None;
        }
        let c = self.data.cmds.iter().find(|c| c.id == a.cmd)?;
        if c.execute == kh2_game_core::reaction_cmd::EXECUTE_LIMIT {
            return None;
        }
        if c.reaction_count != 0 {
            let n = self.uses.entry(c.reaction_count).or_insert(0);
            *n = (*n + 1).min(999);
        }
        if !self.list.iter().any(|e| e.id == a.from && !e.removed) {
            return None;
        }
        if matches!(c.cmd_action, 1 | 2) {
            sora.motor.vel = Vec3::ZERO;
        }
        self.reactions.push((a.from, a.cmd, a.pos));
        Some(a.cmd)
    }

    /// The enemy nearest to `p` and alive (for lock-on and the host).
    pub fn nearest(&self, p: Vec3) -> Option<&Enemy> {
        self.list.iter().filter(|e| e.alive()).min_by(|a, b| a.pos().distance(p).total_cmp(&b.pos().distance(p)))
    }

    /// The host calls the scripts made that the host does not serve (group, index, count).
    pub fn unserved_calls(&self) -> Vec<(u16, u16, u64)> {
        self.trace.missing()
    }
}

/// kh2-rs `enemy_ai::motion_events`: frame triggers of type 7 the last tick crossed.
fn motion_events(anim: &Anim) -> Vec<(u16, u16)> {
    let (Some(t), Some(m)) = (anim.triggers(), anim.motion()) else { return Vec::new() };
    if anim.frame == anim.prev_frame {
        return Vec::new();
    }
    let g = |f: f32| f * crate::anim::LOGIC_HZ / m.fps.max(1.0);
    t.crossed(g(anim.prev_frame), g(anim.frame)).filter(|f| f.id == 7).map(|f| (f.params.first().copied().unwrap_or(0) as u16, f.params.get(1).copied().unwrap_or(0) as u16)).collect()
}

/// kh2-rs `enemy_ai::trigger_bits`.
fn trigger_bits(anim: &Anim) -> [u8; 2] {
    let (Some(t), Some(m)) = (anim.triggers(), anim.motion()) else { return [0; 2] };
    let g = |f: f32| f * crate::anim::LOGIC_HZ / m.fps.max(1.0);
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

/// The `magi` row of a cast kind (kh2-rs `magic_rules::magi_row`).
fn rules_magi(p: &crate::data::Params, kind: u32) -> kh2_game_core::magic_rules::MagiRow {
    kh2_game_core::magic_rules::magi_row(&p.magi, kind).cloned().unwrap_or_default()
}
