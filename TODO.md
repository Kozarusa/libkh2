# TODO (round 7)

1. [~] Limits that deal damage: not reachable from kh2-rs's rules. Measured on the disc (`limits_hold_the_members_and_their_motions_carry_no_attacks`): the LIMI sub-motions played on Sora and the members carry no attack windows; the limit scripts ask only for motions (2.47), effects (2.50) and PAX elements (2.59); the damage comes from the limit's own objects (the kind 5000 objects of the type 0x14 entry, made by the limit task) which kh2-rs does not port or document. Done instead: the members are in the limit state for the whole limit (kh2-rs `LimitMembers { away: true }`), so they no longer fight on their own during it.
2. [x] Summon classes: kh2-rs summon.rs's pure modules compiled from source (ai, stitch, genie, scripted, rules + locomotion_want); Stitch's class (moves in a camera plane, blast atkp 0x4ba hits every enemy once per attack, MP refill, heal) and Genie's (follow with the members' rules, Potion) run; the arrival event option (appear, sstm 0x84 wait). Test summons_arrive_run_their_scripts_and_leave: Stitch's blast 33 hits, Genie's potion. Not done: Chicken Little's / Peter Pan's classes (scripted needs their per-tick pre pass and the target search wiring), summon limits (the limit task's summon branch), the camera frame is a stand-in behind Sora.
3. [x] Area set-up and waves: kh2-rs enemy.rs's group code (Member .. BattleGroups: area_groups, area_signals, TriggerTable, KindTwo, ring placement, kind-6, counters) and struggle's Trig compiled from source; AreaRun transcribes spawn_waves / fire_group_triggers (kh2_area_run, kh2_area_raise_event, kh2_area_take_events). Test area_runs_set_up_groups_and_waves: tt04 programs 1 / 80 make Dusk and M_EX940 waves, eh06's kind-2 groups make M_EX870 and M_EX920, hb09 program 58 its Shadow waves; removals raise events 3 / 2. Not ported: the story's program choice (the host picks), the tables of the defeated across areas, the field objects that raise signals (the host raises them).
4. [ ] Struggle mission manager.
5. [ ] Talk / event hooks, camera modes.
6. [ ] docs/FAITHFULNESS.md.
7. [ ] C# console sample, binding diff script.

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
