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

/// Hit, push, blow and clash rules (`reaction_rules.rs`, pure; `motor.rs` takes `exe_f32` from it).
#[path = "../../../../kh2-rs/crates/kh2-game-core/src/reaction_rules.rs"]
pub mod reaction_rules;

/// The field camera's rules (`camera_rules.rs`: follow, lock-on, look-around, collision, springs;
/// oracle-tested in kh2-rs, `tests/orig_l2_p2.rs`).
#[path = "../../../../kh2-rs/crates/kh2-game-core/src/camera_rules.rs"]
pub mod camera_rules;
