# Sounds and effects for hosts

libkh2 plays no sound and draws no effect. It tells the host when the original would, through
**cues** (`kh2_poll_cue`, `kh2_cue`, the `KH2_CUE_*` kinds) and a few older event queues. This page
is the table of every source and how a host turns the numbers into files.

## Where the cues come from

| cue | source in the original (kh2-rs) | params | owner |
|---|---|---|---|
| `KH2_CUE_SOUND` 1 | motion frame trigger 8 (`audio::motion_sounds` -> `motion_sound_request`) | bank, index | Sora, members, enemies, summon |
| `KH2_CUE_VOICE` 2 | frame triggers 13 / 14 (`motion_sounds`: Sora's voice id, an enemy's line + priority) | voice or line, priority | same |
| `KH2_CUE_EFFECT` 3 | frame trigger 1, "effect caster" (`fx::motion_effects`) | element id of the owner's effect file | same |
| `KH2_CUE_FOOTSTEP` 4 | frame trigger 2 (`audio` footsteps, `kh2_audio::footstep::footstep_number`) | footstep kind | same |
| `KH2_CUE_DUST` 5 | frame trigger 19 (`fx::FootDust`) | dust kind | same |
| `KH2_CUE_SCRIPT_EFFECT` 6 | an enemy script's effect start (`KH2_EN_EFFECT`), a limit script's 2.59 / 2.50 (model "limit") | element id, (limit: participant word) | enemy, limit |
| `KH2_CUE_SOUND_NUMBER` 7 | the mission's start / end / failure sounds (header +0x10..0x18) and the orbs' bounce / pickup (`FUN_002c3318`) | number low, high 16 bits | Sora (placed at the orb) |

Every cue carries the owner (0 Sora, 1 member slot, 2 enemy id, 3 summon), the owner's model, its
position (cm) and the motion frame. Cues are collected after `kh2_tick` (Sora), `kh2_party_tick`
(members, summon, limit) and `kh2_enemies_tick` (enemies, mission); at most 1024 wait.

Older queues that carry the same moments (kept for compatibility): `KH2_EN_EFFECT`
(`kh2_poll_enemy_event`), the limit events (`kh2_poll_limit_event`: motion and effect requests), the
member events, `kh2_mission_take_sounds`, `KH2_EV_CAST_STARTED` (its `voice`).

Not cued (the original plays them from code libkh2 does not run): hit sparks and hit sounds of a
landed attack (kh2-rs `PlaySe::hit` by the victim's reaction class: derive from `KH2_EN_HIT` /
`KH2_EN_HIT_SORA`), menu and HUD sounds, music, area ambience, cutscene audio, the limits' and the
spells' own effect objects beyond their start.

## The audio engine

`kh2_audio_init()` then `kh2_audio_tick(h, out, frames)` every audio callback: the library plays
the sound cues it made since the last call through kh2-rs's software sound driver
(`kh2_audio::ssl::Driver`, the game's SE sequencer on a software SPU2 with the tables of the disc's
`LIBSSL.IRX`) and renders 16-bit stereo at 48 kHz. Played: trigger-8 motion sounds of every actor
(Sora's body and keyblades, the members, the enemies, the summon) and the mission's numbered
sounds, all centred at full level (decided: the host has the camera; kh2-rs `audio::placement`
is the original's distance and pan rule). Not played: voices (the voice table), footsteps (the
area's sound info), music, hit sounds.

Measured (`sounds_of_a_jump_an_attack_and_fire_render_to_pcm`): a run, a jump, a 40-swing combo
and a Fire cast start 8 sounds; peak 4,864, RMS 463 of 32,767. Driver survey
(`KH2_SE_SURVEY=1 ... survey_of_silent_sound_effects`): of the 10,499 entries of the 147 global
banks (`se/seNNN.seb`), 34 render silent within one second and 9 do not start (no wave bank); the
objects' own banks (`obj/*.apdx`) were not surveyed. The 1,131 silent effects kh2-rs's notes count
were not reproduced with this driver on the global banks; their fixes live on an unmerged kh2-rs
branch and are not used here.

### Round 11: placement, hit sounds, music

- Placement: `kh2_audio_set_listener(pos, forward, up)`; a placed sound takes the volume and pan of
  kh2-rs `sound::placement` (`kh2_audio::positional::volume_pan`, the source in the listener's frame).
  Measured: a keyblade swing 2 m in front plays louder than 50 m away (259,619 vs 179,418 energy).
- Hit sounds: every landed hit is a `KH2_CUE_HIT` cue (the attack row's `EnemyReaction`, the
  attacker's model: Sora's keyblade, an enemy's own); the engine plays kh2-rs `PlaySe::hit`'s rule
  (light 0 / 1, heavy 2 / 3 of the attacker's own bank, alternating).
- Music: `kh2_audio_play_bgm(n)` plays `bgm/musicNNN.bgm` with its wave bank in the field slot.
  Measured: music 50 renders at -29.6 dBFS RMS; the reference recording of a Twilight Town fight
  (PCSX2 loopback with music and effects, 25 s) is -14.4 dBFS: the library is about 15 dB quieter,
  kh2-extras' master level 0.6 and the recording's own output level both unknown in that gap; no
  correction was applied. tt04's map programs set no music pair (the game picks the field music from
  elsewhere, not traced).
- Not played yet: Sora's battle voices (the voice table and the area's bank, kh2-rs `sora_voice`),
  footsteps (the area's sound info, `footstep_number`).

### Round 13 (Final Mix): voices, footsteps, the output level

- `kh2_audio_set_area("tt04")` gives the engine the area: its `arif` record's battle voice bank and
  its `12soundinfo.bar` footstep record.
- Voices (kh2fm-rs `sora_voice`, `enemy_voice`, `play_voices`): Sora's triggers 13 / 14 name a voice
  id; the `vtbl` record picks a line (`kh2_audio::voice::pick_line`, the game's generator) from
  `voice/<lang>/battle/<world><bank>_sora.vsb` (language and speaker names from the executable). An
  enemy's trigger is a line of its own bank (`.apdx` entry 0x22) with the trigger's priority. One
  slot per speaker keeps the priority rule; the volume is at least `MIN_VOLUME`, centred when loud.
  decided: the other party's NeoStatus (the -3 line of a pick) is 0.
- Footsteps (kh2fm-rs `footsteps`): trigger 2's kind, the owner's `objentry` NeoStatus, the ground
  material under Sora (cue `params[1]`), a step counter per owner, `footstep_number` -> a numbered
  sound placed at the feet. Not ported: the ground mover's stop step (kind 8) and the dust.
- Measured on the Final Mix disc in tt04: 23 footstep cues -> 19 sounds, 4 voice cues -> 4 lines.
- Output level: the mixer's master factor is now 1.0 (the SPU2 main volume 0x3fff at the end of the
  0x10b0b0 chain; 1.x used kh2-extras' 0.6). Music 50 still renders at -30.1 dBFS RMS against the
  reference recording's -14.4: the rest of the gap was not found in the driver's rules (the BGM's own
  header volume +0xc and the slot volume are applied as the IOP code does); the recording mixes
  music and effects at PCSX2's output level, so it is not a like-for-like reference.

### Round 15

- The ground mover's stop step and start dust (FUN_0017af10, kh2fm-rs `footsteps`): letting the stick
  go while walking or running cues footstep kind 8 and dust 8 / 9; the first step out of idle cues dust
  10 / 11 (`the_stop_step_and_the_start_dust_cue`).
- Music level: the PCSX2 references in `Public\SFX` are recordings of play (music, effects and voices
  mixed at the emulator's output level); no recording of music alone exists, so no like-for-like level
  comparison is possible. The library renders the BGM with the IOP's own volume chain (master 0x3fff,
  the BGM header's +0xc volume, the slot volume); not compared further.

## From a cue to a file

The sound number of a trigger-8 cue is `bank * 1000 + index` (kh2-rs `motion_sound_request`):

- bank **500**: the owner's own bank, the sound block inside `obj/<model>.apdx` (entry `se..`);
- any other bank: the global bank `se/se<bank, 3 digits>.seb`, entry `index`.

With the sound export of kh2-extras (a separate tool that writes WAVs from the user's disc):

| what | kh2-extras output |
|---|---|
| global bank `se/seNNN.seb`, entry `i` | `sounds/se/seNNN/e<i, 3 digits>.wav` |
| an object's own bank (`obj/<model>.apdx`), entry `i` | `sounds/se/obj/<model>/e<i, 3 digits>.wav` |
| an object's voice bank (apdx entry 0x22), line `i` | `sounds/voice/obj/<model>/<i, 3 digits>.wav` |
| voice files `voice/**.vsb` | `sounds/voice/...` by the bank's path |

So `KH2_CUE_SOUND` with params (200, 22) from `M_EX010` plays `sounds/se/se200/e022.wav`; with
(500, 35) from `W_EX030` it plays `sounds/se/obj/W_EX030/e035.wav`. Sora's voice cues name a voice
id that kh2-rs maps through the voice table and the area's bank (`sora_voice`), not a file index;
a host without that table can use the id as a key of its own.

Footsteps: the sample depends on the floor's material and the area's sound info
(`kh2_audio::footstep::footstep_number`); a host maps (footstep kind, `kh2_state.ground_material`)
to its own samples.

Volume and pan: the original places a sound by its source position against the camera (kh2-rs
`audio::placement`: full level 0x3fff at the camera, falling with distance; pan by the side). The
cue gives the position; the host's 3D audio does the rest.

## From an effect cue to a PAX

`KH2_CUE_EFFECT` / `KH2_CUE_SCRIPT_EFFECT` name an element **id** of the owner's effect file: the
PAX (BAR type 18) inside `obj/<model>.apdx` (for enemies the model's or its motion set's name,
kh2-rs `enemy.rs`); for model "limit" the PAX of the running limit's `limit/us/<file>.bar`. Every
PAX element whose `id` matches starts (kh2-rs `fx::start`), bound to the element's bone of the owner
(`kh2_fx::Element::bone`, flags `BIND*`) with the element's translation, rotation and scale. kh2-rs's
`kh2-fx` crate decodes the PAX and runs its particle programs; a host can use it, or its own
renderer, keyed by (model, element id).
