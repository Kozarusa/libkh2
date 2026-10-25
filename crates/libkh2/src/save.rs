//! Sora's save record as bytes (`kh2_state_save` / `kh2_state_load`).
//!
//! decided: the record is what the game keeps between rooms (level, experience, HP/MP and their
//! maxima, AP, spell levels, the weapon set, the given and equipped abilities, the drive gauge,
//! owned forms and form levels) plus the place (position, yaw). The moment-to-moment state (motion,
//! combo, cast, form in progress, enemies) is not saved: a loaded Sora stands in the field state,
//! out of any form. The status (strength, magic, defense) is rebuilt from the level, as the game's
//! status rebuild does (FUN_0019f920).
//!
//! Layout, little-endian: "KH2S", u16 version (1), then the fields in the order of `save`.

use glam::Vec3;

use crate::sora::Sora;

const MAGIC: &[u8; 4] = b"KH2S";
const VERSION: u16 = 1;
const MAX_LIST: usize = 256;

struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn list(&mut self, v: &[u16]) {
        let n = v.len().min(MAX_LIST);
        self.u16(n as u16);
        for &x in &v[..n] {
            self.u16(x);
        }
    }
}

struct R<'a>(&'a [u8]);

impl R<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (a, b) = self.0.split_at_checked(N)?;
        self.0 = b;
        a.try_into().ok()
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take()?))
    }
    fn i32(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.take()?))
    }
    fn f32(&mut self) -> Option<f32> {
        let v = f32::from_le_bytes(self.take()?);
        v.is_finite().then_some(v)
    }
    fn list(&mut self) -> Option<Vec<u16>> {
        let n = self.u16()? as usize;
        if n > MAX_LIST {
            return None;
        }
        (0..n).map(|_| self.u16()).collect()
    }
}

/// The record read back, before it is applied.
struct Record {
    level: i32,
    exp: i32,
    hp: i32,
    max_hp: i32,
    mp: i32,
    max_mp: i32,
    magic_levels: [u8; 6],
    weapon_set: u8,
    granted: Vec<u16>,
    loadout: Vec<u16>,
    drive_whole: i32,
    drive_max: i32,
    owned: [bool; 6],
    form_levels: [u8; 6],
    pos: Vec3,
    yaw: f32,
}

impl Sora {
    /// The save record (see the module doc).
    pub fn save(&self) -> Vec<u8> {
        let mut w = W(Vec::with_capacity(128));
        w.0.extend_from_slice(MAGIC);
        w.u16(VERSION);
        for v in [self.level, self.exp, self.hp, self.max_hp, self.mp, self.max_mp] {
            w.i32(v);
        }
        for v in self.magic_levels {
            w.u8(v);
        }
        w.u8(self.weapon_set);
        w.list(&self.granted);
        w.list(&self.loadout_ids);
        w.i32(i32::from(self.drive.points.whole));
        w.i32(i32::from(self.drive.points.max));
        for k in 0..6 {
            w.u8(self.drive.owned[k] as u8);
        }
        for v in self.drive.level {
            w.u8(v);
        }
        for v in self.motor.pos.to_array() {
            w.f32(v);
        }
        w.f32(self.motor.yaw);
        w.0
    }

    /// Applies a save record; nothing changes when it is not one (wrong magic, version, length or
    /// values out of range).
    pub fn load(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        let mut r = R(bytes);
        if r.take::<4>().as_ref() != Some(MAGIC) {
            return Err("not a save record");
        }
        if r.u16() != Some(VERSION) {
            return Err("unknown save record version");
        }
        let rec = (|| {
            let level = r.i32()?;
            let exp = r.i32()?;
            let hp = r.i32()?;
            let max_hp = r.i32()?;
            let mp = r.i32()?;
            let max_mp = r.i32()?;
            let mut magic_levels = [0u8; 6];
            for v in &mut magic_levels {
                *v = r.u8()?;
            }
            let weapon_set = r.u8()?;
            let granted = r.list()?;
            let loadout = r.list()?;
            let drive_whole = r.i32()?;
            let drive_max = r.i32()?;
            let mut owned = [false; 6];
            for v in &mut owned {
                *v = r.u8()? != 0;
            }
            let mut form_levels = [0u8; 6];
            for v in &mut form_levels {
                *v = r.u8()?;
            }
            let pos = Vec3::new(r.f32()?, r.f32()?, r.f32()?);
            let yaw = r.f32()?;
            r.0.is_empty().then_some(Record { level, exp, hp, max_hp, mp, max_mp, magic_levels, weapon_set, granted, loadout, drive_whole, drive_max, owned, form_levels, pos, yaw })
        })()
        .ok_or("save record truncated or malformed")?;
        let ok = (1..=99).contains(&rec.level)
            && (0..=9_999_999).contains(&rec.exp)
            && (1..=9999).contains(&rec.max_hp)
            && (0..=rec.max_hp).contains(&rec.hp)
            && (0..=9999).contains(&rec.max_mp)
            && (0..=rec.max_mp).contains(&rec.mp)
            && rec.weapon_set <= 2
            && (0..=9).contains(&rec.drive_max)
            && (0..=rec.drive_max).contains(&rec.drive_whole)
            && rec.form_levels.iter().all(|&l| (1..=7).contains(&l))
            && rec.pos.abs().max_element() < 1.0e7;
        if !ok {
            return Err("save record values out of range");
        }
        // Out of any form first (the record keeps no form in progress).
        let mut fresh = Sora::new(self.game.clone(), rec.pos);
        fresh.pose = self.pose;
        fresh.auto_revive = self.auto_revive;
        fresh.set_level(rec.level);
        fresh.exp = rec.exp;
        fresh.max_hp = rec.max_hp;
        fresh.hp = rec.hp;
        fresh.max_mp = rec.max_mp;
        fresh.mp = rec.mp;
        fresh.magic_levels = rec.magic_levels;
        fresh.weapon_set = rec.weapon_set;
        fresh.granted = rec.granted;
        fresh.drive.points.whole = rec.drive_whole as _;
        fresh.drive.points.max = rec.drive_max as _;
        fresh.drive.owned = rec.owned;
        fresh.drive.level = rec.form_levels;
        fresh.motor.yaw = rec.yaw;
        fresh.wanted_yaw = rec.yaw;
        if !rec.loadout.is_empty() {
            fresh.set_loadout(&rec.loadout);
        }
        *self = fresh;
        Ok(())
    }
}
