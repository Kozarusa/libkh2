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

/// FM 0x1a2108 (USA `FUN_001a0898`): the difficulty step of the damage routine. The difficulty word
/// at FM 0x32dfc8, the victim object (status +0x25c) and the attacker object (a2) with +0x588 bit 4
/// for a party member; against `enemy::difficulty_scale`. Stand-in: FM 0x19ba60 answers 1.
#[test]
fn orig_fm_1a2108_difficulty_matches_the_game() {
    let Some(mut ee) = machine("oracle test of FM 0x1a2108 (difficulty)") else { return };
    ee.hooks.insert(FM_OBJECT_EXISTS, |e: &mut Ee| e.gpr[2] = 1);
    let mut r = Rng::new(0x1a2108);
    for n in 0..CASES {
        let diff = r.below(5) as u8;
        let (vp, ap) = (r.below(2) == 0, r.below(2) == 0);
        let dmg = [r.below(5000) as i32, 0, -(r.below(10) as i32), 1][r.below(4) as usize];
        zero(&mut ee, STATUS, 0x300);
        zero(&mut ee, OBJ, 0x600);
        zero(&mut ee, ATTACKER_OBJ, 0x600);
        ee.w32(0x32dfc8, diff as u32);
        ee.w32(STATUS + 0x25c, OBJ);
        ee.w32(OBJ + 0x588, if vp { 4 } else { 0 });
        ee.w32(ATTACKER_OBJ + 0x588, if ap { 4 } else { 0 });
        let game = ee.call(0x1a2108, &[STATUS as u64, dmg as i64 as u64, ATTACKER_OBJ as u64], &[], STACK).unwrap_or_else(|f| panic!("case {n}: {f:?}")) as u32 as i32;
        assert_eq!(game, kh2::enemy::difficulty_scale(dmg, diff, vp, ap), "case {n}: difficulty {diff} victim party {vp} attacker party {ap} damage {dmg}");
    }
}

/// The player's jump mover (round 15; FM addresses of the unchanged USA bodies: FM 0x17d130 = USA
/// `FUN_0017bee0`, FM 0x17c430 = `FUN_0017b1e0`, FM 0x17c930 = `FUN_0017b6e0`). The mover keeps
/// clock +0xd0, rise time +0xd4, base +0xd8 and rise +0xdc (game Y points down: rise = -height),
/// actor flags +0x160 (0: gravity on). Against kh2fm-rs's `motor::Jump` (`Jump::up` / `fall` /
/// `target_y`), which libkh2's Sora uses. Float results compare within 1e-3 units: the game's
/// `sinf` (FM 0x2f8960) is its own routine.
const MOVER: u32 = 0x0189_0000;
const ZERO_VEC: u32 = 0x018a_0000;

#[test]
fn orig_fm_17d130_jump_rise_time_matches_the_game() {
    use kh2_game_core::motor::Jump;
    let Some(mut ee) = machine("oracle test of FM 0x17d130 (rise time)") else { return };
    let mut r = Rng::new(0x17d130);
    for n in 0..CASES {
        let h = r.below(60000) as f32 / 100.0;
        ee.call(0x17d130, &[], &[h], STACK).unwrap_or_else(|f| panic!("case {n}: {f:?}"));
        let port = Jump::up(h, 0.0).rise;
        assert!((ee.fpr[0] - port).abs() <= 1e-4 * port.max(1.0), "case {n}: h {h}: game {} lib {port}", ee.fpr[0]);
    }
}

#[test]
fn orig_fm_17c430_jump_launch_matches_the_game() {
    use kh2_game_core::motor::Jump;
    let Some(mut ee) = machine("oracle test of FM 0x17c430 (jump launch)") else { return };
    ee.hooks.insert(0x17c400, |_e: &mut Ee| {});
    let mut r = Rng::new(0x17c430);
    for n in 0..CASES {
        let h = (r.below(80000) as f32 - 20000.0) / 100.0;
        zero(&mut ee, MOVER, 0x200);
        ee.call(0x17c430, &[MOVER as u64], &[h], STACK).unwrap_or_else(|f| panic!("case {n}: {f:?}"));
        let g = |o: u32, ee: &mut Ee| f32::from_bits(ee.r32(MOVER + o));
        let (clock, rise, base, up) = (g(0xd0, &mut ee), g(0xd4, &mut ee), g(0xd8, &mut ee), g(0xdc, &mut ee));
        // The port: a jump of h (base 0 here, height h) or a fall already under way from -h above.
        let j = if h >= 0.0 { Jump::up(h, 0.0) } else { Jump::fall(-h, 0.0) };
        let close = |a: f32, b: f32| (a - b).abs() <= 1e-3 * b.abs().max(1.0);
        assert!(close(clock, j.t) && close(rise, j.rise), "case {n}: h {h}: game clock {clock} rise {rise}, lib {} {}", j.t, j.rise);
        if h >= 0.0 {
            assert!(close(base, 0.0) && close(up, -h), "case {n}: base {base} rise height {up}");
        } else {
            // A fall: base -h below in game Y (h above in the app's), no rise height.
            assert!(close(base, h) && close(up, 0.0), "case {n}: base {base} up {up}");
        }
    }
}

#[test]
fn orig_fm_17c930_jump_curve_matches_the_game() {
    use kh2_game_core::motor::Jump;
    let Some(mut ee) = machine("oracle test of FM 0x17c930 (jump curve)") else { return };
    // FM 0x16c028 (the ground point the curve is measured from) answers a zero vector.
    ee.hooks.insert(0x16c028, |e: &mut Ee| e.gpr[2] = ZERO_VEC as u128);
    let mut r = Rng::new(0x17c930);
    for n in 0..CASES {
        let h = 1.0 + r.below(40000) as f32 / 100.0;
        let mut j = Jump::up(h, 0.0);
        j.t = r.below((j.rise * 300.0) as u64 + 1) as f32 / 100.0;
        zero(&mut ee, MOVER, 0x200);
        zero(&mut ee, ZERO_VEC, 0x10);
        let w = |o: u32, v: f32, ee: &mut Ee| ee.w32(MOVER + o, v.to_bits());
        w(0xd0, j.t, &mut ee);
        w(0xd4, j.rise, &mut ee);
        w(0xd8, 0.0, &mut ee);
        w(0xdc, -h, &mut ee);
        // The result is min(curve - ground, +0x120 x DAT_00349e10) (the branch's delay slot loads the limit): far
        // above so it never applies.
        w(0x120, 1.0e9, &mut ee);
        ee.call(0x17c930, &[0, MOVER as u64], &[], STACK).unwrap_or_else(|f| panic!("case {n}: {f:?}"));
        let game = -ee.fpr[0];
        let port = j.target_y();
        assert!((game - port).abs() <= 1e-3 * port.abs().max(1.0), "case {n}: h {h} t {} rise {}: game {game} lib {port}", j.t, j.rise);
    }
}

#[path = "common/actor.rs"]
mod actor;

/// FM 0x18ca48 (USA `FUN_0018b700`): the collapse of `EnemyReaction` values into classes (a jump table
/// at FM 0x36cc28), every byte, against `reaction_rules::reaction_class`.
#[test]
fn orig_fm_18ca48_reaction_class_matches_the_game() {
    use kh2_game_core::reaction_rules::reaction_class;
    let Some(mut ee) = machine("oracle test of FM 0x18ca48 (reaction class)") else { return };
    for raw in 0..=255u8 {
        let game = ee.call(0x18ca48, &[raw as u64], &[], STACK).expect("call") as u8;
        assert_eq!(game, reaction_class(raw), "reaction {raw}");
    }
}

/// FM 0x1c20e8 (USA `FUN_001bff00`): the `patn` conversion of a hit's reaction by the victim's status
/// bits, with a victim actor laid out (`common/actor.rs`: mover flags bit 2 and +0x160 bit 0x40 make it
/// air-like; FM 0x19c090, the riding test, answers 0) and an attacker (+0x588 bit 0x4000), the disc's
/// `patn` rows where FM 0x1d49510 points. Against `reaction_rules::effective_reaction`.
#[test]
fn orig_fm_1c20e8_reaction_conversion_matches_the_game() {
    use actor::{hit, obj};
    use kh2_game_core::reaction_rules::{effective_reaction, PatnContext};
    let Some(mut ee) = machine("oracle test of FM 0x1c20e8 (patn conversion)") else { return };
    let Some(mut d) = fm_disc() else { return };
    let game = d.game().expect("Sora loads");
    let rows = game.params.reaction.as_ref().expect("reaction data").patn.clone();
    assert!(!rows.is_empty());
    ee.hooks.insert(0x19c090, |e: &mut Ee| e.gpr[2] = 0);
    const TABLE: u32 = 0x018b_0000;
    const VICTIM_A: u32 = 0x018c_0000;
    const MOVER_A: u32 = 0x018d_0000;
    const ATTACKER_A: u32 = 0x018e_0000;
    const HIT_A: u32 = 0x018f_0000;
    actor::zero(&mut ee, TABLE, 8 + 32 * rows.len() as u32);
    ee.w32(TABLE + 4, rows.len() as u32);
    for (i, r) in rows.iter().enumerate() {
        let at = TABLE + 8 + 32 * i as u32;
        ee.write(at, &[r.status_bit]);
        ee.write(at + 1, &r.map);
    }
    ee.w32(0x1d49510, TABLE);
    let mut r = Rng::new(0x1c20e8);
    for n in 0..CASES {
        let status = ((r.below(1 << 31) as u32) * 2 + r.below(2) as u32) & [0xffff_ffff, 0x1ff, 0x184][r.below(3) as usize];
        let original = r.below(19) as u8;
        let air = [(0u32, 0u32), (0x40, 0), (0, 2)][r.below(3) as usize];
        let attacker_4000 = r.below(2) == 0;
        let has_attacker = r.below(4) != 0;
        actor::actor(&mut ee, VICTIM_A, MOVER_A, air.0, 0, air.1);
        actor::actor(&mut ee, ATTACKER_A, MOVER_A + 0x100, 0, if attacker_4000 { 0x4000 } else { 0 }, 0);
        actor::zero(&mut ee, HIT_A, 0x40);
        ee.w32(HIT_A + hit::ATTACKER, if has_attacker { ATTACKER_A } else { 0 });
        ee.write(HIT_A + hit::EFFECTIVE, &[original, original]);
        ee.call(0x1c20e8, &[status as u64, HIT_A as u64, VICTIM_A as u64], &[], STACK).unwrap_or_else(|f| panic!("case {n}: {f:?}"));
        let got = ee.r8(HIT_A + hit::EFFECTIVE);
        let _ = obj::FLAGS_160;
        let ctx = PatnContext { status, grounded: air == (0, 0), attacker_flag_4000: has_attacker && attacker_4000 };
        assert_eq!(got, effective_reaction(original, &rows, &ctx), "case {n}: status {status:#x} original {original} air {air:?} attacker {has_attacker}/{attacker_4000}");
    }
}

/// FM 0x1b73e8 (Final Mix only, called by the status set-up FM 0x1a11e0 and the level-up FM 0x1b72c0
/// on a `lvup` row): the row's AP byte (+7), times 1.5 in Critical for characters 1 and 14. Against
/// `enemy::critical_ap`, every AP byte, character and difficulty.
#[test]
fn orig_fm_1b73e8_critical_ap_matches_the_game() {
    let Some(mut ee) = machine("oracle test of FM 0x1b73e8 (Critical AP)") else { return };
    const ROW: u32 = 0x0190_0000;
    for diff in 0..=4u8 {
        ee.w32(0x32dfc8, diff as u32);
        for character in [0u32, 1, 2, 3, 13, 14, 15] {
            for ap in 0..=255u8 {
                ee.write(ROW, &[0, 0, 0, 0, 0, 0, 0, ap]);
                let game = ee.call(0x1b73e8, &[ROW as u64, character as u64], &[], STACK).expect("call") as u32 as i32;
                assert_eq!(game, kh2::enemy::critical_ap(ap, character, diff), "difficulty {diff} character {character} ap {ap}");
            }
        }
    }
}

/// FM 0x1b70d0 (USA `FUN_001b51e0`, the EXP payout's add): the experience total at FM 0x32f210 plus
/// the amount, held at 9,999,999 (no floor), then the members' level checks. Stand-ins (logged as
/// called, answering nothing): the level routines FM 0x188fe0, 0x176b58, 0x1b71e8, 0x1a8b68 and the
/// tail FM 0x188bb0. Against the add of `Sora::report_defeat` (`(exp + amount).min(9_999_999)`).
#[test]
fn orig_fm_1b70d0_experience_add_matches_the_game() {
    let Some(mut ee) = machine("oracle test of FM 0x1b70d0 (experience add)") else { return };
    for a in [0x188fe0u32, 0x176b58, 0x1b71e8, 0x1a8b68, 0x188bb0] {
        ee.hooks.insert(a, |e: &mut Ee| e.gpr[2] = 0);
    }
    let mut r = Rng::new(0x1b70d0);
    for n in 0..CASES {
        let total = [r.below(10_000_000) as i32, 9_999_999, 0, 9_999_000][r.below(4) as usize];
        let amount = [r.below(5000) as i32, r.below(20_000_000) as i32, 1][r.below(3) as usize];
        ee.w32(0x32f210, total as u32);
        ee.call(0x1b70d0, &[amount as u64], &[], STACK).unwrap_or_else(|f| panic!("case {n}: {f:?}"));
        let game = ee.r32(0x32f210) as i32;
        assert_eq!(game, (total + amount).min(9_999_999), "case {n}: {total} + {amount}");
    }
}
