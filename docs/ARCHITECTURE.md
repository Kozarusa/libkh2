# Architecture

One page for new contributors. See CONTRIBUTING.md for what you need to build.

## Crates

| Crate | What it is |
|---|---|
| `crates/libkh2` | The library (`kh2` rlib + cdylib): the C ABI (`src/ffi.rs`) and the glue modules (`sora.rs`, `enemy.rs`, `party.rs`, `spell.rs`, `hud.rs`, `audio.rs`, ...). Tests in `tests/`. |
| `crates/kh2-pure` (`libkh2-pure`, used as `kh2_game_core`) | kh2fm-rs's pure game rules compiled in place: `motor.rs`, `world.rs`, `reaction_rules.rs`, `camera_rules.rs`, `magic_rules.rs`, `party_rules.rs` by `#[path]`; the drive, ability, item, party-battle, rng, reaction-command spans cut by `build.rs`. |
| `crates/kh2-enemy-host` | kh2fm-rs's enemy script host, the shared enemy object, every enemy kind file, the HUD rule spans (`hud_menu`, `hud_gauge`, `hud_target`), limits, summons, struggle and talk spans, cut by `build.rs`. |
| `crates/bevy-shim`, `bevy-shim-derive` | The few `bevy::prelude` names the compiled files mention (glam, `default()`, no-op `Resource` / `Message` derives), so they build without Bevy. |
| `crates/viewer` | A test viewer that drives the library and draws the HUD. |
| kh2fm-rs crates (path dependencies) | Decoders and runtimes used as libraries: `kh2-archive`, `kh2-bar`, `kh2-tables`, `kh2-model`, `kh2-anim`, `kh2-map`, `kh2-bdx` (the script VM), `kh2-image`, `kh2-audio`, `kh2-exe`; tests also use `kh2-ee` (the EE emulator for oracle tests), `kh2-factory`, `kh2-fx`. |

## How kh2fm-rs code is compiled in place

kh2fm-rs is a Bevy application; libkh2 reuses its rules without Bevy and without copying text:

- Whole pure files are included with `#[path = "../../../kh2fm-rs/crates/..."]` inside a module whose
  `use` lines provide what the file expects.
- Files that mix rules with Bevy systems are cut by the build scripts: `span(text, start, last)` copies
  from a start marker to the end of the item that begins at the last marker (braces and brackets
  counted), into `OUT_DIR`, and the crate `include!`s it. A few spans get a visibility edit
  (`pub` fields); nothing else is changed.
- Where kh2fm-rs's code is a Bevy system, libkh2 transcribes it as plain glue (class c in
  docs/FAITHFULNESS.md) and names the system it follows.
- `KH2RS_COMMIT` pins the kh2fm-rs commit; `scripts/drift_check.sh` lists drift.

## Data flow of a tick

1. The host calls `kh2_tick(handle, input)`: `Sora::tick` reads the stick through the camera view, runs
   the state machine (ground, air, attack, roll / Quick Run, magic, guard, hurt, ...), the wall test, the
   mover (`Motor::move_by` against the collision `World`), then steps the body's motion (`Anim`) and the
   keyblades', firing the magic and item triggers.
2. `kh2_party_tick`, `kh2_summon_tick`, `kh2_enemies_tick`: the members (kh2fm-rs party rules and their
   scripts), the summon, and each enemy's BDX script (`kh2_bdx::Runtime` over the enemy host), then the
   hit tests both ways (`Enemies::combat`), the spells (`spell.rs`), damage (`enemy::damage` and its
   Final Mix steps), victim reactions and defeats (EXP, drops, form experience).
3. The limit and the area run; then `kh2_hud_tick` builds the 2D draw list (SEQD players and the text
   engine), `kh2_audio_tick` turns the cues of the tick (motion triggers, hits, voices, footsteps) into
   PCM through the software sound driver.
4. The host reads state, events (`kh2_poll_*`), bones / meshes and the draw list, and draws.

All game data comes from the user's disc at `kh2_init` (`data.rs` `Disc`: the archive, the tables, the
executable's constants); none is in the repository.
