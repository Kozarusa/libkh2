# libkh2

Sora's controller from Kingdom Hearts II as an embeddable library, in the spirit of
[libsm64](https://github.com/libsm64/libsm64): the host engine (Unity, Godot, your own) supplies
collision triangles and per-tick input, libkh2 runs Sora's movement and attack logic and answers
his position, facing, state, motion and frame, the live attack windows and his posed skeleton.

libkh2 ships no game data. It reads the user's own disc image (the USA PS2 release) at `kh2_init`;
nothing from the disc is stored in this repository or compiled into the library.

The movement and attack rules are [kh2-rs](../kh2-rs)'s, several of them oracle-tested against
the original executable there; libkh2 compiles kh2-rs's pure modules in place without Bevy and adds
a plain-struct controller, a C ABI and examples. `DESIGN.md` says exactly what is reused, what is
new glue, and what v1 does not cover.

## Build

Needs Rust 1.88+ and the kh2-rs source tree next to this folder (`../kh2-rs`, read-only).

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

Pure-Rust example host: `cargo run --release -p libkh2 --example host -- <disc image>`.

Unity: `unity/Kh2.cs` (P/Invoke bindings and a `MonoBehaviour` driver; documented, not built here).

## API tour

```c
kh2_init("Kingdom Hearts II (USA).iso");        /* or NULL with KH2_ISO set */
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

## Numbers (this machine, release build)

| | |
|---|---|
| `kh2.dll` | about 375 KB (thin LTO, stripped, panic = abort) |
| clean release build | seconds; 16 crates, glam the only third-party one, no Bevy |
| tick without pose | about 0.5 us |
| tick with pose (body 228 bones + keyblade 12, full rig) | about 22 us |

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

Not in v1: enemies, spell objects and their damage, the second keyblade of a form, summons, sounds and effects, moving platforms. See `DESIGN.md`.

## Credits

The KH2 file formats are documented by [OpenKH](https://github.com/OpenKH/OpenKH) (see kh2-rs's
CREDITS). The API boundary takes inspiration from libsm64 and from the third-party
[kh2-combat-kit](https://github.com/steferro78-HIV/kh2-combat-kit) (no code or text used).

Licence: MIT OR Apache-2.0, as kh2-rs.
