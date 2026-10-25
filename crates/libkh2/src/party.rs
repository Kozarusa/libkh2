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
//! decided: no member scripts (the specials, Cure and the item handlers are never feasible), no
//! limits, revive, warp through the trail (a member beyond `WARP_DISTANCE` is put back beside Sora),
//! the members are not hit by enemies; their stats are the level 1 rows of their `lvup` tables, their
//! weapons the base items (Mage's Staff 75, Knight's Shield 49, as kh2-rs's harness aid).

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
    pub stats: kh2_enemy_host::data::Stats,
}

/// The party tables (kh2-rs `PartyData`, the parts the follow and the battle subset use).
pub struct PartyData {
    pub consts: Consts,
    pub members: Vec<MemberData>,
    pub ai: HashMap<u8, ClassAi>,
    pub orders: [[u8; 4]; 4],
    pub half: f32,
    pub stop: (f32, f32),
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
            // decided: the level 1 row of the member's `lvup` table (character index = NeoStatus - 1).
            let l = lvup.get((row.neo_status as usize).saturating_sub(1)).and_then(|t| t.first());
            let stats = kh2_enemy_host::data::Stats {
                level: 1,
                max_hp: 20,
                strength: l.map_or(2, |l| l.strength as i32),
                magic: l.map_or(2, |l| l.magic as i32),
                defense: l.map_or(2, |l| l.defense as i32),
                max_taken: i32::MAX,
                min_taken: 1,
                max_dealt: i32::MAX,
                weakness: [100; 7],
                ..Default::default()
            };
            members.push(MemberData {
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
                stats,
                rig,
            });
        }
        Ok(PartyData { consts, members, ai, orders, half, stop })
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
        }
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

/// One tick of the followers in the field (no enemies): kh2-rs `party.rs`'s follow path.
pub fn tick(data: &PartyData, followers: &mut [Follower], player: Vec3, player_yaw: f32, world: &World, fall_max: f32, pose: bool) {
    tick_battle(data, followers, player, player_yaw, world, fall_max, pose, None);
}

/// One tick of the followers with the battle subset when `enemies` is given and one is within
/// the class's battle distance of Sora.
#[allow(clippy::too_many_arguments)]
pub fn tick_battle(data: &PartyData, followers: &mut [Follower], player: Vec3, player_yaw: f32, world: &World, fall_max: f32, pose: bool, mut enemies: Option<(&mut crate::enemy::Enemies, &crate::data::Params)>) {
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
        .map(|(en, _)| en.list.iter().filter(|e| e.alive() && e.data.kind.status).map(|e| (e.id, crate::enemy::hurt_volumes(&e.data.rig, &e.anim.pose, e.matrix()).into_iter().map(|v| ((v.a + v.b) * 0.5, v.r)).collect())).collect())
        .unwrap_or_default();
    for (i, f) in followers.iter_mut().enumerate() {
        let m = &data.members[f.member];
        if f.motor.pos.distance(player) > WARP_DISTANCE {
            f.motor = Motor { pos: points.get(i).copied().flatten().unwrap_or(player), grounded: true, ..Motor::default() };
            f.attack = None;
        }
        let pos = f.motor.pos;
        let to_player = player - pos;
        let ground_dir = to_player.with_y(0.0).normalize_or_zero();
        let facing = f.motor.forward();
        let ai = data.ai.get(&m.class);
        let in_battle = ai.is_some_and(|a| !foes.is_empty() && foes.iter().any(|(_, s)| s.iter().any(|(ct, _)| ct.distance(player) < battle::battle_distance(a.params.battle, 0, data.half) + a.params.range)));
        let order = data.orders[battle::motion_state(in_battle, f.weapon.is_some())];
        if f.timer > 0.0 {
            f.timer -= 1.0;
        }
        let free = f.attack.is_none();
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
                let scene = battle::TargetScene { member: pos, player, attacker: None, player_target: None, player_hit: None, cands: &cands, held: &[], attack_gate: false, previous: f.target.and_then(|(id, s)| Some((index(id)?, s))) };
                let shape_of = |t: (u32, usize)| foes.iter().find(|x| x.0 == t.0).and_then(|x| x.1.get(t.1).copied());
                let too_far = battle::battle_distance(ai.params.battle, 0, data.half) < relation(pos, player).2;
                if let Some((chooser, test)) = battle::dispatch(m.class, 0) {
                    let now = f.target.filter(|_| !too_far).and_then(|t| Some(((index(t.0)?, t.1), shape_of(t)?.0)));
                    let counted = f.choosing.current.and_then(|k| ai.records.get(k)).is_some_and(|r| r.flags & battle::flag::NOT_COUNTED == 0);
                    if battle::needs_retarget(test, false, now, counted, &ai.params, &scene) {
                        f.target = battle::choose_target(chooser, false, &ai.params, &scene).map(|(fi, si)| (foes[fi].0, si));
                    }
                }
                let tshape = f.target.and_then(shape_of);
                let (tdy, tdir, tgd) = tshape.map_or((0.0, Vec3::ZERO, 0.0), |(ct, _)| relation(pos, ct));
                let target_ok = tshape.is_some() && !too_far;
                let now = MemberNow { standing: f.motor.grounded, free: true, status: true, mp: 0, mp_gate: false, mp_charge: false, target_ok, dy: tdy, gd: tgd, facing_dot: facing.dot(tdir), radius: tshape.map_or(0.0, |s| s.1) };
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
                        _ => false,
                    }
                };
                let chain = f.motor.grounded && target_ok;
                let mut choosing = std::mem::take(&mut f.choosing);
                let choice = battle::choose(&ai.records, &mut choosing, &ai.params, m.class, &now, false, chain, |_| Some(0), can, kh2_game_core::rng::friend_roll, kh2_game_core::rng::friend_rand);
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
        if f.attack.is_none() {
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
        // The member's live attacks against the enemies (kh2-rs `combat.rs` for the party's fighters).
        if let Some((en, params)) = enemies.as_mut() {
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
                    let dmg = crate::enemy::damage(&atk, &m.stats, &e.stats);
                    let dir = (e.pos() - f.motor.pos).with_y(0.0).normalize_or_zero();
                    if let Some(r) = params.reaction.as_ref() {
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
