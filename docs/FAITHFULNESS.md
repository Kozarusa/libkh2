# Faithfulness

How close each public function of `kh2.h` is to the original game, in three classes:

- **a** the result is decided by kh2-rs code that an oracle test in kh2-rs compared with the
  original (the original ran in kh2-ee; kh2-rs ledger status `verified`), compiled here unchanged;
- **b** the result is decided by kh2-rs code read from the original (ledger `read` / `tested`) but
  not oracle-tested, compiled here unchanged;
- **c** new libkh2 glue: records, handles, the C ABI, or a transcription of a kh2-rs Bevy system
  without Bevy. The glue's rules underneath are named with their own class.

"Compiled unchanged" means `#[path]` or a span cut from the kh2-rs file by a build script
(`crates/kh2-pure/build.rs`, `crates/kh2-enemy-host/build.rs`); the text is not edited (one span
has a visibility change: `MissionHost` and its fields are made `pub`).

## Evidence per kh2-rs file

`python scripts/faithfulness.py` prints this table from kh2-rs's function ledger
(`sheets/functions.json`, filled by `kh2 ledger scan`): the original functions each compiled file
cites and their status. Counts are per file (a span compiles part of its file).

| kh2-rs file | use | cited | verified | tested | ported | read | other |
|---|---|---|---|---|---|---|---|
| kh2-game-core/src/motor.rs | in place | 44 | 20 | 7 | 0 | 13 | 4 |
| kh2-game-core/src/world.rs | in place | 24 | 20 | 1 | 0 | 3 | 0 |
| kh2-game-core/src/reaction_rules.rs | in place | 48 | 3 | 10 | 0 | 33 | 2 |
| kh2-game-core/src/camera_rules.rs | in place | 74 | 15 | 6 | 0 | 48 | 5 |
| kh2-game-core/src/magic_rules.rs | in place | 78 | 17 | 14 | 0 | 45 | 2 |
| kh2-game-core/src/party_rules.rs | in place | 67 | 19 | 14 | 1 | 32 | 1 |
| kh2-game-core/src/reaction_parts.rs | in place | 13 | 0 | 4 | 0 | 8 | 1 |
| kh2-game-core/src/rng.rs | span | 11 | 5 | 1 | 4 | 0 | 1 |
| kh2-game-core/src/shared/abilities.rs | span | 42 | 12 | 9 | 0 | 20 | 1 |
| kh2-game-core/src/shared/drive.rs | span | 63 | 16 | 7 | 0 | 39 | 1 |
| kh2-game-core/src/shared/party.rs | span | 12 | 6 | 3 | 0 | 3 | 0 |
| kh2-game-core/src/shared/enemy.rs | in place | 10 | 1 | 2 | 0 | 7 | 0 |
| kh2-game-core/src/shared/enemy_obj.rs | in place | 27 | 5 | 9 | 0 | 13 | 0 |
| kh2-game-core/src/shared/reaction.rs | in place | 13 | 2 | 2 | 0 | 9 | 0 |
| kh2-game-core/src/shared/struggle.rs | in place | 113 | 11 | 14 | 0 | 88 | 0 |
| kh2-game-core/src/shared/prize.rs | in place | 42 | 5 | 12 | 0 | 25 | 0 |
| kh2-game-core/src/shared/summon.rs | span | 49 | 3 | 13 | 0 | 32 | 1 |
| kh2-game-core/src/progress.rs | span | 206 | 98 | 22 | 3 | 81 | 2 |
| kh2-player/src/controller_rules.rs | in place | 18 | 10 | 0 | 0 | 6 | 2 |
| kh2-player/src/player_reaction.rs | span | 25 | 1 | 4 | 0 | 20 | 0 |
| kh2-player/src/prize.rs | span | 92 | 22 | 27 | 0 | 41 | 2 |
| kh2-party/src/reaction_cmd.rs | span | 55 | 6 | 9 | 0 | 39 | 1 |
| kh2-party/src/party_battle.rs | span | 111 | 13 | 25 | 2 | 69 | 2 |
| kh2-party/src/items.rs | span | 97 | 15 | 18 | 0 | 63 | 1 |
| kh2-party/src/friend_host.rs | in place | 81 | 7 | 19 | 0 | 54 | 1 |
| kh2-party/src/limit.rs | span | 43 | 7 | 10 | 0 | 25 | 1 |
| kh2-party/src/limit_visual.rs | span | 9 | 1 | 2 | 0 | 6 | 0 |
| kh2-party/src/limit_script.rs | span | 37 | 2 | 5 | 0 | 30 | 0 |
| kh2-party/src/summon.rs | span | 337 | 55 | 67 | 2 | 198 | 15 |
| kh2-enemy/src/enemy_obj.rs | in place | 5 | 0 | 4 | 0 | 1 | 0 |
| kh2-enemy/src/enemy_host.rs | in place | 194 | 39 | 28 | 0 | 117 | 10 |
| kh2-enemy/src/enemy.rs | span | 100 | 32 | 27 | 0 | 40 | 1 |
| kh2-event/src/struggle.rs | span | 72 | 8 | 13 | 1 | 50 | 0 |
| kh2-event/src/npc.rs | span | 136 | 13 | 28 | 1 | 93 | 1 |
| kh2-bdx/src | crate | 281 | 36 | 34 | 0 | 202 | 9 |
| kh2-anim/src | crate | 33 | 0 | 2 | 0 | 31 | 0 |
| kh2-tables/src | crate | 13 | 0 | 3 | 0 | 10 | 0 |

Oracle tests the ledger names for the class-a rows below (kh2-rs test paths):
`kh2-game-core/tests/orig_coct.rs::orig_141b50_matches_the_game`, `orig_143840_matches_the_game`,
`the_built_object_matches_the_game` (collision build and the mover); `kh2-player/tests/orig_l2_p1.rs::orig_16a948_matches_the_game`,
`orig_106f78_matches_the_game` (controller rules); `kh2-game-core/tests/orig_l2_p2.rs::orig_13f600_matches_the_game`,
`orig_13f740_matches_the_game` (camera); `kh2-game-core/src/party_rules.rs::party_follow_step_matches_fun_0010e140`,
`party_steer_matches_fun_00110468`, `party_trail_matches_fun_0010b270` (party follow);
`kh2-party/src/items.rs::item_effect_matches_the_game` (item effects); `kh2-game-core/tests/orig_status.rs::orig_19f920_matches_the_game`
(the status rebuild); `kh2-game-core/tests/orig_frame.rs::orig_14c6b0_matches_the_game`, `orig_14c6e0_matches_the_game`,
`orig_14c728_matches_the_game` (the mission clock).

## Per function

Test names are libkh2's (`crates/libkh2/tests/`); every function is also in `fuzz.rs`
(`the_c_api_survives_random_calls`) and its records in `layout.rs` (`c_layouts_match_kh2_h`).

| function | class | what decides the result | libkh2 tests |
|---|---|---|---|
| kh2_init, kh2_terminate, kh2_version, kh2_last_error | c | library state; the disc tables through kh2-tables (b) | `calls_before_init_and_bad_arguments_return_errors`, `movement_matches_the_disc_values` |
| kh2_surfaces_load, kh2_set_collision | c | host triangles into kh2-rs `World`; the queries are world.rs (a) | `the_floor_normal_points_up_and_the_wall_is_solid`, `hidden_groups_are_not_solid` |
| kh2_surfaces_load_map | a | world.rs COCT build (`the_built_object_matches_the_game`) | `an_area_collision_loads_and_sora_stands_on_it`, `areas_load_and_sora_walks_on_them` |
| kh2_create, kh2_delete, kh2_set_position, kh2_get_state, kh2_poll_event | c | handles and records | `movement_matches_the_disc_values` |
| kh2_tick, kh2_tick_field | c | sora.rs transcribes kh2-rs `sora::tick`; movement is motor.rs (a, `orig_143840_matches_the_game`) and controller_rules.rs (a, `orig_16a948_matches_the_game`) | `sora_runs_at_run_speed_and_stops_at_the_wall`, `a_held_jump_peaks_at_the_jump_height_and_lands`, `walking_off_a_ledge_falls_with_the_fall_cap`, `the_scripted_replay_matches_the_golden_trace`, `battle_replays_the_same` |
| kh2_set_target, kh2_clear_target, kh2_lock_on_pick | c | the lock-on point; the pick is kh2-rs `nearest` within `sstm` LockOnRange | `the_field_camera_follows_collides_and_locks_on` |
| kh2_report_hit | c | the combo window of the attack table (kh2-anim triggers, b) | `the_combo_advances_only_inside_the_window_after_a_hit`, `without_a_hit_the_window_does_not_chain` |
| kh2_hit_taken | b | reaction_rules `victim_outcome` (read; 3 of 48 cited verified) | `hits_flinch_blow_knock_out_and_guard` |
| kh2_set_hp, kh2_revive, kh2_set_high_jump, kh2_set_magic_levels, kh2_set_mp, kh2_set_weapon_set, kh2_set_pose_enabled | c | setters | `hits_flinch_blow_knock_out_and_guard`, `level_one_spells_cast_pay_and_charge` |
| kh2_set_abilities, kh2_set_loadout, kh2_get_loadout_block | b | shared/abilities.rs (12 of 42 verified, e.g. `scan_gate_matches_the_game`; the block itself read) | `the_loadout_sets_combo_lengths_growth_and_survival` |
| kh2_set_growth | b | the `fmab` rows through abilities.rs | `growth_abilities_use_the_fmab_rows` |
| kh2_audio_set_area | c | kh2fm-rs `arif_record` / `area_soundinfo`; the voice pick `kh2_audio::voice` and `footstep_number` are kh2fm-rs code read from the original | `voices_and_footsteps_play_on_final_mix` |
| kh2_set_difficulty | a | `enemy::difficulty_scale` against FM 0x1a2108 (`orig_fm_1a2108_difficulty_matches_the_game`) | the oracle test |
| kh2_hud_message, kh2_hud_message_confirm | c | kh2fm-rs `hud_msgwin` with kh2_image `window` (read: 0x1aaf20, 0x1ab170, 0x1ab370) | `the_message_window_opens_types_waits_and_closes` |
| kh2_hud_info | c | kh2fm-rs `hud_info` (FUN_00150428 / 0x150688 / 0x1507d8 read) | `the_information_bar_opens_scrolls_and_closes` |
| kh2_hud_area_card | c | kh2fm-rs `hud_card` (place table, font 2 at (256, 192), 120 frames) | `the_party_panels_and_the_area_card_draw_on_final_mix` |
| kh2_set_dodge_roll | c | kh2fm-rs `square_ground` / `dash_step` transcribed (FM 0x178eb0, the A180 trigger 34 at tick 10) | `the_dodge_roll_taps_and_holds` |
| kh2_bone_count, kh2_bone_parents, kh2_motion_tag, kh2_motion_slot_count | c | the loaded model / motion set | `any_motion_slot_samples_like_the_animator` |
| kh2_get_bones, kh2_get_rest_bones, kh2_motion_slot, kh2_motion_info, kh2_sample_motion | b | kh2-anim pose and blend, kh2-model rest pose (read); anim.rs transcribes kh2-rs skel.rs | `any_motion_slot_samples_like_the_animator` |
| kh2_get_attack_volumes | b | the motion's attack triggers (kh2-anim, read) | `the_ground_and_air_combos_play_the_attack_table` |
| kh2_jump_height | a | `prty` JumpHeight / High Jump row through motor.rs; the jump mover (rise time, launch, curve) oracle-tested against FM 0x17d130 / 0x17c430 / 0x17c930 (round 15) | `a_held_jump_peaks_at_the_jump_height_and_lands` |
| kh2_cast | b | magic_rules.rs (17 of 78 verified, e.g. `orig_17bbc0_matches_the_game`; the cast flow read) and spell.rs's transcription (c) | `level_one_spells_cast_pay_and_charge`, `spells_fly_and_hit_enemies`, `a_homing_shot_turns_by_its_rate` |
| kh2_damage, kh2_drive_outgoing_damage | a | (Final Mix, round 13: against FM 0x1a1e1c / 0x1a22b0 / 0x1a2340 with the Final Mix status layout, 3000 cases each, 0 differ; plus the Final Mix last step `low_hp_scale`) the damage formula `kh2::enemy::damage` against the original FUN_001a0610 in kh2-ee (libkh2 `oracle.rs::orig_1a0610_damage_matches_the_game`, 3000 cases; it found and fixed two differences of kh2-rs's formula), the boosts FUN_001a0a18 / FUN_001a0aa8 (`orig_1a0a18_1a0aa8_attacker_bonuses_match_the_game`) | `spells_report_attack_stages_with_disc_rows_and_damage` |
| kh2_use_item | a | items.rs item effect (`item_effect_matches_the_game`); the use motion is glue | `items_heal_through_the_use_motion_and_defeats_raise_the_level` |
| kh2_set_level | a | the status rebuild (`orig_19f920_matches_the_game`) with the `lvup` rows | `level_ups_give_the_lvup_rows_abilities_and_the_record_saves` |
| kh2_report_defeat, kh2_give_bonus | b | experience and `bons` rows (progress.rs spans, read) | `items_heal_through_the_use_motion_and_defeats_raise_the_level`, `bonus_levels_member_levels_and_form_ability_lists` |
| kh2_item_info, kh2_set_equipment, kh2_party_set_equipment | b | `item_stat` sums and the resistance fold (kh2-rs data.rs / shared/party.rs, read) | `equipment_builds_the_status_and_items_are_described` |
| kh2_form_abilities, kh2_granted_abilities | b | form levels and their abilities (read) | `bonus_levels_member_levels_and_form_ability_lists` |
| kh2_state_save, kh2_state_load | c | libkh2's own record (v3) | `level_ups_give_the_lvup_rows_abilities_and_the_record_saves` |
| kh2_drive_setup, kh2_drive_gain, kh2_report_hit_atkp, kh2_drive_enter, kh2_drive_revert, kh2_drive_objects | b | shared/drive.rs rules (16 of 63 verified, e.g. `summon_gauge_matches_the_game`); drive.rs transcribes `drive_tick` (c) | `drive_forms_enter_swap_and_revert` |
| kh2_drive_set_second_keyblade | c | the left-hand weapon | `valor_master_final_carry_a_second_keyblade_on_the_left_hand` |
| kh2_party_tick | c | party.rs transcribes `party_tick`; follow is party_rules.rs (a, `party_follow_step_matches_fun_0010e140`), battle choice party_battle.rs (b, 13 of 111 verified) | `donald_and_goofy_follow_sora`, `donald_and_goofy_fight_an_enemy` |
| kh2_party_add, kh2_party_state, kh2_party_set_vitals, kh2_party_set_items, kh2_poll_member_event | c | records | `members_take_hits_fall_revive_use_items_and_fight_in_every_style` |
| kh2_party_set_level, kh2_party_set_style, kh2_party_bones | b | member levels, the battle styles, the member pose (read) | `bonus_levels_member_levels_and_form_ability_lists`, `members_take_hits_fall_revive_use_items_and_fight_in_every_style` |
| kh2_limit_status, kh2_limit_start | b | limit.rs rules and limit_script.rs thread (read) | `a_limit_with_donald_runs_its_script_and_counts_down`, `limits_hold_the_members_and_their_motions_carry_no_attacks` |
| kh2_limit_set_world, kh2_limit_state, kh2_poll_limit_event | c | records | `limit_motions_play_on_the_bodies_and_auto_limit_offers` |
| kh2_summon_status, kh2_summon, kh2_summon_bones | b | summon.rs modules (55 of 337 verified, e.g. `summon_arrival_choice_matches_the_game`; the classes read) and shared/drive.rs | `summons_arrive_run_their_scripts_and_leave` |
| kh2_summon_set_owned, kh2_summon_set_arrival_event, kh2_summon_state, kh2_poll_summon_event | c | records | `summons_arrive_run_their_scripts_and_leave` |
| kh2_enemy_spawn, kh2_enemies_tick, kh2_enemy_bones | b | the enemies' own BDX scripts (kh2-bdx, 36 of 281 verified) answered by enemy_host.rs (39 of 194 verified); enemy.rs transcribes `ai_update` (c) | `twilight_town_enemies_run_their_scripts_fight_and_die`, `every_enemy_kind_runs` |
| kh2_enemy_count, kh2_enemy_state, kh2_poll_enemy_event, kh2_enemies_prune, kh2_enemies_clear, kh2_enemies_set_mission | c | records, loading | `twilight_town_enemies_run_their_scripts_fight_and_die`, `struggle_opponents_fight_with_mission_counters` |
| kh2_area_spawn, kh2_area_run, kh2_area_raise_event | b | enemy.rs area groups, signals, kind-2 waves (32 of 100 verified); area.rs transcribes `spawn_waves` (c) | `area_battle_groups_spawn_and_fight`, `area_runs_set_up_groups_and_waves` |
| kh2_area_take_events, kh2_area_end | c | records | `area_runs_set_up_groups_and_waves` |
| kh2_mission_state, kh2_mission_orbs | b | struggle.rs and shared/struggle.rs (the clock's `orig_14c6b0_matches_the_game` family verified, the rest read), prize rules; struggle.rs glue (c, compared below) | `the_struggle_mission_runs_its_clock_orbs_and_result`, `the_struggle_glue_knocks_out_what_kh2_rs_orbs_knocked_out_says` |
| kh2_mission_take_sounds | c | records | `the_struggle_mission_runs_its_clock_orbs_and_result` |
| kh2_talk_begin, kh2_talk_end | b | npc.rs `talk_shot` and camera_rules `start_talk` / `talk_pose` (read); talk.rs glue (c, compared below) | `a_talk_locks_sora_and_cuts_the_camera_to_the_talk_view` |
| kh2_area_setup | a | kh2-ard `run_area` (orig 0x181148, `orig_181148_matches_the_game`) and `entity_for_entrance` (FUN_001981a0, `orig_1981a0_matches_the_game`) | `an_area_setup_lists_townspeople_with_their_talks` |
| kh2_area_get_object, kh2_poll_cue | c | records; the cues read the motions' frame triggers (kh2-anim, b) as kh2-rs's audio / fx systems do | `motions_give_sound_and_effect_cues` |
| kh2_texture_info, kh2_texture_get | b | kh2-model's texture decoder (CLUT, swizzle, alpha; read) | `sora_mesh_skins_with_his_bones` |
| kh2_mesh_prepare, kh2_mesh_get | c | CPU skinning of kh2-model's decoded meshes (b) by the current bones | `sora_mesh_skins_with_his_bones` |
| kh2_hud_tick, kh2_hud_draw_list | b | kh2-rs `hud_menu` layout rules and executable tables (read), `kh2_image` SEQD player and text engine (read); the node glue (c) | `the_command_menu_draws_and_casts_fire` |
| kh2_hud_load, kh2_hud_input, kh2_hud_set_items, kh2_hud_poll_event, kh2_hud_texture_info, kh2_hud_texture_get | c | loading, menu input, records | `the_command_menu_draws_and_casts_fire` |
| kh2_surface_object_create, kh2_surface_object_move, kh2_surface_object_delete | c | libkh2's own moving collision (the world rebuilt, the ride rule) | `a_moving_platform_carries_sora` |
| kh2_audio_tick | b | kh2-audio's software sound driver (SE sequencer, SPU2 voices; read) | `sounds_of_a_jump_an_attack_and_fire_render_to_pcm` |
| kh2_audio_init | c | loading | `sounds_of_a_jump_an_attack_and_fire_render_to_pcm` |
| kh2_reaction_offer, kh2_reaction_accept | b | reaction_cmd.rs offers (6 of 55 verified), reaction_parts.rs (read), player_reaction.rs paired motions | `a_reaction_command_is_offered_accepted_and_delivered`, `a_soldier_offers_its_reaction_command_and_its_script_moves_sora` |
| kh2_reaction_force_offer | c | a test hook | `a_reaction_command_is_offered_accepted_and_delivered` |
| kh2_camera_tick | a | camera_rules.rs rig (15 of 74 verified, `orig_13f600_matches_the_game`, `orig_13f740_matches_the_game`); camera.rs drives it as kh2-rs `camera::tick` | `the_field_camera_follows_collides_and_locks_on`, `a_talk_locks_sora_and_cuts_the_camera_to_the_talk_view` |
| kh2_camera_reset | c | the table row choice | `the_field_camera_follows_collides_and_locks_on` |

Totals (134 functions, Final Mix): class a 9 (kh2_jump_height, kh2_set_difficulty, kh2_surfaces_load_map, kh2_use_item, kh2_set_level,
kh2_camera_tick, kh2_damage, kh2_drive_outgoing_damage, kh2_area_setup), class b 50, class c 75. The classes rest
on kh2fm-rs's ledger, whose oracle evidence was gathered on the USA executable (its `main-fm` image
rows carry the Final Mix addresses); kh2_damage and kh2_drive_outgoing_damage were b in 2.0.0 (the Final Mix oracle differed) and are a
again in 2.1.0: the differences were the test's USA status layout (docs/DEVIATIONS.md):
most of the game rules are kh2-rs code read from the original but not oracle-tested; the records,
setters and the transcribed Bevy flows are glue.

## libkh2's own oracle tests (`crates/libkh2/tests/oracle.rs`)

Run the original from the user's executable in kh2-rs's kh2-ee (read-only dependency) on seeded
cases, every case compared:

| test | original | libkh2 / kh2-rs code | result |
|---|---|---|---|
| `orig_1a0610_damage_matches_the_game` | FUN_001a0610 (damage) | `kh2::enemy::damage` | 3000 / 3000 after two fixes: a row with +5 byte 2 or more uses no stat (kh2-rs used strength), the attacker's maximum dealt caps even at 0 (kh2-rs took 0 as none) |
| `orig_1a0a18_1a0aa8_attacker_bonuses_match_the_game` | FUN_001a0a18, FUN_001a0aa8, FUN_00176be8, FUN_00176c40 | `abilities::attacker_bonuses` | 3000 / 3000 |
| `orig_1a0be0_hp_is_low_matches_the_game` | FUN_001a0be0 | `abilities::hp_is_low` (Defender's gate) | 3000 / 3000 |

Not done (not cheap): the jump curve and the wall slide are kh2-rs's motor.rs, already oracle-tested
there (`orig_143840_matches_the_game` and the mover family); the attack-table choice and the
controller states are large state machines on the actor object that need the whole actor laid out.

## Glue checked against kh2-rs values

- `the_struggle_glue_knocks_out_what_kh2_rs_orbs_knocked_out_says`: for reactions 0..19 on Seifer's
  mission, the counters' drop and the orbs launched by the glue equal kh2-rs `orbs_knocked_out`
  (FUN_001caeb0) for both sides.
- `a_talk_locks_sora_and_cuts_the_camera_to_the_talk_view`: the camera view after `begin` equals
  kh2-rs `talk_pose` of the shot `talk_shot` reads (N_BB010_RTN, N_BB060_RTN, N_BB020_RTN).
- `the_field_camera_follows_collides_and_locks_on`: `kh2_lock_on_pick` takes the nearest point
  within the disc's `sstm` LockOnRange, as kh2-rs `nearest` does.
- `the_scripted_replay_is_stable_on_the_disc`, `battle_replays_the_same`: golden traces of the glue
  (no kh2-rs trace exists to compare: kh2-rs runs only under Bevy).

## A / B against kh2-rs's own Sora (round 11)

`ab_against_kh2_rs_sora` (tests/disc.rs) replays the per-tick `trace` lines of kh2-rs's app
(`KH2_TRACE=1 KH2_TURBO=1`, `KH2_KEYS`, tt04 from the same start) through `Sora::tick` on tt04's
collision and compares the positions tick by tick (`KH2_AB_TRACE=<trace>`, `KH2_AB_KEYS=<the same
keys>`; skipped without the file).

| Scenario | Ticks | Worst | Notes |
|---|---|---|---|
| walk W 2-6 s, jump at 7 s | 899 | 0.5 cm | ground walk, run, jump and landing |
| three J presses at 2 / 2.4 / 2.8 s toward the wall, jump at 4 s, J at 4.2 s | 779 | 0.7 cm | the ground combo bounces off the wall (A320), the air attack too (A350) |

Divergence found and fixed: libkh2 had no attack wall recoil (kh2-rs `combat::detect_hits` terrain
cast + `sora::receive_wall_hits`); at tick 141 kh2-rs entered Hurt A320 and libkh2 swung on (202 cm
apart). Ported as `Sora::wall_test`. Remaining difference, by design: kh2-rs casts on Bevy
transforms two frames old (the body's propagated transform and the keyblade's bone attachment), so
its recoil starts two ticks later than the library's (24.6 cm worst with `KH2_AB_LAG=0`). The
library casts on this tick's matrices, the original's order (FUN_001894d0 fills the volumes from the
current bone matrices); `Sora::wall_cast_lag = 2` reproduces kh2-rs and the test uses it.

## Final Mix oracle tests of round 14

Against the Final Mix executable in kh2-ee (`tests/oracle.rs`, 3000 seeded cases or every input):

| FM function (USA) | libkh2 code | Result | Used by |
|---|---|---|---|
| 0x1c35a0 (0x1c1340) growth levels from the ability bits | `AbilityRules::growth_levels` | match | kh2_set_loadout, kh2_set_abilities (the growth they give) |
| 0x1a1cc0 -> 0x1a1c90 (0x1a04b0 / 0x1a0480) form timer length | `FormTimer::start` (Final Mix numbering) | match, forms 1..6, levels 0..7 | kh2_drive_enter |
| 0x1a1b68 (0x1a0358) form timer change; 0x1a1c30 (0x1a0420) whole points | `FormTimer::add`, `whole_points` | match | kh2_drive_gain, kh2_report_hit_atkp, the drain |
| 0x17d130 / 0x17c430 / 0x17c930 (0x17bee0 / 0x17b1e0 / 0x17b6e0) jump rise time, launch, curve | `motor::Jump` (`up`, `fall`, `target_y`) | match within 1e-3 (the game's own `sinf`) | Sora's jumps, leaps, falls (kh2_jump_height, every tick in the air) |
| 0x1a2108 (0x1a0898) difficulty step | `enemy::difficulty_scale` | match | kh2_set_difficulty, every hit path |
| 0x1a1e20, 0x1a22b0, 0x1a2340, 0x1a2478 (round 13) damage, bonuses, low HP | `enemy::damage`, `low_hp_scale`, `attacker_bonuses`, `hp_is_low` | match | kh2_damage and the hit paths |

The classes of the table above do not move: those functions also run rules not covered by these
tests (the drive entry checks and the Anti / Final roll, the loadout's stat effects). Not oracle-tested,
with the reason:

- (Round 15: the jump curve is now oracle-tested, through the mover's own fields +0xd0..+0xdc.)
- Wall slide, attack-table selection, hit reaction choice, the cast flow: they read and
  write the whole player actor (movers, motion controller, command object, +0x48c status) through
  dozens of calls; a faithful actor layout in test RAM is a project of its own (kh2fm-rs has the USA
  ones for the pure parts: `orig_l2_p3` ground mover, `orig_17bbc0` magic).
- Magic cost and charge (FM 0x1bc9e4 is a changed function, 1020 bytes, not examined).
- Level / EXP: the level-up reads the save record of the progress object, not laid out here.
- Decoders and the sound driver (textures, motions, SEQD, SPU2): compared with the data on the disc,
  not with code.
