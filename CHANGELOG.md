# Changelog

The C API version is `kh2_version()` (`KH2_VERSION` in `kh2.h`). Each entry names what a host sees.

## 2.3.0 (round 15)

- Oracle tests of the jump mover (rise time, launch, curve) against Final Mix; kh2_jump_height is class a.
- Reflect: the clash table by attack type and the enemy's clash reaction.

## 2.2.0 (round 14)

- API: every function in `kh2.h` documented (units, ownership, errors); `docs/MIGRATION.md` (1.x to 2.x)
  and the version policy in the README.
- Oracle tests against the Final Mix executable: growth levels, the form timer (length, change,
  whole points), the difficulty step.
- Reflect: the guard for the barrier's life (read from the spell file's particle data), clashes with
  enemy attacks (`KH2_EN_CLASH`), the counter-bursts; Magnet's Wisdom / Master / Final attacks.
- HUD: the INFORMATION bar (`kh2_hud_info`); the party panels' place checked against kh2fm-rs.
- Difficulty: `kh2_set_difficulty` (Beginner / Standard / Proud / Critical) in every hit path; the
  low-HP step in the members' and the summon's hit paths too.

## 2.1.0 (round 13)

- Damage on Final Mix: the Final Mix damage routine disassembled (FM 0x1a1e20); `enemy::low_hp_scale`
  (its new last step, `sstm` +0x178) in Sora's hit paths; the damage and attacker-bonus oracle
  tests run the Final Mix code with its status layout and match 3000 / 3000 (strict again).
- Dodge Roll by the Final Mix rule: root motion, `DodgeRollStarTime` invincibility, trigger 34's
  hold into the Quick Run.
- Limit Form's own limits (the KH1 rows) through `kh2_limit_start`, run by the Limit Form body's script.
- Audio: voices (Sora's battle lines through the `vtbl` pick, enemies' own banks) and footsteps
  (the area's sound info, the ground material), `kh2_audio_set_area`; the master level is the
  game's full level (1.0, was 0.6).
- Magnet: its attack (658) at the target and the pull of its victims (reaction class 5); Reflect
  still pays only.
- HUD on Final Mix: Japanese text (the disc's font set, two planes), the party panels with faces and
  rings, the area-name card (`kh2_hud_area_card`).
- docs/FM-CHANGED.md: the 106 Final Mix functions of Sora's systems that differ, with what is known;
  docs/ABILITIES.md: the 171 Final Mix abilities and what libkh2 does with each.

## 2.0.0 (round 12): Final Mix

- The library reads the Kingdom Hearts II Final Mix disc (`SLPM_666.75`) and only it: `kh2_init`
  refuses another disc. It builds against `../kh2fm-rs` (the Final Mix copy of kh2-rs, not public;
  `KH2RS_COMMIT`). The USA build is the `usa` branch (tag `usa-final`, 1.2.0).
- Forms in the Final Mix numbering (Limit Form 3, Master 4, Final 5, Anti 6): `kh2_drive_setup`
  takes 7 level bytes; save records are format 4 (USA records do not load). Limit Form enters and
  reverts with its own body.
- Dodge Roll: `kh2_set_dodge_roll` (Square tap while moving: A180; hold: Quick Run).
- Ground movement is kh2fm-rs's; the A / B against kh2fm-rs's Sora matches within 0.7 cm.
- Damage: the oracle test now runs the Final Mix executable's damage and differs (report only,
  docs/DEVIATIONS.md); kh2_damage is class b.
- Tests read `KH2_ISO_FM` (or `KH2_ISO`); the golden replays were regenerated on Final Mix.

## 1.2.0 (round 11)

- HUD: Sora's HP / MP panel and drive gauge (kh2-rs `hud_gauge`, the node core: parents, anchors,
  ring wedge clip, reveal), the target gauge and the lock-on reticle (`hud_target`, projected with
  the library's camera), the danger alarm cue; the viewer draws the HUD over its view and saves PNG
  frames. Party panels, banners, INFORMATION bar, message / tutorial windows, area card: not yet.
- Magic: Reflect pays its MP (its trigger 17 at frame 0 was stepped past); every spell at every
  level fires. Magnet's / Reflect's effects still need kh2-rs's spell host (docs/DEVIATIONS.md).
- Audio: `kh2_audio_set_listener` (volume and pan, kh2-rs `sound::placement`), hit sounds
  (`KH2_CUE_HIT`, kh2-rs `PlaySe::hit`), area music `kh2_audio_play_bgm`; loudness measured.
- Sora: the attack wall recoil (a BG Hit swing into a wall: A320 pushed back, or A350 launched in
  the air), found by the A / B against kh2-rs's Sora (0.5 / 0.7 cm worst over two scripted runs,
  docs/FAITHFULNESS.md).

## 1.1.0 (round 10)

- Runtime meshes like libsm64: `kh2_mesh_prepare` / `kh2_mesh_get` (any body skinned on the CPU by
  its current bones: positions, normals, uvs, colours, indices per submesh), `kh2_texture_info` /
  `kh2_texture_get` (decoded RGBA); `docs/MESHES.md`, `unity/Kh2Mesh.cs`.
- Field HUD v1: `kh2_hud_load / input / tick / draw_list / poll_event / set_items /
  texture_info / texture_get`: the command menu (Magic, Items, Drive pages) and the reaction
  capsule as a 2D draw list from the game's art and font (kh2-rs `hud_menu` + `kh2_image`); a
  chosen spell is cast, Attack attacks, the reaction is accepted; `docs/HUD.md`.
- Moving collision objects: `kh2_surface_object_create / move / delete`; Sora rides them.
- Audio engine like `sm64_audio_tick`: `kh2_audio_init` / `kh2_audio_tick` (kh2-rs's software sound
  driver, 16-bit stereo at 48 kHz); keyblade swing sounds are cued.
- Magic: every spell at levels 1 to 3 checked on the disc; Magnet / Reflect gaps documented.
- `scripts/ci.ps1`, `scripts/ci.sh`.

## 1.0.0 (round 9)

- Sound and effect cues: `kh2_poll_cue` (`kh2_cue`, `KH2_CUE_*`): the motions' frame triggers 1, 2,
  8, 13 / 14 and 19 of Sora, the members, the enemies and the summon, the scripts' effect starts,
  the mission's numbered sounds, the collision group switches; `docs/SOUND-AND-EFFECTS.md` maps the
  numbers to the PAX elements and to kh2-extras' WAV files.
- Areas for hosts: `kh2_area_setup` / `kh2_area_get_object` (kh2-rs `run_area`, oracle-tested
  there): every object of the loaded spawn groups (townspeople, field objects, save points, the
  player's start) with talk id and command, the collision mask, camera type and music; the scripts'
  collision group switches (hosts 1.36 / 1.37, 1.46 / 1.47) are applied.
- Damage: checked against the original FUN_001a0610 in kh2-ee (`tests/oracle.rs`); two fixes (rows
  with +5 byte 2 or more use no stat; the attacker's maximum dealt caps even at 0). The attacker
  boosts and Defender's gate are oracle-checked too. `docs/FAITHFULNESS.md`: class a 7 of 110.
- Limits: the damage dead end documented with evidence (`limit_effect_files_carry_no_hit_programs`).
- Round 8 (0.9.0, tagged): release check from the zip (paths remapped out of the DLL,
  `tests/package.rs`), Seifer's bat hits Sora, members' and summons' hits knock orbs out, the soak
  test (`tests/soak.rs`), event queues capped at 1024, `docs/FOR-HOST-AUTHORS.md`.
- Disc tests run serialised (process-wide script tables).

## 0.8.0 (round 7)

- Areas: `kh2_area_run` runs an area's battle program as the game sets it up (kh2-rs `spawn_waves`
  compiled from source: groups, kind-2 waves, kind-6 groups, counters, signals);
  `kh2_area_raise_event`, `kh2_area_take_events`, `kh2_area_end`.
- Enemies: host 1.0's created objects and the calls on other objects are served (M_EX870 acts).
- Struggle: the mission manager (kh2-rs struggle.rs) starts with an area run whose program has a
  mission command: counters, clock, layouts, the mission script, the orb pool, the result 159 / 160;
  `kh2_mission_state`, `kh2_mission_orbs`, `kh2_mission_take_sounds`. tt04 programs 182 to 184
  (184 Seifer).
- Summons: Stitch's and Genie's own classes (blast, potion, follow); `kh2_summon_set_arrival_event`;
  summon events 7 to 11.
- Limits: the members are held for the whole limit.
- Talks: `kh2_talk_begin` / `kh2_talk_end` (the talk camera from the target's record, Sora held);
  `kh2_cam_view.mode` reports 2 for lock-on and 4 for talk.
- `docs/FAITHFULNESS.md` (every function classed against kh2-rs's ledger), `examples/csharp` (a
  .NET console host through `unity/Kh2.cs`, which now compiles outside Unity with `KH2_NO_UNITY`
  and binds all 107 functions), `scripts/check_bindings.py`.
## 0.7.0 (round 6)

- Members' own BDX scripts (kh2-rs `friend_host`): Donald's Cure heals, Specials (Goofy's blade,
  Donald's spells ...) run with their attack objects; member events 7 (script mode), 8 (effect),
  9 (healed); `kh2_member_event` gains `text`.
- Limits: the LIMI motions play on Sora and the members; Auto Limit offers a partner's limit in the
  reaction slot and `kh2_reaction_accept` starts it.
- Summons: `kh2_summon_set_owned`, `kh2_summon_status`, `kh2_summon`, `kh2_summon_state`,
  `kh2_summon_bones`, `kh2_poll_summon_event` (arrival, the object's own script, timer, level,
  dismiss / leave).
- Areas: `kh2_area_spawn` (an area's own battle groups with their spawn records).
- Memory: enemy kinds load on demand and are freed (`kh2_enemies_prune`, `kh2_enemies_clear`);
  122 MiB -> 18 MiB idle.
- Struggle: `kh2_enemies_set_mission` (the counters the opponents' scripts read).
- Items and equipment: `kh2_item_info`, `kh2_set_equipment`, `kh2_party_set_equipment`; the save
  record is v3 (bonus rewards, equipment).
- Header: the record types that had function names are now `kh2_item_info_t`,
  `kh2_limit_state_t`, `kh2_summon_state_t`, `kh2_enemy_state_t`, `kh2_reaction_offer_t` (source
  change for C hosts; the ABI is unchanged). `tests/layout.rs` checks every record against a C compiler.

## 0.6.0 (round 6, first step)

- Member scripts and limit motions (see 0.7.0); the version was bumped with them.

## 0.5.0 (round 5)

- Party members as characters (HP/MP, status by level, enemy hits, KO and revive, styles, item
  handlers), limits, reaction commands from the enemies' reaction parts and the script's requests
  to Sora (paired motions, `KH2_STATE_REACTION`), bonus levels, members' level-ups, form ability
  lists, every enemy kind run, packaging, the Unity sample and the Python example, performance and
  deviation docs.

## 0.4.0 (round 4)

- Enemies under their BDX scripts, spell flight, party battle subset, reaction command offers,
  level-up rewards, the save record, battle replays, the viewer with enemies, the fuzz test and the
  API reference.

## 0.3.0 and earlier (rounds 1 to 3)

- Sora's controller (movement, combos, guard, growth, magic, drive forms, items, levels), the field
  camera, motion export, party following, golden traces, the viewer.
