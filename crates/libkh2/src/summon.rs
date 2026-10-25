//! Summons: Stitch, Genie, Chicken Little, Peter Pan.
//!
//! A transcription of kh2-rs `kh2-party/src/summon.rs`'s `tick` flow (the command test, the start:
//! motion 0x8a, the cost in drive points, the companions away, the arrival wait `sstm` 0x88; the
//! arrival: gauge mode 3 with `drive_rules::summon_timer`, the object of the `sumn` row's Entity1 at
//! Sora's side, its script in `appear_skip`, Sora's motion 0x82; mode 3 each tick: the drain
//! (`drain_per_tick` times Summon Boost), the segments into the summon experience
//! (`SummonExp::add`), landed hits' `DriveDrain` (`summon_gauge_change`); the end at timer 0 or
//! Dismiss: the object's script in `leave`, the companions back once it is gone). The rules are
//! kh2-rs's `drive_rules` (compiled from source). The summon object's script runs through the party
//! members' host (`friend_host`), its motion requests play on its body.
//!
//! decided: no arrival event (Summon Effects off: `appear_skip`); the companions are away at once
//! (no fade); the object appears 250 cm in front of Sora (no free-spot search); the summon classes'
//! own AI (Stitch's moves and blast, Genie's follow and records, Chicken Little's / Peter Pan's
//! classes, kh2-rs `stitch` / `genie` / `scripted`) is not ported: the object stands and plays what
//! its script asks; the script's group-10 calls of the summon host (`SummonHost`) are answered by the
//! members' host (counted as unserved where it has none); summon limits not ported; Genie's Drive!
//! not ported.

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use kh2_game_core::drive_rules::{self as rules, FormTimer, GaugeMode, SummonExp, SummonTuning};
use kh2_tables::tables as t;

use crate::anim::{Anim, Rig};
use crate::data::{Disc, LoadError};
use crate::friend::FriendBrain;
use crate::sora::Sora;

/// kh2-rs `summon::cmd::DISMISS` and the motion requests.
pub const DISMISS: u16 = 0x13c;
const MOTION_SUMMON: usize = 0x8a;
const MOTION_APPEAR: usize = 0x82;

/// The summon tables.
pub struct SummonData {
    pub rows: Vec<t::Sumn>,
    pub cmds: Vec<t::Cmd>,
    pub tuning: SummonTuning,
    pub drive: rules::Tuning,
    pub levels: kh2_tables::player::FormLevels,
    pub objs: Vec<t::Objentry>,
    pub table: Option<kh2_bdx::HostTable>,
    pub free: Option<kh2_game_core::party_rules::FreePointConsts>,
    pub atkp: kh2_enemy_host::data::GameData,
    /// Stitch's and Genie's classes (kh2-rs `stitch` / `genie`, compiled from summon.rs): constants
    /// and records from the executable; the members' follow rules Genie runs; FUN_0017da30's orders.
    pub stitch: Option<(stitch::Consts, Vec<kh2_game_core::party_battle::Record>)>,
    pub genie: Option<(genie::Consts, Vec<kh2_game_core::party_battle::Record>)>,
    pub follow: Option<kh2_game_core::party_rules::Consts>,
    pub orders: Option<[[u8; 4]; 4]>,
    /// `prty` rows and their offsets (walk, run, turn of a NeoMoveset).
    pub prty: Vec<t::PrefPrty>,
    pub prty_offsets: Vec<u32>,
}

use kh2_enemy_host::summon::{genie, stitch};

impl Disc {
    pub fn summons(&mut self) -> Result<SummonData, LoadError> {
        let rows = kh2_tables::load::<t::Sumn>(&mut self.0, &t::SUMN).map_err(|e| LoadError(format!("sumn: {e}")))?.rows;
        let cmds = kh2_tables::load::<t::Cmd>(&mut self.0, &t::CMD).map_err(|e| LoadError(format!("cmd: {e}")))?.rows;
        let objs = kh2_tables::load::<t::Objentry>(&mut self.0, &t::OBJENTRY).map_err(|e| LoadError(format!("objentry: {e}")))?.rows;
        let sstm = kh2_tables::player::load_sstm(&mut self.0).map_err(|e| LoadError(format!("sstm: {e}")))?;
        let d = crate::drive::DriveData::load(self)?;
        let exe = self.exe();
        let atkp = kh2_enemy_host::data::GameData { atkp: kh2_tables::load::<t::Atkp>(&mut self.0, &t::ATKP).map(|d| d.rows).unwrap_or_default() };
        let prty = kh2_tables::load::<t::PrefPrty>(&mut self.0, &t::PREF_PRTY).map(|d| d.rows).unwrap_or_default();
        let prty_offsets: Vec<u32> = self
            .read("03system.bin")
            .ok()
            .and_then(|sys| {
                let top = kh2_bar::parse(&sys).ok()?;
                let inner = kh2_bar::parse(kh2_bar::find(&top, "pref")?.data).ok()?;
                let b = kh2_bar::find(&inner, "prty")?.data;
                let n = u32::from_le_bytes(b.get(0..4)?.try_into().ok()?) as usize;
                (0..n).map(|i| b.get(4 + 4 * i..8 + 4 * i).map(|x| u32::from_le_bytes(x.try_into().unwrap()))).collect::<Option<Vec<u32>>>()
            })
            .unwrap_or_default();
        let e = exe.as_deref();
        Ok(SummonData {
            stitch: e.and_then(|e| Some((stitch::Consts::from_exe(e)?, stitch::records(e)?))),
            genie: e.and_then(|e| Some((genie::Consts::from_exe(e)?, genie::records(e)?))),
            follow: e.and_then(kh2_game_core::party_rules::Consts::from_exe),
            orders: e.and_then(kh2_game_core::party_battle::motion_orders),
            prty,
            prty_offsets,
            rows,
            cmds,
            tuning: SummonTuning::from_sstm(&sstm),
            drive: d.tuning,
            levels: d.levels,
            objs,
            table: exe.as_deref().and_then(kh2_bdx::HostTable::from_exe),
            free: exe.as_deref().and_then(kh2_game_core::party_rules::FreePointConsts::from_exe),
            atkp,
        })
    }
}

impl SummonData {
    /// `FUN_001d4ca0`: the 1-based row index of a summon command.
    pub fn index_of(&self, c: u16) -> Option<u8> {
        self.rows.iter().position(|r| r.command == c).map(|i| i as u8 + 1)
    }
}

/// The summon object (kh2-rs `SummonObject`).
pub struct SummonObject {
    pub model: String,
    pub body: Anim,
    pub brain: Option<FriendBrain>,
    pub trace: kh2_enemy_host::enemy_ai::AiTrace,
    pub pos: Vec3,
    pub yaw: f32,
    pub neo: u16,
    /// The mode asked of the object's script on its next update.
    pub pending: Option<String>,
    /// The class AI's state (kh2-rs `ai::SummonAi`: Stitch's and Genie's parts), the object's walk /
    /// run / turn / radius, and the enemies the running blast hit.
    pub stitch: stitch::StitchAi,
    pub genie: genie::GenieAi,
    pub moves: (f32, f32, f32, f32),
    pub blast_hit: std::collections::HashSet<u32>,
    pub using: bool,
    /// The script's current mode.
    pub mode: Option<String>,
}

impl SummonObject {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(Quat::from_rotation_y(self.yaw), self.pos)
    }
}

/// Something the summon did.
#[derive(Clone, Debug, PartialEq)]
pub enum SummonEvent {
    Started { command: u16, index: u8, cost: u8 },
    /// The object is out (gauge mode 3): the timer and its segments.
    Arrived { model: String, timer: f32, segments: u8 },
    /// The script switched mode (`appear_skip`, `idle`, `leave` ...).
    Mode(String),
    /// The class AI started record `index` (Stitch: stand, move, blast, refill, link ...; Genie:
    /// follow, potion).
    Record { index: usize },
    /// Stitch's blast hit enemy `id`; his MP refill; his heal of Sora; Genie's potion on Sora.
    Blast { id: u32 },
    Refill,
    Heal { hp: i32 },
    Potion,
    LevelUp(u8),
    /// The end: timer or dismiss; then the object is gone and the companions are back.
    Leaving { dismissed: bool },
    Gone,
}

/// Why a summon command is refused (kh2-rs `SummonState::command_ok` and the menu's tests).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SummonRefusal {
    NotASummon,
    CharmNotOwned,
    Busy,
    PartyMissing,
    InForm,
    NotEnoughPoints,
    Airborne,
}

/// The summon state (kh2-rs `SummonState`).
#[derive(Default)]
pub struct Summons {
    pub index: u8,
    /// `DAT_01d5e368`: 0 none, 1 an arrival event, 2 out, 3 leaving.
    pub state: u8,
    pub sequence: bool,
    pub arrival: Option<f32>,
    pub leaving: bool,
    pub gauge: GaugeMode,
    pub timer: FormTimer,
    pub segments: u8,
    pub exp: SummonExp,
    /// The charms the host says are owned (by `sumn` row item).
    pub owned: Vec<u16>,
    pub object: Option<SummonObject>,
    pub events: Vec<SummonEvent>,
    /// The companions are away while the summon runs.
    pub companions_away: bool,
    /// The Summon Effects option: the arrival plays as an event (`appear`, `sstm` 0x84 wait, state 1).
    pub arrival_event: bool,
    /// Genie's byte 7 (a random owned form, FUN_001c6a88) and Peter Pan's.
    pub extra: u8,
}

impl Summons {
    fn cap(&self, data: &SummonData) -> u8 {
        rules::summon_level_cap(data.rows.iter().filter(|r| self.owned.contains(&r.item)).count() as u32)
    }

    /// The summon commands the menu lists (owned charms) with their status.
    pub fn status(&self, data: &SummonData, sora: &Sora, party_ok: bool) -> Vec<(u16, Result<(), SummonRefusal>)> {
        data.rows.iter().map(|r| (r.command, self.check(data, sora, party_ok, r.command))).collect()
    }

    fn check(&self, data: &SummonData, sora: &Sora, party_ok: bool, c: u16) -> Result<(), SummonRefusal> {
        let row = data.rows.iter().find(|r| r.command == c).ok_or(SummonRefusal::NotASummon)?;
        let cmd = data.cmds.iter().find(|x| x.id == c).ok_or(SummonRefusal::NotASummon)?;
        if !self.owned.contains(&row.item) {
            return Err(SummonRefusal::CharmNotOwned);
        }
        if self.sequence || self.arrival.is_some() || self.state != 0 {
            return Err(SummonRefusal::Busy);
        }
        if !party_ok {
            return Err(SummonRefusal::PartyMissing);
        }
        if sora.drive.in_form {
            return Err(SummonRefusal::InForm);
        }
        if cmd.flags >> 1 & 1 != 0 && !sora.motor.grounded {
            return Err(SummonRefusal::Airborne);
        }
        if (sora.drive.points.whole as u32) < cmd.cost as u32 {
            return Err(SummonRefusal::NotEnoughPoints);
        }
        Ok(())
    }

    /// The Summon command (Execute 72): motion 0x8a, the cost, the arrival wait.
    pub fn summon(&mut self, data: &SummonData, sora: &mut Sora, party_ok: bool, c: u16) -> Result<(), SummonRefusal> {
        self.check(data, sora, party_ok, c)?;
        let cmd = data.cmds.iter().find(|x| x.id == c).ok_or(SummonRefusal::NotASummon)?;
        let slot = sora.body.rig.mset.slot(MOTION_SUMMON, kh2_anim::PlayerState::BattleWeapon);
        sora.body.restart(slot, false, 8.0);
        sora.drive.points.spend(cmd.cost);
        self.index = data.index_of(c).unwrap_or(0);
        self.sequence = true;
        self.state = 2;
        self.arrival = Some(if self.arrival_event { data.tuning.wait_event } else { data.tuning.wait });
        if self.arrival_event {
            self.state = 1;
        }
        // Peter Pan's byte 7 is 1 (kh2-rs `summon::tick`); Genie's is a random owned form, here form 1
        // (decided: the forms the player owns are the host's).
        self.extra = if c == kh2_enemy_host::summon::cmd::PETER_PAN || c == kh2_enemy_host::summon::cmd::GENIE { 1 } else { 0 };
        self.companions_away = true;
        self.events.push(SummonEvent::Started { command: c, index: self.index, cost: cmd.cost });
        Ok(())
    }

    /// Dismiss (Execute 0x13c): accepted while the summon is out.
    pub fn dismiss(&mut self) -> bool {
        if self.state != 2 || self.leaving || self.object.is_none() {
            return false;
        }
        self.leave(true);
        true
    }

    /// kh2-rs `summon::leave`: state 3, the object's script in `leave`, gauge mode 1.
    fn leave(&mut self, dismissed: bool) {
        self.state = 3;
        self.leaving = true;
        self.gauge = GaugeMode::Normal;
        if let Some(o) = self.object.as_mut() {
            o.pending = Some("leave".into());
        }
        self.events.push(SummonEvent::Leaving { dismissed });
    }

    /// A hit Sora landed (its `atkp` DriveDrain) while the timer runs (FUN_00178748 -> FUN_001788d8).
    pub fn landed(&mut self, data: &SummonData, drive_drain: i16) {
        if self.gauge != GaugeMode::Summon || drive_drain == 0 || self.timer.left == 0.0 {
            return;
        }
        let gain = rules::summon_gauge_change(&data.tuning, f32::from(drive_drain));
        let lost = self.timer.change(&data.drive, GaugeMode::Summon, 0, gain);
        self.add_exp(data, lost);
    }

    fn add_exp(&mut self, data: &SummonData, segments: i32) {
        let cap = self.cap(data);
        for l in self.exp.add(&data.levels, cap, segments, false) {
            self.events.push(SummonEvent::LevelUp(l));
        }
    }

    /// One tick (kh2-rs `summon::tick` and the object's script).
    pub fn tick(&mut self, data: &SummonData, disc: &mut Disc, sora: &mut Sora, world: &kh2_game_core::world::World, battle: bool, enemies: Option<&mut crate::enemy::Enemies>) {
        // The arrival thread: the wait, then gauge mode 3 and the object.
        if let Some(w) = self.arrival.as_mut() {
            *w -= rules::FRAME_UNIT;
            if *w <= 0.0 {
                self.arrival = None;
                let (timer, segments) = rules::summon_timer(&data.drive, self.exp.level);
                self.timer = timer;
                self.segments = segments;
                self.gauge = GaugeMode::Summon;
                let slot = sora.body.rig.mset.slot(MOTION_APPEAR, kh2_anim::PlayerState::BattleWeapon);
                sora.body.restart(slot, false, 0.0);
                if let Some(row) = data.rows.get(self.index as usize - 1)
                    && let Some(obj) = data.objs.iter().find(|o| o.object_id == row.entity1)
                {
                    let mset = obj.animation_name.trim_end_matches(".mset").to_string();
                    let rig: Option<Arc<Rig>> = (!mset.is_empty()).then(|| disc.rig_named(&obj.model_name, &mset).ok().map(Arc::new)).flatten();
                    let prog = disc.read(&format!("obj/{}.mdlx", obj.model_name)).ok().and_then(|f| kh2_bar::parse(&f).ok()?.into_iter().find(|e| e.kind == 3).and_then(|e| kh2_bdx::Program::parse(e.data).ok()));
                    let pos = sora.motor.pos + sora.motor.forward() * 250.0;
                    if let Some(rig) = rig {
                        let brain = prog.map(|p| FriendBrain::new(p, obj.flags & 8 != 0, data.table.as_ref(), 8, &obj.model_name));
                        let p = kh2_game_core::party_battle::pref_row(&data.prty_offsets, obj.neo_moveset as usize, 1).and_then(|k| data.prty.get(k));
                        let radius = rig.model.shapes.iter().find(|s| s.kind == 0 || s.kind == 4).map_or(35.0, |s| s.radius.abs());
                        let moves = (p.map_or(0.0, |p| p.walk_speed), p.map_or(0.0, |p| p.run_speed), p.map_or(0.0, |p| p.turn_speed), radius);
                        let mode = if self.arrival_event { "appear" } else { "appear_skip" };
                        self.object = Some(SummonObject {
                            model: obj.model_name.clone(),
                            body: Anim::new(rig),
                            brain,
                            trace: Default::default(),
                            pos,
                            yaw: sora.motor.yaw + std::f32::consts::PI,
                            neo: obj.neo_status,
                            pending: Some(mode.into()),
                            mode: None,
                            stitch: Default::default(),
                            genie: Default::default(),
                            moves,
                            blast_hit: Default::default(),
                            using: false,
                        });
                        self.events.push(SummonEvent::Arrived { model: obj.model_name.clone(), timer: timer.left, segments });
                    }
                }
                self.state = 2;
            }
        }
        // Mode 3: the drain; the timer at 0 ends the summon.
        if self.gauge == GaugeMode::Summon {
            let factor = sora.loadout.as_ref().map_or(1.0, |l| l.block().summon_drain);
            let drain = rules::drain_per_tick(&data.drive, battle, factor);
            let lost = self.timer.change(&data.drive, GaugeMode::Summon, 0, -drain);
            self.add_exp(data, lost);
            if self.timer.left <= 0.0 && self.state == 2 && !self.leaving {
                self.leave(false);
            }
        }
        // The object's script (the members' host) and its motions.
        let mut gone = false;
        if let Some(o) = self.object.as_mut() {
            if let Some(b) = o.brain.as_mut() {
                let sw = crate::friend::ScriptWorld { player: sora.motor.pos, player_yaw: sora.motor.yaw, world, in_battle: battle, victims: &[], targets: &[], atkp: &data.atkp, free: data.free.as_ref() };
                let want = o.pending.take().map(|m| (m, 0));
                let out = crate::friend::update(b, &o.body, o.pos, o.yaw, [2, 3, 1, 0], None, want, &mut o.trace, &sw);
                // Stitch's class drives his body (the script's own requests are not applied, kh2-rs).
                if let Some(slot) = out.slot.filter(|_| o.neo != stitch::NEO) {
                    let looping = o.body.rig.mset.get(slot).and_then(|s| s.motion.as_ref()).is_some_and(|m| m.frame_return < m.frame_end);
                    o.body.restart(Some(slot), looping, 4.0);
                }
                if let Some(v) = out.moved {
                    o.pos += v;
                }
                if let Some(y) = out.yaw {
                    o.yaw = y;
                }
                if out.mode.is_some() && out.mode != o.mode {
                    if let Some(m) = &out.mode {
                        self.events.push(SummonEvent::Mode(m.clone()));
                    }
                    o.mode = out.mode;
                }
                // The script hides the object at its leave's end (host 1.63 / the hidden bit).
                gone = b.obj.hidden;
            }
            if o.mode.as_deref() != Some("leave") {
                class_tick(data, o, sora, battle, enemies, &mut self.events, self.extra);
            }
            o.body.step(true);
            if self.leaving && (gone || o.brain.is_none() || o.body.finished) {
                gone = true;
            }
        }
        if gone && self.leaving {
            self.object = None;
        }
        if self.leaving && self.object.is_none() {
            self.leaving = false;
            self.sequence = false;
            self.state = 0;
            self.companions_away = false;
            self.events.push(SummonEvent::Gone);
        }
    }
}

/// The slot a class motion request plays (kh2-rs: the number when the set has it, else FUN_0017da30's order).
fn class_slot(data: &SummonData, rig: &Rig, m: i32, battle: bool) -> Option<usize> {
    if m >= 0 && rig.mset.has(m as usize) {
        return Some(m as usize);
    }
    let order = data.orders?[kh2_game_core::party_battle::motion_state(battle, false)];
    kh2_game_core::party_battle::remap(m, order, |x| rig.mset.has(x))
}

/// kh2-rs `summon::objects`'s class part for Stitch (FUN_001155e0 and its records) and Genie (the
/// members' update with his two records). decided: Stitch's camera-space plane is placed in the
/// frame of a camera behind Sora (450 cm back, 250 cm up, looking at him; the host's real camera is
/// not known to the library); his blast (atkp 0x4ba, FUN_00118110) hits every living enemy once per
/// attack object with Sora's status as the attacker's.
fn class_tick(data: &SummonData, o: &mut SummonObject, sora: &mut Sora, battle: bool, mut enemies: Option<&mut crate::enemy::Enemies>, events: &mut Vec<SummonEvent>, extra: u8) {
    let motion_done = o.body.finished;
    if o.neo == stitch::NEO
        && let Some((k, recs)) = data.stitch.as_ref()
    {
        let in_view = enemies.as_ref().map_or(0, |e| e.list.iter().filter(|x| x.alive() && !x.removed).count());
        let now = stitch::Now { battle, limit: false, hp: (sora.hp, sora.max_hp), mp: (sora.mp, sora.max_mp), in_view, motion_done };
        let st = &mut o.stitch;
        let out = stitch::tick(k, recs, st, &now);
        if let Some((i, _)) = out.started {
            events.push(SummonEvent::Record { index: i });
        }
        if out.heal {
            sora.hp = kh2_game_core::magic_rules::healed(sora.hp, sora.max_hp, 1);
            events.push(SummonEvent::Heal { hp: sora.hp });
        }
        if out.refill {
            sora.mp = sora.max_mp;
            sora.charge = Default::default();
            events.push(SummonEvent::Refill);
        }
        if st.attack.is_none() {
            o.blast_hit.clear();
        }
        if out.blast
            && let Some(en) = enemies.as_mut()
            && let Some(atk) = sora.game.params.atkp(stitch::BLAST_ATKP).cloned()
        {
            let from = kh2_enemy_host::data::Stats { level: sora.level, max_hp: sora.max_hp, strength: sora.strength, magic: sora.magic, defense: sora.defense, max_taken: i32::MAX, min_taken: 1, max_dealt: i32::MAX, weakness: [100; 7], ..Default::default() };
            let cam = sora.motor.pos - sora.motor.forward() * 450.0 + Vec3::Y * 250.0;
            for e in en.list.iter_mut().filter(|e| e.alive() && !e.removed && e.data.kind.status) {
                if !o.blast_hit.insert(e.id) {
                    continue;
                }
                let dmg = crate::enemy::damage(&atk, &from, &e.stats);
                let dir = (e.pos() - cam).with_y(0.0).normalize_or_zero();
                if let Some(r) = sora.game.params.reaction.as_ref() {
                    crate::enemy::Enemies::receive(e, &atk, dmg, dir, r);
                }
                en.events.push(crate::enemy::EnemyEvent::Hit { id: e.id, atkp: atk.id as i16, damage: dmg, hp: e.hp });
                events.push(SummonEvent::Blast { id: e.id });
            }
        }
        if std::mem::take(&mut st.motion_new)
            && let Some(m) = st.motion
        {
            let rig = o.body.rig.clone();
            if let Some(slot) = class_slot(data, &rig, m as i32, battle) {
                let looping = rig.mset.get(slot).and_then(|x| x.motion.as_ref()).is_some_and(|mo| mo.frame_return < mo.frame_end);
                o.body.play(Some(slot), looping, 0.0);
            }
        }
        if st.parent {
            // FUN_00117320 in the camera's frame (game camera axes x right, y down, z forward).
            let p = stitch::camera_point(k, st.pos);
            let fwd = sora.motor.forward();
            let cam = sora.motor.pos - fwd * 450.0 + Vec3::Y * 250.0;
            let look = (sora.motor.pos + Vec3::Y * 100.0 - cam).normalize_or_zero();
            let right = look.cross(Vec3::Y).normalize_or_zero();
            let up = right.cross(look);
            o.pos = cam + right * p[0] - up * p[1] + look * p[2];
            o.yaw = sora.motor.yaw + std::f32::consts::PI - st.pos[3];
        }
        return;
    }
    if o.neo == genie::NEO
        && let (Some((k, recs)), Some(fc)) = (data.genie.as_ref(), data.follow.as_ref())
    {
        use kh2_game_core::party_rules as pr;
        let g = &mut o.genie;
        if !g.init {
            g.init = true;
            g.cmd = genie::start_command(k, extra);
            g.form = Some(genie::form(k, g.cmd));
        }
        g.form = None;
        if o.using && motion_done {
            o.using = false;
        }
        if o.using {
            return;
        }
        let now = genie::Now { battle, hp: (sora.hp, sora.max_hp) };
        let out = genie::decide(k, recs, g, &now);
        if let Some((i, _)) = out.started {
            events.push(SummonEvent::Record { index: i });
        }
        let (walk, run, turn, radius) = o.moves;
        let (pos, player) = (o.pos, sora.motor.pos);
        let facing = Vec3::new(o.yaw.sin(), 0.0, o.yaw.cos());
        let t = fc.thresholds(genie::CLASS);
        let to_player = player - pos;
        let ground_dir = to_player.with_y(0.0).normalize_or_zero();
        let (mut dir, mut speed) = (facing, 0.0);
        if out.follow {
            let d = pr::follow_distance(&t, radius, fc);
            let point = pr::follow_points(player, sora.motor.yaw, [Some((pos, d)), None], fc)[0].unwrap_or(player);
            let input = pr::FollowIn { to_point: point - pos, to_player: to_player.normalize_or_zero(), dist: to_player.length(), ground_dir, facing, walk, run };
            let row = if battle { pr::ROW_BATTLE_PLAYER } else { pr::ROW_FIELD };
            (dir, speed) = pr::follow_step_row(&mut g.follow, &input, &t, row, fc);
            speed = genie::follow_speed(k, speed, walk, sora.motor.vel.length());
        }
        let rig = o.body.rig.clone();
        if out.potion {
            g.follow.mode = 0;
            if let Some(slot) = class_slot(data, &rig, genie::M_USE as i32, true) {
                o.body.play(Some(slot), false, 0.0);
                o.using = true;
            }
            // decided: the potion's effect lands at once (the game waits for trigger 6 of motion 9).
            sora.apply_item_mult(genie::POTION_ITEM, 100);
            events.push(SummonEvent::Potion);
        }
        let (d, sp) = pr::steer(&mut g.follow, dir, speed, walk, run, fc);
        let sp = if out.potion { 0.0 } else { sp };
        let delta = d.with_y(0.0).normalize_or_zero() * sp;
        o.pos = (pos + delta).with_y(player.y);
        if delta != Vec3::ZERO {
            o.yaw = kh2_game_core::motor::wrap_angle(o.yaw + kh2_game_core::motor::wrap_angle(delta.x.atan2(delta.z) - o.yaw).clamp(-turn, turn));
        } else if g.follow.turning {
            o.yaw = kh2_game_core::motor::wrap_angle(o.yaw + kh2_game_core::motor::wrap_angle(ground_dir.x.atan2(ground_dir.z) - o.yaw).clamp(-turn, turn));
        }
        if !out.potion
            && let Some(want) = kh2_enemy_host::summon::locomotion_want(&k.loco, o.body.slot.map_or(0, |c| c as i32), o.body.finished, sp, walk, run)
            && let Some(sl) = class_slot(data, &rig, want, battle)
            && o.body.slot != Some(sl)
        {
            let looping = rig.mset.get(sl).and_then(|x| x.motion.as_ref()).is_some_and(|mo| mo.frame_return < mo.frame_end);
            o.body.play(Some(sl), looping, 8.0);
        }
    }
}
