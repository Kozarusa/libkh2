# Remaining (v1.1.0)

Round 10 added meshes, the HUD's command menu, moving collision objects, the audio engine, the
spell levels and the CI scripts. Still open from it:

- HUD: Sora's HP / MP panel and drive gauge, party panels, target gauge, lock-on reticle, banners,
  INFORMATION bar, message window, mission panels, clip / scissor (docs/HUD.md); the HUD in the
  viewer (the test rasterises a PNG frame instead).
- Unity: `unity/Kh2Mesh.cs` is not compiled (no UnityEngine here).
- Magnet makes no pull / attack (its .mag script in the BDX VM, kh2-rs `VmCast`, not ported);
  Reflect never reaches its trigger 17. Limit Form: not on the USA disc.
- Audio: voices, footsteps, music, hit sounds, positional volume / pan; the objects' own sound
  banks were not surveyed for silent effects.
- Moving collision: the world is rebuilt on every object change (fine for a few objects on small
  maps; slow on big COCT maps); objects carry Sora only (not members or enemies).

# Remaining (v1.0.0)

What is still open, honestly; everything listed below as done is tested.

1. Limit damage: not in the data (no hit programs in the 19 limit effect files, no object or attack
   calls in the limit scripts, no limit mode in the members' scripts); it is the executable's limit
   task code, which kh2-rs has not ported (docs/DEVIATIONS.md, `limit_effect_files_carry_no_hit_programs`).
2. Not run here: the Unity sample (`unity/`, `docs/UNITY.md`), the Linux / macOS builds
   (`examples/build_min.sh`, the `nm` path of `scripts/check_bindings.py`), unzipping the package on
   Linux (Windows PowerShell writes backslash paths into the zip). Only Windows + MSVC + .NET 9 ran.
3. Sounds and effects are cues only: the host plays them; hit sparks / hit sounds, menus, music and
   ambience are not cued; Sora's voice ids need the game's voice table (not exported).
4. Townspeople and field objects are data for the host (`kh2_area_setup`); their scripts, dialogue
   (libretto programs, windows), save points, chests, doors and area jumps are the host's; no moving
   collision beyond the scripts' group switches.
5. Struggle: the result menu and the game over are reported, not run; an object's centre is feet + 80 cm.
6. Faithfulness: 7 of 110 functions rest on oracle-tested rules, 46 on kh2-rs rules read from the
   original but not oracle-tested, 57 are glue (docs/FAITHFULNESS.md). Next cheap oracle targets:
   none left that need no actor layout; the attack-table choice and the controller states need the
   whole actor object laid out in kh2-ee.

# TODO (round 9)

1. [~] Limit damage: dead end with evidence (item 1 above).
2. [x] Sound and effect cues (`kh2_poll_cue`), docs/SOUND-AND-EFFECTS.md with the kh2-extras mapping.
3. [x] Area set-up for hosts (`kh2_area_setup`, `kh2_area_get_object`), collision group switches.
4. [x] Oracle tests in libkh2 (`tests/oracle.rs`): damage (two fixes), attacker bonuses, hp_is_low.
5. [x] Soak 5 min release (1,278,165 ticks, heap 34.6 -> 41.1 MiB, same replay), fuzz 40,000 calls,
   package from a fresh folder (C, Python, C#), version 1.0.0, tag v1.0.0.

# TODO (round 8)

1. [x] Release check from the unpacked zip (C, Python, C# against the packaged DLL); package built
   with remapped paths; `tests/package.rs` scans for disc files and machine paths.
2. [x] Seifer's bat hits Sora (`seifers_bat_hits_sora_and_knocks_his_orbs_out`); members' and
   summons' hits knock orbs out; limit damage: dead end documented (item 1 above).
3. [x] Soak (`tests/soak.rs`, `KH2_SOAK_SECS`): release and debug, 10 min each; event queues capped
   at 1024.
4. [x] Documentation pass: README, docs/FOR-HOST-AUTHORS.md, API-REFERENCE regenerated, DEVIATIONS.
# TODO (round 7)

1. [~] Limits that deal damage: not reachable from kh2-rs's rules. Measured on the disc (`limits_hold_the_members_and_their_motions_carry_no_attacks`): the LIMI sub-motions played on Sora and the members carry no attack windows; the limit scripts ask only for motions (2.47), effects (2.50) and PAX elements (2.59); the damage comes from the limit's own objects (the kind 5000 objects of the type 0x14 entry, made by the limit task) which kh2-rs does not port or document. Done instead: the members are in the limit state for the whole limit (kh2-rs `LimitMembers { away: true }`), so they no longer fight on their own during it.
2. [x] Summon classes: kh2-rs summon.rs's pure modules compiled from source (ai, stitch, genie, scripted, rules + locomotion_want); Stitch's class (moves in a camera plane, blast atkp 0x4ba hits every enemy once per attack, MP refill, heal) and Genie's (follow with the members' rules, Potion) run; the arrival event option (appear, sstm 0x84 wait). Test summons_arrive_run_their_scripts_and_leave: Stitch's blast 33 hits, Genie's potion. Not done: Chicken Little's / Peter Pan's classes (scripted needs their per-tick pre pass and the target search wiring), summon limits (the limit task's summon branch), the camera frame is a stand-in behind Sora.
3. [x] Area set-up and waves: kh2-rs enemy.rs's group code (Member .. BattleGroups: area_groups, area_signals, TriggerTable, KindTwo, ring placement, kind-6, counters) and struggle's Trig compiled from source; AreaRun transcribes spawn_waves / fire_group_triggers (kh2_area_run, kh2_area_raise_event, kh2_area_take_events). Test area_runs_set_up_groups_and_waves: tt04 programs 1 / 80 make Dusk and M_EX940 waves, eh06's kind-2 groups make M_EX870 and M_EX920, hb09 program 58 its Shadow waves; removals raise events 3 / 2. Not ported: the story's program choice (the host picks), the tables of the defeated across areas, the field objects that raise signals (the host raises them).
4. [x] Struggle mission manager: kh2-rs struggle.rs (build, MissionHost, orbs, orb_tick, note_event), shared/struggle.rs (Active, Clock, Counter, Layout, the end) and kh2-player prize.rs (launch, flight, reach, draw-in, sweep) compiled from source; started by kh2_area_run when a program has a mission command; kh2_mission_state / kh2_mission_orbs / kh2_mission_take_sounds. Test the_struggle_mission_runs_its_clock_orbs_and_result: tt04 programs 182 / 183 / 184 (Seifer): orbs knocked out and taken, clock to 0, result 159. Also host 1.0 created objects (M_EX870 now acts). Not done: the opponents' bat attacks never hit Sora (no Struggle weapon objects in kh2-rs); result menu / game over reported, not run.
5. [x] Talk / event hooks, camera modes: kh2_talk_begin / kh2_talk_end (kh2-rs npc.rs talk_shot, MOTION_TALK compiled from source; the talk_step glue: face, turn, lock, talk camera via Rig::start_talk / end_talk); kh2_cam_view.mode reports 2 for lock-on, 4 for talk. Test a_talk_locks_sora_and_cuts_the_camera_to_the_talk_view (N_BB010_RTN, N_BB060_RTN, N_BB020_RTN: the view equals kh2-rs talk_pose). Not done: the dialogue (libretto programs, windows) and the townspeople themselves stay the host's.
6. [x] docs/FAITHFULNESS.md: all 107 functions classed (a 4, b 48, c 55) with the kh2-rs ledger evidence per compiled file (scripts/faithfulness.py reads ../kh2-rs/sheets/functions.json) and libkh2 test names; new glue-vs-kh2-rs test the_struggle_glue_knocks_out_what_kh2_rs_orbs_knocked_out_says.
7. [x] C# console sample (examples/csharp, .NET 9: built and run on the disc: the floor run, then Seifer's Struggle to the result "won" at the clock's end) through unity/Kh2.cs (now KH2_NO_UNITY-clean, 107 of 107 functions bound); scripts/check_bindings.py diffs the DLL's PE export table against kh2.h and Kh2.cs (107 / 107 / 107 agree). Version 0.8.0, CHANGELOG.

# TODO (round 6)

1. [x] Member scripts: kh2-rs friend_host.rs compiled in place, friend_ai transcribed (friend.rs): Donald's Cure heals Sora, Specials (Goofy's blade ...) run with their attack objects hitting enemies, no unserved host calls in the test (`member_scripts_cast_cure_and_specials`); limit motions (LIMI sets) play on Sora and the members (Goofy's limits: 299 ticks held), Auto Limit offers a partner's limit in the reaction slot and accept starts it (`limit_motions_play_on_the_bodies_and_auto_limit_offers`). Not ported: the limits' own damage (Comet's / the participants' attack objects come from objects of the LIMI set the library does not create), the scripts' effects (reported), Auto Limit needs the limit tables loaded (any kh2_limit_* call).
2. [x] Summons (summon.rs): command status (charms owned by the host, party, form, drive points, airborne), the start (motion 0x8a, cost, companions away), the arrival (summon timer of drive_rules, the sumn Entity1 object with its own script in appear_skip through the members' host), the drain and summon level, Dismiss / timer end (leave), events and bones in the C API (disc test `summons_arrive_run_their_scripts_and_leave`: all four arrive, run, leave; no unserved host calls). Gaps: the summon classes' AI (Stitch / Genie / Chicken Little / Peter Pan move and attack in their class code, kh2-rs stitch / genie / scripted modules), the arrival event, summon limits, Genie's Drive!, the companions' fades.
3. [x] Area battle groups (area.rs: kh2-rs area_groups; `kh2_area_spawn`; spawn kind / parameters reach the scripts): tt03's Dusks act on tt03's collision, eh06's M_EX870 acts with its record (1 of 4; the others wait for signals / counters), test `area_battle_groups_spawn_and_fight`. Not ported: the areas' signals, triggers, waves, missions (kh2-rs TriggerTable, struggle), story-state program choice. M_EX850 not placed by any scanned area. Enemies hitting members: done since round 5 (their attacks' volumes); targeting a member is not in the enemies' scripts.
4. [x] Enemy kinds load on demand through a second disc reader and are freed when unused (kh2_enemies_prune / kh2_enemies_clear); heap 122 MiB -> 17.6 MiB idle, 28 MiB with 50 enemies of 3 kinds (docs/PERFORMANCE.md); fuzz covers members, limits, summons, area spawns, prune / clear (64 arms); golden / replay green.
5. [x] Struggle opponents (Seifer, N_EX500_BTL, N_EX600_BTL) from tt04's own groups with host-kept mission counters (kh2_enemies_set_mission, host 4.29): they move, block, take hits, no unserved calls (test struggle_opponents_fight_with_mission_counters, docs/STRUGGLE.md). Not ported: the mission manager (layout, timer, orb objects, win/lose events); the opponents' bat attacks come from mission objects, so they never hit Sora.
6. [x] Items / equipment (equipment.rs: kh2-rs sora_status / member_status item_stat sums, resistance fold, the items' abilities into the loadout; kh2_item_info, kh2_set_equipment, kh2_party_set_equipment; saved in the record v3; test equipment_builds_the_status_and_items_are_described). The inventory stays the host's.
7. [x] Safety audit (docs/SAFETY.md, ffi.rs header), tests/layout.rs (generated by scripts/gen_layout_test.py; MSVC static_asserts on 18 records / 169 offsets: all match; found five typedefs named like functions, now ..._t), CHANGELOG.md, version 0.7.0.

# TODO (round 5)

1. [x] Party members as characters: HP/MP (plrp), status from lvup by level, enemy hits with reaction motions, KO / revive timer, styles 0..3, danger + emergency pass + HP/MP item handlers from a host stock (disc test `members_take_hits_fall_revive_use_items_and_fight_in_every_style`). Not ported: Specials/Cure (need the member scripts), equipment, auto-heal of reserve members.
2. [x] Limits: kh2-rs limit.rs rules and limit_script.rs's script thread compiled from source (libkh2-enemy-host build.rs spans); status per command with the following party, start (MP, countdown, event 22 to the limit file's script, event 23 sub-commands), hit count, events for the script's LIMI motions / PAX elements / named events (disc test `a_limit_with_donald_runs_its_script_and_counts_down`: Comet starts, its script runs 601 ticks, ends). Missing, precisely: the participants' threads (FUN_001b3118 starts event 22 on the first participant's own script: Donald's / Goofy's member scripts, which play the limit's attacks; libkh2 runs no member scripts), the LIMI motion playback on the bodies (kh2-rs limit_visual), the summon limits (summon.rs), the HUD gauge animations, Auto Limit's reaction offer (rules compiled, not wired), the saved best count.
3. [x] Script-to-Sora commands (kh2-rs player_reaction consume: paired motion from the enemy's set via Anim::play_external, State::Reaction, velocity, queued motion, unhittable; serial / motion-done back to the scripts) and native offers from reaction parts (kh2-rs reaction_parts.rs + part_offer compiled): the Soldier offers 34 by itself, the accept reaches its script, which plays its paired motion on Sora (disc test `a_soldier_offers_its_reaction_command_and_its_script_moves_sora`: 83 ticks, 1230 cm). 27 kinds carry reaction parts. Not ported: Mickey's Charge command, clashes.
4. [x] Bonus levels (kh2-rs give_bonus: Sora max HP/MP/drive + gauge_carry + items, members' rows; saved in the record v2), members' level-ups with the shared EXP (raise_level, Sword column), `kh2_form_abilities` (disc test `bonus_levels_member_levels_and_form_ability_lists`). Not ported: slot upgrades (no equipment), the level window, the stat-rise messages.
5. [x] All 54 kh2-rs enemy kinds run under their scripts (docs/ENEMY-KINDS.md from `every_enemy_kind_runs`: 47 defeated in the run; M_EX850 / M_EX870 wait for an area set-up and are not running); 18 areas' collision load and Sora walks (`areas_load_and_sora_walks_on_them`).
6. [x] scripts/package.ps1 -> dist/libkh2 (18 files, no disc data, -Zip), unity/sample (Kh2Battle.cs + README), examples/python/kh2_run.py (run: Sora defeats a Soldier at tick 233, from the packaged DLL too).
7. [x] docs/PERFORMANCE.md (bench example: 20k ticks/s alone, 2.2k with 50 enemies; 122 MiB heap), README quick start, DESIGN round 5 decisions, docs/DEVIATIONS.md, API-REFERENCE regenerated.

# TODO (round 4)

1. [x] Enemies under their BDX scripts.
2. [x] Spell flight.
3. [x] Party attacks and AI (battle subset: style 0 target, action records, attack motions, damage; no scripts/limits/Cure).
4. [x] Reaction command offers (kh2-rs reaction_cmd: offer, status, generic accept, event 11 to the script; disc test). Limits: not done (kh2-rs limit.rs needs member scripts and the limit BAR task; the cmd rows with Execute 81 are refused by accept). Script-to-Sora PlayerReactionCmd not consumed.
5. [x] Level-up rewards (kh2-rs raise_level: stat rises, the weapon set's lvup ability given), save record bytes, battle replay (two runs equal + golden). decided: HP/MP rises are the bonus levels (bons), not lvup: not ported. The party's generator word is per party (PartyRng) for replays. Porting-back: kh2-rs defeat_payout overflows i32 for table*row > 2^31 (debug panic).
6. [x] Viewer with enemies and party fighting (E spawns, scripted smoke KH2_VIEWER_FRAMES).
7. [x] C API fuzz test (found and fixed: camera table sized by a stray handle, i32 overflows in damage/payout/item paths, unbounded positions reaching the collision grid), docs/API-REFERENCE.md generated from kh2.h.

# Round 3 (done)

1. [x] Spell attacks: stages, atkp rows, magi sizes, damage helper, Cure heal.
2. [x] Second keyblade and weapon bones for drive forms.
3. [x] Ability system (Loadout set by the host).
4. [x] Items, HP/MP/drive state, level and EXP.
5. [x] Party members: field following (attacks: plan in DESIGN.md).
6. [x] Golden-trace suite, docs/PORTING-BACK.md.
7. [x] Windowed sample host (crates/viewer).

Later: spell flight paths, the ability system's full block, summons, enemies.
