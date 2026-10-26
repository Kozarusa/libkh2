//! Moving collision objects (round 10, like libsm64's surface objects): triangles the host places
//! with a transform it changes every tick; the collision world is the static surfaces plus every
//! object's triangles at its current place, and a body standing on an object rides with it.
//!
//! decided: the world is rebuilt from all triangles when an object is created, moved or deleted
//! (kh2-rs's `World` has no incremental insert; its moving pool `add_moving` takes COCT files and
//! the mover libkh2 uses does not sweep it, see docs/DEVIATIONS.md); riding: when Sora is grounded
//! and an object's top lies under his feet (within 12 cm, at the object's previous place), the
//! object's motion since then (translation and the yaw part of its rotation) moves him too, before
//! his tick.

use glam::{Mat4, Quat, Vec3, Vec3Swizzles};

use crate::{world_from_surfaces, Surface, World};

/// A placement: position (cm) and Euler rotation (radians, applied X, then Y, then Z), scale 1.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Transform {
    pub pos: Vec3,
    pub rot: Vec3,
}

impl Transform {
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_rotation_translation(Quat::from_rotation_z(self.rot.z) * Quat::from_rotation_y(self.rot.y) * Quat::from_rotation_x(self.rot.x), self.pos)
    }
}

pub struct Object {
    pub id: u32,
    pub tris: Vec<Surface>,
    pub now: Mat4,
    /// The place the last ride check saw.
    pub before: Mat4,
}

impl Object {
    fn placed(&self, m: Mat4) -> impl Iterator<Item = Surface> + '_ {
        self.tris.iter().map(move |t| Surface { v: t.v.map(|v| m.transform_point3(Vec3::from(v)).to_array()), ..*t })
    }
}

/// The static surfaces of a world (as triangles with their flags, materials and groups).
pub fn surfaces_of(w: &World) -> Vec<Surface> {
    w.tris.iter().enumerate().map(|(i, t)| Surface { v: t.v.map(|v| v.to_array()), flags: t.flags, group: w.group_of(i), material: t.material, _pad: 0 }).collect()
}

/// The world of `base` plus every object at its current place (the base's switched-off groups kept).
pub fn combined(base: &[Surface], objects: &[Object], groups: u64) -> World {
    let mut all = base.to_vec();
    for o in objects {
        all.extend(o.placed(o.now));
    }
    let mut w = world_from_surfaces(&all);
    w.enabled_groups = groups;
    w
}

/// The ride of a body with feet at `feet`: the motion of the object it stands on since the last
/// check, as (new position, yaw added). `None` when it stands on none.
pub fn ride(objects: &[Object], feet: Vec3, grounded: bool) -> Option<(Vec3, f32)> {
    if !grounded {
        return None;
    }
    for o in objects.iter().filter(|o| o.now != o.before) {
        let top = o.placed(o.before).filter_map(|t| {
            let [a, b, c] = t.v.map(Vec3::from);
            let n = (b - a).cross(c - a);
            if n.y <= 0.0 {
                return None;
            }
            // The triangle's height under (x, z), if (x, z) lies inside its projection.
            let (p, q, r) = (a.xz(), b.xz(), c.xz());
            let f = feet.xz();
            let d = (q - p).perp_dot(r - p);
            if d.abs() < 1e-9 {
                return None;
            }
            let u = (f - p).perp_dot(r - p) / d;
            let v = (q - p).perp_dot(f - p) / d;
            (u >= 0.0 && v >= 0.0 && u + v <= 1.0).then_some(a.y + u * (b.y - a.y) + v * (c.y - a.y))
        });
        if top.into_iter().any(|y| (feet.y - y).abs() <= 12.0) {
            let delta = o.now * o.before.inverse();
            let moved = delta.transform_point3(feet);
            let fwd = delta.transform_vector3(Vec3::Z).with_y(0.0);
            let yaw = if fwd.length_squared() > 1e-9 { fwd.x.atan2(fwd.z) } else { 0.0 };
            return Some((moved, yaw));
        }
    }
    None
}
