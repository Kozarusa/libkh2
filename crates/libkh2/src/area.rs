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
