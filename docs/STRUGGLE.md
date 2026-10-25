# Struggle and story opponents

The Struggle opponents and the story battles' opponents are enemy kinds with their own BDX scripts
(`N_EX570_BTL` Seifer, `N_EX500_BTL`, `N_EX600_BTL` and the others of `docs/ENEMY-KINDS.md`). A
host plays such a fight with:

```c
kh2_surfaces_load_map("tt04");                       /* the sandlot */
kh2_area_spawn(sora, "tt04", 77, 0);                 /* tt04's btl program 77: Seifer's group */
kh2_enemies_set_mission(sora, 1, 100, 100, 0);       /* the orbs: slot 0 Sora's, slot 1 the opponent's */
/* each tick: kh2_tick, kh2_party_tick, kh2_enemies_tick; on KH2_EN_HIT move an orb from the
   opponent to Sora, on KH2_EN_HIT_SORA the other way; the scripts read the counters (host 4.29) */
```

The disc test `struggle_opponents_fight_with_mission_counters` runs Seifer (program 77),
N_EX500_BTL (182) and N_EX600_BTL (183) from tt04's own groups on tt04's collision for 60 s each
with a test host keeping the orbs: each moves, blocks and takes hits under its script, and every
host call it makes is served. Their scripts hold a minimum HP (1) for the story battles; Seifer's
can reach 0.

## The mission manager (0.8)

`kh2_area_run(sora, "tt04", 184)` runs Seifer's Struggle match the way the game sets it up: the
group's program carries the mission command (opcode 0x15, `TT04_MS02`), so the library starts the
mission manager from kh2-rs `struggle.rs` compiled from source (`build`, the mission script with
`MissionHost`, `Active::flow_tick`, the counters and the clock, `knock_out`, `orb_tick` on the prize
rules). Programs 182 (`TT04_MS00`, N_EX500_BTL) and 183 (`TT04_MS01`, N_EX600_BTL) are the other two.
`kh2_enemies_tick` then ticks it: every reaction Sora or the opponent takes knocks 3 or 15 orbs out
of its counter into the pool, the first fighter in reach takes each back (one up on its side), the
start layout plays, the HUD and the clock come up, the clock or a full counter ends the match and the
script sends 159 (won) or 160 (lost). `kh2_mission_state` reads it, `kh2_mission_orbs` places the
orbs, `kh2_area_take_events` gives the global events.

The disc test `the_struggle_mission_runs_its_clock_orbs_and_result` plays the three: Sora attacks,
100 orbs are knocked out of the opponent, the 60 s clock runs out (end "time"), the script sends 159
(Sora has more orbs: 144 to 166 against 0).

Not done: the opponents' bat attacks still never hit Sora (kh2-rs has no Struggle weapon objects),
the result menu (record 0x1a) and the game over are reported (`menu`, `game_over`), not run; the
layout events' pictures are not drawn (their timing is kept). See `DEVIATIONS.md`.