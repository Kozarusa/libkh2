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
/// The functions in the Final Mix executable (SLPM_666.75; kh2fm-rs's function map pairs them with
/// the USA 0x1a0610, 0x1a0a18, 0x1a0aa8 and 0x1a0be0 the comments name).
const FM_DAMAGE: u32 = 0x1a1e1c;
/// FM 0x19ba60 (USA 0x19a3f8), the object-exists test the damage routine's helpers ask.
const FM_OBJECT_EXISTS: u32 = 0x19ba60;
/// FM 0x352130 (USA 0x360130): the pointer to the `sstm` record the routine reads (+0x94 Anti's
/// factor, Final Mix +0x178 the low-HP percent).
const FM_SSTM_PTR: u32 = 0x352130;
const SSTM: u32 = 0x0185_0000;
const FM_BONUS_A: u32 = 0x1a22b0;
const FM_BONUS_B: u32 = 0x1a2340;
const FM_HP_LOW: u32 = 0x1a2478;

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
/// Layout (USA 0x1a0610; Final Mix FM 0x1a1e1c, disassembled: the same code with the victim's
/// Defender byte at +0x1fc, its object at +0x25c, Anti as form 6, then a new last step,
/// `kh2::enemy::low_hp_scale`): a0 the victim's status block (+0 HP, +4 max HP, +0x18c
/// defense u16, +0x1a0 max taken, +0x1a4 min taken, +0x1a8.. the element percents, +0x1ae the
/// general percent, +0x1fc Defender's byte, +0x240 the low-HP ratio (f32), +0x25c the victim
/// object); a1 the hit record (+0xc the
/// attacker object, whose +0x48c is its status block: +0x188 strength, +0x18a magic, +0x198 max
/// dealt, +0x19c min dealt; object +0x588 flags); a2 the attack row (`atkp`, 48 bytes); a3 0 (the
/// type-5 row reads the victim's +4).
///
/// Round 12 ran this with the USA layout and no stand-in for FM 0x19ba60 and saw 368 of 3000
/// cases differ: those were the test's state, not the rules (with the Final Mix layout every case
/// matches). Stand-ins: FM 0x19ba60 (the object-exists test) answers 0, so the boosts of `FUN_001a0898`
/// (difficulty and party bonus) and Anti's factor do not apply; the attacker's boosts
/// (`FUN_001a0a18`, `FUN_001a0aa8`) read a zeroed block and pass the value through; Defender's
/// byte is 0, the attacker object's flags 0 (no Berserk), the global `DAT_0035d268` is 0
/// (`FUN_001a0968` passes through). Those parts are libkh2's `attacker_bonuses` / `defender`, not
/// `damage`.
#[test]
fn orig_1a0610_damage_matches_the_game() {
    let Some(mut ee) = machine("oracle test of 0x1a0610 (damage)") else { return };
    ee.hooks.insert(FM_OBJECT_EXISTS, |e: &mut Ee| e.gpr[2] = 0);
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
        let ratio = [0.0f32, 0.25, 0.5, 1.0, r.below(1000) as f32 / 1000.0][r.below(5) as usize];
        let percent = [100i32, 50, 200, 150, r.below(300) as i32][r.below(5) as usize];
        let hp = [hp, 0, max_hp / 4, max_hp / 2][r.below(4) as usize];
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
        zero(&mut ee, SSTM, 0x200);
        ee.w32(FM_SSTM_PTR, SSTM);
        ee.w32(SSTM + 0x178, percent as u32);
        ee.write(VICTIM + 0x240, &ratio.to_le_bytes());
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
        // Final Mix: the boosts sit 4 bytes later than on USA (+0x210 / +0x212 / +0x214).
        for o in [0x210, 0x212, 0x214] {
            w16(&mut ee, ATTACKER + o, 100);
        }
        ee.w32(ATTACKER + 0x198, max_dealt as u32);
        ee.w32(ATTACKER + 0x19c, min_dealt as u32);
        ee.write(ROW + 4, &[ty, crit]);
        w16(&mut ee, ROW + 6, power);
        ee.write(ROW + 9, &[element]);
        let game = ee.call(FM_DAMAGE, &[VICTIM as u64, HIT as u64, ROW as u64, 0], &[], STACK).unwrap_or_else(|f| panic!("case {n}: the original faulted: {f:?}")) as u32 as i32;
        // The library's.
        let from = kh2_enemy_host::data::Stats { strength, magic, max_dealt, min_dealt, ..Default::default() };
        let to = kh2_enemy_host::data::Stats { max_hp, defense, max_taken, min_taken, weakness: weak, ..Default::default() };
        let port = kh2::enemy::low_hp_scale(kh2::enemy::damage(&atk, &from, &to), hp, max_hp, ratio, percent);
        if game.max(0) != port {
            diffs.push(format!("case {n}: type {ty} crit {crit} power {power} element {element} str {strength} mag {magic} dealt {min_dealt}..{max_dealt} def {defense} taken {min_taken}..{max_taken} weak {weak:?} hp {hp} maxhp {max_hp} ratio {ratio} percent {percent}: game {game} lib {port}"));
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
/// Layout (Final Mix, FM 0x1a22b0 / 0x1a2340 / 0x177ba0 / 0x177bf8 disassembled: the USA code
/// with the block fields after the bits 4 bytes later and the object at +0x25c; the rules are the
/// same): the attacker's status block, its ability block at +0x1cc (bits +0, combo lengths
/// +0x2c / +0x2d, element percents +0x44..+0x48, boosts +0x4a..+0x4e), its object at +0x25c
/// (whose +0x48c is the status and +0xb04 the hit counter); the row's +9 element and +0x2e kind.
/// Round 12's 1954 differing cases were the USA layout. No stand-ins.
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
        ee.write(blk + 0x2c, &[b.ground_combo as u8, b.air_combo as u8]);
        for (o, v) in [(0x44, b.fire), (0x46, b.blizzard), (0x48, b.thunder), (0x4a, b.combo_boost), (0x4c, b.air_combo_boost), (0x4e, b.reaction_boost)] {
            w16(&mut ee, blk + o, v);
        }
        ee.w32(ATTACKER + 0x25c, ATTACKER_OBJ);
        ee.w32(ATTACKER_OBJ + 0x48c, ATTACKER);
        ee.write(ATTACKER_OBJ + 0xb04, &[hits]);
        ee.write(ROW + 9, &[element]);
        ee.write(ROW + 0x2e, &[kind]);
        let a = ee.call(FM_BONUS_A, &[0, dmg as u64, ATTACKER as u64, ROW as u64], &[], STACK).unwrap_or_else(|f| panic!("case {n}: 0x1a0a18 faulted: {f:?}")) as u32 as i32;
        let game = ee.call(FM_BONUS_B, &[0, a as u32 as u64, ATTACKER as u64, ROW as u64], &[], STACK).unwrap_or_else(|f| panic!("case {n}: 0x1a0aa8 faulted: {f:?}")) as u32 as i32;
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
    assert!(diffs.is_empty(), "{} of {CASES} cases differ", diffs.len());
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
        let game = ee.call(FM_HP_LOW, &[VICTIM as u64], &[], STACK).expect("0x1a0be0") & 1 != 0;
        assert_eq!(game, hp_is_low(hp, max), "case {n}: hp {hp} max {max}");
    }
}

/// The disc for the tables the rules read (the same image the executable comes from).
fn fm_disc() -> Option<kh2::data::Disc> {
    let iso = std::env::var_os("KH2_ISO_FM").or_else(|| std::env::var_os("KH2_ISO"))?;
    kh2::data::Disc::open(std::path::Path::new(&iso)).ok()
}

const STATUS: u32 = 0x0186_0000;
const OBJ: u32 = 0x0187_0000;
const SSTM_REC: u32 = 0x0188_0000;

/// Writes the disc's `sstm` record where FM 0x352130 points.
fn put_sstm(ee: &mut Ee, sstm: &kh2_tables::player::Sstm) {
    for (k, w) in sstm.words.iter().enumerate() {
        ee.w32(SSTM_REC + 4 * k as u32, *w);
    }
    ee.w32(FM_SSTM_PTR, SSTM_REC);
}

/// FM 0x1c35a0 (USA `FUN_001c1340`): the growth levels from the ability bits through the table at
/// FM 0x36dde8 (five groups of five bytes, the highest level first), written to block +0x20..+0x24,
/// against `AbilityRules::growth_levels`, which libkh2's loadout uses (Dodge Roll is group 4).
#[test]
fn orig_fm_1c35a0_growth_levels_match_the_game() {
    use kh2_game_core::abilities::{AbilityRules, Block};
    let Some(mut ee) = machine("oracle test of FM 0x1c35a0 (growth levels)") else { return };
    let Some(mut d) = fm_disc() else { return };
    let rules = AbilityRules::load(&mut d.0).expect("ability rules");
    let bits: Vec<u16> = rules.growth_bits.iter().flatten().copied().filter(|&b| b != 0).collect();
    assert_eq!(bits.len(), 20, "five groups of four levels on Final Mix");
    let mut r = Rng::new(0x1c35a0);
    for n in 0..CASES {
        let mut b = Block::default();
        for &bit in &bits {
            if r.below(4) == 0 {
                b.set_bit(bit);
            }
        }
        zero(&mut ee, ATTACKER, 0x100);
        for (k, w) in b.bits.iter().enumerate() {
            ee.w32(ATTACKER + 4 * k as u32, *w);
        }
        ee.call(0x1c35a0, &[ATTACKER as u64], &[], STACK).unwrap_or_else(|f| panic!("case {n}: {f:?}"));
        let game: [u8; 5] = ee.read(ATTACKER + 0x20);
        assert_eq!(game, rules.growth_levels(&b), "case {n}: bits {:x?}", b.bits);
    }
}

/// FM 0x1a1cc0 -> FM 0x1a1c90 (USA `FUN_001a04b0` / `FUN_001a0480`): the form timer's length,
/// `sstm` +0x68 times the timer level + 3 (Anti, form 6: `sstm` +0x8c - 2), written to status
/// +0x1b4 / +0x1b8, against `FormTimer::start` with the Final Mix numbering. Stand-ins: FM 0x1a1950
/// and 0x1a1a50 (the gauge's mode and colour) return at once.
#[test]
fn orig_fm_1a1cc0_form_timer_start_matches_the_game() {
    use kh2_game_core::drive_rules::{FormTimer, Tuning};
    let Some(mut ee) = machine("oracle test of FM 0x1a1cc0 (form timer)") else { return };
    let Some(mut d) = fm_disc() else { return };
    let sstm = kh2_tables::player::load_sstm(&mut d.0).expect("sstm");
    let t = Tuning::from_sstm(&sstm);
    ee.hooks.insert(0x1a1950, |_e: &mut Ee| {});
    ee.hooks.insert(0x1a1a50, |_e: &mut Ee| {});
    put_sstm(&mut ee, &sstm);
    for form in 1..=6u8 {
        for level in 0..=7u8 {
            zero(&mut ee, STATUS, 0x300);
            ee.call(0x1a1cc0, &[STATUS as u64, form as u64, level as u64], &[], STACK).expect("call");
            let left = f32::from_bits(ee.r32(STATUS + 0x1b4));
            let total = f32::from_bits(ee.r32(STATUS + 0x1b8));
            let port = FormTimer::start(&t, kh2::drive::IDS, form, level);
            assert_eq!((left, total), (port.left, port.total), "form {form} level {level}");
        }
    }
}

/// FM 0x1a1b68 (USA `FUN_001a0358`): a change of the form timer, a gain turned into a loss in Anti
/// (the object's form word +0xb50 is 6), clamped to 0..=full; and FM 0x1a1c30 (USA `FUN_001a0420`),
/// the whole points left, one less while full. Against `FormTimer::add` / `whole_points`. Stand-in:
/// FM 0x19ba60 (the object exists) answers 1; the gauge mode is not 3 (no redraw call).
#[test]
fn orig_fm_1a1b68_form_timer_change_matches_the_game() {
    use kh2_game_core::drive_rules::{FormTimer, Tuning};
    let Some(mut ee) = machine("oracle test of FM 0x1a1b68 (form timer change)") else { return };
    let Some(mut d) = fm_disc() else { return };
    let sstm = kh2_tables::player::load_sstm(&mut d.0).expect("sstm");
    let t = Tuning::from_sstm(&sstm);
    ee.hooks.insert(FM_OBJECT_EXISTS, |e: &mut Ee| e.gpr[2] = 1);
    put_sstm(&mut ee, &sstm);
    let mut r = Rng::new(0x1a1b68);
    for n in 0..CASES {
        let form = 1 + r.below(6) as u8;
        let total = t.point_units * (3 + r.below(5)) as f32;
        let left = [total, 0.0, total * (r.below(1000) as f32 / 1000.0)][r.below(3) as usize];
        let delta = [(r.below(2000) as f32 - 1000.0) / 7.0, -total, total, 0.0][r.below(4) as usize];
        zero(&mut ee, STATUS, 0x300);
        zero(&mut ee, OBJ, 0xc00);
        ee.w32(STATUS + 0x25c, OBJ);
        ee.w32(OBJ + 0xb50, form as u32);
        ee.w32(STATUS + 0x1b4, left.to_bits());
        ee.w32(STATUS + 0x1b8, total.to_bits());
        ee.call(0x1a1b68, &[STATUS as u64], &[delta], STACK).unwrap_or_else(|f| panic!("case {n}: {f:?}"));
        let game = f32::from_bits(ee.r32(STATUS + 0x1b4));
        let mut port = FormTimer { left, total };
        port.add(kh2::drive::IDS, form, delta);
        assert_eq!(game, port.left, "case {n}: form {form} left {left} total {total} delta {delta}");
        let whole = ee.call(0x1a1c30, &[STATUS as u64], &[], STACK).expect("whole") as u32 as i32;
        assert_eq!(whole, port.whole_points(&t), "case {n}: whole points of {} / {total}", port.left);
    }
}
