# Performance and memory

Measured with `cargo run --release -p libkh2 --example bench -- <disc image>` on the development
PC (a desktop x86-64, release build, one thread). One tick is everything a battle tick does:
Sora's controller and posed skeleton, Donald and Goofy (follow, battle choice, posed bodies and
weapons, their hits and the enemies' hits on them), every enemy's BDX script, movement, posed
skeleton and the hit tests both ways. The enemies are a mix of Shadows, Soldiers and Dusks around
Sora (made unkillable so the count holds); Sora attacks every 6 ticks.

| enemies | ticks/s | ms/tick | x real time (60 Hz) | heap during the run (MiB) |
| --- | --- | --- | --- | --- |
| 0 | 20017 | 0.050 | 334 | 122.8 |
| 1 | 16864 | 0.059 | 281 | 122.9 |
| 5 | 9964 | 0.100 | 166 | 123.3 |
| 10 | 7051 | 0.142 | 118 | 123.6 |
| 20 | 4510 | 0.222 | 75 | 124.4 |
| 50 | 2168 | 0.461 | 36 | 126.8 |

Loading (`Disc::open`, Sora, every enemy kind and the party) takes about 0.2 s. Heap, from a
counting allocator in the bench: Sora's data 8.2 MiB; all 54 enemy kinds (models, motion sets,
scripts, paired motions, reaction parts) and the party 114 MiB more; 122 MiB in all. A spawned enemy
costs about 0.08 MiB (its runtime, posed skeleton and object).

Notes:
- The cost per enemy is about 8 microseconds a tick, dominated by posing the skeleton; a host that
  draws only Sora can skip enemy poses only for enemies whose hit shapes it does not need.
- The enemy data is loaded for every kind at the first `kh2_enemy_spawn` (and by
  `Disc::enemies`); loading kinds on demand would cut the 114 MiB to what a battle uses (a possible
  change, not done: the load is one place in `enemy.rs`).
- The C API adds a mutex lock per call and no allocation in the tick path beyond the library's own.
