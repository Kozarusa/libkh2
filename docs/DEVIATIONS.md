# Known deviations from the original

libkh2 runs kh2-rs's rules; where kh2-rs differs from the game, libkh2 does too, and libkh2 leaves
out or simplifies some parts of its own. Both lists are here. kh2-rs keeps its own list (its
`agents/DEVIATIONS.md`); the open items that reach the library's systems are summarised below.

## From kh2-rs (open there, inherited here)

- Movement against the original (the oracle rig): on one recorded run the first ticks of a walk are
  a few units off, the facing follows the stick later than the original, and the walk stops at a
  wall the original slides past. kh2-rs's capsule slide is its approximation (the game's own
  collision walker exists in kh2-rs but is not wired).
- Collision groups: `kh2_set_collision` switches groups 0..63 on one object; the original switches
  mesh groups by a byte up to 255.
- Aerial Recovery is on the attack button (the original uses jump); explicit actions start from
  attack (the original: Square). kh2-rs's choice, kept.
- Render-side items (camera field of view, lighting, effects) are not the library's: it draws
  nothing.

## Final Mix against the USA branch (2.0.0)

`main` reads the Final Mix disc (`SLPM_666.75`) through kh2fm-rs; the `usa` branch (tag
`usa-final`) is the USA build and gets no further work. What differs:

- Forms: Final Mix numbering (Valor 1, Wisdom 2, Limit 3, Master 4, Final 5, Anti 6): seven form
  slots in `kh2_drive_setup` (7 level bytes, owned bits 1..5) and in save records (format 4; a
  version-3 record of the USA build does not load). Limit Form enters with its own body
  (`W_EX010_KH1F`, one keyblade) and the Dodge Roll growth group. Its own limits (the KH1 rows of
  `limt`, commands 698 / 701 / 704 / 683) run through `kh2_limit_start` on the Limit Form body's
  script (2.1.0, kh2fm-rs `run_kh1_script`): the motions it asks for play on Sora; decided: an
  attack record's leap (host 6.67) is not applied, the hand bones (host 1.235) are 0, the reaction
  offers of host 1.165 are kept in `Limits::kh1_offer` (no C API yet).
- Dodge Roll (2.1.0, the FM rule of FM 0x179530 from kh2fm-rs's final-mix.md): moving + Square
  turns Sora to the stick and plays A180 with blend 0; no velocity (the roll travels by root
  motion); invincibility is the Dodge Roll level's `fmab` +0x40 `DodgeRollStarTime` (10 ticks at
  level 1). Square still held at A180's frame trigger 34 (tick 10) starts the Quick Run along the
  facing when learned; the idle range (trigger 2, tick 36) lets Sora act again. 2.0.0 used the
  Quick Run row's speed and star time (kh2fm-rs's app stand-in).
- Ground mover: kh2fm-rs's `Motor::ground_velocity` (speed along the stick); the USA build used the
  oracle-tested `ground_step` that kh2fm-rs does not have. The A / B against kh2fm-rs's Sora matches
  (0.5 cm over 1080 ticks, 0.7 cm over 960 with the wall recoil).
- Damage (2.1.0, disassembled): FM 0x1a1e20..0x1a2104 is the USA routine with the Final Mix status
  layout (Defender's byte +0x1fc, the object +0x25c, the element boosts +0x210..+0x214, the ability
  block's combo lengths and boosts 4 bytes later), Anti as form 6, the difficulty helper's extra
  Critical case (200 % taken / 125 % dealt; difficulty is not modelled), and one new last step for
  every type (`enemy::low_hp_scale`: HP at or below max HP x status +0x240 scales the damage by
  `sstm` +0x178). Round 12's 368 / 1954 differing cases were the test's USA layout; the oracle tests
  are strict again and match 3000 / 3000. Open: the writer of status +0x240 (libkh2 passes 0.0, so
  the step is applied in Sora's hit paths but only changes a hit on a victim at 0 HP); the party and
  summon hit paths skip it.
- Text: the Final Mix disc's messages are Japanese; the HUD's Japanese glyphs (`kh2_image` text kind
  `Japanese`, the font set's Japanese companion) are not drawn yet. kh2fm-rs's English text import
  (a USA disc copy in the user's config folder) is not used: libkh2 reads one disc.

## libkh2's own

Sora
- No game over: at HP 0 he stays down until `kh2_revive` (or stands up after 180 ticks with
  `auto_revive`).
- Magnet and Reflect, the Final form's scatter, summons: not ported. Spells start at Sora's chest.
- The save record is between-rooms status plus the place, not a mid-motion snapshot.
- HP/MP growth comes from bonus rewards only (`kh2_give_bonus`); no equipment, so no stat items.

Party
- Since 0.6 Donald and Goofy run their own scripts (Specials, Cure); the scripts' effects are
  reported as events, not played. The members' items take effect when chosen (no use motion).
- Their weapon set byte is 0 (Sword column of the level-up abilities); their weapons are the base
  items (Mage's Staff, Knight's Shield) unless the host equips others.
- A member beyond 30 m is put back beside Sora (no warp through the trail).
- Auto Healing of reserve members, Auto Change, the party record across areas: not ported.

Limits
- The limit task, its countdown, hit count, the limit file's own script, the LIMI motions on the
  bodies and Auto Limit's offer run, and the members stay in the limit state; the limits' damage
  does not: the LIMI motions carry no attack windows (measured), the damage comes from the limit
  task's own objects (kind 5000), which kh2-rs neither ports nor documents. Summon limits and the
  HUD are not ported either.

Summons
- The command, arrival (no event), timer, level, dismiss and leave run, and the object's own
  script; Stitch's and Genie's classes run (Stitch's camera plane is placed in a stand-in camera
  frame behind Sora), Chicken Little's and Peter Pan's classes do not (they stand and play their
  scripts).

Enemies and reaction commands
- An area's battle program runs on request (`kh2_area_run`: groups, kind-2 waves, kind-6 groups,
  counters, signals); the program is the host's choice (no story), the defeated are not remembered
  across areas, the field objects that raise signals are the host's (`kh2_area_raise_event`).
  M_EX850 waits for such a set-up and stays idle (see `ENEMY-KINDS.md`).
- The mission manager (kh2-rs struggle.rs, `kh2_mission_state`) starts with an area run whose
  program has a mission command. Approximations: an object's centre is its feet + 80 cm (no body
  shape points); the result menu (record 0x1a), the game
  over, the layout events' pictures and the bit-3 collect latch are reported, not run; the sounds
  are handed over by number. Every hit counts for the orbs, Sora's, the members' and the summons' (`hits_from_members_before_the_enemy_tick_knock_orbs_out`); the opponents' bats hit Sora with their own attack shapes when he leaves them room to swing (`seifers_bat_hits_sora_and_knocks_his_orbs_out`).
- No camera: every enemy counts as on screen. No clashes. Prize drops are reported as counts.
- A defeat by a party member pays no experience (the event carries the values).
- Talks are the host's to start (`kh2_talk_begin`: the library does the player's side, the camera and the lock; the dialogue, the target's motion and its facing after the talk are the host's). A talk record on a bone is read through the bones the host gives, else the rest pose (kh2-rs reads the last pose).
- Reaction commands: talk, save, shop and limit rows through the reaction slot are refused;
  Mickey's Charge command is not consumed.

Inputs the original cannot get
- Positions must be finite and within 1e7 units; sticks are held to -1..1; maximum HP / MP to
  9999; a damage query to 0..999,999 (fuzz-driven bounds, beyond anything the game holds).

Limits deal no damage. The evidence (rounds 8 and 9), a dead end without the executable's limit task:
- A limit file (`limit/us/<name>.bar`) holds the limit's script (type 3), the `LIMI` set (type 0x14:
  per-participant motion sets, the A9xx sub-motions libkh2 plays, with no attack triggers), its
  effect file (type 18, PAX), sounds, gauge pictures and voice.
- The effect files decode with kh2-rs's kh2-fx; none of the 19 uses a particle program of the hit
  family (pppKeHitBall, pppKeHitChk, pppKeHitChkPxB, pppKeHitBorn, pppKeThHitBorn); the PAX
  elements carry a bone, a transform and a sound, no attack row (`limit_effect_files_carry_no_hit_programs`).
- Comet's script (donald.bar) runs 600 ticks making only these host calls: 0.21, 1.23, 1.94, 1.131,
  1.147, 1.165 and 1.221 every tick, 1.183, 2.23, 2.48, 2.49, 2.55, one 2.59 (the effect), 2.60 (the
  gauge) every tick; sending its sub-commands 281 / 283 (event 23) adds none. No object is created
  (no host 1.0) and no attack object (no host 2.10).
- The members' own scripts have no limit mode (Donald's modes: idle, battle, fire, blizzard,
  thunder, cure, ...; Goofy's: blade, turbo, tornado, ... -- Goofy Tornado is his Special, which
  libkh2 runs since 0.7, not a limit).
So the hits come from the executable's limit task code (`FUN_001b2c10` / `FUN_001b2e08` and the
kind-5000 objects it makes), which kh2-rs has not ported; there is no data to drive them from.

Process-wide script state: the compiled kh2-rs script hosts keep the game's few global tables as
process-wide statics (the shared memory blocks, the collision group edits, the area mask, the group
enemy sum). Through the C API this is invisible (one library state, calls serialised). Rust users
who tick separate `Enemies` / `AreaRun` sets on several threads at once share those tables; the disc
tests serialise themselves for this reason.

Magic levels and the last two spells (round 10, `every_spell_at_every_level_casts_with_its_own_row`):
levels 1 to 3 of Fire, Blizzard, Thunder and Cure use their own `magc` rows, commands, cast kinds
(0 / 2 / 4) and `atkp` sub rows (0 / 512 / 1024). Magnet and Reflect cast and pay their MP (round 11
fixed Reflect: its trigger 17 sits at frame 0 of the cast motion, and the cast motion starts after
the tick's trigger walk, so the next walk began at frame 0.5 and stepped past it; the first walk on a
cast motion now starts before frame 0). Their effects are not simulated: Magnet's pull and hits
(atkp 658, the victims' reaction 5) and Reflect's guard (atkp 573, a type-3 row that clashes with
enemy attacks) come from their own .mag scripts, which kh2-rs runs in the BDX interpreter with its
spell host (`magic.rs` `VmCast` / `SpellHost`: the effect handles 1.138 / 1.139, 0.38 / 0.39, the
attack objects, the persistent blocks of 2.21), about a thousand lines tied to Bevy effect
entities; porting it is the next step (TODO.md). Limit Form (Final
Mix only) is not on the USA disc kh2-rs targets: nothing to port.
