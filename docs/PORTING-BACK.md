# Moving libkh2's ported layer into kh2-rs as a Bevy-free core

libkh2 keeps two kinds of kh2-rs code:

* **Compiled in place** (no drift possible beyond what the build and the tests catch): `motor.rs`,
  `world.rs`, `reaction_rules.rs`, `camera_rules.rs`, `magic_rules.rs`, `party_rules.rs`,
  `controller_rules.rs`, and, through `crates/kh2-pure/build.rs`, the pure spans of
  `shared/drive.rs` (`rules`), `shared/abilities.rs`, `kh2-party/src/items.rs` (`Vitals`,
  `item_effect`) and `progress.rs` (`defeat_payout`).
* **Transcribed** (the drift risk): `crates/libkh2/src/sora.rs` (from `kh2-player/src/sora.rs::tick`
  and `receive_hits`, `magic.rs`'s cast flow, `drive.rs::drive_tick`, `items.rs`'s use flow),
  `anim.rs` (from `skel.rs::Animator`), `camera.rs` (from `camera.rs::tick` and `cast_world`),
  `drive.rs` (`DriveRules::object_files`), `party.rs` (`party.rs`'s field follow).

This page is a plan for kh2-rs's owners; libkh2 does not touch kh2-rs.

## Target shape in kh2-rs

A crate `kh2-sora-core` (no Bevy, depends on the pure crates) with:

1. `Anim` (the body of `skel::Animator` without `joints`, `turn` and the spawn code). `skel.rs`
   keeps a Bevy component that owns an `Anim` and writes the joint transforms from `anim.pose`.
2. `SoraCore` = the fields of `shared::sora::Sora` that are not entities (`state`, `motor`, `hits`,
   `finishers`, `queued_attack`, `connected`, `push`, `blow`, `timer`, `dash_*`, `hover`,
   `last_move`, `lunge_*`, `invuln`, `leaf_bracer`, `hp`, `mp`), plus the body `Anim` and the
   weapons' `Anim`s.
3. `SoraCore::tick(&mut self, ctx: &TickCtx, input: &PadInput) -> Vec<Event>` where `TickCtx`
   carries what the system reads from resources today: `&GameData` rows, `&World`, the target
   points of the enemies (`Vec<TargetPoint>`), the battle flag, the growth levels, the ability
   chain, the reaction data, and the swing constants. Everything `tick` now does through
   `Query`/`Res` becomes a field of `TickCtx` or an `Event` (sounds, effects, form changes, the log
   lines).
4. `SoraCore::hit(&mut self, ctx, &HitIn) -> HitResult` from `receive_hits`' body.
5. The magic cast, the item use and the drive change task as methods on `SoraCore`, their Bevy
   systems reduced to: collect inputs, call, apply the events (effects, voices, swapping meshes).

## Steps (each one keeps kh2-rs's tests green)

1. Move `Animator`'s non-ECS body into `Anim`; `skel::animate` calls `Anim::step`. Smallest step,
   no behaviour change (the pose tests in `skel.rs` cover it).
2. Introduce `SoraCore` holding the fields above; `Sora` becomes `{ core: SoraCore, body: Entity,
   weapons: Vec<Entity>, lock, target }`. Mechanical rename.
3. Move `tick`'s match on `state` into `SoraCore::tick`, starting with the states libkh2 already
   runs (ground, air, attack, guard, quick run, glide, magic, hurt, blown, down, form change). The
   closures (`start_attack`, `request`, `square_explicit`, `start_jump`) become methods, as in
   libkh2's `sora.rs`.
4. Move `receive_hits`, the cast acceptance and trigger 17, the item use and the drive change
   task the same way.
5. libkh2 then depends on `kh2-sora-core` by path and drops its transcriptions; its golden traces
   (`tests/golden_disc.rs`) are the check that nothing changed.

Effort: about 2 to 4 days in kh2-rs, mostly steps 3 and 4. Risk: the system's order inside
FixedUpdate (`Sim::Sora` before `Sim::Pose`, the weapon sync before) must be kept by the callers.

## Until then

`scripts/drift_check.sh` lists changes in both groups since `KH2RS_COMMIT`; when the transcribed
files changed, compare the diff with libkh2's copy, update it, re-bless the golden traces only
for deliberate changes, and bump `KH2RS_COMMIT`.
