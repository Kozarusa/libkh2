//! A stand-in for the `bevy` crate, used under the dependency name `bevy` by `libkh2-pure` only.
//!
//! kh2-rs's movement and collision modules (`kh2-game-core/src/motor.rs`, `world.rs`) are plain
//! math over glam types; they import `bevy::prelude::*` for `Vec3`, `Vec2`, `Quat`, `default()` and
//! the `Resource` derive. This crate provides exactly those names so the files compile unchanged,
//! without pulling Bevy into the library. Bevy 0.19 re-exports glam 0.32, the same types.

pub mod prelude {
    pub use glam::*;
    pub use libkh2_bevy_shim_derive::Resource;

    /// `bevy::prelude::default`.
    pub fn default<T: Default>() -> T {
        T::default()
    }
}
