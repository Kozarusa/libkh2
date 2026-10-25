# Skinning Sora with libkh2 and kh2-extras glb files

libkh2 does not export meshes. A host gets Sora's mesh from `kh2-extras` (a sibling research
tool built on kh2-rs: `kh2-extras export models --iso <disc> --out <dir>` writes `.glb` files of
the user's own disc into a folder outside any repository) and drives its skeleton with libkh2's
bone matrices. This page says how the two line up. It was written from kh2-extras' source
(`crates/kh2-export/src/model.rs`); the combination itself is untested here.

## How the glb is built

* Node `i` is bone `i` of the MDLX (bone nodes come first), with the same parent indices that
  `kh2_bone_parents` returns.
* Axes are kept: kh2 model space is right-handed, Y up, feet at 0. Only translations are scaled by
  0.01 (centimetres to metres).
* The skin's inverse bind matrices are the inverses of `kh2_model::rest_pose`, the same matrices
  `kh2_get_rest_bones` returns (before the 0.01 scale).

## Driving the glb skeleton

Per tick, after `kh2_tick`:

1. `kh2_get_bones(h, 0, 0, mats, n)`: model-space bone matrices relative to Sora's root
   (the evaluated rig with IK and constraints, cross-fades included).
2. For each bone, convert to the glb space: `M' = S * M * S^-1` with `S = scale(0.01)` (only the
   translation column changes: multiply it by 0.01).
3. Set each joint node's local transform to `parent'^-1 * M'` (or write `M'` as the joint's world
   matrix under a root node placed at `kh2_state.pos * 0.01` with rotation `yaw` about Y).
4. The keyblade: `kh2_get_bones(h, 1, 0, ...)` is relative to Sora's hand bone 178; its glb
   (`W_EX010.glb`) is parented under that bone.

World space instead: `kh2_get_bones(h, which, 1, ...)` already includes position and yaw.

## Unity

Unity is left-handed. Mirror X on the way in: `M_unity = F * M' * F` with `F = diag(-1, 1, 1, 1)`
(`Kh2.ToUnity(float[], int)` in `unity/Kh2.cs` does both the mirror and the scale). Import the glb
with a glTF importer (glTFast or UnityGLTF), which mirrors the same axis, so the joint order and
the bind pose match. Drive the joints in `LateUpdate` after `FixedUpdate` ran `kh2_tick`.

## Blender

Blender imports the glb with Y-up converted to Z-up (the importer's default). Apply the same axis
conversion to libkh2's matrices (`C * M' * C^-1`, `C` the Y-up to Z-up rotation) or import with
"+Y Up" unchecked and keep them as they are. For offline animation, `kh2_sample_motion` samples
any slot at any frame without ticking Sora; `kh2_motion_info` gives its frame range and rate
(`fps / 60` frames per 60 Hz tick). Root motion comes back separately as (x, y, z, yaw) and is not
in the bone matrices, matching how the animator draws (poses relative to the root).

## Drive forms

Valor, Master and Final hold a second keyblade: `kh2_get_bones(h, 2, 0, ...)`, relative to body
bone 86 (the left hand); its glb is the same keyblade model.


A form swaps Sora's body (`P_EX100_BTLF` etc.). After `KH2_EV_FORM_CHANGED`, load the form's glb
(`kh2_drive_objects` gives the objentry id; the model name is the form model) and rebind; the bone
count can differ from the base body's.
