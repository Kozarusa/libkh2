//! The Struggle mission manager (kh2-rs `kh2-event/src/struggle.rs`, WP-7E), one per area run.
//!
//! Compiled from kh2-rs source (`kh2_enemy_host::struggle`, `shared::struggle`, `prize`): the
//! mission file's header, layouts, counters and clock (`build`), the mission script and its host
//! (`MissionHost`), the end (`Active::flow_tick`, `finish_step`), the orbs a reaction knocks out
//! (`knock_out`, `orbs_knocked_out`) and the orb pool's frame (`orb_tick` on the prize rules).
//! This file is the glue of kh2-rs's Bevy systems `start_mission`, `sora_orb_loss`,
//! `enemy_orb_loss`, `mission_tick` and `run_orbs`, line for line where they are not Bevy.
//!
//! decided: an object's centre (`FUN_0016b7d0`, the body's first shape of kind 1) is its feet plus
//! `CENTRE` cm [approx: the library has no body shape points]; the latch of bit 3 of the global flag
//! word (`collect_latch`) is not kept (no prizes run in the library); sounds are handed to the host.

use std::collections::HashSet;

use glam::Vec3;
use kh2_enemy_host::prize::{self, Consts};
use kh2_enemy_host::shared::struggle::{Trig, EV_CLOCK_END, EV_PLAYER_KO};
use kh2_enemy_host::struggle::{self as st, Active, Finish, Taker};
use kh2_enemy_host::Entity;

use crate::data::Disc;
use crate::enemy::Enemies;
use crate::sora::Sora;

/// The height of an object's centre above its feet (cm) [approx].
pub const CENTRE: f32 = 80.0;

/// The result of a mission once its script sent it (global events 159 / 160).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Won,
    Lost,
}

/// One running mission manager.
pub struct Struggle {
    pub a: Box<Active>,
    /// The orb constants (`orb_consts`: the prize constants with the orb kinds), None when the
    /// executable was not readable (no orbs then).
    pub consts: Option<Consts>,
    unknown: HashSet<(u16, u16)>,
    last_hp: Option<i32>,
    /// The global events the manager sent this tick (id, argument), for the area's signals and the host.
    pub sent: Vec<(u32, i32)>,
    /// Interface sounds asked for (`FUN_002c3318`), and the orbs' bounce / pickup sounds with their place.
    pub sounds: Vec<(u32, Option<Vec3>)>,
    /// The manager ended (`FUN_001b9518`); `game_over` when it ended with the game-over path.
    pub ended: bool,
    pub game_over: bool,
    /// The end's last step asked for the result menu (record 0x1a); the manager stays.
    pub menu: bool,
}

/// App space (cm, Y up) and game space (Y down, Z mirrored).
fn flip(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.y, -v.z)
}

impl Struggle {
    /// kh2-rs `start_mission`: the mission file `msn/us/<name>.bar` built, event 2 of its script run
    /// at once and event 3 started as its thread, the start layout played (`Active::begin`).
    pub fn start(disc: &mut Disc, id: u32, name: &str, orb_pool: bool, triggers: &[Trig]) -> Result<Struggle, String> {
        let exe = disc.exe();
        let table = exe.as_deref().and_then(kh2_bdx::HostTable::from_exe);
        let kinds = disc.read("03system.bin").map(|s| st::event_kinds(&s)).unwrap_or_default();
        let consts = exe.as_deref().and_then(st::orb_consts);
        let path = format!("msn/us/{name}.bar");
        let file = disc.read(&path).map_err(|_| format!("{path} is not on the disc"))?;
        let mut a = st::build(id, name, file, orb_pool, table.as_ref())?;
        let mut unknown = HashSet::new();
        let mut rt = a.rt.take();
        if let Some(rt) = rt.as_mut() {
            let mut host = st::MissionHost { a: &mut a, unknown: &mut unknown };
            rt.event(&mut host, 2, &[]);
            rt.start_thread(0, 3, &[]);
        }
        a.rt = rt;
        a.kinds = kinds;
        a.triggers = triggers.to_vec();
        a.begin();
        Ok(Struggle { a: Box::new(a), consts, unknown, last_hp: None, sent: Vec::new(), sounds: Vec::new(), ended: false, game_over: false, menu: false })
    }

    /// The counters as host 4.29 sees them (slot 0 the player's orbs, slot 1 the opponent's).
    pub fn counters(&self) -> [i32; 3] {
        [0, 1, 2].map(|i| self.a.counter_value(i))
    }

    pub fn outcome(&self) -> Option<Outcome> {
        match self.a.result {
            Some("win") => Some(Outcome::Won),
            Some("lose") => Some(Outcome::Lost),
            _ => None,
        }
    }

    /// One frame, after Sora's and the enemies' ticks: the orbs the hits of the frame knocked out
    /// (`sora_orb_loss`, `enemy_orb_loss`), then `mission_tick` (the layouts and the manager's update,
    /// the clock, the knock-out, the script, the global events, the last step) and `run_orbs`.
    pub fn tick(&mut self, sora: &Sora, en: &mut Enemies, world: &kh2_game_core::world::World) {
        self.sent.clear();
        if self.ended {
            return;
        }
        let a = &mut *self.a;
        if let Some(c) = self.consts.as_ref() {
            for &r in &sora.reactions_taken {
                st::knock_out(a, c, true, r, flip(sora.motor.pos + Vec3::Y * CENTRE));
            }
            for e in &en.list {
                for &r in &e.struck {
                    st::knock_out(a, c, false, r, flip(e.pos() + Vec3::Y * CENTRE));
                }
            }
        }
        a.flow_tick();
        if a.clock.tick() {
            a.events.push((EV_CLOCK_END, 0));
        }
        // FUN_001cec00: with host 1.317's bit the player's knock-out is global event 0x5a.
        if sora.hp <= 0 && self.last_hp.is_some_and(|h| h > 0) && a.ko_loses {
            a.events.push((EV_PLAYER_KO, 0));
        }
        self.last_hp = Some(sora.hp);
        let mut rt = a.rt.take();
        if let Some(rt) = rt.as_mut() {
            let mut host = st::MissionHost { a, unknown: &mut self.unknown };
            rt.update_script(&mut host);
        }
        // Events raised while delivering are delivered in the same frame (the game's calls nest).
        let mut guard = 0;
        while !a.events.is_empty() && guard < 16 {
            guard += 1;
            for (id, arg) in std::mem::take(&mut a.events) {
                st::note_event(a, id, arg);
                if let Some(rt) = rt.as_mut() {
                    let mut host = st::MissionHost { a, unknown: &mut self.unknown };
                    rt.event(&mut host, 10, &[id, arg as u32]);
                }
                self.sent.push((id, arg));
            }
        }
        a.rt = rt;
        self.sounds.extend(a.sounds.drain(..).map(|s| (s, None)));
        self.run_orbs(sora, en, world);
        // decided: a host that never takes the sounds keeps the last 256.
        if self.sounds.len() > 256 {
            let extra = self.sounds.len() - 256;
            self.sounds.drain(..extra);
        }
        let a = &mut *self.a;
        en.mission = Some([0, 1, 2].map(|i| a.counter_value(i)));
        en.event_active = a.play.is_some();
        let Some(f) = a.pending.take() else { return };
        let gone = match f {
            Finish::Menu => {
                self.menu = true;
                false
            }
            Finish::Event(ev, arg) => {
                self.sent.push((ev, arg));
                match a.trigger(ev, arg) {
                    Some(_) => true,
                    None if ev == 8 => {
                        self.game_over = true;
                        true
                    }
                    None => false,
                }
            }
            Finish::GameOver => {
                self.game_over = true;
                true
            }
            Finish::Gone => true,
        };
        if gone {
            // FUN_001b9518: the elements, the clock and the orbs go with the manager.
            self.ended = true;
            a.orbs.clear();
            en.event_active = false;
        }
    }

    /// kh2-rs `run_orbs`: the orb pool's frame; the first fighter in reach takes an orb, one up on
    /// its side's counter (`FUN_001d6620(1, side)`).
    fn run_orbs(&mut self, sora: &Sora, en: &Enemies, world: &kh2_game_core::world::World) {
        let Some(c) = self.consts.as_ref() else { return };
        let a = &mut *self.a;
        if a.orbs.is_empty() {
            return;
        }
        let mut takers = Vec::new();
        if sora.hp > 0 {
            let extra = sora.loadout.as_ref().map_or(0.0, |l| l.block().draw_radius);
            let skip = st::pickup_skipped(sora.motion_id());
            takers.push(Taker { ent: Entity(0), player: true, feet: flip(sora.motor.pos), centre: flip(sora.motor.pos + Vec3::Y * CENTRE), extra, skip });
        }
        for e in en.list.iter().filter(|e| e.alive() && !e.removed) {
            let skip = st::pickup_skipped(e.anim.slot);
            takers.push(Taker { ent: Entity(u64::from(e.id)), player: false, feet: flip(e.pos()), centre: flip(e.pos() + Vec3::Y * CENTRE), extra: 0.0, skip });
        }
        let mut picked = Vec::new();
        let mut i = 0;
        while i < a.orbs.len() {
            let o = &mut a.orbs[i];
            let radius = c.kinds.get(o.p.kind as usize).map_or(0.0, |k| k.radius);
            let (alive, bounce, side) = st::orb_tick(o, c, |s, v| prize::sweep(Some(world), radius, s, v), &takers);
            if let Some(se) = bounce {
                self.sounds.push((se, Some(flip(o.p.pos))));
            }
            if let Some(side) = side {
                picked.push((side, o.p.kind, o.taker));
            }
            if alive {
                i += 1;
            } else {
                a.orbs.swap_remove(i);
            }
        }
        for (side, kind, taker) in picked {
            if let (Some(se), Some(t)) = (c.kinds.get(kind as usize).map(|k| k.pickup_se), takers.iter().find(|t| Some(t.ent) == taker)) {
                self.sounds.push((se, Some(flip(t.centre))));
            }
            a.change(side, |k| k.add(1));
        }
    }

    /// The orbs' places (app cm) and kinds.
    pub fn orbs(&self) -> Vec<(Vec3, u8)> {
        self.a.orbs.iter().map(|o| (flip(o.p.pos), o.p.kind)).collect()
    }
}
