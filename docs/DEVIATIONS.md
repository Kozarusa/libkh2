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
- An area's battle groups spawn on request (`kh2_area_spawn`), but its signals, waves, triggers,
  events and missions do not run; the Struggle's mission manager is the host's (`docs/STRUGGLE.md`).
  M_EX850 and M_EX870 wait for such a set-up and stay idle (see `ENEMY-KINDS.md`).
- No camera: every enemy counts as on screen. No clashes. Prize drops are reported as counts.
- A defeat by a party member pays no experience (the event carries the values).
- Reaction commands: talk, save, shop and limit rows through the reaction slot are refused;
  Mickey's Charge command is not consumed.

Inputs the original cannot get
- Positions must be finite and within 1e7 units; sticks are held to -1..1; maximum HP / MP to
  9999; a damage query to 0..999,999 (fuzz-driven bounds, beyond anything the game holds).
