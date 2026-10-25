//! Oracle tests of libkh2's own code against the original executable (round 9): kh2-rs's kh2-ee
//! runs the function from the user's `SLUS_210.05` (read from `KH2_ISO`) and the library runs on
//! the same inputs; every case is compared. Skipped without a disc.
//!
//! The harness follows kh2-rs's oracle tests (`kh2-player/tests/orig_l2_p1.rs`): the original's
//! records are laid out at the offsets the decompilation reads, cases come from a seeded generator
//! (a failing case number reproduces), stand-ins are listed per test.

use kh2_ee::Ee;
use kh2_factory::fuzz::{Rng, STACK};

const VICTIM: u32 = 0x0180_0000;
const ATTACKER_OBJ: u32 = 0x0181_0000;
const ATTACKER: u32 = 0x0182_0000;
const HIT: u32 = 0x0183_0000;
const ROW: u32 = 0x0184_0000;
const CASES: u32 = 3000;

fn machine(what: &str) -> Option<Ee> {
    let elf = kh2_factory::exe_or_skip(what)?;
    let mut ee = Ee::new();
    ee.load_elf(&elf).expect("elf");
    Some(ee)
}

fn zero(ee: &mut Ee, at: u32, len: u32) {
    ee.write(at, &vec![0u8; len as usize]);
}

fn w16(ee: &mut Ee, at: u32, v: u16) {
    ee.write(at, &v.to_le_bytes());
}

/// `FUN_001a0610`, the game's per-hit damage (attacker status, victim status, attack row), against
/// `kh2::enemy::damage` (kh2-rs `GameData::damage`), which every hit in libkh2 uses.
///
/// Layout (decompilation of 0x1a0610): a0 the victim's status block (+0 HP, +4 max HP, +0x18c
/// defense u16, +0x1a0 max taken, +0x1a4 min taken, +0x1a8.. the element percents, +0x1ae the
/// general percent, +0x1f8 Defender's byte, +0x24c the victim object); a1 the hit record (+0xc the
/// attacker object, whose +0x48c is its status block: +0x188 strength, +0x18a magic, +0x198 max
/// dealt, +0x19c min dealt; object +0x588 flags); a2 the attack row (`atkp`, 48 bytes); a3 0 (the
/// type-5 row reads the victim's +4).
///
/// Stand-ins: `FUN_0019a3f8` (the object-exists test) answers 0, so the boosts of `FUN_001a0898`
/// (difficulty and party bonus) and Anti's factor do not apply; the attacker's boosts
/// (`FUN_001a0a18`, `FUN_001a0aa8`) read a zeroed block and pass the value through; Defender's
/// byte is 0, the attacker object's flags 0 (no Berserk), the global `DAT_0035d268` is 0
/// (`FUN_001a0968` passes through). Those parts are libkh2's `attacker_bonuses` / `defender`, not
/// `damage`.
#[test]
fn orig_1a0610_damage_matches_the_game() {
    let Some(mut ee) = machine("oracle test of 0x1a0610 (damage)") else { return };
    ee.hooks.insert(0x19a3f8, |e: &mut Ee| e.gpr[2] = 0);
    let mut r = Rng::new(0x1a0610);
    let mut checked = 0;
    let mut diffs = Vec::new();
    for n in 0..CASES {
        let ty = [0u8, 0, 0, 1, 5, 6][r.below(6) as usize];
        let crit = [0u8, 0, 1, 2][r.below(4) as usize];
        let power = r.below(400) as u16;
        let element = r.below(7) as u8;
        let mut atk = kh2_tables::tables::Atkp { sub_id: 0, id: 1, r#type: ty, critical_adjust: crit, power, team: 0, element, enemy_reaction: 0, effect_on_hit: 0, knockback_strength1: 0, knockback_strength2: 0, unknown: 0, flags: 0, refact_self: 0, refact_other: 0, reflected_motion: 0, reflect_hit_back: 0, reflect_action: 0, reflect_hit_sound: 0, reflect_rc: 0, reflect_range: 0, reflect_angle: 0, damage_effect: 0, switch: 0, interval: 0, floor_check: 0, drive_drain: 0, revenge_damage: 0, attack_tr_reaction: 0, combo_group: 0, random_effect: 0, kind: 0, hp_drain: 0 };
        atk.id = n as u16;
        let strength = r.below(120) as i32;
        let magic = r.below(120) as i32;
        let max_dealt = [i32::MAX, 0, r.below(300) as i32][r.below(3) as usize];
        let min_dealt = [0, 1, r.below(10) as i32][r.below(3) as usize];
        let max_hp = 1 + r.below(2000) as i32;
        let hp = r.below(max_hp as u64 + 1) as i32;
        let defense = r.below(100) as i32;
        let max_taken = [i32::MAX, 9999, r.below(500) as i32][r.below(3) as usize];
        let min_taken = [0, 1, r.below(5) as i32][r.below(3) as usize];
        let mut weak = [100i32; 7];
        for x in &mut weak {
            *x = [100, 0, 50, 150, 200, r.below(256) as i32][r.below(6) as usize];
        }
        // The original's records.
        zero(&mut ee, VICTIM, 0x300);
        zero(&mut ee, ATTACKER_OBJ, 0x600);
        zero(&mut ee, ATTACKER, 0x300);
        zero(&mut ee, HIT, 0x20);
        zero(&mut ee, ROW, 0x30);
        ee.w32(VICTIM, hp as u32);
        ee.w32(VICTIM + 4, max_hp as u32);
        w16(&mut ee, VICTIM + 0x18c, defense as u16);
        ee.w32(VICTIM + 0x1a0, max_taken as u32);
        ee.w32(VICTIM + 0x1a4, min_taken as u32);
        for (k, w) in weak.iter().enumerate() {
            ee.write(VICTIM + 0x1a8 + k as u32, &[*w as u8]);
        }
        ee.w32(HIT + 0xc, ATTACKER_OBJ);
        ee.w32(ATTACKER_OBJ + 0x48c, ATTACKER);
        w16(&mut ee, ATTACKER + 0x188, strength as u16);
        w16(&mut ee, ATTACKER + 0x18a, magic as u16);
        // The attacker's element boosts (Fire / Blizzard / Thunder Boost, percent; 100 none).
        for o in [0x20c, 0x20e, 0x210] {
            w16(&mut ee, ATTACKER + o, 100);
        }
        ee.w32(ATTACKER + 0x198, max_dealt as u32);
        ee.w32(ATTACKER + 0x19c, min_dealt as u32);
        ee.write(ROW + 4, &[ty, crit]);
        w16(&mut ee, ROW + 6, power);
        ee.write(ROW + 9, &[element]);
        let game = ee.call(0x1a0610, &[VICTIM as u64, HIT as u64, ROW as u64, 0], &[], STACK).unwrap_or_else(|f| panic!("case {n}: the original faulted: {f:?}")) as u32 as i32;
        // The library's.
        let from = kh2_enemy_host::data::Stats { strength, magic, max_dealt, min_dealt, ..Default::default() };
        let to = kh2_enemy_host::data::Stats { max_hp, defense, max_taken, min_taken, weakness: weak, ..Default::default() };
        let port = kh2::enemy::damage(&atk, &from, &to);
        if game.max(0) != port {
            diffs.push(format!("case {n}: type {ty} crit {crit} power {power} element {element} str {strength} mag {magic} dealt {min_dealt}..{max_dealt} def {defense} taken {min_taken}..{max_taken} weak {weak:?} maxhp {max_hp}: game {game} lib {port}"));
        }
        checked += 1;
    }
    eprintln!("0x1a0610: {checked} cases, {} differ", diffs.len());
    for d in diffs.iter().take(8) {
        eprintln!("  {d}");
    }
    assert!(diffs.is_empty(), "{} of {checked} cases differ", diffs.len());
}

/// `FUN_001a0a18` (element boosts) then `FUN_001a0aa8` (Combo / Air Combo / Reaction Boost, with
/// `FUN_00176be8` / `FUN_00176c40`: the hit counter at object +0xb04 capped by the block's combo
/// lengths) against `kh2_game_core::abilities::attacker_bonuses`, which libkh2 applies to every hit
/// Sora, his members' and his summon's attacks land.
///
/// Layout: the attacker's status block, its ability block at +0x1cc (kh2-rs `Block`: bits +0, combo
/// lengths +0x28 / +0x29, element percents +0x40..+0x44, boosts +0x46..+0x4a), its object at +0x24c
/// (whose +0x48c is the status and +0xb04 the hit counter); the row's +9 element and +0x2e kind.
/// No stand-ins.
#[test]
fn orig_1a0a18_1a0aa8_attacker_bonuses_match_the_game() {
    use kh2_game_core::abilities::{attacker_bonuses, Block, BIT_AIR_COMBO_BOOST, BIT_COMBO_BOOST, BIT_REACTION_BOOST};
    let Some(mut ee) = machine("oracle test of 0x1a0a18 / 0x1a0aa8 (attacker bonuses)") else { return };
    let mut r = Rng::new(0x1a0aa8);
    let mut diffs = Vec::new();
    for n in 0..CASES {
        let mut b = Block::default();
        for bit in [BIT_COMBO_BOOST, BIT_AIR_COMBO_BOOST, BIT_REACTION_BOOST] {
            if r.below(2) == 0 {
                b.set_bit(bit);
            }
        }
        b.ground_combo = r.below(8) as i8;
        b.air_combo = r.below(8) as i8;
        let pct = |r: &mut Rng| [100u16, 120, 150, 200, r.below(400) as u16][r.below(5) as usize];
        b.fire = pct(&mut r);
        b.blizzard = pct(&mut r);
        b.thunder = pct(&mut r);
        b.combo_boost = r.below(30) as u16;
        b.air_combo_boost = r.below(30) as u16;
        b.reaction_boost = r.below(60) as u16;
        let hits = r.below(12) as u8;
        let element = r.below(8) as u8;
        let kind = r.below(8) as u8;
        let dmg = r.below(2000) as i32;
        zero(&mut ee, ATTACKER, 0x300);
        zero(&mut ee, ATTACKER_OBJ, 0xc00);
        zero(&mut ee, ROW, 0x30);
        let blk = ATTACKER + 0x1cc;
        for (k, w) in b.bits.iter().enumerate() {
            ee.w32(blk + 4 * k as u32, *w);
        }
        ee.write(blk + 0x28, &[b.ground_combo as u8, b.air_combo as u8]);
        for (o, v) in [(0x40, b.fire), (0x42, b.blizzard), (0x44, b.thunder), (0x46, b.combo_boost), (0x48, b.air_combo_boost), (0x4a, b.reaction_boost)] {
            w16(&mut ee, blk + o, v);
        }
        ee.w32(ATTACKER + 0x24c, ATTACKER_OBJ);
        ee.w32(ATTACKER_OBJ + 0x48c, ATTACKER);
        ee.write(ATTACKER_OBJ + 0xb04, &[hits]);
        ee.write(ROW + 9, &[element]);
        ee.write(ROW + 0x2e, &[kind]);
        let a = ee.call(0x1a0a18, &[0, dmg as u64, ATTACKER as u64, ROW as u64], &[], STACK).unwrap_or_else(|f| panic!("case {n}: 0x1a0a18 faulted: {f:?}")) as u32 as i32;
        let game = ee.call(0x1a0aa8, &[0, a as u32 as u64, ATTACKER as u64, ROW as u64], &[], STACK).unwrap_or_else(|f| panic!("case {n}: 0x1a0aa8 faulted: {f:?}")) as u32 as i32;
        let mut atk = row();
        atk.element = element;
        atk.kind = kind;
        let port = attacker_bonuses(&b, hits, &atk, dmg);
        if game != port {
            diffs.push(format!("case {n}: dmg {dmg} element {element} kind {kind} hits {hits} block {b:?}: game {game} lib {port}"));
        }
    }
    eprintln!("0x1a0a18 + 0x1a0aa8: {CASES} cases, {} differ", diffs.len());
    for d in diffs.iter().take(5) {
        eprintln!("  {d}");
    }
    assert!(diffs.is_empty());
}

fn row() -> kh2_tables::tables::Atkp {
    kh2_tables::tables::Atkp { sub_id: 0, id: 1, r#type: 0, critical_adjust: 0, power: 0, team: 0, element: 0, enemy_reaction: 0, effect_on_hit: 0, knockback_strength1: 0, knockback_strength2: 0, unknown: 0, flags: 0, refact_self: 0, refact_other: 0, reflected_motion: 0, reflect_hit_back: 0, reflect_action: 0, reflect_hit_sound: 0, reflect_rc: 0, reflect_range: 0, reflect_angle: 0, damage_effect: 0, switch: 0, interval: 0, floor_check: 0, drive_drain: 0, revenge_damage: 0, attack_tr_reaction: 0, combo_group: 0, random_effect: 0, kind: 0, hp_drain: 0 }
}

/// `FUN_001a0be0` (HP at or below a quarter, the gate of Defender) against
/// `kh2_game_core::abilities::hp_is_low`, which libkh2's hits use for Defender. No stand-ins.
#[test]
fn orig_1a0be0_hp_is_low_matches_the_game() {
    use kh2_game_core::abilities::hp_is_low;
    let Some(mut ee) = machine("oracle test of 0x1a0be0") else { return };
    let mut r = Rng::new(0x1a0be0);
    for n in 0..CASES {
        let max = [r.below(10000) as i32, -(r.below(100) as i32), 0, 1, 3, 4][r.below(6) as usize];
        let hp = [r.below(10000) as i32, max / 4, max / 4 + 1, -(r.below(10) as i32)][r.below(4) as usize];
        ee.w32(VICTIM, hp as u32);
        ee.w32(VICTIM + 4, max as u32);
        let game = ee.call(0x1a0be0, &[VICTIM as u64], &[], STACK).expect("0x1a0be0") & 1 != 0;
        assert_eq!(game, hp_is_low(hp, max), "case {n}: hp {hp} max {max}");
    }
}
