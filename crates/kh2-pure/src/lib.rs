//! kh2-rs's pure movement code, compiled from its own source files (read-only, not copied).
//!
//! `kh2-game-core` depends on Bevy as a whole, but its `motor` (ground mover, jump mover, facing
//! turn, sphere sweep; the ground mover functions are oracle-tested against the original
//! executable in kh2-rs) and `world` (collision queries over COCT polygons) modules only use
//! glam math. They are included here with `#[path]`, against the `bevy` shim crate, so libkh2 runs
//! exactly that code. The crate is depended on under the name `kh2_game_core`, so kh2-player's
//! `controller_rules.rs` (which imports `kh2_game_core::motor`) can be included the same way.

#[path = "../../../../kh2-rs/crates/kh2-game-core/src/motor.rs"]
pub mod motor;

#[path = "../../../../kh2-rs/crates/kh2-game-core/src/world.rs"]
pub mod world;

/// The one helper `motor.rs` takes from `reaction_rules` (`GroundConsts::from_exe`,
/// `MoverTable::from_exe`): a little-endian float at a virtual address of the ELF executable.
pub mod reaction_rules {
    /// The word at virtual address `va` of a 32-bit little-endian ELF image, through its program
    /// headers (`None` when no loadable segment holds it).
    pub fn exe_f32(elf: &[u8], va: u32) -> Option<f32> {
        exe_bytes(elf, va, 4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// `len` bytes at virtual address `va` of a 32-bit little-endian ELF image.
    pub fn exe_bytes(elf: &[u8], va: u32, len: usize) -> Option<&[u8]> {
        let u16_at = |o: usize| elf.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
        let u32_at = |o: usize| elf.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
        if elf.get(0..4)? != b"\x7fELF" {
            return None;
        }
        let (phoff, phentsize, phnum) = (u32_at(0x1c)? as usize, u16_at(0x2a)?, u16_at(0x2c)?);
        for i in 0..phnum {
            let p = phoff + i * phentsize;
            let (kind, off, vaddr, filesz) = (u32_at(p)?, u32_at(p + 4)?, u32_at(p + 8)?, u32_at(p + 16)?);
            if kind == 1 && va >= vaddr && (va - vaddr) as usize + len <= filesz as usize {
                let start = off as usize + (va - vaddr) as usize;
                return elf.get(start..start + len);
            }
        }
        None
    }
}
