# Contributing to libkh2

## What you need

libkh2 compiles code from a sibling source tree in place (path dependencies and `#[path]` modules), so
a clone of this repository alone does not build:

| Branch | Sibling tree (next to this folder) | Disc image the tests read |
|---|---|---|
| `main` (2.x, Final Mix) | `../kh2fm-rs`, the Final Mix copy of the game project: **not public**, ask the project owner | your own Kingdom Hearts II Final Mix disc image |
| `usa` (1.x, frozen) | `../kh2-rs`: `git clone https://github.com/Kozarusa/kh2-rust-rewrite.git kh2-rs` | your own Kingdom Hearts II (USA) disc image |

`KH2RS_COMMIT` names the sibling commit the library was last checked against;
`sh scripts/drift_check.sh` lists what changed in the sibling since then.

No game data is in this repository and none may be added: no files from the disc, no decompiled text,
no extracted tables, textures, sounds or text. The library reads the user's own disc at run time.
Keep machine-specific paths, user names and disc file names out of code and docs.

## Building and testing

```
cargo build --release
cargo test --release                      # without a disc: the disc tests print "skipped" and pass
KH2_ISO_FM=<your disc image> cargo test --release
powershell -ExecutionPolicy Bypass -File scripts/ci.ps1    # everything a change must pass (Windows)
sh scripts/ci.sh                                             # the same on Linux / macOS (untested)
```

Optional environment variables: `KH2_NAMES` (a name list such as OpenKH's `kh2idx.txt`; the archive
then finds the world message files and the worlds' own HUD art), `KH2_SOAK_SECS` (the soak test's
length), `KH2_FUZZ_STEPS`.

## Changes

- Every rule that comes from the original game names where it was read (the function address of the
  Final Mix executable, and the USA one when it exists) and is tested; a rule compared with the
  executable in kh2-ee goes into `tests/oracle.rs` and `docs/FAITHFULNESS.md`.
- A behaviour that is not the original's is listed in `docs/DEVIATIONS.md` with the reason ("decided:").
- A change to `kh2.h` updates `unity/Kh2.cs`, the fuzz test, `docs/API-REFERENCE.md`
  (`python scripts/gen_api_ref.py`) and `CHANGELOG.md`; `python scripts/check_bindings.py` must agree.
- Versions follow the policy in `README.md` and `docs/MIGRATION.md`.
