# TODO (round 5)

1. [x] Party members as characters: HP/MP (plrp), status from lvup by level, enemy hits with reaction motions, KO / revive timer, styles 0..3, danger + emergency pass + HP/MP item handlers from a host stock (disc test `members_take_hits_fall_revive_use_items_and_fight_in_every_style`). Not ported: Specials/Cure (need the member scripts), equipment, auto-heal of reserve members.
2. [x] Limits: kh2-rs limit.rs rules and limit_script.rs's script thread compiled from source (libkh2-enemy-host build.rs spans); status per command with the following party, start (MP, countdown, event 22 to the limit file's script, event 23 sub-commands), hit count, events for the script's LIMI motions / PAX elements / named events (disc test `a_limit_with_donald_runs_its_script_and_counts_down`: Comet starts, its script runs 601 ticks, ends). Missing, precisely: the participants' threads (FUN_001b3118 starts event 22 on the first participant's own script: Donald's / Goofy's member scripts, which play the limit's attacks; libkh2 runs no member scripts), the LIMI motion playback on the bodies (kh2-rs limit_visual), the summon limits (summon.rs), the HUD gauge animations, Auto Limit's reaction offer (rules compiled, not wired), the saved best count.
3. [x] Script-to-Sora commands (kh2-rs player_reaction consume: paired motion from the enemy's set via Anim::play_external, State::Reaction, velocity, queued motion, unhittable; serial / motion-done back to the scripts) and native offers from reaction parts (kh2-rs reaction_parts.rs + part_offer compiled): the Soldier offers 34 by itself, the accept reaches its script, which plays its paired motion on Sora (disc test `a_soldier_offers_its_reaction_command_and_its_script_moves_sora`: 83 ticks, 1230 cm). 27 kinds carry reaction parts. Not ported: Mickey's Charge command, clashes.
4. [x] Bonus levels (kh2-rs give_bonus: Sora max HP/MP/drive + gauge_carry + items, members' rows; saved in the record v2), members' level-ups with the shared EXP (raise_level, Sword column), `kh2_form_abilities` (disc test `bonus_levels_member_levels_and_form_ability_lists`). Not ported: slot upgrades (no equipment), the level window, the stat-rise messages.
5. [x] All 54 kh2-rs enemy kinds run under their scripts (docs/ENEMY-KINDS.md from `every_enemy_kind_runs`: 47 defeated in the run; M_EX850 / M_EX870 wait for an area set-up and are not running); 18 areas' collision load and Sora walks (`areas_load_and_sora_walks_on_them`).
6. [ ] Packaging, Unity sample, Python ctypes example.
7. [ ] Performance report, docs pass.

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
