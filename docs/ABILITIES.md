# Abilities on the Final Mix disc

Every ability of the Final Mix item table (171, read by kh2fm-rs `AbilityRules` from the disc; ids and
bits only, no names: the disc's text is not copied here) and what libkh2 does with it. Regenerate with
`cargo test --release -p libkh2 --test disc final_mix_abilities` (writes `local/abilities-fm.tsv`).

- **stat**: the ability block's effect (`Block::apply`: combo lengths, boosts, MP Haste, Defender ...).
- **growth**: a growth ability (High Jump, Quick Run, Aerial Dodge, Glide, and Final Mix's Dodge Roll, ids 564 to 567).
- **attack table**: an action Sora's attack table gates with it (the ability starts its record).
- **bit read**: a bit the controller reads (Scan, Leaf Bracer, Second Chance, Once More, Auto Limit, Auto Change, Auto Healing).
- **not used**: party members' abilities (kind party), the members' and the forms' actions whose tables libkh2 does not
  gate this way, field abilities, and the abilities with no effect found in kh2fm-rs.

Counts: attack table 25, bit read 7, growth 20, not used 92, stat 27.

| id | bit | kind | AP | libkh2 |
|---|---|---|---|---|
| 82 | 1 | action | 2 | attack table |
| 94 | 2 | growth | 2 | growth |
| 95 | 3 | growth | 2 | growth |
| 96 | 4 | growth | 2 | growth |
| 97 | 5 | growth | 2 | growth |
| 98 | 6 | growth | 2 | growth |
| 99 | 7 | growth | 2 | growth |
| 100 | 8 | growth | 2 | growth |
| 101 | 9 | growth | 2 | growth |
| 102 | 10 | growth | 3 | growth |
| 103 | 11 | growth | 3 | growth |
| 104 | 12 | growth | 3 | growth |
| 105 | 13 | growth | 3 | growth |
| 106 | 14 | growth | 3 | growth |
| 107 | 15 | growth | 3 | growth |
| 108 | 16 | growth | 3 | growth |
| 109 | 17 | growth | 3 | growth |
| 137 | 18 | action | 4 | attack table |
| 138 | 19 | support | 2 | bit read |
| 158 | 20 | support | 2 | not used |
| 162 | 21 | support | 1 | stat |
| 163 | 22 | support | 1 | stat |
| 165 | 24 | action | 2 | not used |
| 166 | 25 | action | 2 | not used |
| 167 | 26 | action | 2 | not used |
| 168 | 27 | action | 3 | not used |
| 176 | 35 | action | 3 | not used |
| 177 | 36 | action | 2 | attack table |
| 187 | 46 | action | 3 | not used |
| 190 | 49 | action | 3 | not used |
| 195 | 54 | action | 2 | not used |
| 198 | 57 | party | 5 | not used |
| 199 | 58 | party | 3 | not used |
| 200 | 59 | party | 3 | not used |
| 201 | 60 | party | 3 | not used |
| 202 | 61 | party | 3 | not used |
| 203 | 62 | party | 3 | not used |
| 204 | 63 | party | 3 | not used |
| 205 | 64 | party | 3 | not used |
| 206 | 65 | party | 3 | not used |
| 207 | 66 | party | 3 | not used |
| 208 | 67 | party | 3 | not used |
| 209 | 68 | party | 3 | not used |
| 210 | 69 | party | 3 | not used |
| 211 | 70 | party | 3 | not used |
| 215 | 74 | action | 2 | not used |
| 216 | 75 | action | 0 | not used |
| 217 | 76 | action | 0 | not used |
| 218 | 77 | action | 0 | not used |
| 219 | 78 | action | 0 | not used |
| 220 | 79 | action | 0 | not used |
| 221 | 80 | action | 0 | not used |
| 223 | 82 | support | 0 | not used |
| 224 | 83 | action | 0 | not used |
| 225 | 84 | action | 0 | not used |
| 246 | 171 | action | 0 | not used |
| 247 | 172 | action | 0 | not used |
| 248 | 1 | action | 0 | attack table |
| 249 | 18 | action | 0 | attack table |
| 257 | 85 | action | 0 | not used |
| 258 | 86 | action | 0 | not used |
| 259 | 21 | support | 0 | not used |
| 261 | 89 | action | 0 | not used |
| 262 | 90 | action | 2 | attack table |
| 263 | 91 | action | 2 | attack table |
| 264 | 92 | action | 2 | attack table |
| 265 | 93 | action | 3 | attack table |
| 266 | 94 | action | 3 | attack table |
| 267 | 95 | action | 5 | attack table |
| 268 | 96 | action | 4 | attack table |
| 269 | 97 | action | 2 | attack table |
| 270 | 98 | action | 2 | attack table |
| 271 | 99 | action | 2 | attack table |
| 272 | 100 | action | 3 | attack table |
| 273 | 101 | action | 3 | attack table |
| 385 | 23 | action | 1 | not used |
| 386 | 31 | action | 1 | not used |
| 387 | 32 | action | 1 | not used |
| 388 | 33 | action | 1 | not used |
| 389 | 56 | action | 2 | not used |
| 390 | 102 | support | 5 | stat |
| 391 | 103 | support | 4 | stat |
| 392 | 104 | support | 2 | stat |
| 393 | 105 | support | 5 | stat |
| 394 | 106 | support | 2 | stat |
| 395 | 107 | support | 5 | stat |
| 396 | 108 | support | 3 | stat |
| 397 | 109 | support | 3 | stat |
| 398 | 110 | support | 5 | stat |
| 399 | 111 | support | 5 | stat |
| 400 | 112 | support | 4 | stat |
| 401 | 113 | support | 4 | stat |
| 402 | 114 | support | 1 | bit read |
| 403 | 115 | support | 2 | not used |
| 404 | 116 | support | 1 | not used |
| 405 | 117 | support | 3 | stat |
| 406 | 118 | support | 4 | stat |
| 407 | 119 | support | 5 | stat |
| 408 | 120 | support | 3 | stat |
| 409 | 121 | support | 4 | stat |
| 410 | 122 | support | 5 | stat |
| 411 | 123 | support | 2 | stat |
| 412 | 124 | support | 3 | stat |
| 413 | 125 | support | 3 | stat |
| 414 | 126 | support | 3 | stat |
| 415 | 127 | support | 4 | bit read |
| 416 | 128 | support | 4 | bit read |
| 417 | 129 | support | 1 | bit read |
| 418 | 130 | support | 5 | bit read |
| 419 | 131 | support | 3 | stat |
| 420 | 132 | support | 3 | bit read |
| 421 | 135 | support | 4 | stat |
| 422 | 136 | support | 5 | stat |
| 423 | 137 | action | 2 | not used |
| 425 | 139 | action | 2 | not used |
| 426 | 140 | action | 2 | not used |
| 427 | 141 | action | 2 | not used |
| 428 | 142 | action | 2 | not used |
| 429 | 143 | action | 2 | not used |
| 430 | 144 | action | 2 | not used |
| 431 | 145 | action | 2 | not used |
| 432 | 146 | action | 2 | not used |
| 433 | 147 | action | 2 | not used |
| 434 | 148 | action | 2 | not used |
| 435 | 149 | action | 2 | not used |
| 436 | 150 | party | 2 | not used |
| 438 | 152 | party | 2 | not used |
| 439 | 153 | action | 2 | not used |
| 440 | 154 | action | 2 | not used |
| 444 | 158 | action | 2 | not used |
| 445 | 159 | action | 2 | not used |
| 446 | 160 | action | 2 | not used |
| 447 | 161 | action | 2 | not used |
| 448 | 162 | action | 2 | not used |
| 519 | 163 | action | 0 | not used |
| 520 | 95 | action | 0 | attack table |
| 521 | 18 | action | 0 | attack table |
| 522 | 99 | action | 0 | attack table |
| 523 | 101 | action | 0 | attack table |
| 525 | 165 | action | 0 | not used |
| 526 | 166 | action | 0 | not used |
| 527 | 167 | action | 0 | not used |
| 528 | 168 | action | 0 | not used |
| 529 | 169 | action | 0 | not used |
| 530 | 170 | action | 0 | not used |
| 539 | 173 | support | 4 | not used |
| 540 | 174 | support | 5 | not used |
| 541 | 175 | support | 1 | not used |
| 542 | 176 | support | 5 | not used |
| 559 | 177 | action | 2 | attack table |
| 560 | 178 | action | 2 | attack table |
| 561 | 179 | action | 3 | attack table |
| 562 | 180 | action | 2 | attack table |
| 564 | 181 | growth | 3 | growth |
| 565 | 182 | growth | 3 | growth |
| 566 | 183 | growth | 3 | growth |
| 567 | 184 | growth | 3 | growth |
| 568 | 185 | action | 1 | not used |
| 569 | 186 | action | 0 | not used |
| 570 | 187 | action | 0 | not used |
| 571 | 188 | action | 0 | not used |
| 572 | 189 | action | 0 | not used |
| 573 | 190 | action | 0 | not used |
| 574 | 191 | action | 0 | not used |
| 575 | 192 | action | 0 | not used |
| 587 | 193 | action | 0 | not used |
| 588 | 194 | action | 0 | not used |
| 589 | 195 | support | 0 | not used |
| 596 | 196 | support | 0 | not used |
| 597 | 197 | support | 0 | not used |
| 598 | 198 | support | 0 | not used |
