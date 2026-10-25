# Changelog

The C API version is `kh2_version()` (`KH2_VERSION` in `kh2.h`). Each entry names what a host sees.

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
