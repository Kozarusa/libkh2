# Performance and memory

Measured with `cargo run --release -p libkh2 --example bench -- <disc image>` on the development
PC (a desktop x86-64, release build, one thread). One tick is everything a battle tick does:
Sora's controller and posed skeleton, Donald and Goofy (follow, battle choice, their own scripts,
posed bodies and weapons, their hits and the enemies' hits on them), every enemy's BDX script,
movement, posed skeleton and the hit tests both ways. The enemies are a mix of Shadows, Soldiers and
Dusks around Sora (made unkillable so the count holds); Sora attacks every 6 ticks.

| enemies | ticks/s | ms/tick | x real time (60 Hz) | heap during the run (MiB) |
| --- | --- | --- | --- | --- |
| 0 | 19891 | 0.050 | 332 | 18.1 |
| 1 | 15901 | 0.063 | 265 | 19.6 |
| 5 | 9685 | 0.103 | 161 | 24.6 |
| 10 | 6612 | 0.151 | 110 | 25.0 |
| 20 | 4252 | 0.235 | 71 | 25.8 |
| 50 | 2100 | 0.476 | 35 | 28.1 |

load 0.03 s; heap: Sora 8.2 MiB, the enemy tables (kinds load on demand) and the party 9.4 MiB more, 17.6 MiB in all
after the runs: 3 kinds were loaded, 3 freed; heap 17.7 MiB

Memory (round 6): the enemy kinds load on demand (`EnemyData::kind`, the first spawn of a kind
reads its model, motion set, script, reaction parts and paired motions through a second reader of
the disc) and are freed when no enemy uses them (`kh2_enemies_prune` drops the enemies that are
gone and frees their kinds; `kh2_enemies_clear` removes all). Before round 6 every kind was loaded
at once: 122 MiB; now the library holds about 18 MiB with Sora and the party and about 2 MiB more
per enemy kind in use. The member scripts add about 4 % per tick against round 5.

Notes:
- The cost per enemy is about 9 microseconds a tick, dominated by posing the skeleton and the
  script.
- The C API adds a mutex lock per call and no allocation in the tick path beyond the library's own.