# libkh2 design

libkh2 is a libsm64-style library: a host engine (Unity, Godot, a C program) owns the world and the
drawing; libkh2 owns Sora's controller. The host passes static collision triangles and per-tick
input; libkh2 answers Sora's position, facing, velocity, state, motion slot and frame, the live
attack windows and, optionally, the posed skeleton. Game data is read from the user's own disc at
`kh2_init` and never stored in this repository or in the built library.

The code that decides how Sora moves is kh2-rs's (`..\kh2-rs`, a read-only sibling). libkh2 adds
glue, a C ABI and examples. This document records what is reused, what is new, and why.

## 1. What kh2-rs has, and what is tied to Bevy

| Piece (kh2-rs) | Crate | Bevy? | Oracle-tested against the original executable? |
|---|---|---|---|
| `motor.rs`: ground mover `ground_step`/`ground_motion` (0x17b040, 0x17af10), `turn_angle`/`fix_radian` (0x17bc30, 0x17bbc0), jump/fall curve `Jump`, `jump_control`, `jump_dy`, `air_velocity`, `Motor::move_by`, `sweep_wall` | kh2-game-core | only `use bevy::prelude::*` for glam types | ground mover: yes (`kh2-game-core/tests/orig_l2_p3.rs`); turn: yes (`kh2-player/tests/orig_l2_p1.rs`); jump curve, air blend, slide: read from the decompilation, not oracle-run |
| `world.rs`: `World` collision (floor probe, ceiling, capsule slide with the party wall filter `stops_party`, mesh groups) | kh2-game-core | same (`Resource` derive, `default()`) | the wall filter and group gating are disc-checked tests; the slide itself is kh2-rs's approximation (the game's exact `Oct` walker exists but kh2-rs leaves it unwired, WP-L1-P10) |
| `controller_rules.rs`: stick through the camera view, walk/run split, wanted yaw (0x177360, 0x1774d8, 0x106f78, 0x16a948) | kh2-player | no (imports `kh2_game_core::motor`) | yes (`orig_l2_p1.rs`) |
| `kh2-anim`: motion sets, motions, triggers, rig/pose evaluation, cross-fade `blend` (0x17dd38 family), attack table `moveset` (`Chain::press` 0x176b00, `check` 0x19f1b0, `select` 0x1cbcf8/0x1cbf48, `explicit` 0x177fa8) | kh2-anim | no | blend constants checked against the executable; selection is a decompilation reading (no oracle run) |
| `kh2-tables`: `pref` prty/plyr/sstm/fmab, `objentry`, `ptya` subfile | kh2-tables | no | layouts sheet-validated |
| `kh2-map`, `kh2-model`, `kh2-archive`, `kh2-bar` | | no | |
| `sora.rs::tick` (the 900-line per-tick state machine), `skel.rs::Animator` (motion playback component), `shared/sora.rs` (`Sora`, `Pad`, `Abilities`) | kh2-player / kh2-game-core | yes: ECS queries over enemies, the camera, materials, weapons, messages | — |

kh2-game-core as a crate depends on all of Bevy 0.19 (render, window, audio) and on kh2-fx,
kh2-audio and the rest; kh2-player adds kh2-enemy and kh2-sound.

## 2. Decision

### (a) Pure pieces: reused by path, compiled unchanged

* Pure crates (`kh2-anim`, `kh2-tables`, `kh2-map`, `kh2-model`, `kh2-archive`, `kh2-bar`) are normal
  path dependencies.
* `motor.rs` and `world.rs` live in the Bevy crate kh2-game-core but only use glam. libkh2 compiles
  **those two files from kh2-rs's source tree** (`#[path]` in `crates/kh2-pure`), against a 20-line
  `bevy` shim crate (`crates/bevy-shim`: re-exports glam 0.32, `default()`, a no-op `Resource`
  derive). Nothing is copied; a change in kh2-rs is picked up on the next build. The shim crate is
  depended on under the name `kh2_game_core`, so `controller_rules.rs` (kh2-player) compiles in place
  the same way inside libkh2.
* The included files carry kh2-rs's own unit tests, which need the rest of kh2-game-core; the shim
  crate is built with `test = false`. Those tests run in kh2-rs.

Measured: no Bevy in the dependency graph (16 crates; glam is the only third-party one), the
release DLL is about 375 KB, and a clean release build takes seconds (see README "Numbers").

### (b) Sora's state machine: re-implemented on plain structs, mirroring `sora::tick`

Two options were weighed:

1. **Headless Bevy App inside the library** (MinimalPlugins, `SoraPlugin`). Rejected: `SoraPlugin`
   loads rigs through the `Disc` resource, spawns skinned meshes (`Assets<Mesh>`,
   `StandardMaterial`, images: the PBR/render stack), and `tick` queries `OrbitCamera`, `Enemy`,
   `Fighter`, `GameData`, `ReactionData`, the struggle `Mission`, summons and the BGM battle flag. A
   headless app would need most of kh2-app's plugin graph (render assets, enemies, camera, effects),
   i.e. the whole game in the DLL, for one character. It also could not take host collision
   without replacing the `World` resource and the area loader.
2. **Plain-struct controller** (chosen). `crates/libkh2/src/sora.rs` is a reduced transcription of
   `kh2-player/src/sora.rs::tick` for the v1 states (Ground, Air, Attack, Guard): the same branch
   order, the same calls into the reused rule functions (`ground_step`, `ground_motion`,
   `turn_to_step`, `jump_control`, `jump_dy`, `air_velocity`, `move_by`, `Chain::press`,
   `Moveset::select`/`explicit`, the trigger windows), the same blend lengths. `anim.rs` is a
   transcription of `skel.rs::Animator` (frame stepping at fps/60 per tick, loop return, root delta,
   the `kh2_anim::blend` cross-fade, `pose_with`) without the Bevy joint entities. Every transcribed
   item names its kh2-rs source in a comment. No numbers are re-derived: speeds, heights, turn rate,
   fall cap, lock-on range, V0 values come from `pref` on the disc; blends and constants are the
   ones kh2-rs uses (with their executable addresses).

The glue (the state machine wiring, the animator transcription, the leap record parser and
`leap_launch`, `motion_state`) is **not** oracle-tested. It is checked by libkh2's own unit tests and,
with a disc, by comparing against values read from the disc (jump apex = `prty` JumpHeight, ground
speed = `prty` RunSpeed per tick).

## 3. API boundary

Inspired, for the boundary only, by libsm64 and by the third-party clean-room
[kh2-combat-kit](https://github.com/steferro78-HIV/kh2-combat-kit) (PC HD build; its README was read
as data, none of its code or text is used): **the host moves and draws, the library decides.**

* Units: kh2 units (centimetres), Y up, Sora's facing yaw 0 = +Z, forward = (sin yaw, 0, cos yaw).
  This is kh2-rs's map space (COCT with Y and Z negated).
* `kh2_init(iso_path)` opens the disc and loads Sora's model, motion set, keyblade, attack table and
  the `pref` rows. `kh2_init` with NULL uses `KH2_ISO`.
* Surfaces: `kh2_surfaces_load(tris, n)`: each triangle carries the COCT attribute word (`flags`;
  `KH2_SURF_WALL` = PARTY_COLLIDE makes a wall solid for Sora, floors are any triangle with normal
  within 60 degrees of up) and a mesh group (0..63, toggled with `kh2_set_group`). Optionally
  `kh2_surfaces_load_map("tt00")` loads an area's own COCT from the disc.
* `kh2_create(x, y, z)` → handle; `kh2_delete(handle)`.
* `kh2_tick(handle, &inputs, &state)`: one 60 Hz logic tick. Inputs: stick (x right, y forward,
  magnitude in 0..1), camera look direction (x, z), buttons held (attack/jump/guard). Presses are
  edges the library derives from the held bits. `kh2_tick_field` runs two ticks (the 30 Hz field
  cadence option).
* State: position, velocity, yaw, grounded, state id, motion slot / motion id / frame / global frame,
  combo counter, attack window (range trigger 10/33 active, its attack id and shape group), the
  combo and finisher windows.
* Hits: the host tests the live attack volumes (`kh2_get_attack_volumes`: world-space capsules of
  the attack shapes of the active trigger's group, on the keyblade and body) against its own
  targets and calls `kh2_report_hit(handle)`; that sets the "a hit landed" latch the combo chain
  needs (`Chain::press`, actor +0x108 bit 0x2000000).
* Target: `kh2_set_target(handle, x, y, z, radius, half_height)` (the host's lock-on choice) feeds
  the attack table's range tests, the leap aim and the attack turn; `kh2_clear_target`.
* Skeleton: `kh2_get_bones(handle, out, max)` model-space bone matrices (root-relative, column
  major, the body transform is position + yaw), `kh2_get_rest_bones`, and the same for the keyblade
  (`which` = 1), which hangs from bone 178.

## 4. What v1 covers and what it does not

Covered: ground walk/run/turn/idle with the motion classes, jump (High Jump height when the growth
level is set), short hop on early release, fall, landing, walking off ledges, ceilings, wall slide;
the ground combo (A300, A302, A304 on the USA disc) and the air combo (A330, A331, A332) through the attack table, with the motions'
own combo/finisher windows, air attack leaps, root motion, gravity hold triggers, the landing cut;
the guard (key 11); host-supplied target; posed skeleton and attack volumes.

Step 1 (decided: USA rules, no dodge roll): Quick Run, Glide, Aerial Dodge and High Jump, transcribed
from `sora::tick`'s QuickRun/Glide/Air branches with the `fmab` row of each level.

Step 2: `reaction_rules.rs` (pure, Bevy only for `Vec3`) is compiled in place too; `Sora::hit` is a
transcription of `sora::receive_hits`. decided: the host computes the damage (it owns the attacker's
stats); Sora's resistances are taken as 100 (no equipment); Second Chance / Once More / Leaf Bracer
and Aerial Recovery / Retaliating Slash are not ported; knock-out waits for `kh2_revive` unless
auto-revive is set (kh2-rs stands up after 180 ticks).

Step 3: `camera_rules.rs` (pure, oracle-tested in kh2-rs `orig_l2_p2.rs`) is compiled in place;
`camera.rs` transcribes `camera::tick`'s field path and `cast_world`. decided: no dynamic blockers,
no talk/event/overhead cameras; the lock-on point is the host's target point (kh2-rs uses the
enemy position + 100); the camera ray tests the static collision (kh2-rs can use render meshes).

Step 4: `magic_rules.rs` compiled in place; `Sora::try_cast` / `magic_trigger` transcribe
`magic.rs`'s cast acceptance and trigger-17 payment. decided: world 0 for the row choice, no spell
scripts (VM), effects or spell attacks (the host spawns them on `SpellFired`), no Magic Lock-On
or Leaf Bracer, the MP regeneration factor 1.0, a request is retried for 30 ticks.

Step 5: `shared/drive.rs` mixes Bevy glue and a pure `rules` module; `crates/kh2-pure/build.rs`
extracts that module's text from kh2-rs at build time (brace-matched) and compiles it. `drive.rs`
transcribes `DriveRules::object_files`/`files` and `drive_tick`'s enter/change/timer/revert.
decided: world 0 for the memt merge, Donald and Goofy present, no form abilities beyond the growth
kind (the ability system is not ported), the second keyblade not loaded (hand 1 id only when the
memt record names it), drain factor 1.0 (no Form Boost), assets loaded on first entry and cached.

Step 6: `anim::sample` evaluates a slot with a fresh rig state (the animator keeps its own);
the disc test checks both agree on A300. Meshes stay out of the library (decided): kh2-extras
exports them as glb with node i = bone i, so the bone matrices drive them (`docs/SKINNING.md`).

Not covered: enemies, magic, summons, items,
the dodge roll, reaction commands, the auto-target, sounds and effects (frame triggers are
reported, not played), the game's exact COCT walker (`Oct`, unwired in kh2-rs too), weapon
switching, moving platforms.

## 4a. Spell attacks (round 3 step 1)

`Sora::spell_stages` names the spells' atkp ids from kh2-rs `magic.rs` (Fire 502, Blizzard 248,
Thunder 250/251/249, Cure 708) and their `magi` offsets; the row is `magic_rules::attack_row`.
decided: the spell objects' movement (Blizzard homing, Fire following, Thunder's multi-target
pattern) and the script VM stay out; the host moves the volume by `life` and `speed`. `kh2_damage`
transcribes `GameData::damage` with the target's caps replaced by a floor of 1.

Round 3 step 3: `shared/abilities.rs` is compiled from kh2-rs's text with two edits made by
`build.rs` (the `KINGDOM_KEY` import restated, the methods over `progress::Progress` removed with
their doc comments). decided: no AP accounting (the host's list is equipped as given); Defender
and MP Rage are not applied (the host computes incoming damage); Leaf Bracer starts on Cure.

Round 3 step 4: `build.rs` extracts the pure spans of two more Bevy files (`items.rs`: `Vitals`,
`item_effect`; `progress.rs`: `defeat_payout`). decided: the host keeps the inventory and the item
counts; level-up abilities and bonus levels are not given; HP and MP maxima stay the `plrp` values
(kh2-rs's record bonuses need the save record).

Round 3 step 5: `party_rules.rs` (pure, Bevy only for `Vec3`/`Vec4`) is compiled in place;
`party.rs` transcribes `party.rs`'s field follow path. decided: no battle AI, attacks, limits,
revive or the trail warp (a member beyond 30 m is put back beside Sora); the members are found by
model name (kh2-rs takes them from the world's memt slots). Plan for attacks: port
`party_battle.rs`'s approach step and its attack table choice (the same moveset selection as
Sora's), about two days.

Round 4 step 1: the enemy script host is kh2-rs's `enemy_host.rs` itself, compiled in place in
`crates/kh2-enemy-host` together with `shared/enemy.rs`, `shared/enemy_obj.rs`,
`shared/reaction.rs` and the enemy kind files (a build script lists them as kh2-enemy's does). The
crate paths they name are provided by the shim (`Stats`, `Rig`, the voice queue, a story with
every flag clear, a counting `AiTrace`). The Bevy systems around them (`ai_attach`, `ai_update`,
`think`, `receive_hits`, the combat tests, `enemy_stats`, `damage`) are transcribed in
`crates/libkh2/src/enemy.rs`. decided: no paired motions on Sora, clashes, reaction commands,
area spawn groups, camera visibility (every enemy is on screen) or prize objects; the enemy's hit
on Sora goes through `Sora::hit` with the game's damage formula; a defeat pays EXP at once.

## 4b. Regression and drift (step 7)

Drift has two sources: (1) the files compiled in place change (caught by the build and the
tests), (2) the transcribed Bevy systems (`sora::tick`, `skel::Animator`, `camera::tick`,
`drive_tick`, `magic.rs`, `receive_hits`) change in kh2-rs and libkh2's copies fall behind
silently. `scripts/drift_check.sh` diffs both groups against `KH2RS_COMMIT`. The replay tests pin
behaviour: the synthetic trace is committed (made-up numbers only); the disc trace is kept as a
digest outside git. decided: no live comparison with kh2-rs's Bevy app (it would need the whole
app headless); the comparison points with kh2-rs are its own test values (the 185-unit jump's
30.1-tick rise) and the shared rule code itself.

## 4c. Polish (step 8)

Every C entry point runs inside `catch_unwind` (`ffi::guard` / `ffi::with`), so a bug becomes an
error return; a mutex poisoned by such a panic is taken over (the state is plain data). The
release profile therefore unwinds instead of aborting (+225 KB). The header carries
`KH2_VERSION`; the crate version is 0.2.0.

## 5. Risks and next steps

* kh2-rs's files are compiled in place; an API change there breaks libkh2's build (fix at the call
  sites in `sora.rs`). Pin a kh2-rs commit in a release.
* The animator and state machine are transcriptions; drift from kh2-rs's `sora::tick` is possible
  when kh2-rs changes. A shared, Bevy-free `SoraCore` inside kh2-rs (the ECS system calling it) would
  remove the duplication; that is a kh2-rs refactor (estimate: 2-3 days, mostly moving `tick`'s body
  into a function over plain inputs).
* Next: Quick Run / Glide / Aerial Dodge (the kh2-rs code exists, ~150 lines of glue), hurt and
  blow reactions with `reaction_rules` (larger: needs the reaction tables), the field camera rules
  (`camera::rules`, oracle-tested, also Bevy-free in content), magic (large).
