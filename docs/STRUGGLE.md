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

What the library does not do (the Struggle's mission manager, kh2-rs `struggle.rs`): the mission's
own layout and timer (the mission BAR and its script), the orb objects (the prize pool `PRIZE_TT`:
the library reports drops as counts), the win / lose events (159 / 160) and the result screens. In
the test the opponents never landed a hit on Sora: their attacks come from objects the mission set-up
makes (the Struggle bats' hit volumes), which the library does not create. A host that wants the
full Struggle keeps the score and the clock itself.
