# Unity sample scene (description; not built in this repository)

A minimal scene that drives Sora with libkh2 and draws him with a glb from kh2-extras.

## Assets

* `Assets/Plugins/kh2.dll` (from `cargo build --release -p libkh2`; `libkh2.so` / `libkh2.dylib`
  on Linux / macOS, renamed `kh2` by Unity's import settings).
* `Assets/Kh2.cs` (from `unity/Kh2.cs`).
* `Assets/Sora/P_EX100.glb` and `W_EX010.glb` (exported from your own disc by
  `kh2-extras export models`, imported with glTFast). Not redistributable; keep them out of
  version control.
* The project's Fixed Timestep set to 1/60 (Project Settings > Time).

## Hierarchy

```
Scene
├── Main Camera            (driven: kh2_camera_tick each FixedUpdate)
├── Level                  (CollisionRoot: MeshColliders of floors and walls)
│   ├── Floor              (a 40 x 40 m plane)
│   └── Walls              (boxes; their side faces become KH2_SURF_WALL triangles)
├── Target                 (a capsule; its position goes to kh2_set_target while Q is held)
└── Sora                   (Kh2Sora component: IsoPath, CollisionRoot = Level, Cam = Main Camera)
    └── P_EX100            (the glb; its joints are written from kh2_get_bones in LateUpdate)
        └── hand bone 178
            └── W_EX010    (the keyblade glb, joints from kh2_get_bones(h, 1, 0, ...))
```

## Script flow

1. `Start`: `kh2_init(IsoPath)`, send the colliders as triangles (`Kh2Sora` does this, mirroring X
   and swapping vertices 1 and 2), `kh2_create` at the GameObject's position, `kh2_camera_reset(h, 0)`.
2. `FixedUpdate`: read the left stick and the buttons (J attack, Space jump, K guard, 1-4 magic,
   F drive Valor, R revert), call `kh2_tick`, then `kh2_camera_tick` with the right stick, and set
   the next tick's `cam_look` from `view.look - view.pos`. Drain `kh2_poll_event`: on
   `KH2_EV_SPELL_FIRED` spawn a particle effect at the hand bone, on `KH2_EV_FORM_CHANGED` swap the
   glb for the form's model.
3. While `state.attack_active`, test `kh2_get_attack_volumes` against the Target's collider; on
   contact call `kh2_report_hit_atkp(h, volume.attack_id)` once per swing.
4. A key (H) calls `kh2_hit_taken(h, 0, 5, dir...)` to try the hurt and blow reactions.
5. `LateUpdate`: `kh2_get_bones(h, 0, 0, mats, n)`, convert each with `Kh2.ToUnity(mats, 16 * i)`
   and assign the joint's local matrix relative to its parent (`kh2_bone_parents`), see
   `docs/SKINNING.md`. Set the Main Camera from the last camera view (converted with `Kh2.ToUnity`).
6. `OnDestroy`: `kh2_delete`, `kh2_terminate`.
