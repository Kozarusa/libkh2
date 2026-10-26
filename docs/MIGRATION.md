# Migrating a host from libkh2 1.x to 2.x

libkh2 2.x is the Final Mix library. 1.x (branch `usa`, last release 1.2.0, tag `usa-final`) read the
USA disc. This page lists what a host written against 1.x must change, release by release.

## 2.0.0: Final Mix

| What | 1.x | 2.x |
|---|---|---|
| Disc | `Kingdom Hearts II` (USA, `SLUS_210.05`) | `Kingdom Hearts II Final Mix` (Japan, `SLPM_666.75`); any other disc makes `kh2_init` return -1 ("this build reads only Final Mix") |
| Form ids | 1 Valor, 2 Wisdom, 3 Master, 4 Final, 5 Anti | 1 Valor, 2 Wisdom, **3 Limit**, 4 Master, 5 Final, 6 Anti |
| `kh2_drive_setup` | `levels` 6 bytes, owned bits 1..4 | `levels` **7** bytes (index = form id), owned bits 1..5 (bit 6, Anti, is never owned) |
| `kh2_drive_enter(form)`, form events | USA ids | Final Mix ids (table above): Master is now 4, Final 5 |
| Save records (`kh2_state_save` / `_load`) | format 3 | format 4 (seven form slots); a format 3 record is refused (-1) |
| Growth | four kinds | `kh2_set_dodge_roll` adds the fifth (Dodge Roll); `kh2_set_growth` is unchanged |
| Text | English | the disc's Japanese text; the HUD draws it with the disc's font |
| Tests / tools | `KH2_ISO` | `KH2_ISO_FM` or `KH2_ISO` naming the Final Mix image |

The numbers behind Sora (jump height, speeds, `fmab` rows, `atkp` damage rows) are read from the disc,
so a host that showed them gets the Final Mix values without a change.

## 2.1.0

Added, nothing removed: `kh2_hud_area_card`, `kh2_audio_set_area`. `kh2_hud_tick` now also fills the
party panels from the followers; the HUD's texture list can grow after `kh2_hud_load` (the members'
faces): look up new texture ids with `kh2_hud_texture_info` when the draw list names one you have not
uploaded. The audio master level is the game's full level (louder than 1.x by about 4 dB).

## 2.2.0

Added, nothing removed: `kh2_set_difficulty`, `kh2_hud_info`, enemy event `KH2_EN_CLASH` (8: an attack
window met Reflect's guard). Damage now passes the difficulty step (Standard by default: no change)
and the Final Mix low-HP step in every hit path.

## 2.3.0

Added, nothing removed: `kh2_hud_message`, `kh2_hud_message_confirm`; Reflect clashes only with
enemy attacks the clash table pairs with its guard; the optional `KH2_NAMES` name list. New dust and
footstep cues (kind 8, dust 8 to 11) appear in `kh2_poll_cue`.

## 2.4.0

Nothing added to or removed from `kh2.h`. Behaviour: in Critical (`kh2_set_difficulty(3)`) Sora's AP
from levels is x 1.5 and bonus HP / MP increases are halved, as the Final Mix executable does;
Reflect clashes only inside the attack rows' cones; enemy scripts get an answer for host 1.222.

## Version policy

`KH2_VERSION` / `kh2_version()` is (major << 16) | (minor << 8) | patch:

- **major**: the disc or the C ABI changes incompatibly (a struct layout, a removed function, a
  changed meaning of an argument, a save format a host may have stored). Hosts must be rebuilt and
  read this page.
- **minor**: functions, constants and event kinds are added; existing calls keep their meaning. A
  host built against x.y runs on x.z for z >= y. A new event kind can appear in a poll loop: ignore
  kinds you do not know.
- **patch**: fixes only.

Deprecated entries stay for the whole major version and are marked `deprecated` in `kh2.h` and here.
There are none in 2.x.
