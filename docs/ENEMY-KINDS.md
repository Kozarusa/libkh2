# Enemy kinds and areas

Every enemy kind kh2fm-rs has a kind file for (`kh2-enemy/src/kinds`, the same 54 files as the USA tree:
kh2fm-rs has no Final Mix-only enemy kind yet) is compiled into libkh2 and
spawns with `kh2_enemy_spawn(handle, "<model>", ...)`. The table is the output of the disc test
`every_enemy_kind_runs` (`cargo test --release -p libkh2 --test disc every_enemy_kind_runs -- --nocapture`):
each kind on a flat floor, 10 s left to its script with Sora standing 3 m away, then 20 s of Sora
attacking it. "moved" is the farthest it went from its spawn point, "motions" the distinct motion
slots it played, "unserved host calls" the calls its script made that the host does not serve.

The table below is the Final Mix disc's run (2.2.0). M_EX210 and M_EX690 make one host call each that
the host does not serve (named in the test's log; not ported).
Areas: `areas_load_and_sora_walks_on_them`, `area_runs_set_up_groups_and_waves` and
`area_battle_groups_spawn_and_fight` run on the Final Mix maps (map/jp read through the map/us names).

Reading the table:
- Every kind loads and runs its own script; 47 of 54 are defeated by Sora's combo in the run.
- M_EX590, M_EX760, M_EX900 move and act but were not hit in the run (they fly or keep away; a
  host that brings Sora to them can hit them).
- M_EX870 acts since 0.8 (host 1.0 created objects and the calls on other objects are served):
  on a flat floor it moved 674 cm, played 3 motions, hit Sora 16 times and was defeated.
- M_EX850 stays in one motion on a flat floor: its script waits for an area set-up the scan does
  not run. It is listed as not running.
- N_EX500_BTL and N_EX600_BTL are story battles whose scripts hold a minimum HP (they end by the
  story, not by HP).

| model | kind | script | moved cm | motions | hit Sora | defeated by Sora | unserved host calls |
| --- | --- | --- | --- | --- | --- | --- | --- |
| M_AL020_FIRE | M_AL020_FIRE | yes | 1127 | 7 | 0 | yes | 0 |
| M_AL020_FIRE_RAW | M_AL020_FIRE_RAW | yes | 1072 | 8 | 0 | yes | 0 |
| M_AL020_ICEE | M_AL020_ICEE | yes | 387 | 8 | 0 | yes | 0 |
| M_AL020_ICEE_RAW | M_AL020_ICEE_RAW | yes | 402 | 11 | 0 | yes | 0 |
| M_EX010 | Soldier | yes | 835 | 11 | 3 | yes | 0 |
| M_EX020 | Shadow | yes | 542 | 7 | 0 | yes | 0 |
| M_EX020_RAW | M_EX020_RAW | yes | 166 | 6 | 0 | yes | 0 |
| M_EX050 | M_EX050 | yes | 700 | 10 | 2 | yes | 0 |
| M_EX060 | M_EX060 | yes | 490 | 10 | 0 | yes | 0 |
| M_EX110 | M_EX110 | yes | 165 | 8 | 0 | yes | 0 |
| M_EX120 | M_EX120 | yes | 495 | 9 | 2 | yes | 0 |
| M_EX130 | M_EX130 | yes | 546 | 13 | 1 | yes | 0 |
| M_EX210 | M_EX210 | yes | 1456 | 12 | 2 | yes | 1 |
| M_EX420 | M_EX420 | yes | 574 | 11 | 1 | yes | 0 |
| M_EX500 | M_EX500 | yes | 581 | 10 | 0 | yes | 0 |
| M_EX510 | M_EX510 | yes | 485 | 9 | 0 | yes | 0 |
| M_EX520 | M_EX520 | yes | 438 | 6 | 0 | yes | 0 |
| M_EX550 | M_EX550 | yes | 568 | 10 | 2 | yes | 0 |
| M_EX560 | M_EX560 | yes | 737 | 12 | 0 | yes | 0 |
| M_EX570 | M_EX570 | yes | 763 | 11 | 3 | yes | 0 |
| M_EX580 | M_EX580 | yes | 1355 | 8 | 2 | yes | 0 |
| M_EX590 | M_EX590 | yes | 4580 | 8 | 0 | no (0 hits) | 0 |
| M_EX600 | M_EX600 | yes | 1695 | 7 | 0 | yes | 0 |
| M_EX610 | M_EX610 | yes | 942 | 9 | 0 | yes | 0 |
| M_EX610_RAW | M_EX610_RAW | yes | 942 | 9 | 0 | yes | 0 |
| M_EX620 | M_EX620 | yes | 915 | 8 | 0 | yes | 0 |
| M_EX630 | M_EX630 | yes | 847 | 13 | 0 | yes | 0 |
| M_EX650 | M_EX650 | yes | 849 | 9 | 0 | yes | 0 |
| M_EX660 | M_EX660 | yes | 1234 | 7 | 1 | yes | 0 |
| M_EX680 | M_EX680 | yes | 1249 | 12 | 1 | yes | 0 |
| M_EX690 | M_EX690 | yes | 454 | 11 | 1 | yes | 1 |
| M_EX700 | M_EX700 | yes | 587 | 10 | 0 | yes | 0 |
| M_EX710 | M_EX710 | yes | 447 | 10 | 5 | yes | 0 |
| M_EX730 | M_EX730 | yes | 487 | 11 | 2 | yes | 0 |
| M_EX740 | M_EX740 | yes | 671 | 9 | 0 | yes | 0 |
| M_EX750 | M_EX750 | yes | 570 | 7 | 0 | yes | 0 |
| M_EX760 | M_EX760 | yes | 1670 | 7 | 0 | no (0 hits) | 0 |
| M_EX770 | M_EX770 | yes | 609 | 6 | 0 | yes | 0 |
| M_EX780 | M_EX780 | yes | 477 | 6 | 0 | yes | 0 |
| M_EX850 | M_EX850 | yes | 0 | 1 | 0 | no (0 hits) | 0 |
| M_EX870 | M_EX870 | yes | 674 | 3 | 16 | yes | 0 |
| M_EX880 | M_EX880 | yes | 612 | 8 | 2 | yes | 0 |
| M_EX890 | M_EX890 | yes | 542 | 9 | 1 | yes | 0 |
| M_EX900 | M_EX900 | yes | 598 | 4 | 1 | no (0 hits) | 0 |
| M_EX910 | M_EX910 | yes | 676 | 13 | 3 | yes | 0 |
| M_EX920 | M_EX920 | yes | 1244 | 8 | 0 | yes | 0 |
| M_EX930 | Dancer | yes | 1391 | 9 | 1 | yes | 0 |
| M_EX940 | M_EX940 | yes | 1033 | 10 | 1 | yes | 0 |
| M_EX950 | M_EX950 | yes | 972 | 9 | 0 | yes | 0 |
| M_EX960 | M_EX960 | yes | 769 | 12 | 0 | yes | 0 |
| M_EX990 | Dusk | yes | 992 | 14 | 0 | yes | 0 |
| N_EX500_BTL | N_EX500 | yes | 867 | 12 | 4 | no (31 hits) | 0 |
| N_EX570_BTL | Seifer | yes | 370 | 11 | 2 | yes | 0 |
| N_EX600_BTL | N_EX600 | yes | 9929 | 13 | 5 | no (10 hits) | 0 |

## Area battle groups (round 6)

`kh2_area_spawn(handle, "<area>", program, with_disabled)` spawns an area's own battle groups (the
spawn records its `btl` program loads, with each record's spawn kind and parameters; kh2-rs
`area_groups`). The disc test `area_battle_groups_spawn_and_fight` lists them for the tt, hb, al, bb,
mu, lk, po, eh, nm, ca, tr, lm, dc, wi and he areas (Twilight Town alone: tt03 to tt41) and runs
tt03's Dusks (all act) and eh06's group with M_EX870: with its spawn record one of the four acts
(the others wait for a signal or a group counter the library does not run; 2 host calls unserved).
M_EX850 is in none of the areas scanned. Enemies hit Donald and Goofy when their attacks reach them
(since round 5); no enemy script picks a member as its target (they aim at the player).

## Areas

`kh2_surfaces_load_map("<area>")` loads an area's own collision (COCT). The disc test
`areas_load_and_sora_walks_on_them` puts Sora on the spawn point of each of these and walks him
forward for 4 s: tt00, tt01, tt07, hb00, hb05, al00, bb00, mu00, lk00, po00, eh00, nm00, ca00, tr00,
lm00, dc00, wi00, es00 all load and he walks on them (hb05 spawns him in the air above a floor). The
areas' scripts (spawn groups, events, doors) are not run: the host spawns the enemies it wants.
