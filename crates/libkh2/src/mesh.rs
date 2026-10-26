//! Runtime meshes (round 10, as libsm64 hands Mario's triangles to the host): the model of a body
//! (from its own `obj/<model>.mdlx`, kh2-rs's `kh2_model` decoder: VIF strips, the textures with
//! their CLUTs unswizzled and the PS2 alpha 0..0x80 mapped to 0..255) skinned on the CPU by the
//! body's current bones, so a host draws without an exporter.
//!
//! Skinning: `kh2_model` gives the vertices posed in the rest pose with up to four (bone, weight)
//! pairs; a vertex is `sum w * (root * pose[j] * inverse(rest[j])) * v`. Normals are not in the
//! model files; decided: they are the area-weighted average of the skinned triangles' normals.

use glam::{Mat4, Vec3};

use crate::anim::Rig;

/// One draw call: a run of `indices` with one texture.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sub {
    pub first: u32,
    pub count: u32,
    /// Index into the model's textures (the library's texture table maps it to a global id).
    pub texture: u32,
    pub transparent: bool,
}

/// A skinned mesh in world (or model) space.
#[derive(Clone, Debug, Default)]
pub struct Skinned {
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    /// RGBA, 1.0 = the PS2 neutral 0x80.
    pub colors: Vec<[f32; 4]>,
    pub indices: Vec<u32>,
    pub subs: Vec<Sub>,
}

/// The mesh of `rig` posed by `pose` (model space, one matrix per bone) under `root`.
pub fn skin(rig: &Rig, pose: &[Mat4], root: Mat4) -> Skinned {
    let rest = kh2_model::rest_pose(&rig.model.bones);
    let mats: Vec<Mat4> = rest.iter().enumerate().map(|(j, r)| root * pose.get(j).copied().unwrap_or(*r) * r.inverse()).collect();
    let mut out = Skinned::default();
    for m in &rig.model.meshes {
        let base = out.positions.len() as u32;
        for (i, p) in m.positions.iter().enumerate() {
            let v = Vec3::from(*p);
            let mut acc = Vec3::ZERO;
            let mut total = 0.0;
            if let (Some(j), Some(w)) = (m.joints.get(i), m.weights.get(i)) {
                for k in 0..4 {
                    if w[k] > 0.0
                        && let Some(mat) = mats.get(j[k] as usize)
                    {
                        acc += mat.transform_point3(v) * w[k];
                        total += w[k];
                    }
                }
            }
            let p = if total > 0.0 { acc / total } else { root.transform_point3(v) };
            out.positions.push(p.to_array());
            out.uvs.push(m.uvs.get(i).copied().unwrap_or_default());
            out.colors.push(m.colors.get(i).copied().unwrap_or([1.0; 4]));
        }
        let first = out.indices.len() as u32;
        out.indices.extend(m.indices.iter().map(|k| k + base));
        out.subs.push(Sub { first, count: m.indices.len() as u32, texture: m.texture as u32, transparent: m.transparent });
    }
    // Normals: area-weighted sums of the triangles' normals.
    let mut n = vec![Vec3::ZERO; out.positions.len()];
    for t in out.indices.as_chunks::<3>().0 {
        let [a, b, c] = t.map(|k| Vec3::from(out.positions[k as usize]));
        let f = (b - a).cross(c - a);
        for k in t {
            n[*k as usize] += f;
        }
    }
    out.normals = n.into_iter().map(|v| v.normalize_or(Vec3::Y).to_array()).collect();
    out
}
