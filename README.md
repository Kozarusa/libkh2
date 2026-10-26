# libkh2

Sora's controller from Kingdom Hearts II as an embeddable library, in the spirit of
[libsm64](https://github.com/libsm64/libsm64): the host engine (Unity, Godot, your own) supplies
collision triangles and per-tick input, libkh2 runs Sora's movement and attack logic and answers
his position, facing, state, motion and frame, the live attack windows and his posed skeleton.

libkh2 ships no game data. It reads the user's own disc image (Kingdom Hearts II Final Mix, the
Japanese PS2 release `SLPM_666.75`; from 2.0.0 the only disc it opens) at `kh2_init`;
nothing from the disc is stored in this repository or compiled into the library.

The movement and attack rules are kh2fm-rs's (`../kh2fm-rs`, the Final Mix copy of the kh2-rs game
project), several of them oracle-tested against
the original executable there; libkh2 compiles kh2-rs's pure modules in place without Bevy and adds
a plain-struct controller, a C ABI and examples. `DESIGN.md` says exactly what is reused, what is
new glue, and what v1 does not cover.

What it is not: not a game or a renderer (no meshes, textures, effects, sound playback or UI: it
hands the host skeletons, motions, events and sound numbers), not a story or menu system (no
cutscenes, dialogue, shops, save menus), not a full port (what differs from the game is listed in
`docs/DEVIATIONS.md`; how close each function is, in `docs/FAITHFULNESS.md`). Host authors start
with `docs/FOR-HOST-AUTHORS.md` (tick order, units, collision, a C loop, common mistakes).

## Status and what a human must test

Tested here on every change (`scripts/ci.ps1`): Windows 10, MSVC, Rust release build, .NET 9, the
Final Mix disc image; 60 disc tests, golden replays, fuzz, the C layout check, the binding diff, oracle
tests against the original executable (kh2-ee), the package scan, a 10-minute soak.

Not tested here, a human should run first:

1. The Unity sample (`unity/`, `docs/UNITY.md`, `unity/Kh2Mesh.cs`): never compiled inside Unity.
2. Linux and macOS: `sh scripts/ci.sh` (the build, `examples/build_min.sh`, `check_bindings.py`'s
   `nm` path); unzipping the package there (PowerShell writes backslash paths into the zip).
3. A real host's audio device fed by `kh2_audio_tick`, and the HUD drawn by a GPU from
   `kh2_hud_draw_list` (only the software rasteriser drew it here).
4. By ear: the hit sounds, the placed volume / pan and the area music through a real device (the
   music measured about 15 dB under a PCSX2 recording, docs/SOUND-AND-EFFECTS.md); by eye: the
   HP / MP panel, drive gauge, target gauge and reticle against the game (docs/HUD.md).

## Quick start per language

- **C:** `include/kh2.h` + `kh2.dll` / `kh2.dll.lib`; `examples/min.c`, built by
  `examples\build_min.bat` (MSVC) or `examples/build_min.sh`.
- **C#:** `unity/Kh2.cs` (define `KH2_NO_UNITY` outside Unity); `dotnet run -c Release --project
  examples/csharp -- <disc image>`.
- **Python:** ctypes, no build: `python examples/python/kh2_run.py [library] [disc image]`.
- **Unity:** copy `kh2.dll` to `Assets/Plugins/`, `unity/Kh2.cs` to `Assets/`, add `Kh2Sora`
  to a GameObject (`docs/UNITY.md`; written for Unity, not run here).

All read the disc image from the argument or `KH2_ISO`. The packaged form (`scripts/package.ps1
-Zip`, `dist/libkh2.zip`) has the same layout with the library in `bin/`.

## Quick start

```
cargo build --release -p libkh2                                   # target/release/kh2.dll
set KH2_ISO=<path to your Kingdom Hearts II Final Mix disc image>     # PowerShell: $env:KH2_ISO="..."
python examples/python/kh2_run.py                                 # Sora, Donald and Goofy vs a Soldier
cargo run --release -p libkh2-viewer                              # a window: E spawns enemies, J attacks
powershell -ExecutionPolicy Bypass -File scripts/package.ps1      # dist/libkh2 for a host engine
```

What there is: Sora's controller (movement, combos, guard, growth abilities, magic, drive forms,
items, levels, bonus rewards, hit reactions, reaction commands with paired motions, limits),
Donald and Goofy (follow, battle AI with styles and item use, HP, knock-out and revive, levels),
every kh2-rs enemy kind under its own BDX script, a field camera helper, motion export and a save
record, area battle programs with their waves and the Struggle's mission manager, talks, runtime
skinned meshes and textures, the HUD's command menu as a draw list, moving collision objects and an
audio engine (`docs/MESHES.md`, `docs/HUD.md`, `docs/SOUND-AND-EFFECTS.md`). The C API is `include/kh2.h` (reference: `docs/API-REFERENCE.md`); what differs from the
game is in `docs/DEVIATIONS.md`, speed and memory in `docs/PERFORMANCE.md`, the enemy kinds in
`docs/ENEMY-KINDS.md`, the Struggle in `docs/STRUGGLE.md`, the C API's safety in `docs/SAFETY.md`, how close each function is to the original in `docs/FAITHFULNESS.md`, changes in `CHANGELOG.md`.

## Build

Needs Rust 1.88+ and, next to this folder, the kh2fm-rs source tree (`../kh2fm-rs`, read-only; the
commit libkh2 was checked against is in `KH2RS_COMMIT`). The kh2fm-rs tree is not public: ask the
project owner for it. Branches:

- `main` (2.x): Final Mix. Needs `../kh2fm-rs` and the Final Mix disc image (`KH2_ISO`, or
  `KH2_ISO_FM` for the tests); a USA disc is refused at `kh2_init`.
- `usa` (tag `usa-final`, 1.2.0): the USA release. Needs the public kh2-rs tree as `../kh2-rs`
  (`git clone https://github.com/Kozarusa/kh2-rust-rewrite.git kh2-rs`) and the USA disc image. No
  further work happens there.

No game data is in any of these repositories: libkh2 reads your own disc image when it starts.

```
cargo build --release -p libkh2         # target/release/kh2.dll (+ kh2.dll.lib), libkh2.so / .dylib elsewhere
cargo test -p libkh2                    # synthetic tests, no disc needed
KH2_ISO=<disc image> cargo test --release -p libkh2 --test disc -- --nocapture   # against the disc
```

C example (`examples/min.c`, uses `include/kh2.h`):

```
examples\build_min.bat                  # from a Developer Command Prompt for VS, repository root
build\min.exe <disc image>
sh examples/build_min.sh && build/min <disc image>     # Linux / macOS
```

C# console example (`examples/csharp`, .NET 9, the `unity/Kh2.cs` bindings without Unity) and the binding check:

```
dotnet run -c Release --project examples/csharp -- <disc image>
python scripts/check_bindings.py        # the DLL's exports against kh2.h and Kh2.cs
```

Pure-Rust example host: `cargo run --release -p libkh2 --example host -- <disc image>`.

**Viewer** (a window; Sora, his keyblades, Donald and Goofy with their weapons, and enemies drawn
as skeleton lines through the field camera; `crates/viewer`, depends on `minifb` only). E spawns a
Shadow, a Soldier and a Dusk in front of Sora: they run their BDX scripts, Donald and Goofy fight
them, Q locks onto the nearest, J and 1-3 hit them, T accepts a reaction command; hits, defeats and
drops print on stderr. `KH2_VIEWER_FRAMES=600` runs a scripted fight and quits
(`KH2_VIEWER_SHOT=<file.ppm>` keeps the last frame).

```
cargo run --release -p libkh2-viewer -- "<path to Kingdom Hearts II - Final Mix (Japan).iso>"        # test room
cargo run --release -p libkh2-viewer -- "<iso>" tt00                                   # an area's collision
```

Keys: WASD move (Shift walks), Space jump, J attack, K guard / Quick Run / Glide, arrows camera,
Q lock onto the red post, 1-4 Fire / Blizzard / Thunder / Cure, P Potion, F Valor, M Master,
R revert, H take a hit, G growth abilities on/off, Esc quit. Events print to the console.
`KH2_VIEWER_FRAMES=160 KH2_VIEWER_SHOT=out.ppm` runs a scripted walk and saves the last frame.

Unity: `unity/Kh2.cs` (P/Invoke bindings and a `MonoBehaviour` driver) and the sample scene in
`docs/UNITY.md` (documented, not built here).

## API tour

```c
kh2_init("Kingdom Hearts II - Final Mix (Japan).iso");        /* or NULL with KH2_ISO set */
kh2_surfaces_load(tris, n);                     /* or kh2_surfaces_load_map("tt00") */
int32_t h = kh2_create(0, 0, 0);
for (;;) {                                      /* every 1/60 s */
    kh2_inputs in = { stick_x, stick_y, cam_look_x, cam_look_z, KH2_BTN_ATTACK * attack_held | ... };
    kh2_state st;
    kh2_tick(h, &in, &st);                      /* kh2_tick_field: two ticks per 30 Hz frame */
    /* draw Sora at st.pos, yaw st.yaw, motion st.motion_id at st.frame,
       or skin with kh2_get_bones(h, 0, 1, mats, n) (world space) */
    if (st.attack_active) {
        kh2_volume v[16];
        int k = kh2_get_attack_volumes(h, v, 16);   /* test against your targets ... */
        if (hit) kh2_report_hit(h);                 /* ... the combo goes on only after a hit */
    }
}
kh2_delete(h); kh2_terminate();
```

* **Space:** kh2 units (cm), right-handed, Y up; yaw 0 faces +Z. Unity: mirror X (see `Kh2.cs`).
* **Surfaces:** a triangle's normal `(v1-v0) x (v2-v0)` points to Sora's side. Walls (normal more
  than 60 degrees from up) stop him when `flags` has `KH2_SURF_WALL` (the COCT PARTY_COLLIDE bit);
  walls are one-sided like COCT walls. `group` (0..63) is switched by `kh2_set_collision`.
* **Input:** stick x right / y forward relative to the camera, the camera's look direction, the
  held buttons (attack, jump, guard). Presses come from the edges; hold jump for the full height.
* **State:** position, velocity, yaw, grounded, state (ground, air, attack, air attack, guard),
  motion slot / id / frame / global frame, combo counter, attack window (atkp id and shape group of
  range trigger 10/33; Sora's are in the keyblade's motions), combo and finisher windows.
* **Target:** `kh2_set_target` passes the host's lock-on point; it drives the attack table's range
  tests, the lunge, the turn triggers 22/28 and the air leaps' aim.
* **Skeleton:** `kh2_get_bones` (body = 0, keyblade = 1; model or world space), `kh2_get_rest_bones`,
  `kh2_bone_parents`. Meshes are not exported in v1: the host loads its own model of Sora.

## Regression and drift

* `tests/scripts/basic.txt` is a scripted input recording (format in `crates/libkh2/src/replay.rs`).
  The synthetic replay is compared with the committed `crates/libkh2/tests/golden/synthetic_basic.trace`
  (`KH2_BLESS=1` rewrites it); the disc replay with a digest in `local/golden_disc.txt` (not in git).
  `cargo run --release -p libkh2 --example replay -- <disc> tests/scripts/basic.txt` prints a trace.
* `tests/golden_disc.rs` runs the main scenarios (movement, combos, growth, hurt, magic, drive,
  items) on the disc and compares every tick's state digest with `local/golden/*.txt` (written on
  the first run; `KH2_BLESS=1` rewrites); the synthetic ones are committed in `tests/golden/`.
  `docs/PORTING-BACK.md` is the plan for moving the ported layer into kh2-rs.
* `KH2RS_COMMIT` is the kh2-rs commit libkh2 was checked against. `sh scripts/drift_check.sh`
  (Git Bash on Windows) lists changes in the kh2-rs files compiled in place, the transcribed ones
  and the path dependencies since then, and exits 1 on drift.

## Threads, errors, versions

* One library state per process behind a mutex: every call may come from any thread; calls are
  serialised, so tick Sora instances from one thread for predictable timing.
* No call panics or aborts across the C ABI: internal errors (including a caught panic) return -1
  and `kh2_last_error` says why. Pointers are only read during the call.
* `KH2_VERSION` in `kh2.h` and `kh2_version()` (major << 16 | minor << 8 | patch); the C example
  refuses a library whose major.minor differs.
* Linux / macOS: `examples/build_min.sh` and the Rust build are written for them but untested
  here (only Windows with MSVC was run).

## Numbers (this machine, release build)

| | |
|---|---|
| `kh2.dll` | about 600 KB (thin LTO, stripped, unwinding for the FFI guard) |
| clean release build | seconds; 16 crates, glam the only third-party one, no Bevy |
| tick without pose | about 0.3 us |
| tick with pose (body 228 bones + keyblade 12, full rig) | about 14 us |

## What works, what does not

Works (checked by `cargo test`): walk/run at the `prty` speeds with the original's walk/run split and
turn rate, jump with the `prty` JumpHeight apex (and the High Jump `fmab` height), short hop on
early release, fall capped at `sstm` FallMax, landing, walking off ledges, wall slide and the party
wall filter, mesh group switching, ground combo A300 → A302 → A304 and air combo A330 → A331 → A332
picked by the disc's attack table with the motions' own windows, guard, root motion.

Growth abilities (`kh2_set_growth`): High Jump, Quick Run, Aerial Dodge, Glide with the disc's `fmab`
rows (disc test `growth_abilities_use_the_fmab_rows`).

Hit reactions (`kh2_hit_taken`): the disc's `atkp` row through kh2-rs's `reaction_rules`
(compiled in place): flinch, push, blow and landing, knock-out (`kh2_revive`), guard and counter,
invulnerability ticks (disc test `hits_flinch_blow_knock_out_and_guard`).

Field camera (`kh2_camera_tick`): kh2-rs's oracle-tested `camera_rules` (follow, right-stick yaw
and zoom, auto-camera, lock-on, look-around, wall collision, springs) with the executable's camera
table; `kh2_lock_on_pick` is the lock-on choice (disc test `the_field_camera_follows_collides_and_locks_on`).

Magic (`kh2_cast`, `kh2_poll_event`): Fire, Blizzard, Thunder, Cure at level 1 (and higher rows by
`kh2_set_magic_levels`) through `magic_rules`: the `magc` row, the `cmd` cost, cast motion and kind,
MP paid at frame trigger 17, MP charge (Cure takes all MP); the host spawns the spell on the event
(disc test `level_one_spells_cast_pay_and_charge`).

Drive forms (`kh2_drive_enter`, `kh2_drive_revert`): Valor, Wisdom, Master, Final (Anti through the
roll) with kh2-rs's drive rules: entry checks, cost, Anti/Final roll, change motions 52/53, the
form body, keyblade and attack table swapped in, the form's growth ability, form timer and
revert, HP/MP refill (disc test `drive_forms_enter_swap_and_revert`).

Animation export: `kh2_sample_motion` (any slot at any frame, root motion separate),
`kh2_motion_info`, `kh2_motion_slot`; skinning with kh2-extras glb files is described in
`docs/SKINNING.md`.

Spell attacks (step 1 of round 3): each fired spell sends `KH2_EV_SPELL_ATTACK` per stage with its
`atkp` row, `magi` size, life, speed, start and direction; `kh2_damage` computes a row's damage
with Sora's `lvup` stats; Cure heals Sora (`KH2_EV_HEALED`).

Second keyblade: Valor, Master and Final hold a second blade on the left hand (bone 86) with the
form's own left-hand motion set (`W_EX010_BTLF_L` ...); bones `which = 2`, attack volumes included,
`kh2_drive_set_second_keyblade` picks the item.

Abilities (`kh2_set_loadout`): kh2-rs's ability block (compiled from its source by `build.rs`) from
the host's list: Guard, Combo Plus, Air Combo Plus, Finishing Plus, Negative Combo, growth
abilities, Berserk Charge, Second Chance / Once More, Leaf Bracer, the element and combo boosts in
`kh2_damage`; a form adds its own list (disc test `the_loadout_sets_combo_lengths_growth_and_survival`).

Items and growth: `kh2_use_item` plays the use motion and applies kh2-rs's oracle-tested
`item_effect` (HP, MP with the charge, drive gauge) 30 frames after the motion's trigger;
`kh2_report_defeat` pays experience (`defeat_payout`) and raises the level from `lvup`; HP, MP,
drive, level and EXP are in `kh2_state` (disc test `items_heal_through_the_use_motion_and_defeats_raise_the_level`).

Party members (`kh2_party_add`, `kh2_party_tick`): Donald and Goofy follow Sora in the field with
kh2-rs's `party_rules` (follow points, follow step with the AI class's thresholds, steer), their
own `prty` speeds and idle/walk/run motions (disc test `donald_and_goofy_follow_sora`). With enemies spawned
they fight: kh2-rs `party_battle` (target choice of style 0, the AI class records from the executable,
`choose`, feasibility, approach step, slot remap), attack motions with their weapons (staff 75,
shield 49) and damage on enemy hurt shapes, reported as enemy `Hit` events (disc test
`donald_and_goofy_fight_an_enemy`). Since 0.5 they are characters: HP/MP from the new game's `plrp`
rows, status from their `lvup` table at their level (`kh2_party_set_level`), enemies hit them
(damage, reaction motions, FUN_00186238's HP method), a knock-out lies down until the revive timer
(`prty` StunTime, `sstm` 0x58 / 0x5c) stands them up, styles 0..3 (`kh2_party_set_style`), the danger
test, emergency pass and HP/MP item handlers spend the stock the host gives (`kh2_party_set_items`);
events via `kh2_poll_member_event` (disc test `members_take_hits_fall_revive_use_items_and_fight_in_every_style`).
Since 0.7 their own BDX scripts run (Cure, Specials: `member_scripts_cast_cure_and_specials`) and they wear equipment (`kh2_party_set_equipment`); items act at once.

Enemies (`kh2_enemy_spawn`, `kh2_enemies_tick`): any kind of kh2-rs's list (Shadow, Soldier, Dusk and
50 more) under its own BDX script through kh2-rs's interpreter and its script host compiled in
place; their motions, attack and hurt volumes, the damage formula both ways, hit reactions, defeat
with EXP paid to Sora (disc test `twilight_town_enemies_run_their_scripts_fight_and_die`: Shadow,
Soldier and Dusk each attack and are defeated).

Spell flight (with enemies): Fire's attack around Sora, Blizzard's shots (homing where the `magi`
turn rate is set, straight at level 1), Thunder's strikes on the target with the ground snap and
the column, the victims' hit memory and row variants, damage with Sora's magic; the cast turns
Sora toward the target (triggers 22/28) (disc test `spells_fly_and_hit_enemies`).

Since 0.7 / 0.8: summons (`summons_arrive_run_their_scripts_and_leave`), limits with Donald and Goofy (no limit damage, see `docs/DEVIATIONS.md`), area battle programs with their waves (`kh2_area_run`), the Struggle's mission manager (`docs/STRUGGLE.md`), talks (`kh2_talk_begin`). Not done: sound and effect playback (numbers go to the host), moving platforms, cutscenes, menus. See `DESIGN.md`.

## Credits

The KH2 file formats are documented by [OpenKH](https://github.com/OpenKH/OpenKH) (see kh2-rs's
CREDITS). The API boundary takes inspiration from libsm64 and from the third-party
[kh2-combat-kit](https://github.com/steferro78-HIV/kh2-combat-kit) (no code or text used).

Licence: MIT OR Apache-2.0, as kh2-rs.

Reaction commands (`kh2_reaction_offer`, `kh2_reaction_accept`): kh2-rs `reaction_cmd`'s pure span
(range trigger 20 offers of the enemies' motions, the `cmd` row status: flags, form mask, DistRange,
MP) is compiled from its source; accept follows FUN_00177c20's generic path (use counter, target
check, CmdAction stop) and hands the command to the target's BDX script as event 11 (disc test
`a_reaction_command_is_offered_accepted_and_delivered`: a Dusk's script handles command 27).
decided: save and shop rows and the script's commands back to Sora are not ported (limits start
through `kh2_limit_start` and Auto Limit, talks through `kh2_talk_begin`); no spawnable kind offers one from its motions, so `kh2_reaction_force_offer` (kh2-rs's
harness offer) drives it.

Level-ups (kh2-rs `Progress::raise_level`): `KH2_EV_LEVEL_UP` carries the strength/magic/defense/AP
rises over the previous `lvup` row and the ability the row gives for the weapon set
(`kh2_set_weapon_set`); given abilities are listed by `kh2_granted_abilities` (disc test
`level_ups_give_the_lvup_rows_abilities_and_the_record_saves`). HP/MP growth comes from
the bonus levels (`kh2_give_bonus`, the `bons` rows). Save record: `kh2_state_save` / `kh2_state_load`
("KH2S" v3, the status and the place, not the moment; malformed records are refused unchanged).
Replays: `battle_replays_the_same` runs Sora, Donald, Goofy and three enemies twice and compares
every tick (and a golden digest file); the members' generator word is kept per party.

Robustness: `tests/fuzz.rs` calls the C API at random (NULLs, stray handles and indices, huge and
non-finite numbers, damaged save records) before `kh2_init` and, with `KH2_ISO`, after it with
Sora, the party and enemies; a panic hook fails the run on any panic (even one the guard catches),
a watchdog on a hang, and a fresh Sora must still tick at the end (`KH2_FUZZ_STEPS`,
`KH2_FUZZ_SEED`). decided: positions, targets, spawn points and collision vertices must be finite
and within 1e7 units (else -1); sticks are held to -1..1 (NaN 0); max HP/MP to 9999; damage to
0..999,999. The full function list with the header's comments: `docs/API-REFERENCE.md`, generated
by `python scripts/gen_api_ref.py` (`--check` tells whether it is up to date).

Packaging: `powershell -ExecutionPolicy Bypass -File scripts/package.ps1 [-Zip]` builds the release
library and lays out `dist/libkh2` (bin/kh2.dll and its import library, include/kh2.h, docs, the
Unity bindings and sample folder, the C, C# and Python examples); it builds with this machine's
paths remapped and refuses to package anything that looks like disc data (`tests/package.rs`
scans the folder and the zip for disc files and machine paths). Quick experiments without a compiler: `python examples/python/kh2_run.py`
(ctypes; Sora, Donald and Goofy against a Soldier, a line per second of game time). Unity:
`unity/sample/README.md`.
