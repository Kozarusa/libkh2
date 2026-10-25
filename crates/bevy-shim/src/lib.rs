//! A stand-in for the `bevy` crate, used under the dependency name `bevy` by `libkh2-pure` only.
//!
//! kh2-rs's movement and collision modules (`kh2-game-core/src/motor.rs`, `world.rs`) are plain
//! math over glam types; they import `bevy::prelude::*` for `Vec3`, `Vec2`, `Quat`, `default()` and
//! the `Resource` derive. This crate provides exactly those names so the files compile unchanged,
//! without pulling Bevy into the library. Bevy 0.19 re-exports glam 0.32, the same types.

/// `bevy::math` (glam).
pub mod math {
    pub use glam::*;
}

pub mod prelude {
    pub use glam::*;
    pub use libkh2_bevy_shim_derive::{Component, Message, Resource};

    /// An entity id (the enemy modules carry one in a few records; no ECS behind it).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
    pub struct Entity(pub u64);

    impl Entity {
        pub const PLACEHOLDER: Entity = Entity(u64::MAX);
    }

    /// Stand-ins for render types named in a field of `shared::enemy::AiBrain` (never used here).
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct AssetId<T>(std::marker::PhantomData<T>);
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct StandardMaterial;
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum AlphaMode {
        Opaque,
        Blend,
    }

    /// `bevy::prelude::default`.
    pub fn default<T: Default>() -> T {
        T::default()
    }
}
