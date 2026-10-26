# Final Mix functions that differ (Sora's play)

The Sora-play systems of kh2fm-rs's function map (`local/fm-diff/functions.tsv`: USA address, size, system,
ledger status, class, FM address): every function classed `changed` (aligned but different) or `absent` (no FM
function aligned; often moved or merged, e.g. USA 0x176be8 is at FM 0x177ba0) in player.sora, drive, magic,
combat.ability, combat.attack and limit. "not examined" means nobody has read the Final Mix body yet.
106 functions; 23 have a note.

| USA | size | system | class | FM | what changed |
|---|---|---|---|---|---|
| 0011b1b4 | 8 | limit | changed | 0011b3fc | not examined |
| 0013d520 | 132 | limit | absent |  | not examined |
| 0013d5a8 | 160 | limit | absent |  | not examined |
| 0013d648 | 144 | limit | absent |  | not examined |
| 0013e1f0 | 156 | magic | changed | 0013e2d4 | not examined |
| 0013e290 | 56 | magic | absent |  | not examined |
| 0013e4c0 | 100 | combat.ability | changed | 0013e5a8 | not examined |
| 0013e528 | 100 | magic | absent |  | not examined |
| 0013e5a0 | 380 | magic | changed | 0013e688 | not examined |
| 001502d0 | 20 | player.sora | changed | 0015065c | not examined |
| 00151260 | 728 | limit | changed | 001516b0 | not examined |
| 00151758 | 20 | limit | changed | 00151bd8 | not examined |
| 00161ed0 | 340 | player.sora | absent |  | not examined |
| 00167310 | 428 | player.sora | changed | 001678e8 | not examined |
| 0016f3b8 | 428 | drive | absent |  | not examined |
| 0016fb20 | 504 | drive | absent |  | not examined |
| 00176b00 | 232 | magic | changed | 00177ab4 | availability check (FM 0x177ab4, 232 -> 784 bytes): the attack-command "may cancel" bit +0x14 0x2000; writer not identified |
| 00176be8 | 84 | combat.ability | absent |  | combo length for Combo Boost (FM 0x177ba0): layout only (block +0x2c); oracle-tested through the bonuses |
| 00176c40 | 84 | combat.ability | absent |  | air combo length for Air Combo Boost (FM 0x177bf8): layout only (block +0x2d) |
| 00176c98 | 224 | combat.ability | absent |  | not examined |
| 00176d78 | 136 | player.sora | absent |  | damage gate (FM 0x177d30): an extra test of (+0x48c)+8 before the HP-share test; not ported |
| 00176e00 | 388 | player.sora | changed | 00177dc8 | not examined |
| 00176f88 | 388 | player.sora | absent |  | not examined |
| 00177110 | 20 | player.sora | absent |  | FM 0x1780d8: calls FM 0x1758b0 then FM 0x17a718 instead of a tail call; not examined |
| 001775a0 | 72 | player.sora | absent |  | not examined |
| 00177d58 | 328 | player.sora | changed | 00178d28 | attack handler (FM 0x178d28): attack start before the special motion reset; extra callback; purpose not identified (kh2fm-rs final-mix.md) |
| 00177ea0 | 260 | player.sora | changed | 00178e80 | Square handler (FM 0x178eb0): **ported** (`square_ground`: guard / Dodge Roll / Quick Run) |
| 00178108 | 392 | player.sora | changed | 00179148 | Quick Run (FM 0x179148 -> 0x1791e8): **ported** (start along a direction for trigger 34) |
| 001783d8 | 220 | player.sora | changed | 0017944c | mover 0x16 mode (FM 0x179450): not examined |
| 00178558 | 48 | limit | absent |  | not examined |
| 001792f0 | 184 | player.sora | changed | 0017a47c | player method (FM 0x17a480): stats +0x54 instead of +0x50 (growth bytes grew by one) |
| 001793a8 | 84 | player.sora | absent |  | not examined |
| 0017d030 | 116 | limit | changed | 0017e280 | not examined |
| 0017e3b8 | 776 | player.sora | changed | 0017f620 | motion request (FM 0x17f620, 776 -> 1408 bytes): not examined |
| 0017e6c0 | 612 | player.sora | absent |  | not examined |
| 00183eb0 | 304 | limit | changed | 00185138 | not examined |
| 00187f88 | 40 | drive | changed | 00189228 | not examined |
| 0018cf50 | 316 | combat.attack | changed | 0018e294 | not examined |
| 00190868 | 68 | combat.ability | changed | 00191c4c | not examined |
| 00196170 | 232 | combat.attack | absent |  | not examined |
| 00196348 | 152 | combat.attack | absent |  | not examined |
| 00196ad0 | 136 | drive | changed | 00198040 | not examined |
| 0019c520 | 332 | limit | absent |  | not examined |
| 0019c670 | 500 | combat.ability | changed | 0019dbc0 | not examined |
| 0019cad0 | 156 | limit | absent |  | not examined |
| 0019cd60 | 496 | combat.ability | changed | 0019e400 | not examined |
| 0019cf58 | 160 | limit | changed | 0019e4a0 | not examined |
| 0019d048 | 108 | limit | absent |  | not examined |
| 0019d0b8 | 236 | limit | changed | 0019e6e8 | not examined |
| 0019f4a8 | 300 | drive | changed | 001a0c18 | drive gauge / timer (FM 0x1a0c18, 300 -> 708 bytes): form 6 for Anti; kh2fm-rs drive rules (timer level) |
| 0019f5d8 | 300 | drive | absent |  | not examined |
| 001a0610 | 648 | combat.ability | changed | 001a1e1c | damage: **ported** (2.1.0): Final Mix status layout, Anti = form 6, new last step `low_hp_scale`; oracle 3000/3000 |
| 001a0898 | 208 | drive | absent |  | damage helper (difficulty / party bonus, FM 0x1a2108): adds the Critical case (200 % taken, 125 % dealt); difficulty not modelled |
| 001a0968 | 176 | drive | absent |  | damage helper (FM 0x1a2200): layout only (object +0x25c) |
| 001a18d8 | 320 | combat.ability | absent |  | not examined |
| 001a1d50 | 232 | combat.ability | changed | 001a34dc | not examined |
| 001a2640 | 120 | combat.attack | absent |  | not examined |
| 001a3700 | 324 | combat.ability | changed | 001a4f5c | not examined |
| 001a3cb0 | 576 | combat.ability | changed | 001a55a8 | not examined |
| 001a6f70 | 124 | drive | changed | 001a8930 | not examined |
| 001a6ff0 | 296 | drive | changed | 001a89f0 | not examined |
| 001a9dd8 | 104 | player.sora | changed | 001ab81c | not examined |
| 001b2c10 | 504 | limit | absent |  | not examined |
| 001b35e8 | 420 | combat.ability | changed | 001b50dc | not examined |
| 001b3a08 | 100 | limit | changed | 001b5680 | not examined |
| 001b3bc0 | 12 | limit | absent |  | not examined |
| 001b3e38 | 196 | limit | changed | 001b5b20 | limit availability (FM 0x1b5b20): kh2fm-rs limit rules |
| 001b4b68 | 104 | drive | absent |  | not examined |
| 001b4c28 | 200 | drive | changed | 001b6b0c | not examined |
| 001b52f8 | 212 | combat.ability | absent |  | not examined |
| 001b53d0 | 280 | combat.ability | changed | 001b72c0 | not examined |
| 001b7730 | 528 | combat.attack | changed | 001b966c | not examined |
| 001ba568 | 232 | drive | absent |  | Drive / Summon refusal (FM 0x1bc568): the numbering after the forms moves up by one; kh2fm-rs drive rules (`Numbering::after`) |
| 001ba9b0 | 572 | magic | changed | 001bc9e4 | magic availability (FM 0x1bc9e4, 572 -> 1020 bytes): not examined |
| 001babf0 | 220 | magic | absent |  | not examined |
| 001bb558 | 100 | limit | changed | 001bd5b0 | not examined |
| 001bb840 | 48 | magic | absent |  | not examined |
| 001bbee8 | 120 | magic | changed | 001bdf38 | not examined |
| 001bbf60 | 12 | drive | changed | 001bdfe0 | not examined |
| 001bc4c8 | 172 | drive | absent |  | not examined |
| 001bc578 | 356 | drive | absent |  | not examined |
| 001bca70 | 292 | drive | absent |  | not examined |
| 001c0d88 | 200 | combat.ability | changed | 001c2f90 | not examined |
| 001c0e50 | 240 | combat.ability | absent |  | not examined |
| 001c0f40 | 1024 | combat.ability | absent |  | not examined |
| 001c1340 | 116 | combat.ability | absent |  | not examined |
| 001c46a8 | 516 | combat.ability | absent |  | not examined |
| 001c77a8 | 68 | drive | absent |  | companions absorbed (FM 0x1c9958 family): kh2fm-rs drive rules |
| 001c77f0 | 252 | drive | changed | 001c9958 | companions present (FM 0x1c9958): Limit Form absorbs none; kh2fm-rs drive rules |
| 001cd538 | 228 | drive | absent |  | not examined |
| 001cd748 | 36 | drive | absent |  | not examined |
| 001cda78 | 288 | drive | changed | 001cfbd8 | not examined |
| 001cdb98 | 288 | drive | absent |  | not examined |
| 001ce718 | 260 | player.sora | absent |  | not examined |
| 001ce820 | 260 | player.sora | absent |  | not examined |
| 001ce928 | 556 | player.sora | changed | 001d0878 | not examined |
| 001cf030 | 328 | player.sora | changed | 001d118c | command handler (FM 0x1d118c): Execute 707 (Limit Form limits) to the limit routine; **ported** (KH1 limits, 2.1.0) |
| 001cf370 | 92 | drive | changed | 001d14e0 | not examined |
| 001cf3d0 | 92 | drive | absent |  | not examined |
| 001d21e0 | 316 | limit | changed | 001d417c | not examined |
| 001d4180 | 244 | combat.attack | changed | 001d6148 | form choice (FM 0x1d6148): Final 5 / Anti 6, ability bit 175 makes Anti certain; **ported** in kh2fm-rs drive rules (RollCtx `anti_certain`, libkh2 passes false) |
| 001d4278 | 204 | drive | absent |  | not examined |
| 001d5c70 | 136 | player.sora | changed | 001d7ccc | not examined |
| 001d5cf8 | 136 | player.sora | changed | 001d7d68 | not examined |
| 001d7cb0 | 2368 | limit | changed | 001da554 | limit task (FM 0x1da554): the KH1 branch; **ported** (`Limits::kh1_tick`) |
| 00223698 | 36 | limit | changed | 00226ea0 | not examined |
