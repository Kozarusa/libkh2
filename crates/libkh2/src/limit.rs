//! Limits: Sora's limit commands with Donald and Goofy.
//!
//! The rules are kh2-rs's, compiled from its source in `libkh2-enemy-host` (`limit.rs`: the `limt`
//! rows joined with their commands, FUN_001d1ce0's usable test, FUN_001b3e38's availability, the
//! task's start events, the hit count, the timer; `limit_script.rs`: the limit BAR's own script
//! thread `ScriptRun` with its host). This module transcribes kh2-rs's `try_limit` (the start: the
//! first usable row of the command, MP spent, the task with the command's `Time`, the script events
//! 22 / 23) and `run_limit_script` (the thread fed Sora's place each tick, the countdown, the tail).
//!
//! decided: the party the rules see is Sora plus the members who follow and are not knocked out;
//! the limit ability items count as learned (the new game's members hold their first limits); the
//! world is the one the host sets (`world`, 0 by default, so world-bound rows are refused). What the
//! script asks for comes back as events: the participants' motions of the limit's `LIMI` set, its
//! PAX elements and named effect events (the library does not play them on the bodies). Not ported:
//! the participants' own threads (the member scripts), the summon limits, the HUD.

use std::collections::HashMap;

use kh2_bdx::HostTable;
use kh2_enemy_host::limit::{self as rules, LimitBar, LimitHits, LimitReceiver, LimitRow, Party, Refusal};
use kh2_enemy_host::limit_script::{script_of, ScriptRun, Stop, TaskState};

use crate::data::{Disc, LoadError};
use crate::sora::Sora;

/// Ticks a script thread may keep running after the countdown ended (kh2-rs `TAIL_TICKS`).
const TAIL_TICKS: u32 = 30;

/// The limit tables.
pub struct LimitData {
    pub rows: Vec<LimitRow>,
    /// Execute 81 commands with no `limt` row: a running limit's sub-commands (event 23).
    pub sub_commands: Vec<u16>,
    pub table: Option<HostTable>,
    /// The limit BARs read so far, by file name.
    bars: std::sync::Mutex<HashMap<String, Option<Vec<u8>>>>,
}

impl Disc {
    pub fn limits(&mut self) -> Result<LimitData, LoadError> {
        use kh2_tables::tables as t;
        let limt = kh2_tables::load::<t::Limt>(&mut self.0, &t::LIMT).map_err(|e| LoadError(format!("limt: {e}")))?.rows;
        let cmds = kh2_tables::load::<t::Cmd>(&mut self.0, &t::CMD).map_err(|e| LoadError(format!("cmd: {e}")))?.rows;
        let items = kh2_tables::load::<t::Item>(&mut self.0, &t::ITEM).map_err(|e| LoadError(format!("item: {e}")))?.rows;
        let table = self.exe().and_then(|e| HostTable::from_exe(&e));
        Ok(LimitData { rows: rules::rows(&limt, &cmds, &items), sub_commands: rules::sub_commands(&limt, &cmds), table, bars: Default::default() })
    }
}

impl LimitData {
    fn bar(&self, disc: &mut Disc, file: &str) -> Option<Vec<u8>> {
        let mut bars = self.bars.lock().unwrap_or_else(|e| e.into_inner());
        bars.entry(file.to_string()).or_insert_with(|| disc.read(&format!("limit/us/{file}")).ok()).clone()
    }

    /// The limit commands in table order (each once).
    pub fn commands(&self) -> Vec<u16> {
        let mut v = Vec::new();
        for r in &self.rows {
            if !v.contains(&r.command) {
                v.push(r.command);
            }
        }
        v
    }
}

/// Something the running limit did.
#[derive(Clone, Debug, PartialEq)]
pub enum LimitEvent {
    Started { command: u16, file: String, time: u16, members: [u8; 3] },
    /// The countdown is over: the hit count (the gauge's record marker).
    Ended { command: u16, hits: u32 },
    /// The script started motion `id` of the limit's `LIMI` set on participant `who` (host 2.47).
    Motion { who: u32, id: usize, blend: f32 },
    /// The script started PAX element `id` on `who` (host 2.59).
    Effect { who: u32, id: u16 },
    /// The script started the named event of the limit file (host 2.50).
    Named(String),
    /// The script thread stopped (returned, or a fault: the limit goes on).
    ScriptStopped(String),
}

/// Sora's limit state: the task, the hit count, the script thread.
#[derive(Default)]
pub struct Limits {
    /// The limit task: command and ticks left (kh2-rs `LimitTask`).
    pub task: Option<(u16, u32)>,
    hits: LimitHits,
    frac: f32,
    thread: Option<LimitReceiver>,
    run: Option<ScriptRun>,
    ended_at: Option<u32>,
    /// The world id the rows test (0: only world-free rows).
    pub world: u16,
    /// Overrides the learned limit ability bits (None: every limit item learned).
    pub abilities: Option<u128>,
    pub events: Vec<LimitEvent>,
    /// The `LIMI` motion sets of the running limit's file (kh2-rs `limit_visual::sets_of`).
    pub sets: HashMap<usize, Vec<kh2_enemy_host::player_reaction::Paired>>,
    /// Events already looked at for motions to play.
    seen: usize,
    /// Sora is held in the reaction state by a limit motion; members held (by slot).
    pub sora_held: bool,
    /// kh2-rs `LimitRng` (FUN_001d25e0's generator for the Auto Limit pick).
    pub auto_seed: u32,
}

/// kh2-rs `limit_visual::sets_of`: the sets of the `LIMI` entry (kind 0x14) in the limit BAR.
pub fn sets_of(bar: &[u8]) -> HashMap<usize, Vec<kh2_enemy_host::player_reaction::Paired>> {
    let Ok(es) = kh2_bar::parse(bar) else { return HashMap::new() };
    es.iter().find(|e| e.kind == rules::KIND_PARTY_SET).map(|e| kh2_enemy_host::player_reaction::paired_entries(e.data)).unwrap_or_default()
}

/// kh2-rs `limit_visual::find`: the sub-motion of set `id` for a character's model byte.
pub fn find(sets: &HashMap<usize, Vec<kh2_enemy_host::player_reaction::Paired>>, id: usize, model: u8) -> Option<&kh2_enemy_host::player_reaction::Paired> {
    let name = kh2_enemy_host::player_reaction::sub_name(model);
    sets.get(&id)?.iter().find(|(n, _)| *n == name)
}

/// Party member ids of the followers who stand (kh2-rs's `Party::present`; 2 Donald, 3 Goofy).
pub fn present(followers: &[crate::party::Follower], data: &crate::party::PartyData) -> Vec<u8> {
    followers.iter().filter(|f| !f.ko).map(|f| data.members[f.member].neo as u8).collect()
}

impl Limits {
    fn party(&self, data: &LimitData, sora: &Sora, present: &[u8]) -> Party {
        let all = data.rows.iter().filter_map(|r| r.ability_bit).filter(|&b| b < 128).fold(0u128, |a, b| a | 1 << b);
        Party { present: present.to_vec(), abilities: self.abilities.unwrap_or(all), form: if sora.drive.in_form { sora.drive.form } else { 0 }, world: self.world, locked: false, mp: sora.mp, summon_gauge: false, summons: Vec::new(), busy: self.task.is_some() }
    }

    /// FUN_001b3e38 for each limit command: Ok, or why it is refused.
    pub fn status(&self, data: &LimitData, sora: &Sora, present: &[u8]) -> Vec<(u16, Result<(), Refusal>)> {
        let p = self.party(data, sora, present);
        data.commands()
            .into_iter()
            .map(|c| {
                let i = rules::row_for_command(&data.rows, c, &p).or_else(|| data.rows.iter().position(|r| r.command == c)).unwrap();
                (c, rules::available(&data.rows[i], &p))
            })
            .collect()
    }

    /// kh2-rs `try_limit` for an accepted limit command. A sub-command (no `limt` row) goes to the
    /// running thread as event 23.
    pub fn start(&mut self, data: &LimitData, disc: &mut Disc, sora: &mut Sora, present: &[u8], command: u16) -> Result<(), Refusal> {
        if sora.hp <= 0 || matches!(sora.state, crate::sora::State::Down) {
            return Err(Refusal::Locked);
        }
        let p = self.party(data, sora, present);
        let Some(first) = data.rows.iter().position(|r| r.command == command) else {
            if data.sub_commands.contains(&command)
                && rules::sub_command_event(self.thread, command).is_some()
                && let Some(run) = self.run.as_mut()
            {
                run.sub_command(command);
                return Ok(());
            }
            return Err(Refusal::Busy);
        };
        let i = rules::row_for_command(&data.rows, command, &p).unwrap_or(first);
        let r = data.rows[i].clone();
        rules::available(&r, &p)?;
        sora.mp = if r.cost == 255 { 0 } else { (sora.mp - r.cost as i32).max(0) };
        let time = (r.time as u32).max(1);
        self.task = Some((command, time));
        self.frac = 0.0;
        self.ended_at = None;
        let bytes = data.bar(disc, &r.file);
        let bar = bytes.as_deref().and_then(|b| kh2_bar::parse(b).ok().map(|es| LimitBar::from_kinds(&es.iter().map(|e| e.kind).collect::<Vec<_>>()))).unwrap_or_default();
        self.thread = rules::event_receiver(&bar, &r);
        self.sets = bytes.as_deref().map(sets_of).unwrap_or_default();
        self.seen = self.events.len();
        self.hits.start(r.id, 0);
        self.events.push(LimitEvent::Started { command, file: r.file.clone(), time: r.time, members: r.members });
        // Event 22 to the BAR's own script (FUN_001b2e08); the participant's thread (FUN_001b3118) needs
        // the member's script, which is not run.
        self.run = None;
        if rules::start_events(&bar, &r).iter().any(|e| e.to == LimitReceiver::LimitScript)
            && let Some(prog) = bytes.as_deref().and_then(script_of)
        {
            let mut task = TaskState::new(command, time as f32);
            task.participants = kh2_enemy_host::limit_script::participants_of(r.members);
            self.run = Some(ScriptRun::start(prog, data.table.as_ref(), task).0);
        }
        Ok(())
    }

    /// An effective hit on an enemy while the limit runs (FUN_001b3dc8).
    pub fn count_hit(&mut self) {
        if self.task.is_some() {
            self.hits.add(1);
        }
    }

    /// One 60 Hz tick: the countdown (times the ability block's limit duration), the script thread.
    pub fn tick(&mut self, sora: &Sora) {
        if let Some((cmd, left)) = self.task.as_mut() {
            let mult = sora.loadout.as_ref().map_or(1.0, |l| l.block().limit_duration);
            *left = left.saturating_sub(rules::timer_ticks(&mut self.frac, mult));
            if *left == 0 {
                self.events.push(LimitEvent::Ended { command: *cmd, hits: self.hits.marker });
                self.task = None;
                self.thread = None;
            }
        }
        let left = self.task.map_or(0, |t| t.1);
        let Some(run) = self.run.as_mut() else { return };
        run.task.player = kh2_enemy_host::enemy_obj::to_game(sora.motor.pos);
        run.task.player_yaw = sora.motor.yaw;
        let v = kh2_enemy_host::enemy_obj::to_game(sora.motor.vel);
        run.task.player_velocity = [v[0], v[1], v[2], 0.0];
        if self.task.is_none() && self.ended_at.is_none() {
            self.ended_at = Some(run.ticks);
        }
        let stop = match run.tick(left).err() {
            None if self.ended_at.is_some_and(|at| run.ticks - at >= TAIL_TICKS) => Some(Stop::Returned),
            other => other,
        };
        for name in run.take_events() {
            self.events.push(LimitEvent::Named(name));
        }
        for m in run.take_motions() {
            self.events.push(LimitEvent::Motion { who: m.who, id: m.id, blend: m.blend });
        }
        for e in run.take_effects() {
            self.events.push(LimitEvent::Effect { who: e.who, id: e.id });
        }
        if let Some(s) = stop {
            let why = match s {
                Stop::Returned => format!("returned after {} ticks; host calls {}", run.ticks, run.summary()),
                Stop::Fault(f) => format!("fault after {} ticks: {f}", run.ticks),
            };
            self.events.push(LimitEvent::ScriptStopped(why));
            self.run = None;
        }
    }

    pub fn running(&self) -> bool {
        self.task.is_some()
    }

    /// kh2-rs `limit_visual::play_motions` / `release`: the script's motion requests (host 2.47) played
    /// on Sora (held in `State::Reaction`) and the members (held by `Follower::limit_hold`); at the end
    /// they get their own motions back. `sora_byte` is the objentry byte +7 of P_EX100.
    pub fn play_motions(&mut self, sora: &mut Sora, followers: &mut [crate::party::Follower], data: &crate::party::PartyData) {
        let new: Vec<(u32, usize, f32)> = self.events[self.seen.min(self.events.len())..].iter().filter_map(|e| if let LimitEvent::Motion { who, id, blend } = e { Some((*who, *id, *blend)) } else { None }).collect();
        self.seen = self.events.len();
        for (who, id, blend) in new {
            if who == kh2_enemy_host::enemy_obj::PLAYER {
                if let Some((_, m)) = find(&self.sets, id, data.sora_byte) {
                    sora.body.play_external(m.clone(), blend);
                    sora.state = crate::sora::State::Reaction;
                    self.sora_held = true;
                }
                continue;
            }
            for f in followers.iter_mut() {
                let md = &data.members[f.member];
                if kh2_enemy_host::limit_script::partner(u32::from(md.neo)) == who
                    && let Some((_, m)) = find(&self.sets, id, md.model_byte)
                {
                    f.body.play_external(m.clone(), blend);
                    f.limit_hold = true;
                }
            }
        }
        if self.task.is_none() {
            if self.sora_held {
                self.sora_held = false;
                if sora.state == crate::sora::State::Reaction {
                    sora.state = if sora.motor.grounded { crate::sora::State::Ground } else { crate::sora::State::Air };
                    if sora.state == crate::sora::State::Air {
                        sora.motor.start_fall();
                    }
                }
            }
            for f in followers.iter_mut() {
                f.limit_hold = false;
            }
        }
    }

    /// kh2-rs `reaction_cmd::auto_limit_offer` (FUN_001d25e0): with nothing else offered, no limit
    /// running and enemies present, a partner with Auto Limit (ability 417) within 5 m offers one of
    /// his limits; the command shown last tick is kept. Returns (command, enough MP).
    pub fn auto_offer(&mut self, data: &LimitData, sora: &Sora, followers: &[crate::party::Follower], party: &crate::party::PartyData, shown: Option<u16>) -> Option<(u16, bool)> {
        if self.task.is_some() {
            return None;
        }
        let present = present(followers, party);
        let base = self.party(data, sora, &present);
        // decided: as for the rows' test (every limit item learned), each partner holds the limit items of his rows.
        let partners: Vec<rules::Partner> = followers
            .iter()
            .filter(|f| !f.ko)
            .map(|f| {
                let neo = party.members[f.member].neo as u8;
                let mut items = f.abilities.clone();
                items.extend(data.rows.iter().filter(|r| r.members[0] == neo && self.abilities.is_none()).map(|r| r.item));
                rules::Partner { member: neo, pos: f.motor.pos, items }
            })
            .collect();
        let c = rules::auto_limit_candidates(&data.rows, &partners, sora.motor.pos, &base, |_| true);
        let cmds: Vec<u16> = c.iter().map(|x| x.command).collect();
        let i = rules::auto_limit_pick(&cmds, shown.unwrap_or(u16::MAX), &mut self.auto_seed)?;
        Some((c[i].command, !c[i].no_mp))
    }

    /// The running limit's hit count.
    pub fn hits(&self) -> u32 {
        self.hits.marker
    }
}
