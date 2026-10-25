//! libkh2: Sora's controller from Kingdom Hearts II as an embeddable library (libsm64-style).
//!
//! The host passes static collision triangles and one input record per 60 Hz tick; the library
//! answers Sora's position, facing, state, motion and attack windows, and his posed skeleton.
//! Game data is read from the user's own disc at run time ([`data::Disc`]); see `DESIGN.md` for
//! what is reused from kh2-rs and what is new. The C ABI is in [`ffi`] and `include/kh2.h`.

pub mod anim;
pub mod camera;
pub mod data;
pub mod drive;
pub mod enemy;
pub mod ffi;
pub mod party;
pub mod replay;
pub mod friend;
pub mod limit;
pub mod save;
pub mod sora;
pub mod spell;

/// kh2-rs `kh2-player/src/controller_rules.rs` (stick through the camera view, walk/run split,
/// wanted yaw; oracle-tested against the original in kh2-rs), compiled from kh2-rs's source.
#[path = "../../../../kh2-rs/crates/kh2-player/src/controller_rules.rs"]
#[allow(dead_code)]
pub mod controller_rules;

pub use kh2_game_core::world::World;

/// The attribute word bit that makes a wall stop Sora (COCT PARTY_COLLIDE, `kh2_map::stops_party`).
pub const SURF_WALL: u32 = kh2_map::flags::PARTY_COLLIDE;

/// One collision triangle from the host.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Surface {
    /// Vertices in world units (centimetres, Y up). The face normal `(v1 - v0) x (v2 - v0)` must point
    /// to the side Sora stands on: up for floors, toward the walkable side for walls (walls are
    /// one-sided, as COCT walls are).
    pub v: [[f32; 3]; 3],
    /// The COCT attribute word. Walls stop Sora when `flags & SURF_WALL` is set (and the low nibble
    /// is not 4, bit 0x100000 clear); floors and ceilings are found by their normal whatever the flags.
    pub flags: u32,
    /// Mesh group 0..63: the visibility-word bit that switches the triangle on and off.
    pub group: u8,
    /// Ground material (byte 0 of a COCT polygon; footstep kind), reported back on the ground.
    pub material: u8,
    pub _pad: u16,
}

/// Builds the collision world from host triangles: a COCT-shaped polygon list with one mesh per
/// group, handed to kh2-rs's `World::from_coct` (the same path an area's own COCT takes).
pub fn world_from_surfaces(tris: &[Surface]) -> World {
    let meshes = (0..64u8).map(|g| kh2_map::Mesh { group: g, ..Default::default() }).collect();
    let polygons = tris
        .iter()
        .filter_map(|t| {
            let [a, b, c] = t.v.map(glam::Vec3::from);
            let n = (b - a).cross(c - a);
            if n.length_squared() < 1e-12 || !n.is_finite() {
                return None;
            }
            let n = n.normalize();
            Some(kh2_map::Polygon { verts: [t.v[0], t.v[1], t.v[2], [0.0; 3]], quad: false, normal: n.to_array(), d: -n.dot(a), flags: t.flags, material: t.material, mesh: u16::from(t.group.min(63)) })
        })
        .collect();
    World::from_coct(&kh2_map::Coct { polygons, meshes, ..Default::default() })
}
