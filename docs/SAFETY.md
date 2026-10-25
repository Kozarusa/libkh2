# Safety of the C API

All `unsafe` code of the library is in `crates/libkh2/src/ffi.rs` (the C ABI) and in one place of
the compiled kh2-rs code it does not own; the simulation (`sora.rs`, `party.rs`, `enemy.rs`, ...)
is safe Rust. Audited in round 6: every `unsafe extern "C" fn` documents its pointer contract under
`# Safety` (shown in `kh2.h` and `docs/API-REFERENCE.md`), and every `unsafe` block is one of the
patterns below.

| pattern | where | why it is sound |
| --- | --- | --- |
| `*out = value` for an output record | every `kh2_*_state`, `kh2_poll_*`, `kh2_item_info`, `kh2_camera_tick` ... | `out` is checked for NULL first (return -1); the contract says it points to a writable record of that type; the record types are `#[repr(C)]` and match `kh2.h` field for field (`tests/layout.rs` checks sizes and all 169 offsets with a C compiler) |
| `write_mats` / `write_str` | bones, rest pose, motion samples, motion tag, last error | NULL-checked inside; they write at most `max` matrices / `len` bytes (strings are cut and NUL-terminated) |
| `std::slice::from_raw_parts(p, n)` over host input | surfaces, loadout, items, ids, save records, lock-on points | NULL or `n == 0` gives an empty slice; `n` is capped where the library keeps the data (64 loadout ids, 16 equipment / charm ids, 1 MiB save records); the slice is copied before the library lock is taken, so the host's buffer is only read during the call |
| `CStr::from_ptr` | init path, area names, enemy kinds | NULL-checked; the contract says NUL-terminated; non-UTF-8 is refused |
| `copy_nonoverlapping` into `root` / `out` | `kh2_sample_motion`, `kh2_state_save` | NULL-checked; the save copies only when `cap` covers the record |
| `std::panic::catch_unwind` around every entry | `with` / `guard` | no panic crosses the ABI (the release profile unwinds); a poisoned lock is taken over, the state is plain data |

Not `unsafe` but part of the same audit (the fuzz test, `tests/fuzz.rs`, 65 arms over every entry
point, NULLs, stray handles, huge and non-finite numbers): positions must be finite and within 1e7
units, sticks are held to -1..1, maxima to 9999, damage to 0..999,999, per-Sora enemy spawns from an
area to 256; allocations sized by host numbers are bounded by those caps (the round 4 fuzz found two
that were not).

Two header fixes came out of the layout check (round 6): the record types `kh2_item_info`,
`kh2_limit_state`, `kh2_summon_state`, `kh2_enemy_state` and `kh2_reaction_offer` had the names of
functions, which is not valid C; they are now `..._t` (the functions and the ABI are unchanged).

The host's side of the contract: pointers valid for the call, records of the header's types, one
library instance per process (the state is global, guarded by a mutex; calls from several threads
are serialised).
