# TODO (round 4)

1. [x] Enemies under their BDX scripts.
2. [x] Spell flight.
3. [x] Party attacks and AI (battle subset: style 0 target, action records, attack motions, damage; no scripts/limits/Cure).
4. [x] Reaction command offers (kh2-rs reaction_cmd: offer, status, generic accept, event 11 to the script; disc test). Limits: not done (kh2-rs limit.rs needs member scripts and the limit BAR task; the cmd rows with Execute 81 are refused by accept). Script-to-Sora PlayerReactionCmd not consumed.
5. [x] Level-up rewards (kh2-rs raise_level: stat rises, the weapon set's lvup ability given), save record bytes, battle replay (two runs equal + golden). decided: HP/MP rises are the bonus levels (bons), not lvup: not ported. The party's generator word is per party (PartyRng) for replays. Porting-back: kh2-rs defeat_payout overflows i32 for table*row > 2^31 (debug panic).
6. [x] Viewer with enemies and party fighting (E spawns, scripted smoke KH2_VIEWER_FRAMES).
7. [ ] C API fuzz test, docs/API-REFERENCE.md.

# Round 3 (done)

1. [x] Spell attacks: stages, atkp rows, magi sizes, damage helper, Cure heal.
2. [x] Second keyblade and weapon bones for drive forms.
3. [x] Ability system (Loadout set by the host).
4. [x] Items, HP/MP/drive state, level and EXP.
5. [x] Party members: field following (attacks: plan in DESIGN.md).
6. [x] Golden-trace suite, docs/PORTING-BACK.md.
7. [x] Windowed sample host (crates/viewer).

Later: spell flight paths, the ability system's full block, summons, enemies.
