# For host authors

How to drive libkh2 from an engine (C, C#, Python, Unity, your own). The exact signatures are in
`include/kh2.h` and `docs/API-REFERENCE.md`; this page is the order of things and the traps.

## One frame

The game logic runs at 60 Hz. Call once per 1/60 s (or `kh2_tick_field` per 30 Hz frame, which
ticks twice), in this order, for each Sora handle:

1. `kh2_tick(h, &inputs, &state)`: Sora (movement, attacks, spells, drive, reactions).
2. Your hit tests: when `state.attack_active`, test `kh2_get_attack_volumes` against your own
   targets and call `kh2_report_hit` on contact (library enemies are tested by the library itself).
3. `kh2_party_tick(h)`: Donald and Goofy (follow, battle AI, their scripts, limits).
4. `kh2_enemies_tick(h)`: the enemies' scripts and hits both ways, then the area run (waves,
   signals), the mission manager (Struggle) and the summons.
5. `kh2_camera_tick(h, &cam_in, &view)` if you use the library's camera; feed
   `view.look - view.pos` (x, z) back as next tick's `cam_look_x / cam_look_z`.
6. Drain the event queues until they return 0: `kh2_poll_event`, `kh2_poll_member_event`,
   `kh2_poll_enemy_event`, `kh2_poll_limit_event`, `kh2_poll_summon_event`,
   `kh2_area_take_events`, `kh2_mission_take_sounds`. Each queue keeps at most 1024 events (the
   mission's sounds 256); unread events are dropped oldest first.
7. Draw: `state.pos` / `state.yaw` and `kh2_get_bones(h, 0, 1, mats, n)` (world space) for Sora,
   `kh2_party_state` / `kh2_party_bones`, `kh2_enemy_state` / `kh2_enemy_bones`,
   `kh2_summon_state` / `kh2_summon_bones`, `kh2_mission_orbs`.

## Space and units

- Centimetres, right-handed, **Y up**. Yaw is radians about +Y; yaw 0 faces **+Z**, forward is
  `(sin yaw, 0, cos yaw)`.
- The game's own space is Y down with Z mirrored; the library converts everything (areas,
  enemies, spawn points) to the space above.
- Unity (left-handed, metres): mirror X and scale by 0.01, swap triangle vertices 1 and 2, negate
  yaw; bone matrices as `S * M * S` with `S = diag(-1, 1, 1)` (`unity/Kh2.cs` has the helpers).
- Bone matrices are column-major 4x4 floats.

## Collision (surfaces)

`kh2_surfaces_load(tris, n)` takes `kh2_surface` records: `v[3][3]` the corners (cm),
`flags` the COCT attribute word, `group` 0..63, `material`.

- The normal `(v1 - v0) x (v2 - v0)` points to the side Sora stands on (floors: up). Wrong
  winding gives a floor he falls through.
- A triangle more than 60 degrees from up is a wall; it blocks only with `KH2_SURF_WALL` in
  `flags`, and only from its front.
- `kh2_set_collision(group, solid)` switches a group (doors, bridges).
- Or `kh2_surfaces_load_map("tt04")`: the area's own collision from the disc. Loading replaces the
  previous surfaces for every Sora.

## A minimal loop in C

```c
#include "kh2.h"

int main(void) {
    if ((kh2_version() >> 8) != (KH2_VERSION >> 8)) return 1;   /* header and DLL agree */
    if (kh2_init(NULL) != 0) return 1;                          /* NULL: the KH2_ISO path */
    kh2_surfaces_load_map("tt04");
    int32_t h = kh2_create(0, 0, 0);
    kh2_party_add(h, 0);                                         /* Donald */
    kh2_area_run(h, "tt04", 184);                                /* Seifer's Struggle match */
    kh2_state st;
    kh2_mission m;
    for (int t = 0; t < 60 * 70; t++) {
        kh2_inputs in = {0};
        in.cam_look_z = 1.0f;
        in.stick_y = 1.0f;
        if (t % 30 == 0) in.buttons |= KH2_BTN_ATTACK;
        kh2_tick(h, &in, &st);
        kh2_party_tick(h);
        kh2_enemies_tick(h);
        kh2_event ev;
        while (kh2_poll_event(h, &ev) == 1) { /* sounds, effects, level ups ... */ }
        if (kh2_mission_state(h, &m) == 1 && m.result) break;
    }
    kh2_delete(h);
    kh2_terminate();
    return 0;
}
```

## Common mistakes

- Ticking at the render rate. The rules count 60 Hz ticks; at 144 Hz everything runs 2.4 times
  too fast. Accumulate time and tick in whole 1/60 s steps.
- Forgetting `cam_look`: the stick is relative to the camera; a zero look direction makes Sora
  run along +Z whatever the camera shows.
- Holding a button state instead of the real pad: presses are edges between ticks; send the held
  state every tick, not a one-tick pulse per frame you render.
- Not draining the event queues: events are dropped when a queue is full.
- Calling `kh2_enemies_tick` before `kh2_tick`: the enemies then react to last tick's Sora.
- Placing Sora at `(0, 0, 0)` in an area whose floor is elsewhere: use the area's spawn point
  (an area run's group player point, or your level's), or he falls.
- Expecting meshes or textures: libkh2 exports skeletons and motions only; the host draws its own
  model of Sora (and of the enemies).
- Shipping game data: the library reads the user's own disc image at run time; nothing from the
  disc belongs in your build (`scripts/package.ps1` checks its own output).
- Treating every call as cheap: `kh2_init` and the first spawn of an enemy kind read the disc
  (tens of milliseconds); spawn early, prune with `kh2_enemies_prune`.
- Threads: every call locks one library state; tick all Sora instances from one thread.
- Errors: a call that fails returns -1 (or 0 / NULL where documented); `kh2_last_error` says why.
  No call panics across the ABI.

## Areas: townspeople, field objects, platforms

`kh2_area_setup(area, map, btl, evt, entrance, &info)` runs the area's three set-up programs the
game's way (kh2-rs `run_area`) and lists every object the loaded spawn groups create
(`kh2_area_get_object`): model, `00objentry` id and type, place, talk id, the command its reaction
part offers (0x1f talk, 0x1e examine), plus the player's start, the camera type and the music. The
program numbers are the story's choice (the host picks them; townspeople usually come with the evt
programs, field objects and save points with map).

What the host does with them:
- Enemies: run them in the library (`kh2_area_run` with the btl program, or `kh2_enemy_spawn`).
- Townspeople and field objects: spawn and draw them yourself (models are not exported). When Sora
  interacts with one (your interact button near it), call `kh2_talk_begin` with its place, yaw,
  talk id and part; play your dialogue for the talk id; `kh2_talk_end` when it closes.
- Save points, chests, doors and exits: yours (no menus, prizes from chests, or area jumps here).
- Moving platforms: the library has no moving collision. What it has is the game's collision
  groups: scripts switch them (hosts 1.36 / 1.37 by group, 1.46 / 1.47 by the area's
  `collision_mask`); the library applies those switches and sends `KH2_CUE_COLLISION_GROUP` so the
  host shows or hides the matching mesh group. A host with its own platforms switches groups
  with `kh2_set_collision` or reloads triangles with `kh2_surfaces_load`.

## Drawing, HUD, audio, platforms (1.1)

- Draw any body without an exporter: `kh2_mesh_prepare` + `kh2_mesh_get` after the ticks, textures
  once per id with `kh2_texture_get` (`docs/MESHES.md`).
- HUD: `kh2_hud_load` once; per tick `kh2_hud_input` (your menu buttons), `kh2_hud_tick` after
  `kh2_enemies_tick`, `kh2_hud_draw_list` into your 2D pass (512 x 448 screen), `kh2_hud_poll_event`
  for items and forms (`docs/HUD.md`).
- Audio: `kh2_audio_init` once; in your audio callback `kh2_audio_tick(h, buf, frames)` (48 kHz
  stereo 16-bit).
- Platforms: `kh2_surface_object_create` once, `kh2_surface_object_move` before `kh2_tick` each
  tick; Sora rides them.
