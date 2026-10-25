//! Donald and Goofy as simple followers: kh2-rs's party rules (`party_rules.rs`, compiled in place:
//! follow points around the player, the follow step with the AI class's thresholds, the steer)
//! driven the way kh2-rs `kh2-party/src/party.rs` drives them in the field, without Bevy.
//!
//! decided: field following only (no battle AI, attacks, limits, revive or warp through the trail;
//! a member farther than `WARP_DISTANCE` is put back next to Sora). The `prty` row is the one of the
//! object's NeoMoveset (kh2-rs `PartyData::prty_row`, `pref_row` with fallback row 1).

use std::sync::Arc;

use glam::Vec3;
use kh2_anim::PlayerState;
use kh2_game_core::motor::Motor;
use kh2_game_core::party_rules::{self as rules, Consts, Follow, FollowIn, Thresholds};
use kh2_game_core::world::World;

use crate::anim::{Anim, Rig};
use crate::data::{Disc, LoadError};

/// Distance from Sora beyond which a follower is placed back beside him (app rule).
pub const WARP_DISTANCE: f32 = 3000.0;
const BLEND: f32 = 8.0;

/// The members libkh2 knows: (model name, AI class from `rules::ai_class` of its objentry row).
pub const MEMBERS: [&str; 2] = ["P_EX020", "P_EX030"];

/// What a follower is made of.
pub struct MemberData {
    pub model: String,
    pub rig: Arc<Rig>,
    pub thresholds: Thresholds,
    pub walk: f32,
    pub run: f32,
    pub turn: f32,
    pub radius: f32,
}

/// The party tables (kh2-rs `PartyData`, the parts the follow uses).
pub struct PartyData {
    pub consts: Consts,
    pub members: Vec<MemberData>,
}

impl Disc {
    /// Loads Donald and Goofy: their objentry rows, rigs, AI classes and `prty` rows, and the
    /// executable's follow constants.
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
        let mut members = Vec::new();
        for model in MEMBERS {
            let row = objs.iter().find(|o| o.model_name == model).ok_or_else(|| LoadError(format!("objentry: no {model}")))?;
            let class = rules::ai_class(row.neo_status, row.neo_moveset).unwrap_or(0);
            let rig = Arc::new(self.rig_named(model, row.animation_name.trim_end_matches(".mset"))?);
            // kh2-rs `party_battle::pref_row(offsets, moveset, 1)`.
            let k = if (row.neo_moveset as usize) < offsets.len() && offsets[row.neo_moveset as usize] != 0 { row.neo_moveset as usize } else { 1 };
            let p = prty.get(k);
            let radius = rig.model.shapes.iter().find(|s| s.kind == 0 || s.kind == 4).map_or(35.0, |s| s.radius.abs());
            members.push(MemberData {
                model: model.to_string(),
                thresholds: consts.thresholds(class),
                walk: p.map_or(0.0, |p| p.walk_speed),
                run: p.map_or(0.0, |p| p.run_speed),
                turn: p.map_or(0.0, |p| p.turn_speed),
                radius,
                rig,
            });
        }
        Ok(PartyData { consts, members })
    }
}

/// One follower.
pub struct Follower {
    pub member: usize,
    pub motor: Motor,
    pub follow: Follow,
    pub body: Anim,
}

impl Follower {
    pub fn new(data: &PartyData, member: usize, at: Vec3) -> Follower {
        let m = &data.members[member];
        Follower { member, motor: Motor { pos: at, grounded: true, ..Motor::default() }, follow: Follow::default(), body: Anim::new(m.rig.clone()) }
    }
}

/// One field tick of the followers of one player (kh2-rs `party.rs`: follow points, follow step,
/// steer, then the mover; the motion is idle / walk / run by the speed).
pub fn tick(data: &PartyData, followers: &mut [Follower], player: Vec3, player_yaw: f32, world: &World, fall_max: f32, pose: bool) {
    let c = &data.consts;
    let mut inputs = [None; 2];
    for (i, f) in followers.iter().take(2).enumerate() {
        let m = &data.members[f.member];
        inputs[i] = Some((f.motor.pos, rules::follow_distance(&m.thresholds, m.radius, c)));
    }
    let points = rules::follow_points(player, player_yaw, inputs, c);
    for (i, f) in followers.iter_mut().enumerate() {
        let m = &data.members[f.member];
        if f.motor.pos.distance(player) > WARP_DISTANCE {
            f.motor = Motor { pos: points.get(i).copied().flatten().unwrap_or(player), grounded: true, ..Motor::default() };
        }
        let point = points.get(i).copied().flatten().unwrap_or(player);
        let to_player = player - f.motor.pos;
        let input = FollowIn {
            to_point: point - f.motor.pos,
            to_player: to_player.normalize_or_zero(),
            dist: to_player.length(),
            ground_dir: to_player.with_y(0.0).normalize_or_zero(),
            facing: f.motor.forward(),
            walk: m.walk,
            run: m.run,
        };
        let (dir, speed) = rules::follow_step_row(&mut f.follow, &input, &m.thresholds, rules::ROW_FIELD, c);
        let (dir, speed) = rules::steer(&mut f.follow, dir, speed, m.walk, m.run, c);
        let flat = dir.with_y(0.0).normalize_or_zero();
        if flat != Vec3::ZERO {
            f.motor.turn_to(flat, m.turn);
        }
        f.motor.vel = flat * speed;
        let dy = (!f.motor.grounded).then(|| f.motor.jump_dy(fall_max));
        f.motor.move_by(world, f.motor.vel, dy);
        let id = if speed <= 0.0 { 0 } else if speed < (m.walk + m.run) * 0.5 { 1 } else { 2 };
        let slot = f.body.rig.mset.slot(id, PlayerState::FieldNoWeapon);
        f.body.play(slot, true, BLEND);
        f.body.step(pose);
    }
}
