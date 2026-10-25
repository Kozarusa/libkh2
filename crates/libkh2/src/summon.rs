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
}

impl Disc {
    pub fn summons(&mut self) -> Result<SummonData, LoadError> {
        let rows = kh2_tables::load::<t::Sumn>(&mut self.0, &t::SUMN).map_err(|e| LoadError(format!("sumn: {e}")))?.rows;
        let cmds = kh2_tables::load::<t::Cmd>(&mut self.0, &t::CMD).map_err(|e| LoadError(format!("cmd: {e}")))?.rows;
        let objs = kh2_tables::load::<t::Objentry>(&mut self.0, &t::OBJENTRY).map_err(|e| LoadError(format!("objentry: {e}")))?.rows;
        let sstm = kh2_tables::player::load_sstm(&mut self.0).map_err(|e| LoadError(format!("sstm: {e}")))?;
        let d = crate::drive::DriveData::load(self)?;
        let exe = self.exe();
        let atkp = kh2_enemy_host::data::GameData { atkp: kh2_tables::load::<t::Atkp>(&mut self.0, &t::ATKP).map(|d| d.rows).unwrap_or_default() };
        Ok(SummonData {
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
        self.arrival = Some(data.tuning.wait);
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
    pub fn tick(&mut self, data: &SummonData, disc: &mut Disc, sora: &mut Sora, world: &kh2_game_core::world::World, battle: bool) {
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
                        self.object = Some(SummonObject { model: obj.model_name.clone(), body: Anim::new(rig), brain, trace: Default::default(), pos, yaw: sora.motor.yaw + std::f32::consts::PI, neo: obj.neo_status, pending: Some("appear_skip".into()), mode: None });
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
                if let Some(slot) = out.slot {
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
