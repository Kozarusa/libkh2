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
  the step only changes a hit on a victim at 0 HP). 2.2.0 searched the Final Mix executable for every
  load and store with offset 0x240: the damage routine is the only float reader, and the word stores
  (FM 0x1802d8, 0x180378 and the others) write other records (the limit task's), so the status field
  is filled by a block copy not found. The step now runs in every hit path (Sora, enemies on Sora and
  the members, the members', the summon's).
- Difficulty (2.2.0, `kh2_set_difficulty`): FM 0x1a2108, oracle-tested: party members take 50 / - /
  200 / 200 % and deal 150 / - / 100 / 125 % (Beginner / Standard / Proud / Critical). Every hit path
  applies it (the summon counts as a party member: decided, its object flag was not read).
  2.3.0 searched every read of the difficulty word (FM 0x32dfc8): four functions read it. Besides the
  damage step (2.4.0, identified by their callers and applied): FM 0x1b73e8 is the `lvup` row's AP
  (byte +7) as the status set-up FM 0x1a11e0 and the level-up FM 0x1b72c0 read it: x 1.5 in Critical
  for Sora (1) and character 14 (oracle-tested; `enemy::critical_ap`); FM 0x1c068c (USA 0x1be500)
  applies a `bons` row: Critical halves its HP and MP increases for characters 1..3 and 14
  (`enemy::critical_bonus`, read, not oracle-run: the function also calls the stat writers). No reader
  is on the EXP payout path: the difficulties have no EXP rule in the executable's code.
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

## Magnet and Reflect (2.1.0)

- Magnet runs as `spell.rs` `Step::Magnet` in the order of MAGNET_*.mag (kh2fm-rs magic-turn.md): after a
  yield attack 658 of the kind at the target (else 300 units ahead), lifted `magi` +0x3c above the ground,
  a sphere of `magi` +0x4c, alive 16 + `magi` +0x40 + `magi` +0x44 frames. Its victims get reaction class 5,
  the pull (kh2fm-rs `reaction_rules::Pull`, FUN_001bf920 / FUN_001bfc10 / FUN_001bfba8, the numbers from
  the executable): drawn toward the sphere with the swirl, no gravity, let go with motion 0 when it is
  finished. decided: the body centre is the position + 80. 2.2.0: in Wisdom, Master and Final the
  second attack 976 (radius `magi` +0x3c x 0.5) joins 16 frames in; Final adds 1440 (x 0.6) for one
  frame at the end. Not done: the effect elements and Final's global flag (every attack on the player).
- Reflect (2.2.0, `spell.rs` `Step::Reflect` in REFLECT_*.mag's order): after a yield the guard (a
  sphere of `magi` +0x50 at Sora's chest) lives as long as the barrier: 27 ticks, the manager life of
  the barrier element's particle data in REFLECT_1..3.mag (elements 0, 2, 12). An enemy attack window
  that touches it clashes once (`EnemyEvent::Clash`, `KH2_EN_CLASH`) and cannot hit Sora. When the
  barrier ends, the clash count against `magi` +0x64 / +0x70 (integers, 5 and 10) picks the burst:
  574 / 576 / 578 for `magi` +0x54 frames, then 667 / 668 / 669 20 larger for `magi` +0x58; no clash,
  no burst. 2.3.0: an enemy window clashes only when the executable's clash table pairs the guard's
  type (3) with the enemy row's type (`ExeConsts::types_clash`, FUN_0018d808), and the enemy plays its
  clash reaction (handler 0x1b2478, `reaction_rules::enemy_clash`: ReflectedMotion and the
  ReflectHitBack push, nothing when RefactSelf says Nothing). 2.4.0: the cone test FUN_0018d308 runs
  both ways (the guard row 573's ReflectRange / ReflectAngle with Sora's facing, the enemy row's with its
  own). decided: the script action (ReflectAction) is not started; the fade-out (8) is not counted.
