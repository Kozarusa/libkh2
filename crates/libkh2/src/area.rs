//! An area's own battle set-up: the enemy groups its `btl` program loads.
//!
//! A transcription of kh2-rs `kh2-enemy/src/enemy.rs`'s `area_groups` (the spawn records of the
//! area file `ard/<area>.ard` that the area's `btl` program names, their entities that are enemies
//! of the kinds the library has, the spawn kind and parameters each record gives its object, the
//! player's spawn point). Positions are the game's turned half a turn about X (`(x, -y, -z)`), a
//! game facing `r` is the yaw `PI - r`.
//!
//! decided: every group of the chosen program is set up at once (the game enables groups through
//! the area's signals and triggers, kh2-rs `TriggerTable`, and spawns waves by the counters; the
//! library spawns every enabled record's enemies and leaves the waves to the host); without a story
//! state the host picks the program (kh2-rs reads it from the story flags).

use glam::Vec3;

use crate::data::Disc;

/// One enemy an area's battle program places.
#[derive(Clone, Debug, PartialEq)]
pub struct AreaEnemy {
    pub model: String,
    pub at: Vec3,
    pub yaw: f32,
    /// The spawn record's group tag and the program that loads it.
    pub group: String,
    pub program: u16,
    /// The entity's spawn kind (host 1.110) and parameters (object +0x820 / +0x824).
    pub spawn_kind: i32,
    pub spawn_params: [i32; 2],
    /// The record is disabled until a signal enables it (kind 5 or flag bit 0).
    pub disabled: bool,
}

/// What `area_battle` found: the programs, the enemies, the player's spawn point in the groups.
#[derive(Clone, Debug, Default)]
pub struct AreaBattle {
    pub programs: Vec<u16>,
    pub enemies: Vec<AreaEnemy>,
    pub player: Option<(Vec3, f32)>,
    /// Enemy objects of the groups the library has no kind for.
    pub skipped: Vec<String>,
}

/// kh2-rs `PLAYER_ENTITY` and the enemy object types.
const PLAYER_ENTITY: u32 = 0x236;
const OBJECT_ENEMY: u8 = 4;

impl Disc {
    /// The enemy groups of area `area`'s `btl` programs (`program` None: every program).
    pub fn area_battle(&mut self, area: &str, program: Option<u16>) -> AreaBattle {
        use kh2_tables::tables as t;
        let mut out = AreaBattle::default();
        // The world's sub-index names the area files (as `coct`).
        if let Some(world) = area.get(0..2) {
            let _ = self.0.apply_names([format!("000{world}.idx").as_str()]);
        }
        let Ok(file) = self.read(&format!("ard/{area}.ard")) else { return out };
        let Some((ard, _)) = kh2_ard::Area::parse(&file) else { return out };
        let rows = kh2_tables::load::<t::Objentry>(&mut self.0, &t::OBJENTRY).map(|d| d.rows).unwrap_or_default();
        for p in ard.scripts.get("btl").into_iter().flatten() {
            out.programs.push(p.id);
            if program.is_some_and(|q| q != p.id) {
                continue;
            }
            for name in p.spawns() {
                for rec in ard.spawns.get(&name).into_iter().flatten() {
                    let kind = (rec.kind & 0xff) as u8;
                    let disabled = kind == 5 || rec.flags() & 1 != 0;
                    for e in &rec.entities {
                        let [x, y, z] = e.position;
                        let at = Vec3::new(x, -y, -z);
                        let yaw = std::f32::consts::PI - e.rotation[1];
                        if e.object == PLAYER_ENTITY {
                            out.player.get_or_insert((at, yaw));
                            continue;
                        }
                        let Some(row) = rows.iter().find(|r| r.object_id == e.object) else { continue };
                        if kh2_enemy_host::KINDS.iter().any(|k| k.model == row.model_name) {
                            if out.enemies.iter().any(|x| x.group == name && x.at == at && x.model == row.model_name) {
                                continue;
                            }
                            out.enemies.push(AreaEnemy {
                                model: row.model_name.clone(),
                                at,
                                yaw,
                                group: name.clone(),
                                program: p.id,
                                spawn_kind: e.spawn_kind as i32,
                                spawn_params: [0x20, 0x24].map(|o| e.raw.get(o..o + 4).map_or(0, |b| i32::from_le_bytes(b.try_into().unwrap()))),
                                disabled,
                            });
                        } else if row.object_type == OBJECT_ENEMY && !out.skipped.contains(&row.model_name) {
                            out.skipped.push(row.model_name.clone());
                        }
                    }
                }
            }
        }
        out
    }
}

use kh2_enemy_host::area::{self as kg, AreaGroup, EventPart, KindTwo, PartCount, PlaceRng, TriggerTable};

/// kh2-rs `spawn_waves` and `fire_group_triggers` (enemy.rs), the area-driven battle for one Sora:
/// the groups of the area's `btl` program `program` (kh2-rs `area_groups`, compiled from source) set
/// up (the members created at load, the disabled records, the signals registered with the trigger
/// table, the kind-2 groups that create their spawn-kind-2 members while Sora stands in one of
/// their volumes, by their timers and waves, the kind-6 groups, the group counters whose removals
/// raise the global events 2 / 3), every tick.
///
/// decided: the program is the host's choice (kh2-rs takes the one the story runs); entrance 0;
/// the tables of the defeated start empty (no story across areas); no event or stopped fight
/// blocks the groups (no mission is running in the library unless the host keeps one).
#[derive(Default)]
pub struct AreaRun {
    pub area: String,
    pub groups: Vec<AreaGroup>,
    pub live: Vec<(AreaGroup, KindTwo, Vec<u16>)>,
    pub events: Vec<EventPart>,
    pub counts: Vec<PartCount>,
    pub triggers: TriggerTable,
    pub off: Vec<u16>,
    rng: PlaceRng,
    /// The global events the groups raised (id, argument), for the host.
    pub raised: Vec<(u32, i32)>,
    /// The mission manager a group's program started (opcode 0x15, kh2-rs `MissionStart`), and why
    /// one could not start.
    pub mission: Option<crate::struggle::Struggle>,
    pub mission_error: Option<String>,
    /// Raised events not yet handed to the signals.
    unfired: Vec<(u32, i32)>,
}

/// SCALE: the compiled group code works in metres (kh2-rs's Bevy space), the library in centimetres.
const M: f32 = kh2_enemy_host::SCALE;

impl AreaRun {
    /// Sets up the groups of program `program` (FUN_00181148 -> the group set-up FUN_00198750).
    pub fn set_up(disc: &mut Disc, area: &str, program: u16, en: &mut crate::enemy::Enemies, world: &kh2_game_core::world::World) -> AreaRun {
        if let Some(w) = area.get(0..2) {
            let _ = disc.0.apply_names([format!("000{w}.idx").as_str()]);
        }
        let shared = match disc.reopen() {
            Ok(d) => kh2_enemy_host::assets::Disc(std::sync::Mutex::new(d.0)),
            Err(_) => return AreaRun::default(),
        };
        if let Some(w) = area.get(0..2) {
            let _ = shared.0.lock().map(|mut a| a.apply_names([format!("000{w}.idx").as_str()]));
        }
        let groups = kg::area_groups(&shared, area);
        let signals = kg::area_signals(&shared, area);
        let mut run = AreaRun { area: area.to_string(), groups: groups.clone(), ..Default::default() };
        kh2_enemy_host::enemy_host::clear_shared_blocks();
        for g in groups.iter().filter(|g| g.program == program) {
            for m in g.members.iter().filter(|m| kg::created_at_load(m, &g.parts[m.part], 0)) {
                spawn_member(en, world, m, g.parts[m.part].id, None);
            }
            let twos: Vec<usize> = (0..g.parts.len()).filter(|&k| g.parts[k].kind == 2 && g.members.iter().any(|m| m.part == k && m.spawn_kind == 2)).collect();
            for part in g.parts.iter().filter(|p| p.disabled) {
                run.off.push(part.id);
            }
            for sig in signals.iter().filter(|s| s.tag == g.name) {
                run.triggers.register(sig.event, sig.arg, sig.id, sig.action);
            }
            for k in twos {
                let two = KindTwo::new(g, k);
                let there = g.members.iter().filter(|m| m.part == k && kg::created_at_load(m, &g.parts[k], 0)).map(|m| m.serial).collect();
                run.live.push((g.clone(), two, there));
            }
            for (k, part) in g.parts.iter().enumerate() {
                let count = kg::counted_at_load(part, 0, &[]);
                run.counts.push(PartCount { id: part.id, count, disabled: part.disabled });
                run.events.push(EventPart { group: g.clone(), part: k, present: Vec::new() });
            }
            // The program's mission command (opcode 0x15) creates the mission manager (kh2-rs WP-7E).
            if let Some((id, name)) = &g.mission
                && run.mission.is_none()
            {
                match crate::struggle::Struggle::start(disc, *id, name, g.orb_pool, &g.triggers) {
                    Ok(m) => run.mission = Some(m),
                    Err(e) => run.mission_error = Some(e),
                }
            }
        }
        run
    }

    /// The mission manager's frame (`crate::struggle::Struggle::tick`); the global events it sent go
    /// to the area's signals on the next `tick`, and to the host with the groups' own.
    pub fn tick_mission(&mut self, sora: &crate::sora::Sora, en: &mut crate::enemy::Enemies, world: &kh2_game_core::world::World) {
        let Some(m) = self.mission.as_mut() else { return };
        m.tick(sora, en, world);
        self.raised.extend(m.sent.iter().copied());
        self.unfired.extend(m.sent.iter().copied());
    }

    /// FUN_00190668: no group creates while an event plays (0x35e3a4) or the fight is stopped.
    fn blocked(&self) -> bool {
        self.mission.as_ref().is_some_and(|m| !m.ended && (m.a.play.is_some() || m.a.stopped))
    }

    /// One tick (kh2-rs `spawn_waves`' per-tick part and `fire_group_triggers`).
    pub fn tick(&mut self, en: &mut crate::enemy::Enemies, world: &kh2_game_core::world::World, sora_pos: glam::Vec3, raised: &[(i32, i32)]) {
        let at = [sora_pos.x, -sora_pos.y, -sora_pos.z];
        let player = sora_pos * M;
        let alive_serials = |en: &crate::enemy::Enemies, pid: u16| -> Vec<u16> { en.list.iter().filter(|e| e.alive() && !e.removed).filter_map(|e| e.group_serial.filter(|s| s.0 == pid).map(|s| s.1)).collect() };
        let blocked = self.blocked();
        for (g, two, prev) in self.live.iter_mut() {
            let pid = g.parts[two.part].id;
            if self.off.contains(&pid) {
                continue;
            }
            let live = alive_serials(en, pid);
            let tp = two.part;
            for m in g.members.iter().filter(|m| m.part == tp && prev.contains(&m.serial) && !live.contains(&m.serial)) {
                two.removed(g, m);
            }
            // FUN_001989d8: while FUN_00190668 holds, the timer stands and nothing is created.
            if !blocked && two.timer > 0.0 {
                two.timer -= 1.0;
            }
            let mut made = Vec::new();
            if !blocked && g.parts[two.part].activators.iter().any(|v| v.contains(at)) {
                let alive = |i: usize| en.list.iter().any(|e| e.alive() && e.group_serial == Some((pid, g.members[i].serial)));
                for i in two.run(g, alive) {
                    let m = &g.members[i];
                    let spot = kg::member_ring(m, player).map(|ring| kg::ring_place_in(&mut self.rng, &ring, world));
                    spawn_member(en, world, m, pid, spot);
                    made.push(m.serial);
                }
            }
            *prev = live.into_iter().chain(made).collect();
        }
        for ep in &self.events {
            let pid = ep.group.parts[ep.part].id;
            for i in kg::kind_six_creates(&ep.group, ep.part, |v| v.contains(at), |s| en.list.iter().any(|e| !e.removed && e.group_serial == Some((pid, s)))) {
                spawn_member(en, world, &ep.group.members[i], pid, None);
            }
        }
        // Group events: a member of spawn kind 0, 1 or 3 removed lowers its record's counter (FUN_001993f8 -> FUN_00199518).
        for i in 0..self.events.len() {
            let (pid, part_idx) = (self.events[i].group.parts[self.events[i].part].id, self.events[i].part);
            let now: Vec<u16> = alive_serials(en, pid).into_iter().filter(|s| self.events[i].group.members.iter().any(|m| m.part == part_idx && m.serial == *s && matches!(m.spawn_kind, 0 | 1 | 3))).collect();
            let gone = self.events[i].present.iter().filter(|s| !now.contains(s)).count();
            self.events[i].present = now;
            for _ in 0..gone {
                for (id, arg) in kg::removal_events(&mut self.counts, i) {
                    self.raised.push((id, arg));
                    self.unfired.push((id, arg));
                }
            }
        }
        // FUN_00197b70 -> FUN_00197670: the raised events switch the groups their signals name.
        let mut events: Vec<(i32, i32)> = raised.to_vec();
        // Each raised event once (the host's `raised` list keeps them until it takes them).
        events.extend(self.unfired.drain(..).map(|(i, a)| (i as i32, a)));
        kh2_enemy_host::enemy_host::set_group_enemy_sum(self.counts.iter().map(|c| c.count).sum());
        for (event, arg) in events {
            for (id, action) in self.triggers.fire(event, arg) {
                if !self.counts.iter().any(|c| c.id == id) {
                    continue;
                }
                match action {
                    0 => self.off.retain(|i| *i != id),
                    1 if !self.off.contains(&id) => self.off.push(id),
                    _ => continue,
                }
                for c in self.counts.iter_mut().filter(|c| c.id == id) {
                    c.disabled = action == 1;
                }
            }
        }
    }
}

/// kh2-rs `spawn_member` / `record_spot`: the member at its record's point on the floor (or the
/// placement given), with its spawn kind, parameters and group serial.
fn spawn_member(en: &mut crate::enemy::Enemies, world: &kh2_game_core::world::World, m: &kg::Member, part_id: u16, spot: Option<glam::Vec3>) -> Option<u32> {
    let at = spot.unwrap_or_else(|| {
        let y = world.ground_m(m.at.x, m.at.z, m.at.y + 1.0).unwrap_or(m.at.y);
        m.at.with_y(y)
    }) / M;
    let id = en.spawn_with(m.kind.model, at, m.yaw, world, m.spawn_kind, Some(m.spawn_params))?;
    if let Some(e) = en.list.iter_mut().find(|e| e.id == id) {
        e.group_serial = Some((part_id, m.serial));
    }
    Some(id)
}

/// One object an area's scripts place (round 9, for the host): townspeople, field objects, save
/// points, chests, the player's spawn point (object 0x236) and the enemies, from the spawn records
/// (`ard/<area>.ard`, kh2-rs `kh2_ard::Area::spawned`).
#[derive(Clone, Debug, PartialEq)]
pub struct AreaObject {
    /// The script (`map`, `btl`, `evt`) and its program that load the group.
    pub script: String,
    pub program: u16,
    /// The spawn group's tag.
    pub group: String,
    /// `00objentry`: id, model, object type (4 enemy, 5 townsperson ...).
    pub object: u32,
    pub model: String,
    pub object_type: u8,
    /// Library space (cm, Y up) and yaw.
    pub at: Vec3,
    pub yaw: f32,
    /// Spawn record +0x28 the talk id (`kh2_talk_target.talk_id`), +0x2c the command its reaction part 0x83
    /// offers (talk 0x1f / examine ...), +0x1c the spawn kind, +0x1e the serial.
    pub talk_id: u16,
    pub part_command: u16,
    pub spawn_kind: u8,
    pub serial: u16,
}

/// What an area's set-up gives the host (kh2-rs `kh2_ard::run::run_area`, orig 0x181148: the map,
/// btl and evt programs run in order): the spawn groups loaded, their objects created for the
/// entrance (FUN_001981a0), the collision group mask the objects' hosts 1.46 / 1.47 switch
/// (`DAT_0035cd18`), the camera type, the music pair and the player's start.
#[derive(Clone, Debug, Default)]
pub struct AreaSetup {
    pub groups: Vec<String>,
    pub objects: Vec<AreaObject>,
    pub collision_mask: u64,
    pub camera_type: u32,
    pub music: Option<[u16; 2]>,
    pub player: Option<(Vec3, f32)>,
}

impl Disc {
    /// Area `area` set up with programs `[map, btl, evt]` at `entrance`.
    pub fn area_setup(&mut self, area: &str, programs: [u8; 3], entrance: u8) -> AreaSetup {
        use kh2_tables::tables as t;
        if let Some(world) = area.get(0..2) {
            let _ = self.0.apply_names([format!("000{world}.idx").as_str()]);
        }
        let Ok(file) = self.read(&format!("ard/{area}.ard")) else { return AreaSetup::default() };
        let Some((ard, _)) = kh2_ard::Area::parse(&file) else { return AreaSetup::default() };
        let rows = kh2_tables::load::<t::Objentry>(&mut self.0, &t::OBJENTRY).map(|d| d.rows).unwrap_or_default();
        let run = kh2_ard::run::run_area(&ard, programs, entrance);
        let to_lib = |e: &kh2_ard::Entity| (Vec3::new(e.position[0], -e.position[1], -e.position[2]), std::f32::consts::PI - e.rotation[1]);
        let mut out = AreaSetup {
            groups: run.groups.iter().map(|g| g.tag.clone()).collect(),
            collision_mask: run.word_35cd18,
            camera_type: run.camera_type,
            music: run.music,
            player: kh2_ard::run::player_start(&ard, &run, entrance).map(|(_, e)| to_lib(e)),
            objects: Vec::new(),
        };
        for g in &run.groups {
            for grp in ard.spawns.get(&g.tag).into_iter().flatten() {
                for e in grp.entities.iter().filter(|e| kh2_ard::run::entity_for_entrance(e, entrance)) {
                    let row = rows.iter().find(|r| r.object_id == e.object);
                    let (at, yaw) = to_lib(e);
                    out.objects.push(AreaObject {
                        script: g.script.to_string(),
                        program: g.program,
                        group: g.tag.clone(),
                        object: e.object,
                        model: row.map_or(String::new(), |r| r.model_name.clone()),
                        object_type: row.map_or(0, |r| r.object_type),
                        at,
                        yaw,
                        talk_id: e.talk_id(),
                        part_command: e.part_command(),
                        spawn_kind: e.spawn_kind,
                        serial: e.serial,
                    });
                }
            }
        }
        out
    }
}
