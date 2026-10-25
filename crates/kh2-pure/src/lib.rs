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

/// Sora's magic rules (`magic_rules.rs`: `magc` row choice, availability, cast motion and kind,
/// MP payment and charge).
#[path = "../../../../kh2-rs/crates/kh2-game-core/src/magic_rules.rs"]
#[allow(dead_code)]
pub mod magic_rules;

/// kh2-rs `shared::drive::rules` (drive points, form timer, form choice, entry checks, memt merge,
/// weapon lookup), extracted from kh2-rs's source by `build.rs`.
#[allow(dead_code, unused_imports, clippy::all)]
pub mod drive_rules {
    use kh2_tables::player::{self, form, DriveForm, FormLevels, Memt, Sstm};
    use kh2_tables::tables as t;
    /// `data::KINGDOM_KEY` (an item id the rules may name).
    pub const KINGDOM_KEY: u16 = 41;
    include!(concat!(env!("OUT_DIR"), "/drive_rules.rs"));
}

/// kh2-rs's item effect and defeat payout, extracted by `build.rs`.
#[allow(dead_code, clippy::all)]
pub mod items {
    include!(concat!(env!("OUT_DIR"), "/items.rs"));

    impl Vitals {
        /// The HP change of `item_effect` without its private helpers (for callers outside).
        pub fn hp_now(&self) -> i32 {
            self.hp
        }
    }
}

/// kh2-rs `shared::abilities` (the ability block and its rules), transformed by `build.rs`.
#[allow(dead_code, unused_imports, clippy::all)]
pub mod abilities {
    include!(concat!(env!("OUT_DIR"), "/abilities.rs"));
}

/// The party members' field rules (`party_rules.rs`: follow points, follow step, steer, warp).
#[path = "../../../../kh2-rs/crates/kh2-game-core/src/party_rules.rs"]
#[allow(dead_code)]
pub mod party_rules;

/// kh2-rs `party_battle` (target choice, action records and choice, approach), from its source by `build.rs`.
#[allow(dead_code, clippy::all)]
pub mod party_battle {
    include!(concat!(env!("OUT_DIR"), "/party_battle.rs"));
}

/// kh2-rs `rng` (the game's generator and the party members' `getRand` / roll), from its source by `build.rs`.
#[allow(dead_code, clippy::all)]
pub mod rng {
    include!(concat!(env!("OUT_DIR"), "/rng.rs"));
}
