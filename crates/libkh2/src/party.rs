//! Donald and Goofy: field following and a battle subset.
//!
//! The rules are kh2-rs's: `party_rules.rs` (follow points, follow step, steer) and
//! `party_battle.rs` (the AI class's action records from the executable, the target choice of the
//! battle styles, the action choice `choose`, the feasibility and range tests, the approach step,
//! the slot remap), compiled from kh2-rs's source; the game's generator is kh2-rs's `rng`. This
//! module transcribes kh2-rs `kh2-party/src/party.rs`'s `party_tick` for the field follow and the
//! battle choice (style 0), the attack command (`Start::Attack`: the entry's motion through the
//! battle row's remap, the air entry's jump), the combo follow-up jump and the running action's root
//! motion; `member_hands` for the weapon; the members' hits use the enemies' hurt shapes and the
//! damage formula like Sora's.
//!
//! Round 5: the members are characters: HP and MP (the new game's `plrp` rows), their status from
//! their `lvup` table at their level plus the `plrp` bytes (kh2-rs `MemberStatus::member_status`),
//! enemies' attacks hit them (kh2-rs `party_receive_hits`: damage, the victim reaction motion,
//! FUN_00186238's HP method `hp_change` with knock-out), the knock-out lies in motion 0x36 and the
//! revive count-down (`revive_time` from `prty` StunTime, `revive_step`, `revive_heal` with the
//! `sstm` words 0x58 / 0x5c) stands them up with motion 0x37; the battle styles 0..3 (`dispatch`
//! by class and style, the style's battle distance; style 1 reads Sora's target); the danger test,
//! the MP gate and the emergency pass (`danger_member`, `mp_gate`, `emergency`); the HP / MP item
//! handlers (`item_pick` with the executable's item orders and fractions, the items' effect through
//! kh2-rs `item_effect`) from the stock the host gives (`Follower::items`).
//!
//! decided: no member scripts, so the specials and Cure (which switch the member's script to a
//! mode) are never feasible; an item takes effect when chosen (no use motion); no limits; warp
//! through the trail (a member beyond `WARP_DISTANCE` is put back beside Sora); the weapons are the
//! base items (Mage's Staff 75, Knight's Shield 49, as kh2-rs's harness aid); no equipment (the
//! status is the `lvup` row plus the `plrp` bytes); the setting bytes are all 0 (a new game's).

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use kh2_game_core::motor::Motor;
use kh2_game_core::party_battle::{self as battle, Choice, ClassAi, Handler, MemberNow};
use kh2_game_core::party_rules::{self as rules, Consts, Follow, FollowIn, Thresholds};
use kh2_game_core::world::World;

use crate::anim::{Anim, Rig};
use crate::data::{Disc, LoadError};

/// Distance from Sora beyond which a follower is placed back beside him (app rule).
pub const WARP_DISTANCE: f32 = 3000.0;
const BLEND: f32 = 8.0;
/// kh2-rs `party.rs`: `MOTION_DOWN` 0x36 (KO), 0x37 the get-up.
const MOTION_DOWN: i32 = 0x36;
const MOTION_GET_UP: i32 = 0x37;
/// An item handler's two choice orders, and the part of a status block it reads.
type ItemLists = (Vec<u16>, Vec<u16>);
type VitalPart = fn(&battle::Vital) -> (i32, i32);

/// The members libkh2 knows (model names) with their weapon items and hand bones (kh2-rs
/// `party.rs`: `DONALD_HAND` 131, `GOOFY_HAND` 101).
pub const MEMBERS: [&str; 2] = ["P_EX020", "P_EX030"];
const WEAPON_ITEMS: [u16; 2] = [75, 49];
const HAND_BONES: [usize; 2] = [131, 101];

/// What a follower is made of.
pub struct MemberData {
    pub model: String,
    pub rig: Arc<Rig>,
    pub weapon: Option<Arc<Rig>>,
    pub hand: usize,
    pub class: u8,
    pub thresholds: Thresholds,
    pub walk: f32,
    pub run: f32,
    pub turn: f32,
    pub jump: f32,
    pub radius: f32,
    /// NeoStatus (2 Donald, 3 Goofy): the `lvup` table and the `plrp` row.
    pub neo: u16,
    /// The new game's `plrp` row: maximum HP and MP, and the record's strength / magic / defense.
    pub hp_max: i32,
    pub mp_max: i32,
    pub base: (i32, i32, i32),
    /// The member's `lvup` table (kh2-rs `MemberStatus::lvup_row`).
    pub lvup: Vec<kh2_tables::tables::Lvup>,
    /// `prty` StunTime (the revive time) and MpDrive.
    pub stun: f32,
    pub mp_drive: f32,
    /// The member's own script (type 3 of its model file) and its objentry flag (kh2-rs `FriendScripts`).
    pub script: Option<(kh2_bdx::Program, bool)>,
    /// The abilities the new game's `plrp` row equips (items with bit 15): a record whose item is one
    /// of them is equipped (FUN_0010f220).
    pub abilities: Vec<u16>,
    /// The objentry byte +7 (`WeaponJoint`): the `%03d0` name of its paired / limit sub-motions.
    pub model_byte: u8,
}

impl MemberData {
    /// kh2-rs `MemberStatus::member_status` without equipment: the `lvup` row of the level (held at
    /// the table's end) plus the record's bytes.
    pub fn status(&self, level: u8) -> (i32, i32, i32) {
        let row = (!self.lvup.is_empty()).then(|| &self.lvup[(level.max(1) as usize).min(self.lvup.len()) - 1]);
        let (s, m, d) = row.map_or((0, 0, 0), |r| (r.strength as i32, r.magic as i32, r.defense as i32));
        (s + self.base.0, m + self.base.1, d + self.base.2)
    }
}

/// The party tables (kh2-rs `PartyData`, the parts the follow and the battle subset use).
pub struct PartyData {
    pub consts: Consts,
    pub members: Vec<MemberData>,
    pub ai: HashMap<u8, ClassAi>,
    pub orders: [[u8; 4]; 4],
    pub half: f32,
    pub stop: (f32, f32),
    /// The `sstm` record (revive heal 0x58, field revive scale 0x5c, revive scale 0x54).
    pub sstm: Option<kh2_tables::player::Sstm>,
    /// FUN_00110f90's MP-gate weights, the item handlers' fractions and choice orders.
    pub mp_gate_w: (f32, f32),
    pub item_frac: (f32, f32),
    pub hp_items: (Vec<u16>, Vec<u16>),
    pub mp_items: (Vec<u16>, Vec<u16>),
    /// The items (`item` of 03system.bin) for the item handlers' effect, and their `item_stat` rows.
    pub items: Vec<kh2_tables::tables::Item>,
    pub item_stats: Vec<kh2_tables::tables::ItemStat>,
    /// The scripts' host table, host 1.247's numbers and the `atkp` rows (the attack objects' rows).
    pub table: Option<kh2_bdx::HostTable>,
    pub free: Option<kh2_game_core::party_rules::FreePointConsts>,
    pub atkp: kh2_enemy_host::data::GameData,
    /// Sora's objentry byte +7 (P_EX100).
    pub sora_byte: u8,
}

impl PartyData {
    fn sstm_f32(&self, off: usize) -> f32 {
        self.sstm.as_ref().filter(|s| s.words.len() > off / 4).map_or(0.0, |s| s.f32(off))
    }
}

impl Disc {
    /// Loads Donald and Goofy: their objentry rows, rigs, weapons, AI classes and `prty` rows, and
    /// the executable's follow constants and action records.
    pub fn party(&mut self) -> Result<PartyData, LoadError> {
        use kh2_tables::tables as t;
        let exe = self.exe().ok_or_else(|| LoadError("no executable".into()))?;
        let consts = Consts::from_exe(&exe).ok_or_else(|| LoadError("party constants not readable".into()))?;
        let objs = kh2_tables::load::<t::Objentry>(&mut self.0, &t::OBJENTRY).map_err(|e| LoadError(format!("objentry: {e}")))?.rows;
        let prty = kh2_tables::load::<t::PrefPrty>(&mut self.0, &t::PREF_PRTY).map_err(|e| LoadError(format!("prty: {e}")))?.rows;
        let sys = self.read("03system.bin")?;
        let offsets: Vec<u32> = (|| {
            let top = kh2_bar::parse(&sys).ok()?;
            let inner = kh2_bar::parse(kh2_bar::find(&top, "pref")?.data).ok()?;
            let b = kh2_bar::find(&inner, "prty")?.data;
            let n = u32::from_le_bytes(b.get(0..4)?.try_into().ok()?) as usize;
            (0..n).map(|i| b.get(4 + 4 * i..8 + 4 * i).map(|x| u32::from_le_bytes(x.try_into().unwrap()))).collect::<Option<Vec<u32>>>()
        })()
        .unwrap_or_default();
        let lvup = kh2_tables::subfile(&mut self.0, &t::LVUP).ok().and_then(|b| kh2_tables::rt::decode_lvup::<t::Lvup>(&b)).unwrap_or_default();
        let drive = crate::drive::DriveData::load(self).ok();
        let mut ai = HashMap::new();
        for class in 0..rules::CLASSES as u8 {
            if let Some(c) = ClassAi::from_exe(&exe, class) {
                ai.insert(class, c);
            }
        }
        let orders = battle::motion_orders(&exe).unwrap_or([[2, 3, 1, 0]; 4]);
        let f = |va: u32| rules::exe_bytes(&exe, va, 4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        let half = f(0x3784ac).unwrap_or(0.5);
        let stop = (f(0x378564).unwrap_or(0.0), f(0x378568).unwrap_or(0.0));
        let g = |va: u32| f(va).unwrap_or(0.0);
        let mp_gate_w = (g(battle::MP_GATE_SLOPE), g(battle::MP_GATE_BASE));
        let item_frac = (g(battle::HP_ITEM_FRACTION), g(battle::MP_ITEM_FRACTION));
        let hp_items = (battle::item_order(&exe, battle::HP_ITEMS_ONE, battle::HP_ITEM_CHOICES), battle::item_order(&exe, battle::HP_ITEMS_MANY, battle::HP_ITEM_CHOICES));
        let mp_items = (battle::item_order(&exe, battle::MP_ITEMS_ONE, battle::MP_ITEM_CHOICES), battle::item_order(&exe, battle::MP_ITEMS_MANY, battle::MP_ITEM_CHOICES));
        let sstm = kh2_tables::player::load_sstm(&mut self.0).ok();
        let plrp = kh2_tables::load::<t::Plrp>(&mut self.0, &t::PLRP).map(|d| d.rows).unwrap_or_default();
        let items = kh2_tables::load::<t::Item>(&mut self.0, &t::ITEM).map(|d| d.rows).unwrap_or_default();
        let item_stats = kh2_tables::load::<t::ItemStat>(&mut self.0, &t::ITEM_STAT).map(|d| d.rows).unwrap_or_default();
        let table = kh2_bdx::HostTable::from_exe(&exe);
        let free = kh2_game_core::party_rules::FreePointConsts::from_exe(&exe);
        let atkp = kh2_enemy_host::data::GameData { atkp: kh2_tables::load::<t::Atkp>(&mut self.0, &t::ATKP).map(|d| d.rows).unwrap_or_default() };
        let mut members = Vec::new();
        for (k, model) in MEMBERS.iter().enumerate() {
            let row = objs.iter().find(|o| o.model_name == *model).ok_or_else(|| LoadError(format!("objentry: no {model}")))?;
            let class = rules::ai_class(row.neo_status, row.neo_moveset).unwrap_or(0);
            let rig = Arc::new(self.rig_named(model, row.animation_name.trim_end_matches(".mset"))?);
            let pk = if (row.neo_moveset as usize) < offsets.len() && offsets[row.neo_moveset as usize] != 0 { row.neo_moveset as usize } else { 1 };
            let p = prty.get(pk);
            let radius = rig.model.shapes.iter().find(|s| s.kind == 0 || s.kind == 4).map_or(35.0, |s| s.radius.abs());
            // kh2-rs `PartyData::weapon_files` (FUN_0017c718): the weapon object of the moveset's
            // `went` row for the item's slot, the hand's `wmst` motion set.
            let weapon = drive.as_ref().and_then(|d| {
                let slot = d.items.iter().find(|i| i.id == WEAPON_ITEMS[k]).map(|i| i.flag1 as u16 | (i.rank as u16) << 8)?;
                let ms = row.neo_moveset as usize;
                let id = kh2_game_core::drive_rules::weapon_object(&d.went, ms, slot)?;
                let w = objs.iter().find(|w| w.object_id == id)?;
                let mset = d.wmst.get(ms).and_then(|n| n.first()).filter(|s| !s.is_empty()).cloned()?;
                self.rig_named(&w.model_name, &mset).ok().map(Arc::new)
            });
            // kh2-rs `MemberStatus::lvup_row`: the table of NeoStatus (1..=13 itself), the new game's
            // `plrp` row 0 of the character (FUN_001c0b80).
            let neo = row.neo_status;
            let table = lvup.get((neo as usize).max(1) - 1).filter(|t| !t.is_empty()).or_else(|| lvup.first()).cloned().unwrap_or_default();
            let pr = plrp.iter().find(|r| r.id == 0 && u16::from(r.character) == neo);
            let script = self.read(&format!("obj/{model}.mdlx")).ok().and_then(|f| kh2_bar::parse(&f).ok()?.into_iter().find(|e| e.kind == 3).and_then(|e| kh2_bdx::Program::parse(e.data).ok())).map(|p| (p, row.flags & 8 != 0));
            let abilities = pr.map_or_else(Vec::new, |r| r.items.iter().filter(|&&i| i & 0x8000 != 0).map(|&i| i & 0x7fff).collect());
            members.push(MemberData {
                model_byte: row.weapon_joint,
                script,
                abilities,
                neo,
                hp_max: pr.map_or(20, |r| i32::from(r.hp)),
                mp_max: pr.map_or(0, |r| i32::from(r.mp)),
                base: pr.map_or((0, 0, 0), |r| (i32::from(r.strength), i32::from(r.magic), i32::from(r.defense))),
                lvup: table,
                stun: p.map_or(0.0, |p| p.stun_time),
                mp_drive: p.map_or(0.0, |p| p.mp_drive),
                model: model.to_string(),
                thresholds: consts.thresholds(class),
                walk: p.map_or(0.0, |p| p.walk_speed),
                run: p.map_or(0.0, |p| p.run_speed),
                turn: p.map_or(0.0, |p| p.turn_speed),
                jump: p.map_or(0.0, |p| p.jump_height),
                radius,
                class,
                weapon,
                hand: HAND_BONES[k],
                rig,
            });
        }
        let sora_byte = objs.iter().find(|r| r.model_name == "P_EX100").map_or(1, |r| r.weapon_joint);
        Ok(PartyData { consts, members, ai, orders, half, stop, sstm, mp_gate_w, item_frac, hp_items, mp_items, items, item_stats, table, free, atkp, sora_byte })
    }
}

/// A running action (kh2-rs `Attacking`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Attacking {
    pub record: Option<usize>,
    pub slot: usize,
    pub until_landed: bool,
}

/// One follower.
pub struct Follower {
    pub member: usize,
    pub motor: Motor,
    pub follow: Follow,
    pub body: Anim,
    pub weapon: Option<Anim>,
    pub choosing: battle::Choosing,
    pub target: Option<(u32, usize)>,
    pub attack: Option<Attacking>,
    pub held: Option<(Vec3, f32)>,
    pub timer: f32,
    pub speed: f32,
    pub want_yaw: Option<f32>,
    /// Hits landed by this member's current action: (enemy id, trigger start).
    pub landed: HashSet<(u32, i16)>,
    pub level: u8,
    pub hp: i32,
    pub hp_max: i32,
    pub mp: i32,
    pub mp_max: i32,
    /// +0x7f8 bit 4 (KO) and the revive timer +0xb40.
    pub ko: bool,
    pub revive: f32,
    /// +0xb00, the invincibility timer.
    pub invuln: f32,
    /// A hurt or get-up motion that holds the member until it ends.
    pub hurt: Option<usize>,
    /// The battle style (setting byte 0): 0..3.
    pub style: u8,
    /// The battle item stock: (item id, count). The host gives it (kh2-rs: the record's item slots).
    pub items: Vec<(u16, u8)>,
    /// Enemy attacks that landed on this member: (enemy id, its motion slot, trigger start).
    pub hit_by: HashSet<(u32, Option<usize>, i16)>,
    /// What happened to this member since the host last took it.
    pub events: Vec<MemberEvent>,
    /// The member's own script (kh2-rs `FriendBrain`) and its call trace.
    pub brain: Option<crate::friend::FriendBrain>,
    pub trace: kh2_enemy_host::enemy_ai::AiTrace,
    /// FUN_00113d48: the mode asked of the script and the MP byte it will cost.
    pub mode_request: Option<(String, u8)>,
    /// Whom the running special was chosen for (its target record).
    pub special_who: Option<SpecialWho>,
    /// The script's current mode, and what it asked: motion slot, queue, movement, facing.
    pub script_mode: Option<String>,
    pub script_slot: Option<usize>,
    pub script_restart: bool,
    pub script_queue: Option<usize>,
    pub script_move: Option<Vec3>,
    pub script_yaw: Option<f32>,
    /// The MP charge after a cost the MP did not cover (magic_rules `pay`).
    pub charge: kh2_game_core::magic_rules::Charge,
    /// A limit's motion holds the body (kh2-rs `LimitHold`): the party's motion choice leaves it.
    pub limit_hold: bool,
    /// The member's equipped abilities (the new game's; the host may change them).
    pub abilities: Vec<u16>,
    /// The member's equipped items (weapon, armor, accessories): their `item_stat` rows add to the status.
    pub equipment: Vec<u16>,
}

/// The target record of a running special: an enemy (by id) or a party member (Cure).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpecialWho {
    Enemy(u32),
    Party(battle::Who),
}

/// Something that happened to a member.
#[derive(Clone, Debug, PartialEq)]
pub enum MemberEvent {
    /// An enemy's attack: atkp row, damage, HP after.
    Hit { by: u32, atkp: i16, damage: i32, hp: i32 },
    KnockedOut,
    Revived { hp: i32 },
    /// An item handler used `item` on `who` (0 Sora, 1 this member, 2 the other member).
    ItemUsed { item: u16, who: u8 },
    /// A level reached (kh2-rs `raise_level`): the new row's ability of the member's column (0 none).
    LevelUp { level: u8, ability: u16 },
    /// A bonus reward's row for this member: maximum HP / MP added, the bonus items.
    Bonus { id: u16, hp: i32, mp: i32, items: [u16; 2] },
    /// The script switched to a mode (a Special, Cure, a limit's part ...).
    ScriptMode { mode: String },
    /// The script started element `id` of the member's effect file.
    Effect { id: u16 },
    /// A script heal (Cure): `who` 0 Sora, 1 this member, 2 the other member; amount.
    Healed { who: u8, amount: i32 },
}

/// kh2-rs `Progress::raise_level` for the members (FUN_001b52f8): while the `lvup` threshold of the
/// member's level is at most the shared experience total, the level rises and the new row's ability
/// of the member's weapon set is given. decided: the members' weapon set byte is 0 (the new game's),
/// so the Sword column; the abilities are reported, not equipped.
pub fn raise_levels(data: &PartyData, followers: &mut [Follower], exp: i32) {
    for f in followers.iter_mut() {
        let t = &data.members[f.member].lvup;
        while f.level < 99 {
            let (Some(old), Some(new)) = (t.get(f.level as usize - 1), t.get(f.level as usize)) else { break };
            if old.exp > exp {
                break;
            }
            f.level += 1;
            f.events.push(MemberEvent::LevelUp { level: f.level, ability: new.sword_ability });
        }
    }
}

impl Follower {
    pub fn new(data: &PartyData, member: usize, at: Vec3) -> Follower {
        let m = &data.members[member];
        Follower {
            member,
            motor: Motor { pos: at, grounded: true, ..Motor::default() },
            follow: Follow::default(),
            body: Anim::new(m.rig.clone()),
            weapon: m.weapon.clone().map(Anim::new),
            choosing: Default::default(),
            target: None,
            attack: None,
            held: None,
            timer: 0.0,
            speed: 0.0,
            want_yaw: None,
            landed: HashSet::new(),
            level: 1,
            hp: m.hp_max,
            hp_max: m.hp_max,
            mp: m.mp_max,
            mp_max: m.mp_max,
            ko: false,
            revive: 0.0,
            invuln: 0.0,
            hurt: None,
            style: 0,
            items: Vec::new(),
            hit_by: HashSet::new(),
            events: Vec::new(),
            brain: m.script.clone().map(|(p, f124)| crate::friend::FriendBrain::new(p, f124, data.table.as_ref(), member, &m.model)),
            trace: Default::default(),
            mode_request: None,
            special_who: None,
            script_mode: None,
            script_slot: None,
            script_restart: false,
            script_queue: None,
            script_move: None,
            script_yaw: None,
            charge: Default::default(),
            limit_hold: false,
            abilities: m.abilities.clone(),
            equipment: vec![WEAPON_ITEMS[member.min(1)]],
        }
    }

    /// kh2-rs `PartyMember::pay_mp` (host 1.276): the MP, or the MP charge when it is short.
    pub fn pay_mp(&mut self, cost: u8, mp_drive: f32) {
        if self.ko || self.hp_max == 0 {
            return;
        }
        match kh2_game_core::magic_rules::pay(self.mp, self.mp_max, cost, mp_drive) {
            kh2_game_core::magic_rules::Paid::Mp(v) => self.mp = v,
            kh2_game_core::magic_rules::Paid::Charge { total } => {
                self.mp = 0;
                self.charge = kh2_game_core::magic_rules::Charge { left: total, total };
            }
        }
    }

    /// The script is busy with a special's mode (+0x390, kh2-rs `script_busy`).
    pub fn script_busy(&self, ai: Option<&ClassAi>) -> bool {
        self.mode_request.is_some() || ai.is_some_and(|a| a.records.iter().any(|r| !r.mode.is_empty() && self.script_mode.as_deref() == Some(r.mode.as_str())))
    }

    /// The member's status block for the damage formula at its level.
    pub fn stats(&self, data: &PartyData) -> kh2_enemy_host::data::Stats {
        let (strength, magic, defense) = data.members[self.member].status(self.level);
        // kh2-rs `MemberStatus::member_status` / `member_ap_resistances`: plus the equipment's rows.
        let (g, ap, weakness) = crate::equipment::gear(&data.items, &data.item_stats, &self.equipment);
        kh2_enemy_host::data::Stats { level: i32::from(self.level), max_hp: self.hp_max, strength: strength + g[0], magic: magic + g[1], defense: defense + g[2], ap, max_taken: i32::MAX, min_taken: 1, max_dealt: i32::MAX, weakness, ..Default::default() }
    }

    fn vital(&self) -> battle::Vital {
        battle::Vital { hp: self.hp, hp_max: self.hp_max, mp: self.mp, mp_max: self.mp_max }
    }

    /// FUN_00186238 through kh2-rs `hp_change`; a knock-out starts the revive timer (FUN_0017ce60).
    pub fn change_hp(&mut self, delta: i32, data: &PartyData) -> kh2_game_core::party_hp::HpChange {
        let scale = if data.sstm.is_some() { data.sstm_f32(0x54) } else { 1.0 };
        let c = kh2_game_core::party_hp::hp_change(&mut self.hp, self.hp_max, 0, &mut self.ko, delta, scale);
        if c.knocked_out {
            self.revive = battle::revive_time(data.members[self.member].stun, 1.0);
            self.attack = None;
            self.held = None;
            self.events.push(MemberEvent::KnockedOut);
        }
        if c.revived {
            self.events.push(MemberEvent::Revived { hp: self.hp });
        }
        c
    }

    pub fn matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(Quat::from_rotation_y(self.motor.yaw), self.motor.pos)
    }

    /// The weapon's world matrix (the hand bone of the drawn pose).
    pub fn weapon_matrix(&self, data: &PartyData) -> Mat4 {
        self.matrix() * self.body.pose.get(data.members[self.member].hand).copied().unwrap_or(Mat4::IDENTITY)
    }
}

/// kh2-rs `party.rs::relation`: the game's height difference, the ground direction and distance.
fn relation(from: Vec3, to: Vec3) -> (f32, Vec3, f32) {
    let d = to - from;
    (-(to.y - from.y), d.with_y(0.0).normalize_or_zero(), d.with_y(0.0).length())
}

/// The game's generator word the members' choices draw from (kh2-rs `rng`'s process-wide word, the
/// boot seed at first). libkh2 keeps one per party so each Sora's battle replays the same whatever
/// else runs in the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PartyRng(pub u64);

impl Default for PartyRng {
    fn default() -> PartyRng {
        PartyRng(kh2_game_core::rng::Lcg::boot().0)
    }
}

/// One tick of the followers in the field (no enemies): kh2-rs `party.rs`'s follow path.
pub fn tick(data: &PartyData, followers: &mut [Follower], rng: &mut PartyRng, sora: &mut crate::sora::Sora, world: &World) {
    tick_battle(data, followers, rng, sora, world, None);
}

/// One tick of the followers (after Sora's) with the battle part when `enemies` is given and one is
/// within the style's battle distance of Sora.
pub fn tick_battle(data: &PartyData, followers: &mut [Follower], rng: &mut PartyRng, sora: &mut crate::sora::Sora, world: &World, enemies: Option<&mut crate::enemy::Enemies>) {
    let _stream = kh2_game_core::rng::stream(0);
    kh2_game_core::rng::set_state(rng.0);
    tick_inner(data, followers, sora, world, enemies);
    rng.0 = kh2_game_core::rng::state();
}

/// The item's effect on a status block (kh2-rs `item_effect`, multiplier 100: the members have no
/// Item Boost here).
fn item_on(data: &PartyData, item: u16, v: &mut battle::Vital) -> bool {
    let Some(row) = data.items.iter().find(|i| i.id == item) else { return false };
    let amount = row.flag1 as u16 | (row.rank as u16) << 8;
    let mut x = kh2_game_core::items::Vitals { hp: v.hp, hp_max: v.hp_max, hp_min: 0, mp: v.mp, mp_max: v.mp_max, ..Default::default() };
    let ok = kh2_game_core::items::item_effect(item, amount, 100, &mut x, 0.0);
    v.hp = x.hp;
    v.mp = x.mp;
    ok
}

fn tick_inner(data: &PartyData, followers: &mut [Follower], sora: &mut crate::sora::Sora, world: &World, mut enemies: Option<&mut crate::enemy::Enemies>) {
    let game = sora.game.clone();
    let params = &game.params;
    let (player, player_yaw, fall_max, pose) = (sora.motor.pos, sora.motor.yaw, params.sstm.fall_max, sora.pose);
    let c = &data.consts;
    let mut inputs = [None; 2];
    for (i, f) in followers.iter().take(2).enumerate() {
        let m = &data.members[f.member];
        inputs[i] = Some((f.motor.pos, rules::follow_distance(&m.thresholds, m.radius, c)));
    }
    let points = rules::follow_points(player, player_yaw, inputs, c);
    // The enemies' hurt shapes (centre, radius) by id: the members' targets.
    let foes: Vec<(u32, Vec<(Vec3, f32)>)> = enemies
        .as_ref()
        .map(|en| en.list.iter().filter(|e| e.alive() && e.data.kind.status).map(|e| (e.id, crate::enemy::hurt_volumes(&e.data.rig, &e.anim.pose, e.matrix()).into_iter().map(|v| ((v.a + v.b) * 0.5, v.r)).collect())).collect())
        .unwrap_or_default();
    // Sora's target as an object index (style 1's `0x34e9b0`): the enemy nearest his target point.
    let player_target = sora.target.and_then(|t| foes.iter().enumerate().flat_map(|(fi, (_, s))| s.iter().enumerate().map(move |(si, &(ct, _))| ((fi, si), ct.distance(t.pos)))).min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0));
    let vitals: Vec<battle::Vital> = followers.iter().map(Follower::vital).collect();
    // The scripts' victims (the enemies' hurt volumes by id) and the target parts host 6.5 walks.
    let victims: Vec<(u32, Vec<crate::enemy::Volume>)> = enemies.as_ref().map(|en| en.list.iter().filter(|e| e.alive() && !e.removed && e.data.kind.status).map(|e| (e.id, crate::enemy::hurt_volumes(&e.data.rig, &e.anim.pose, e.matrix()))).collect()).unwrap_or_default();
    let targets: Vec<kh2_enemy_host::friend_host::SearchPart> = enemies
        .as_ref()
        .map(|en| {
            en.list
                .iter()
                .filter(|e| e.alive() && !e.removed && e.data.kind.status)
                .flat_map(|e| {
                    let vols = crate::enemy::hurt_volumes(&e.data.rig, &e.anim.pose, e.matrix());
                    let shapes: Vec<_> = e.data.rig.model.shapes.iter().filter(|s| s.kind == kh2_model::collision::kind::BATTLE || s.kind == kh2_model::collision::kind::TARGET).collect();
                    shapes
                        .into_iter()
                        .zip(vols)
                        .enumerate()
                        .filter(|(_, (s, _))| s.kind == kh2_model::collision::kind::TARGET)
                        .map(|(i, (s, v))| kh2_enemy_host::friend_host::SearchPart { entity: kh2_enemy_host::Entity(u64::from(e.id)), shape: i, part: s.group, point: (v.a + v.b) * 0.5, radius: v.r })
                        .collect::<Vec<_>>()
                })
                .collect()
        })
        .unwrap_or_default();
    let n = followers.len();
    for i in 0..n {
        let (left, right) = followers.split_at_mut(i);
        let (f, right) = right.split_first_mut().unwrap();
        let mut other: Option<&mut Follower> = left.first_mut().or(right.first_mut());
        let m = &data.members[f.member];
        if f.invuln > 0.0 {
            f.invuln -= 1.0;
        }
        // A reaction or get-up motion that ended frees the member.
        if let Some(slot) = f.hurt
            && (f.body.finished || f.body.slot != Some(slot))
        {
            f.hurt = None;
        }
        let in_battle_now = !foes.is_empty();
        // FUN_0017ca70: the KO'd member's revive count-down; at 0 FUN_0017cc20 heals maxHP x sstm +0x58.
        if f.ko && battle::revive_step(&mut f.revive, 1.0, in_battle_now, data.sstm_f32(0x5c)) {
            let heal = battle::revive_heal(f.hp_max, data.sstm_f32(0x58)).max(1);
            if f.change_hp(heal, data).revived {
                let rig = f.body.rig.clone();
                let order = data.orders[battle::motion_state(in_battle_now, f.weapon.is_some())];
                let up = battle::remap(MOTION_GET_UP, order, |s| rig.mset.has(s));
                if up.is_some() {
                    f.body.restart(up, false, 4.0);
                    f.hurt = up;
                }
            }
        }
        if f.motor.pos.distance(player) > WARP_DISTANCE {
            f.motor = Motor { pos: points.get(i).copied().flatten().unwrap_or(player), grounded: true, ..Motor::default() };
            f.attack = None;
        }
        let pos = f.motor.pos;
        let to_player = player - pos;
        let ground_dir = to_player.with_y(0.0).normalize_or_zero();
        let facing = f.motor.forward();
        let ai = data.ai.get(&m.class);
        let in_battle = ai.is_some_and(|a| !foes.is_empty() && foes.iter().any(|(_, s)| s.iter().any(|(ct, _)| ct.distance(player) < battle::battle_distance(a.params.battle, f.style, data.half) + a.params.range)));
        let order = data.orders[battle::motion_state(in_battle, f.weapon.is_some())];
        if f.timer > 0.0 {
            f.timer -= 1.0;
        }
        // KO'd: lie in motion 0x36 (FUN_0017e2d8), nothing else.
        if f.ko {
            let rig = f.body.rig.clone();
            let down = battle::remap(MOTION_DOWN, order, |s| rig.mset.has(s));
            if f.hurt.is_none() && f.body.slot != down && down.is_some() {
                f.body.restart(down, false, 4.0);
            }
        }
        let busy = f.script_busy(ai) || f.limit_hold;
        let free = f.attack.is_none() && f.hurt.is_none() && !f.ko && !busy;
        // The MP charge runs out into full MP (kh2-rs `Charge::tick`).
        if f.charge.active() && f.charge.tick(1.0) {
            f.mp = f.mp_max;
        }
        let mut delta = Vec3::ZERO;
        let mut start: Option<(Option<usize>, battle::Entry)> = None;
        let mut follow_up: Option<battle::FollowUp> = None;
        if free {
            let point = points.get(i).copied().flatten().unwrap_or(player);
            let follow_player = |fo: &mut Follower, row: usize| {
                let input = FollowIn { to_point: point - pos, to_player: to_player.normalize_or_zero(), dist: to_player.length(), ground_dir, facing, walk: m.walk, run: m.run };
                rules::follow_step_row(&mut fo.follow, &input, &m.thresholds, row, c)
            };
            let (dir, speed);
            f.want_yaw = None;
            if !in_battle {
                f.target = None;
                (dir, speed) = follow_player(f, rules::ROW_FIELD);
            } else {
                let ai = ai.unwrap();
                // The target (style 0, FUN_0010dcf0 / FUN_0010cf90 via kh2-rs `refresh_target`).
                let cands: Vec<battle::Cand> = foes.iter().enumerate().flat_map(|(fi, (_, shapes))| shapes.iter().enumerate().map(move |(si, &(centre, radius))| battle::Cand { fighter: fi, shape: si, centre, radius, enabled: true, targetable: true })).collect();
                let index = |id: u32| foes.iter().position(|x| x.0 == id);
                let scene = battle::TargetScene { member: pos, player, attacker: None, player_target, player_hit: None, cands: &cands, held: &[], attack_gate: false, previous: f.target.and_then(|(id, s)| Some((index(id)?, s))) };
                let shape_of = |t: (u32, usize)| foes.iter().find(|x| x.0 == t.0).and_then(|x| x.1.get(t.1).copied());
                let too_far = battle::battle_distance(ai.params.battle, f.style, data.half) < relation(pos, player).2;
                if let Some((chooser, test)) = battle::dispatch(m.class, f.style) {
                    let now = f.target.filter(|_| !too_far).and_then(|t| Some(((index(t.0)?, t.1), shape_of(t)?.0)));
                    let counted = f.choosing.current.and_then(|k| ai.records.get(k)).is_some_and(|r| r.flags & battle::flag::NOT_COUNTED == 0);
                    if battle::needs_retarget(test, false, now, counted, &ai.params, &scene) {
                        f.target = battle::choose_target(chooser, false, &ai.params, &scene).map(|(fi, si)| (foes[fi].0, si));
                    }
                }
                let tshape = f.target.and_then(shape_of);
                let (tdy, tdir, tgd) = tshape.map_or((0.0, Vec3::ZERO, 0.0), |(ct, _)| relation(pos, ct));
                let target_ok = tshape.is_some() && !too_far;
                // FUN_00110f90: the MP gate of this frame; FUN_0010f830: the member in danger.
                let roll = kh2_game_core::rng::friend_roll();
                let player_v = battle::Vital { hp: sora.hp, hp_max: sora.max_hp, mp: sora.mp, mp_max: sora.max_mp };
                let me_v = f.vital();
                let other_v = vitals.iter().enumerate().find(|(k, _)| *k != i).map(|(_, v)| *v);
                let danger = battle::danger_member(player_v, me_v, other_v, false);
                let mp_gate = danger.is_some() || battle::mp_gate(roll, ai.params.mp_chance, f.mp, f.mp_max, data.mp_gate_w.0, data.mp_gate_w.1);
                let now = MemberNow { standing: f.motor.grounded, free: true, status: true, mp: f.mp, mp_gate, mp_charge: f.charge.active(), target_ok, dy: tdy, gd: tgd, facing_dot: facing.dot(tdir), radius: tshape.map_or(0.0, |s| s.1) };
                let equipped = |r: &battle::Record| f.abilities.contains(&(r.item as u16));
                let has_script = f.brain.is_some();
                // FUN_00113d48 phase 1: a mode to switch to, the script free to take it, FUN_0010f220.
                let special_ok = |r: &battle::Record| !r.mode.is_empty() && has_script && battle::feasible(r, &now, Some(0), equipped(r));
                let stock: Vec<u16> = f.items.iter().filter(|x| x.1 > 0).map(|x| x.0).collect();
                let item_choice = |h: Handler| -> Option<(battle::Who, u16)> {
                    let (frac, lists, value): (f32, &ItemLists, VitalPart) = match h {
                        Handler::HpItem => (data.item_frac.0, &data.hp_items, |v| (v.hp, v.hp_max)),
                        _ => (data.item_frac.1, &data.mp_items, |v| (v.mp, v.mp_max)),
                    };
                    battle::item_pick(danger, me_v, player_v, other_v, frac, value, &lists.0, &lists.1, |_| 0, |it| now.standing && stock.contains(&it))
                };
                let toward = |fo: &mut Follower| {
                    if target_ok {
                        battle::approach_step(&mut fo.follow, tdir, tgd, tdy, &m.thresholds, m.walk, m.run, data.stop)
                    } else {
                        follow_player(fo, rules::ROW_BATTLE_PLAYER)
                    }
                };
                let skip = battle::repeated(f.choosing.last, m.class);
                let any_attack = |skip: Option<usize>| ai.records.iter().enumerate().any(|(k, r)| r.flags & battle::flag::ATTACK != 0 && Some(k) != skip && r.handler == Handler::Attack && battle::feasible(r, &now, Some(0), false));
                let (current, timer) = (f.choosing.current, f.timer);
                let can = |k: usize, cont: bool| {
                    let r = &ai.records[k];
                    let waited = timer > 0.0;
                    match r.handler {
                        Handler::Attack => battle::feasible(r, &now, Some(0), false),
                        Handler::Approach => (if cont { waited } else { current != Some(k) }) && !any_attack(skip),
                        Handler::Wait => (!cont || waited) && !any_attack(skip),
                        Handler::GoofyWait => (!cont || waited) && target_ok,
                        Handler::HpItem | Handler::MpItem => battle::feasible(r, &now, Some(0), equipped(r)) && item_choice(r.handler).is_some(),
                        Handler::Special => special_ok(r),
                        // FUN_00113e68: as the special, when someone's HP is below the entry's +0x3c.
                        Handler::Cure => special_ok(r) && r.entry.is_some_and(|e| battle::cure_target(danger, me_v, player_v, other_v, e.cure).is_some()),
                        _ => false,
                    }
                };
                let chain = f.motor.grounded && target_ok;
                let mut choosing = std::mem::take(&mut f.choosing);
                // FUN_0010f8d8 first when someone is in danger, else FUN_0010fcd0.
                let urgent = if danger.is_some() { battle::emergency(&ai.records, &mut choosing, true, false, |k| can(k, false), kh2_game_core::rng::friend_rand) } else { None };
                let choice = match urgent {
                    Some(k) => Choice::Do(k),
                    None => battle::choose(&ai.records, &mut choosing, &ai.params, m.class, &now, false, chain, |_| Some(0), can, kh2_game_core::rng::friend_roll, kh2_game_core::rng::friend_rand),
                };
                f.choosing = choosing;
                if let Choice::FollowUp(k) = choice {
                    follow_up = ai.followups.get(k as usize).copied();
                }
                let continuing = matches!(choice, Choice::Keep(_));
                let record = match choice {
                    Choice::Keep(k) | Choice::Do(k) => Some(k),
                    _ => None,
                };
                match record.map(|k| &ai.records[k]) {
                    Some(r) => match r.handler {
                        Handler::Attack => {
                            (dir, speed) = toward(f);
                            if let Some(e) = r.entry {
                                start = Some((record, e));
                            }
                        }
                        Handler::Approach => {
                            if !continuing {
                                f.timer = kh2_game_core::rng::range(ai.params.wait_a.0, ai.params.wait_a.1);
                            }
                            if target_ok {
                                f.want_yaw = Some(tdir.x.atan2(tdir.z));
                            }
                            (dir, speed) = (facing, 0.0);
                        }
                        Handler::Wait => {
                            if !continuing {
                                f.timer = kh2_game_core::rng::range(ai.params.wait_b.0, ai.params.wait_b.1);
                            }
                            (dir, speed) = toward(f);
                        }
                        Handler::Special | Handler::Cure => {
                            // FUN_00113d48 phase 0: the target kept, +0xaf4 the entry's MP byte, the
                            // script's mode switched to the record's name.
                            (dir, speed) = (facing, 0.0);
                            f.special_who = if r.handler == Handler::Cure { r.entry.and_then(|e| battle::cure_target(danger, me_v, player_v, other_v, e.cure)).map(SpecialWho::Party) } else { f.target.map(|t| SpecialWho::Enemy(t.0)) };
                            f.mode_request = Some((r.mode.clone(), r.entry.map_or(0, |e| e.mp as u8)));
                        }
                        Handler::HpItem | Handler::MpItem => {
                            (dir, speed) = (facing, 0.0);
                            if let Some((who, item)) = item_choice(r.handler) {
                                // decided: the effect now (the game plays the item motion first).
                                let used = match who {
                                    battle::Who::Player => sora.apply_item_mult(item, 100),
                                    battle::Who::Me => {
                                        let mut v = f.vital();
                                        let ok = item_on(data, item, &mut v);
                                        f.mp = v.mp;
                                        f.change_hp(v.hp - f.hp, data);
                                        ok
                                    }
                                    battle::Who::Other => match other.as_mut() {
                                        Some(o) => {
                                            let mut v = o.vital();
                                            let ok = item_on(data, item, &mut v);
                                            o.mp = v.mp;
                                            o.change_hp(v.hp - o.hp, data);
                                            ok
                                        }
                                        None => false,
                                    },
                                };
                                if used && let Some(s) = f.items.iter_mut().find(|x| x.0 == item && x.1 > 0) {
                                    s.1 -= 1;
                                    let w = match who {
                                        battle::Who::Player => 0,
                                        battle::Who::Me => 1,
                                        battle::Who::Other => 2,
                                    };
                                    f.events.push(MemberEvent::ItemUsed { item, who: w });
                                }
                            }
                        }
                        Handler::GoofyWait => {
                            if continuing && let Some(h) = f.held {
                                (dir, speed) = h;
                            } else {
                                f.timer = kh2_game_core::rng::range(ai.params.wait_a.0, ai.params.wait_a.1);
                                (dir, speed) = toward(f);
                                f.held = Some((dir, m.run));
                            }
                        }
                        _ => (dir, speed) = toward(f),
                    },
                    None if matches!(choice, Choice::Stand) => {
                        let (d, _) = toward(f);
                        (dir, speed) = (d, 0.0);
                        if d != Vec3::ZERO {
                            f.want_yaw = Some(d.x.atan2(d.z));
                        }
                    }
                    None => (dir, speed) = toward(f),
                }
            }
            let (d, sp) = rules::steer(&mut f.follow, dir, speed, m.walk, m.run, c);
            f.speed = sp;
            delta = d.with_y(0.0).normalize_or_zero() * sp;
        }
        // What the choice started (kh2-rs `Start::Attack`, `Start::FollowUp`).
        let started = start.is_some() || follow_up.is_some();
        if let Some((rec, e)) = start {
            let rig = f.body.rig.clone();
            if let Some(slot) = battle::remap(e.motion, order, |s| rig.mset.has(s)) {
                f.body.restart(Some(slot), false, BLEND);
                f.attack = Some(Attacking { record: rec, slot, until_landed: false });
                f.landed.clear();
                f.speed = 0.0;
                delta = Vec3::ZERO;
                if let Some((ct, _)) = f.target.and_then(|t| foes.iter().find(|x| x.0 == t.0).and_then(|x| x.1.get(t.1).copied())) {
                    let (dy, gdir, _) = relation(f.motor.pos, ct);
                    f.motor.yaw = gdir.x.atan2(gdir.z);
                    if e.air != 0.0 {
                        let h = (e.air - dy).clamp(0.0, m.jump);
                        f.motor.start_jump(h);
                        f.held = Some((gdir, m.run));
                    } else {
                        f.held = None;
                    }
                }
            }
        } else if let Some(fu) = follow_up {
            let rig = f.body.rig.clone();
            if let Some(slot) = battle::remap(kh2_anim::id::JUMP as i32, order, |s| rig.mset.has(s)) {
                let dir = battle::followup_dir(f.motor.yaw, &fu, std::f32::consts::PI / 180.0);
                f.body.restart(Some(slot), false, 0.0);
                f.attack = Some(Attacking { record: None, slot, until_landed: true });
                f.landed.clear();
                f.speed = 0.0;
                delta = Vec3::ZERO;
                f.motor.start_leap(fu.height);
                f.held = Some((dir, fu.speed));
            }
        }
        // The script's own motion while it runs a special (kh2-rs `party_tick`: `script_slot` played
        // while the member stands, its movement and facing).
        if busy && !f.limit_hold && f.attack.is_none() && f.hurt.is_none() && !f.ko {
            if let Some(slot) = f.script_slot.filter(|_| std::mem::take(&mut f.script_restart)) {
                let looping = f.body.rig.mset.get(slot).and_then(|s| s.motion.as_ref()).is_some_and(|x| x.frame_return < x.frame_end);
                f.body.restart(Some(slot), looping, BLEND);
            } else if f.body.finished
                && let Some(q) = f.script_queue.take()
            {
                f.script_slot = Some(q);
                f.body.restart(Some(q), false, BLEND);
            }
            if let Some(v) = f.script_move.take() {
                delta = v.with_y(0.0);
            }
            if let Some(y) = f.script_yaw.take() {
                f.motor.yaw = y;
            }
        }
        // A running action: root motion, the jump's run, free again once it ended on the ground.
        if let Some(at) = f.attack
            && !started
        {
            let (root_t, root_yaw) = f.body.root_delta;
            f.motor.yaw = kh2_game_core::motor::wrap_angle(f.motor.yaw + root_yaw);
            delta = (Quat::from_rotation_y(f.motor.yaw - f.body.root.yaw) * root_t).with_y(0.0);
            if !f.motor.grounded
                && let Some((d, sp)) = f.held
            {
                delta += d * sp;
            }
            let over = if at.until_landed { f.motor.grounded } else { f.body.finished || f.body.slot != Some(at.slot) };
            if over && f.motor.grounded {
                f.attack = None;
                f.held = None;
            }
        }
        let airborne = (!f.motor.grounded).then(|| f.motor.jump_dy(fall_max));
        f.motor.move_by(world, delta, airborne);
        if f.attack.is_none() && f.hurt.is_none() && !f.ko && !busy {
            if f.follow.turning {
                f.motor.turn_to(ground_dir, m.turn);
            } else if let Some(y) = f.want_yaw {
                f.motor.turn_to(Vec3::new(y.sin(), 0.0, y.cos()), m.turn);
            } else if delta != Vec3::ZERO {
                f.motor.turn_to(delta, m.turn);
            }
            let rig = f.body.rig.clone();
            let motion = if f.speed <= 0.0 { kh2_anim::id::IDLE } else if f.speed < m.run { kh2_anim::id::WALK } else { kh2_anim::id::RUN };
            f.body.play(battle::remap(motion as i32, order, |s| rig.mset.has(s)), true, BLEND);
        }
        f.body.step(pose);
        if let Some(w) = f.weapon.as_mut() {
            w.follow(&f.body);
            if pose {
                w.pose_now();
            }
        }
        // The member's own script (kh2-rs `friend_update`), after the body's step.
        let to_m = Mat4::from_scale(Vec3::splat(kh2_enemy_host::SCALE));
        let hand_root = f.matrix() * f.body.pose.get(m.hand).copied().unwrap_or(Mat4::IDENTITY);
        let hand_obj = f.weapon.as_ref().map(|wa| (wa.pose.iter().map(|b| to_m * hand_root * *b).collect::<Vec<_>>(), to_m * hand_root));
        let (member, mpos, myaw) = (f.member, f.motor.pos, f.motor.yaw);
        if let Some(brain) = f.brain.as_mut()
            && !f.ko
        {
            let sw = crate::friend::ScriptWorld { player, player_yaw, world, in_battle: in_battle_now, victims: &victims, targets: &targets, atkp: &data.atkp, free: data.free.as_ref() };
            brain.fields.free = f.attack.is_none() && f.hurt.is_none();
            brain.fields.target_valid = f.target.is_some() || f.special_who.is_some();
            let tp = f.target.and_then(|t| foes.iter().find(|x| x.0 == t.0).and_then(|x| x.1.get(t.1).copied()));
            brain.fields.target_point = tp.map(|x| x.0);
            brain.fields.target_radius = tp.map_or(0.0, |x| x.1);
            brain.fields.special = f.special_who.map(|w| match w {
                SpecialWho::Enemy(id) => kh2_enemy_host::party::SpecialOn::Enemy(kh2_enemy_host::Entity(u64::from(id))),
                SpecialWho::Party(battle::Who::Player) => kh2_enemy_host::party::SpecialOn::Player,
                SpecialWho::Party(battle::Who::Me) => kh2_enemy_host::party::SpecialOn::Me,
                SpecialWho::Party(battle::Who::Other) => kh2_enemy_host::party::SpecialOn::Other,
            });
            // The hand-0 object (host 1.234): its drawn bones and root in kh2-rs's Bevy space (metres).
            match hand_obj {
                Some((bones, root)) => {
                    let wobj = brain.fields.weapon.get_or_insert_with(|| kh2_enemy_host::friend_host::WeaponObj { handle: kh2_enemy_host::friend_host::WEAPON_HANDLE + member as u32, linked: true, pos: mpos, yaw: myaw, vel: Vec3::ZERO, motion: None, bones: Vec::new(), root: Mat4::IDENTITY });
                    wobj.bones = bones;
                    wobj.root = root;
                }
                None => brain.fields.weapon = None,
            }
            let req = f.mode_request.take();
            let out = crate::friend::update(brain, &f.body, f.motor.pos, f.motor.yaw, order, ai, req, &mut f.trace, &sw);
            if let Some(s) = out.slot {
                f.script_slot = Some(s);
                f.script_restart = true;
                f.script_queue = None;
            }
            if out.queued.is_some() {
                f.script_queue = out.queued;
            }
            f.script_move = out.moved;
            f.script_yaw = out.yaw;
            if let Some(t) = out.invuln {
                f.invuln = t;
            }
            if let Some(cost) = out.paid {
                f.pay_mp(cost, m.mp_drive);
            }
            if let Some(found) = out.searched {
                f.target = found;
            }
            if out.mode != f.script_mode {
                if let Some(md) = &out.mode {
                    f.events.push(MemberEvent::ScriptMode { mode: md.clone() });
                }
                f.script_mode = out.mode;
            }
            for id in out.effects {
                f.events.push(MemberEvent::Effect { id });
            }
            // Hosts 2.10 / 2.17 hits (kh2-rs `party_script_hits` and the attack objects' hits).
            let caster = f.stats(data);
            let mut hits: Vec<(u32, kh2_tables::tables::Atkp)> = out.object_hits.iter().filter_map(|h| Some((h.enemy, data.atkp.atkp.get(h.row)?.clone()))).collect();
            for id in out.target_hits {
                let Some(atk) = params.atkp(id).cloned() else { continue };
                match f.special_who {
                    Some(SpecialWho::Enemy(t)) => hits.push((t, atk)),
                    Some(SpecialWho::Party(who)) if matches!(atk.r#type, 5 | 6) => {
                        let amount = match who {
                            battle::Who::Player => {
                                let to = kh2_enemy_host::data::Stats { max_hp: sora.max_hp, ..caster };
                                let a = crate::enemy::damage(&atk, &caster, &to);
                                sora.hp = (sora.hp + a).clamp(0, sora.max_hp);
                                a
                            }
                            battle::Who::Me => {
                                let a = crate::enemy::damage(&atk, &caster, &caster);
                                f.change_hp(a, data);
                                a
                            }
                            battle::Who::Other => match other.as_mut() {
                                Some(o) => {
                                    let to = o.stats(data);
                                    let a = crate::enemy::damage(&atk, &caster, &to);
                                    o.change_hp(a, data);
                                    a
                                }
                                None => 0,
                            },
                        };
                        f.events.push(MemberEvent::Healed { who: match who { battle::Who::Player => 0, battle::Who::Me => 1, battle::Who::Other => 2 }, amount });
                    }
                    _ => {}
                }
            }
            if let Some(en) = enemies.as_mut() {
                for (id, atk) in hits {
                    let Some(e) = en.list.iter_mut().find(|e| e.id == id && e.alive() && !e.removed) else { continue };
                    let dmg = crate::enemy::damage(&atk, &caster, &e.stats);
                    let dir = (e.pos() - f.motor.pos).with_y(0.0).normalize_or_zero();
                    if let Some(r) = game.params.reaction.as_ref() {
                        crate::enemy::Enemies::receive(e, &atk, dmg, dir, r);
                    }
                    let (eid, hp) = (e.id, e.hp);
                    en.events.push(crate::enemy::EnemyEvent::Hit { id: eid, atkp: atk.id as i16, damage: dmg, hp });
                    if hp <= 0 && !e.defeated {
                        e.defeated = true;
                        let (table, row) = en.data.experience(e.enmp, en.world_level);
                        en.events.push(crate::enemy::EnemyEvent::Defeated { id: eid, enmp: e.enmp, lvpm_experience: table, enmp_experience: row, paid: 0 });
                    }
                }
            }
            // The special is over when the script left its mode.
            if !f.script_busy(ai) && f.mode_request.is_none() {
                f.special_who = None;
            }
        }
        // The enemies' live attacks against this member (kh2-rs `party_receive_hits`): not while KO'd
        // or invincible.
        if let Some(en) = enemies.as_mut()
            && !f.ko
            && f.invuln <= 0.0
            && let Some(reaction) = params.reaction.as_ref()
        {
            let hurt = crate::enemy::hurt_volumes(&m.rig, &f.body.pose, f.matrix());
            let hurt = if hurt.is_empty() { vec![crate::enemy::Volume { a: f.motor.pos + Vec3::Y * m.radius, b: f.motor.pos + Vec3::Y * 90.0, r: m.radius }] } else { hurt };
            let to = f.stats(data);
            f.hit_by.retain(|k| en.list.iter().any(|e| e.id == k.0 && e.anim.slot == k.1));
            for e in en.list.iter().filter(|e| e.alive() && !e.removed) {
                for (atkp, start, v) in crate::enemy::live_attacks(&e.anim, e.matrix()) {
                    let key = (e.id, e.anim.slot, start);
                    if f.ko || f.hit_by.contains(&key) || !hurt.iter().any(|h| crate::enemy::overlap(&v, h)) {
                        continue;
                    }
                    f.hit_by.insert(key);
                    let Some(atk) = params.atkp(atkp) else { continue };
                    let useless = kh2_game_core::reaction_rules::is_ineffective(to.weakness[(atk.element as usize).min(6)], to.weakness[6]);
                    let dmg = if useless { crate::enemy::damage(atk, &e.stats, &to) } else { crate::enemy::damage(atk, &e.stats, &to).max(1) };
                    let ko = f.change_hp(-dmg, data).knocked_out;
                    f.events.push(MemberEvent::Hit { by: e.id, atkp, damage: dmg, hp: f.hp });
                    let dir = (f.motor.pos - e.pos()).with_y(0.0).normalize_or_zero();
                    let from_front = f.motor.forward().dot(-dir) > 0.0;
                    use kh2_game_core::reaction_rules::{self as rr, VictimAction, VictimCtx};
                    let ctx = VictimCtx { status: 0, grounded: f.motor.grounded, attacker_flag_4000: false, from_front, hit_part_plain: true };
                    let out = rr::victim_outcome(&rr::HitRow::from_atkp(atk), useless, &ctx, &reaction.patn, &reaction.defaults, &reaction.consts);
                    let motion = match out.action {
                        VictimAction::Flinch { motion, fallback } | VictimAction::FlinchPush { motion, fallback, .. } => Some((motion as i32, Some(fallback as i32))),
                        VictimAction::Class14 { motion, .. } | VictimAction::Blow { motion, .. } => Some((motion as i32, None)),
                        _ => None,
                    };
                    if let Some((mo, fb)) = motion.filter(|_| !ko) {
                        let rig = f.body.rig.clone();
                        let slot = battle::remap(mo, order, |s| rig.mset.has(s)).or_else(|| fb.and_then(|x| battle::remap(x, order, |s| rig.mset.has(s))));
                        if slot.is_some() {
                            f.body.restart(slot, false, 2.0);
                            f.hurt = slot;
                            f.attack = None;
                            f.held = None;
                            f.landed.clear();
                        }
                    }
                }
            }
        }
        // The member's live attacks against the enemies (kh2-rs `combat.rs` for the party's fighters).
        if let Some(en) = enemies.as_mut() {
            let mut attacks = crate::enemy::live_attacks(&f.body, f.matrix());
            if let Some(w) = &f.weapon {
                attacks.extend(crate::enemy::live_attacks(w, f.weapon_matrix(data)));
            }
            for (atkp, startf, v) in attacks {
                for e in en.list.iter_mut().filter(|e| e.alive() && !e.removed && e.data.kind.status) {
                    if f.landed.contains(&(e.id, startf)) || !crate::enemy::hurt_volumes(&e.data.rig, &e.anim.pose, e.matrix()).iter().any(|h| crate::enemy::overlap(&v, h)) {
                        continue;
                    }
                    let Some(atk) = params.atkp(atkp).cloned() else { continue };
                    f.landed.insert((e.id, startf));
                    let dmg = crate::enemy::damage(&atk, &f.stats(data), &e.stats);
                    let dir = (e.pos() - f.motor.pos).with_y(0.0).normalize_or_zero();
                    if let Some(r) = game.params.reaction.as_ref() {
                        crate::enemy::Enemies::receive(e, &atk, dmg, dir, r);
                    }
                    let (id, hp) = (e.id, e.hp);
                    en.events.push(crate::enemy::EnemyEvent::Hit { id, atkp, damage: dmg, hp });
                    if hp <= 0 && !e.defeated {
                        e.defeated = true;
                        let (table, row) = en.data.experience(e.enmp, en.world_level);
                        // A member's defeat pays no experience to Sora here (kh2-rs `by_player` false);
                        // the event carries the values for the host.
                        en.events.push(crate::enemy::EnemyEvent::Defeated { id, enmp: e.enmp, lvpm_experience: table, enmp_experience: row, paid: 0 });
                    }
                }
            }
        }
    }
}
