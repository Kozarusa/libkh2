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

## Round 15: every `changed` function compared instruction by instruction

USA body against the Final Mix body (capstone disassembly, branch and call targets ignored; a trailing
insertion past the end of the USA body is the next functions of the FM span and is dropped). "same code"
means only offsets and constants differ; the operand changes list the first ones. One line of meaning
where it was read.

| USA | FM | system | bytes | instructions | operands (USA -> FM) | meaning |
|---|---|---|---|---|---|---|
| ﻿0011b1b4 | 0011b3fc | limit | 8->8 | same code, other offsets / constants | 0x4a60->0x78f0 | not identified |
| 0013e1f0 | 0013e2d4 | magic | 156->220 | +1 / -0 instructions | 0x1a4->0x1b4;0x1b0->0x1c0 | not identified |
| 0013e4c0 | 0013e5a8 | combat.ability | 100->204 | same code, other offsets / constants | 0x1a4->0x1b4;0x1b0->0x1c0 | not identified |
| 0013e5a0 | 0013e688 | magic | 380->520 | +6 / -6 instructions | 0x194->0x1a4;0x1a4->0x1b4;0x1bc->0x1cc;0x1c8->0x1d8;0x1e60->0x1f80;0x1e84->0x1fa4;0x344->0 | not identified |
| 001502d0 | 0015065c | player.sora | 20->24 | +1 / -0 instructions | 0x4b8->0x4d0;0x644->0x668 | not identified |
| 00151260 | 001516b0 | limit | 728->776 | +21 / -11 instructions | -0x6d4c->-0x4dc4;->;0x180->0x18c;0x184->0x190;0x18c->0x198;0x33c->0x348;0x354->0x360;0x38- | not identified |
| 00151758 | 00151bd8 | limit | 20->20 | same code, other offsets / constants | 0x4b8->0x4d0;0xae8->0xb30 | not identified |
| 00167310 | 001678e8 | player.sora | 428->488 | +20 / -7 instructions | ->;0x48->0x48;0x50->0x54;0x52->0x56;1->1 | not identified |
| 00176b00 | 00177ab4 | magic | 232->784 | +1 / -0 instructions | 0x2a->0x2e | availability check: the command flag offset 0x2a -> 0x2e and the may-cancel test of stats +0x14 bit 0x2000 (kh2fm-rs final-mix.md) |
| 00176e00 | 00177dc8 | player.sora | 388->824 | same code, other offsets / constants | -0x3658->-0x16a8;-0x3668->-0x16b8;-0x7bd0->-0xad0;0x1c9->0x1c6;0x240->0x250;0x24c->0x25c;0 | the attack start (FM 0x177dc8): status fields 0x10 later (+0x240 -> +0x250, the object +0x24c -> +0x25c) and the form count 6 -> 7: Final Mix layout and numbering only |
| 00177d58 | 00178d28 | player.sora | 328->340 | +17 / -14 instructions | -0xf88->0x1068;->;0x154->0x14;0x36->0x35;0xb40->0xb40;0xc->0xb40;0xc->0xc;8->8 | not identified |
| 00177ea0 | 00178e80 | player.sora | 260->360 | +25 / -0 instructions |  | Square handler: ported (Dodge Roll / Quick Run) |
| 00178108 | 00179148 | player.sora | 392->448 | +44 / -31 instructions | ->;0x14->0x14;0x18->0x18;0x1c->0x1c;0x28->0x28;0xc->0xc;1,0x1781e0->1,0x17924c;1->1 | Quick Run: ported (start along a direction) |
| 001783d8 | 0017944c | player.sora | 220->716 | +1 / -0 instructions | 6->7 | mover 0x16 mode: one compare 6 -> 7 (the form numbering); the rest of the FM span is the next functions |
| 001792f0 | 0017a47c | player.sora | 184->272 | +1 / -0 instructions | 0x50->0x54 | stats +0x50 -> +0x54 (growth bytes 4 -> 5) plus one instruction |
| 0017d030 | 0017e280 | limit | 116->144 | +11 / -6 instructions | 0x570->0x570 | not identified |
| 0017e3b8 | 0017f620 | player.sora | 776->1408 | +4 / -0 instructions | -0x5a6c->-0x3a54;0x38->0x37;1,0x17e3f8->1,0x17f660;1,0x17e59c->1,0x17f814;1,0x17e5ac->1,0x | motion request: 4 new instructions and new branch targets; behaviour not identified |
| 00183eb0 | 00185138 | limit | 304->320 | +41 / -37 instructions | ->;0,0x183f10->0,0x185198;0x30->0x30;0xc0->0xd0 | not identified |
| 00187f88 | 00189228 | drive | 40->44 | +2 / -1 instructions | 0x11f0->8 | drive: a table offset; not identified |
| 0018cf50 | 0018e294 | combat.attack | 316->396 | +35 / -21 instructions | ->;0x18->0x20;0x20->0x18;0xc-> | not identified |
| 00190868 | 00191c4c | combat.ability | 68->56 | +9 / -12 instructions |  | ability helper: a shorter body (+9 / -12); not identified |
| 00196ad0 | 00198040 | drive | 136->144 | +1 / -3 instructions | -0x4fa4->-0x2f54;0x26->0x28;0x34->0x33;0x38->0x37;0xc8c->-0xfac;6->7 | not identified |
| 0019c670 | 0019dbc0 | combat.ability | 500->836 | +84 / -0 instructions | -0x2d98->-0xdc0;0x36->0x35;0x96c->0x9e0;0x974->0x9e8 | not identified |
| 0019cd60 | 0019e400 | combat.ability | 496->156 | +0 / -85 instructions | 0x96c->0x9e0 | not identified |
| 0019cf58 | 0019e4a0 | limit | 160->584 | +86 / -0 instructions | 0x96c->0x9e0 | not identified |
| 0019d0b8 | 0019e6e8 | limit | 236->352 | +28 / -0 instructions | -0x4c58->-0x2c00;0x38->0x37;0x974->0x9e8 | not identified |
| 0019f4a8 | 001a0c18 | drive | 300->708 | +5 / -4 instructions | 0x238->0x244;0x23c->0x248;0x244->0x254 | form timer object: status +0x238.. -> +0x244..; layout |
| 001a0610 | 001a1e1c | combat.ability | 648->1172 | +25 / -0 instructions | 0x130->0x2130;0x1f8->0x1fc;0x24c->0x25c;0x36->0x35;0x4e->0x52;5->6 | damage: ported and oracle-tested (2.1.0) |
| 001a1d50 | 001a34dc | combat.ability | 232->516 | +71 / -0 instructions | -0x140->-0x1fe0;0x34->0x33;0x40->0x50 | not identified |
| 001a3700 | 001a4f5c | combat.ability | 324->480 | +39 / -0 instructions | -0x4b84->-0x2b4c;-0x4b88->-0x2b50;0x30->0x34;0x38->0x37;1,0x1a37c0->1,0x1a50b8;1,0x1a37cc- | not identified |
| 001a3cb0 | 001a55a8 | combat.ability | 576->728 | +89 / -51 instructions | -0x1860->0x790;-0x4b64->-0x2b2c;-0x4b68->-0x2b30;->;0x16a0->0x38d0;0x18,0x68,0xf4,0x01->0x | not identified |
| 001a6f70 | 001a8930 | drive | 124->188 | +20 / -4 instructions | -0x20->-0x30;->;0x10->0x10;0x20->0x30 | not identified |
| 001a6ff0 | 001a89f0 | drive | 296->312 | +7 / -3 instructions | -0x17d0->-0x4520;-0x40->-0x50;->;0x10->0x12;0x1c9->0x1c7;0x30->0x34;0x30->0x38;0x34->0x33; | not identified |
| 001a9dd8 | 001ab81c | player.sora | 104->116 | +7 / -4 instructions | -> | not identified |
| 001b35e8 | 001b50dc | combat.ability | 420->896 | +93 / -0 instructions | -0x2d98->-0xdc0;0x36->0x35;0x5c->0x60;0x6f40->-0x61f0 | not identified |
| 001b3a08 | 001b5680 | limit | 100->136 | +18 / -9 instructions | 0x10->0x10 | not identified |
| 001b3e38 | 001b5b20 | limit | 196->320 | +49 / -18 instructions | -> | not identified |
| 001b4c28 | 001b6b0c | drive | 200->276 | +7 / -6 instructions | 0x242c->0x3524;0x34->0x33;0xc8c->-0xfac;1,0x1b4cac->1,0x1b6b94 | not identified |
| 001b53d0 | 001b72c0 | combat.ability | 280->296 | +29 / -25 instructions | -0xb->-0xb;->;5->5 | not identified |
| 001b7730 | 001b966c | combat.attack | 528->1212 | +1 / -0 instructions | -0x2860->-0x7260;-0xde0->0x1210;-0xe20->0x11d0;0x1d6->0x1d5;0x36->0x35 | not identified |
| 001ba9b0 | 001bc9e4 | magic | 572->1020 | +1 / -0 instructions | 0x1ba9f8->0x1bca30;0x84c->0x8c0;0x970->0x9e4 | not identified |
| 001bb558 | 001bd5b0 | limit | 100->248 | +38 / -1 instructions | 0x84c->0x8c0;0x974->0x9e8 | not identified |
| 001bbee8 | 001bdf38 | magic | 120->168 | +12 / -0 instructions | 0x9d4->0xa44 | not identified |
| 001bbf60 | 001bdfe0 | drive | 12->12 | same code, other offsets / constants | 0x9d4->0xa44 | not identified |
| 001c0d88 | 001c2f90 | combat.ability | 200->208 | +16 / -14 instructions | -0x4268->-0x2220;0x24->0x28;0x28->0x2c;0x30->0x34;0x38->0x37;0x42->0x44;0x4c->0x52;4->5 | not identified |
| 001c77f0 | 001c9958 | drive | 252->340 | +26 / -4 instructions | -0x3fac->-0x1f54;->;0x38->0x37;1->1;2->2 | not identified |
| 001cda78 | 001cfbd8 | drive | 288->576 | same code, other offsets / constants | -0x140->0x1ec0;-0x17a0->-0x44d0;-0x3c84->-0x1c44;0x2dbc->0x3db0;0x2dbe->0x3db2;0x34->0x33; | drive: same code, Final Mix addresses and form count 10 -> 11 |
| 001ce928 | 001d0878 | player.sora | 556->1132 | +132 / -0 instructions | -0x148->0x1eb8;-0x14a8->0xcb8;-0x3cb4->-0x1c74;-0x3cb8->-0x1c78;0x36->0x35;0x38->0x37;1,0x | player method (FM 0x1d0878): 132 new instructions with the form count 6 -> 7; not identified |
| 001cf030 | 001d118c | player.sora | 328->348 | +23 / -26 instructions | 0x48->0x49 | command handler: Execute 707 (Limit Form) sent to the limit routine; ported (2.1.0) |
| 001cf370 | 001d14e0 | drive | 92->192 | same code, other offsets / constants | -0x3c98->-0x1c58;0x36->0x35;0x38->0x37;0x4b8->0x24b8;7->8 | drive: same code, Final Mix addresses and the count 7 -> 8 |
| 001d21e0 | 001d417c | limit | 316->336 | +6 / -1 instructions | -0x56b8->-0x45d8;->;0x1d7->0x1d6;0x38->0x40;0x40->0x38;1->1 | not identified |
| 001d4180 | 001d6148 | combat.attack | 244->604 | +31 / -2 instructions | -0x140->-0x1fe0;0x130->0x2130;0x34->0x33;0x36->0x35;4->5;5->6 | form choice: ported in kh2fm-rs drive rules (Final 5, Anti 6, `anti_certain`) |
| 001d5c70 | 001d7ccc | player.sora | 136->156 | +22 / -17 instructions |  | not identified |
| 001d5cf8 | 001d7d68 | player.sora | 136->152 | +21 / -17 instructions |  | not identified |
| 001d7cb0 | 001da554 | limit | 2368->2468 | +9 / -6 instructions | -0x391c->-0x1890;-0x394c->-0x18c0;-0x3960->-0x18d4;-0x3990->-0x1904;-0x39c0->-0x1934;-0x39 | not identified |
| 00223698 | 00226ea0 | limit | 36->48 | +4 / -2 instructions |  | not identified |

56 functions: 7 differ only in offsets and constants (the Final Mix status / block layout,
the seven form slots, moved globals); the rest change code. The Sora-play ones that change behaviour are ported
(damage, Square / roll / Quick Run, the Limit Form command); the others are listed "not identified".
