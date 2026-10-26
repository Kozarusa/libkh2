//! A player / enemy actor laid out in kh2-ee's RAM for oracle tests against the Final Mix executable:
//! the field offsets the Final Mix disassembly reads (damage FM 0x1a1e20, the jump mover FM 0x17c930,
//! the form timer FM 0x1a1b68, the reaction conversion FM 0x1c20e8). Only the fields a test sets are
//! written; the rest of the record is zero.

#![allow(dead_code)]

use kh2_ee::Ee;

/// Actor (object) fields.
pub mod obj {
    /// Its status block (`+0x48c`, the record the damage and form code read).
    pub const STATUS: u32 = 0x48c;
    /// Its mover object; the mover's flags word is at +8 (bit 2: the air-like mover).
    pub const MOVER: u32 = 0xc;
    pub const MOVER_FLAGS: u32 = 8;
    /// Controller flags (+0x160: bit 0x20 / 0x1000000 no gravity, bit 0x40 air-like).
    pub const FLAGS_160: u32 = 0x160;
    /// Actor class flags (+0x588: bit 4 party member, bit 0x80 player-only rules, bit 0x4000 the
    /// attacker flag the reaction conversion reads).
    pub const FLAGS_588: u32 = 0x588;
    /// The form word (+0xb50: 6 is Anti on Final Mix).
    pub const FORM: u32 = 0xb50;
    /// The combo hit counter.
    pub const HITS: u32 = 0xb04;
    /// The jump mover's clock, rise time, base and rise (game Y down).
    pub const JUMP_CLOCK: u32 = 0xd0;
    pub const JUMP_RISE_TIME: u32 = 0xd4;
    pub const JUMP_BASE: u32 = 0xd8;
    pub const JUMP_RISE: u32 = 0xdc;
}

/// Status block fields (Final Mix layout: 4 bytes later than USA from +0x1f8 on).
pub mod status {
    pub const HP: u32 = 0;
    pub const MAX_HP: u32 = 4;
    pub const DEFENSE: u32 = 0x18c;
    pub const MAX_TAKEN: u32 = 0x1a0;
    pub const MIN_TAKEN: u32 = 0x1a4;
    pub const ELEMENTS: u32 = 0x1a8;
    pub const FORM_TIMER_LEFT: u32 = 0x1b4;
    pub const FORM_TIMER_FULL: u32 = 0x1b8;
    pub const ABILITY_BLOCK: u32 = 0x1cc;
    pub const DEFENDER: u32 = 0x1fc;
    pub const LOW_HP_RATIO: u32 = 0x240;
    /// The object the status belongs to.
    pub const OBJECT: u32 = 0x25c;
}

/// Hit record fields (the second argument of the damage routine and of the reaction conversion).
pub mod hit {
    /// The attacker object.
    pub const ATTACKER: u32 = 0x10;
    /// Flags (bit 4: ineffective).
    pub const FLAGS: u32 = 0x18;
    /// The effective reaction (written) and the row's original reaction (read).
    pub const EFFECTIVE: u32 = 0x26;
    pub const ORIGINAL: u32 = 0x27;
}

pub fn zero(ee: &mut Ee, at: u32, len: u32) {
    ee.write(at, &vec![0u8; len as usize]);
}

/// An actor at `at` with its mover at `mover`: flags +0x160, +0x588 and the mover's flags.
pub fn actor(ee: &mut Ee, at: u32, mover: u32, flags_160: u32, flags_588: u32, mover_flags: u32) {
    zero(ee, at, 0xc00);
    zero(ee, mover, 0x40);
    ee.w32(at + obj::MOVER, mover);
    ee.w32(mover + obj::MOVER_FLAGS, mover_flags);
    ee.w32(at + obj::FLAGS_160, flags_160);
    ee.w32(at + obj::FLAGS_588, flags_588);
}
