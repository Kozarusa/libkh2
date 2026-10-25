# libkh2 C API reference

Generated from `include/kh2.h` by `scripts/gen_api_ref.py`; do not edit by hand.

## Sections

- [Core](#core)
- [Magic](#magic)
- [Drive forms](#drive-forms)
- [Animation export](#animation-export)
- [Party members (kh2-rs party_rules + party_battle: field follow; with spawned enemies
 * kh2_party_tick runs the battle part: target choice of the member's style (0..3), action records,
 * attack motions, weapons, damage reported as kh2_enemy_event hits; enemies hit the members (HP from
 * the new game's plrp rows, status from lvup at the member's level), knock-out and the revive timer,
 * the danger test, emergency pass and HP/MP item handlers from the host's item stock; since 0.6 the
 * members' own BDX scripts run (kh2-rs friend_host): Specials (Fire, Thunder, Blade, Tornado ...) and
 * Cure switch the script's mode, its attack objects hit enemies, its heals heal; during a limit the
 * limit's motions play on the members' bodies. Not ported: equipment, the scripts' effects (reported).](#party-members-kh2-rs-partyrules--partybattle-field-follow-with-spawned-enemies--kh2partytick-runs-the-battle-part-target-choice-of-the-members-style-03-action-records--attack-motions-weapons-damage-reported-as-kh2enemyevent-hits-enemies-hit-the-members-hp-from--the-new-games-plrp-rows-status-from-lvup-at-the-members-level-knock-out-and-the-revive-timer--the-danger-test-emergency-pass-and-hpmp-item-handlers-from-the-hosts-item-stock-since-06-the--members-own-bdx-scripts-run-kh2-rs-friendhost-specials-fire-thunder-blade-tornado--and--cure-switch-the-scripts-mode-its-attack-objects-hit-enemies-its-heals-heal-during-a-limit-the--limits-motions-play-on-the-members-bodies-not-ported-equipment-the-scripts-effects-reported)
- [Limits (kh2-rs limit.rs rules + limit_script.rs: Sora's limit commands with Donald and Goofy)
 * The party is Sora plus the members who follow and stand. kh2_limit_start spends the MP, starts the
 * countdown (the command's Time, times Combination Boost) and the limit file's own script, whose
 * requests come back as limit events; the countdown and script run in kh2_party_tick, enemy hits during
 * the limit count. Not ported: the participants' own threads (the members' scripts play the attacks),
 * summon limits, the HUD; the limit's motions are reported, not played.](#limits-kh2-rs-limitrs-rules--limitscriptrs-soras-limit-commands-with-donald-and-goofy--the-party-is-sora-plus-the-members-who-follow-and-stand-kh2limitstart-spends-the-mp-starts-the--countdown-the-commands-time-times-combination-boost-and-the-limit-files-own-script-whose--requests-come-back-as-limit-events-the-countdown-and-script-run-in-kh2partytick-enemy-hits-during--the-limit-count-not-ported-the-participants-own-threads-the-members-scripts-play-the-attacks--summon-limits-the-hud-the-limits-motions-are-reported-not-played)
- [Summons (kh2-rs summon.rs flow + drive_rules: Stitch, Genie, Chicken Little, Peter Pan)
 * The host says which charms are owned; kh2_summon spends the drive points, Donald and Goofy go
 * away, after the arrival wait the summon object appears at Sora's side with its own script
 * ("appear_skip"), the summon timer (gauge mode 3) drains with Summon Boost and feeds the summon
 * level; at 0 or on Dismiss its script plays "leave", then the companions come back. Ticked by
 * kh2_party_tick. Not ported: the summons' class AI (Stitch's moves and blast, Genie's records ...),
 * the arrival event, summon limits, Genie's Drive!.](#summons-kh2-rs-summonrs-flow--driverules-stitch-genie-chicken-little-peter-pan--the-host-says-which-charms-are-owned-kh2summon-spends-the-drive-points-donald-and-goofy-go--away-after-the-arrival-wait-the-summon-object-appears-at-soras-side-with-its-own-script--appearskip-the-summon-timer-gauge-mode-3-drains-with-summon-boost-and-feeds-the-summon--level-at-0-or-on-dismiss-its-script-plays-leave-then-the-companions-come-back-ticked-by--kh2partytick-not-ported-the-summons-class-ai-stitchs-moves-and-blast-genies-records---the-arrival-event-summon-limits-genies-drive)
- [Enemies](#enemies)
- [Field camera helper](#field-camera-helper)

## Core

libkh2: Sora's controller from Kingdom Hearts II as an embeddable library (libsm64-style). The host owns the world and the drawing; libkh2 owns Sora. Call kh2_init once with the path of the user's own disc image, load collision triangles, create a Sora, then call kh2_tick once per 60 Hz logic tick (or kh2_tick_field per 30 Hz frame) with the input and read back the state. Units: kh2 units (centimetres), right-handed, Y up. Sora's yaw 0 faces +Z; forward is (sin yaw, 0, cos yaw). Matrices are 16 floats, column-major. Every function returns -1 (or a negative value) on error; kh2_last_error explains it. The library keeps one global state (like libsm64) guarded by a mutex: every call is safe from any thread and calls are serialised (no parallel ticks). Pointers passed in are only read during the call. No call panics or aborts through the ABI: internal errors return -1 with a message.

The version this header describes; kh2_version() returns the library's (same packing).

- `KH2_VERSION_MAJOR` = `0`
- `KH2_VERSION_MINOR` = `6`
- `KH2_VERSION_PATCH` = `0`
- `KH2_VERSION` = `((KH2_VERSION_MAJOR << 16) | (KH2_VERSION_MINOR << 8) | KH2_VERSION_PATCH)`

Surface attribute: the COCT PARTY_COLLIDE bit. A wall-like triangle (normal more than 60 degrees from up) stops Sora when it carries it (and its low nibble is not 4, bit 0x100000 clear). Floors and ceilings are found by their normal whatever the flags.

- `KH2_SURF_WALL` = `0x10u`

One collision triangle. The face normal (v[1]-v[0]) x (v[2]-v[0]) must point to the side Sora stands on: up for floors, toward the walkable side for walls (walls are one-sided).

### `kh2_surface`

```c
typedef struct kh2_surface {
    float v[3][3];
    uint32_t flags;    /* COCT attribute word, e.g. KH2_SURF_WALL for a solid wall */
    uint8_t group;     /* mesh group 0..63, switched by kh2_set_collision */
    uint8_t material;  /* ground material (footstep kind), reported back in kh2_state */
    uint16_t _pad;
} kh2_surface;
```

- `KH2_BTN_ATTACK` = `1u` — X: attack
- `KH2_BTN_JUMP` = `2u` — Circle: jump (hold for the full height)
- `KH2_BTN_GUARD` = `4u` — Square: guard

### `kh2_inputs`

```c
typedef struct kh2_inputs {
    float stick_x, stick_y;       /* left stick: x right, y forward (away from the camera), length 0..1 */
    float cam_look_x, cam_look_z; /* the camera's horizontal look direction in world space */
    uint32_t buttons;             /* KH2_BTN_* held this tick; presses are derived from the edges */
} kh2_inputs;
```

kh2_state.state

- `KH2_STATE_GROUND` = `0`
- `KH2_STATE_AIR` = `1`
- `KH2_STATE_ATTACK` = `2`
- `KH2_STATE_AIR_ATTACK` = `3`
- `KH2_STATE_GUARD` = `4`
- `KH2_STATE_QUICK_RUN` = `5`
- `KH2_STATE_GLIDE` = `6`
- `KH2_STATE_HURT` = `7`
- `KH2_STATE_BLOWN` = `8`
- `KH2_STATE_DOWN` = `9` — knocked out: kh2_revive
- `KH2_STATE_COUNTER` = `10` — the guard repelled a hit
- `KH2_STATE_MAGIC` = `11` — a cast motion plays
- `KH2_STATE_FORM_CHANGE` = `12` — a drive change motion plays
- `KH2_STATE_REACTION` = `13` — a reaction command's paired motion plays (no input)

### `kh2_state`

```c
typedef struct kh2_state {
    float pos[3];
    float vel[3];          /* horizontal velocity per tick (the mover's) */
    float yaw;
    int32_t grounded;
    int32_t state;         /* KH2_STATE_* */
    int32_t motion_slot;   /* slot of Sora's motion set (motion id * 4 + variant), -1 none */
    int32_t motion_id;     /* motion id (slot / 4): 0 idle, 1 walk, 2 run, 3 jump, 4 fall, 5 land, ... */
    float frame;           /* frame in the motion's own frame units */
    float gframe;          /* frame in 60 Hz global frames (the unit of motion triggers) */
    int32_t motion_finished;
    int32_t blending;      /* a cross-fade into this motion is running */
    int32_t combo;         /* attacks accepted in the current chain */
    int32_t attack_active; /* an attack window (range trigger 10/33) is live: test kh2_get_attack_volumes */
    int32_t attack_id;     /* the first live window's attack id (atkp), -1 none */
    int32_t attack_group;  /* the first live window's collision shape group, -1 none */
    int32_t combo_window;  /* range trigger 11 (allow combo) covers the frame */
    int32_t finisher_window; /* range trigger 50 (allow finisher) */
    int32_t weapon_out;    /* the battle variant of the motion set is in use (keyblade out) */
    int32_t ground_material; /* material of the floor under Sora, -1 airborne */
    int32_t invulnerable;  /* invincibility ticks are running */
    int32_t hp, max_hp;
    int32_t mp, max_mp;
    float mp_charge;       /* MP charge frames left, 0 when none */
    int32_t form;          /* active drive form: 0 none, 1 Valor, 2 Wisdom, 3 Master, 4 Final, 5 Anti */
    int32_t drive_points, drive_fraction;
    float form_time, form_time_total;
    int32_t level, exp;    /* level 1..99 and the experience total */
    int32_t strength, magic, defense;
} kh2_state;
```

A live attack capsule in world units: segment a-b, radius.

### `kh2_volume`

```c
typedef struct kh2_volume {
    float a[3];
    float b[3];
    float radius;
    int32_t attack_id;
    int32_t group;
} kh2_volume;
```

### `kh2_init`

```c
int32_t kh2_init(const char *iso_path);
```

Opens the disc image (NULL: the KH2_ISO environment variable) and loads Sora's model, motion set, keyblade, attack table and movement parameters. Nothing is copied out of the disc.

### `kh2_version`

```c
uint32_t kh2_version(void);
```

### `kh2_terminate`

```c
void kh2_terminate(void);
```

### `kh2_last_error`

```c
uint32_t kh2_last_error(char *buf, uint32_t len);
```

The last error message; returns its full length.

### `kh2_surfaces_load`

```c
int32_t kh2_surfaces_load(const kh2_surface *tris, uint32_t count);
```

Replaces the static collision. Returns the triangle count kept (degenerate ones are dropped).

### `kh2_surfaces_load_map`

```c
int32_t kh2_surfaces_load_map(const char *area);
```

Replaces the static collision with an area's own collision from the disc, e.g. "tt00".

### `kh2_set_collision`

```c
void kh2_set_collision(uint32_t group, int32_t solid);
```

Switches mesh group `group` (0..63) solid or not.

### `kh2_create`

```c
int32_t kh2_create(float x, float y, float z);
```

Creates a Sora at (x, y, z); settles on a floor within a step below. Returns a handle.

### `kh2_delete`

```c
void kh2_delete(int32_t handle);
```

### `kh2_set_position`

```c
int32_t kh2_set_position(int32_t handle, float x, float y, float z, float yaw);
```

Warps Sora (velocity and jump cleared).

### `kh2_tick`

```c
int32_t kh2_tick(int32_t handle, const kh2_inputs *inputs, kh2_state *state);
```

One 60 Hz logic tick. `state` may be NULL.

### `kh2_tick_field`

```c
int32_t kh2_tick_field(int32_t handle, const kh2_inputs *inputs, kh2_state *state);
```

Two ticks with the same input: one 30 Hz field frame.

### `kh2_get_state`

```c
int32_t kh2_get_state(int32_t handle, kh2_state *state);
```

### `kh2_set_target`

```c
int32_t kh2_set_target(int32_t handle, float x, float y, float z, float radius, float half_height);
```

The host's lock-on target point (an enemy's target-shape centre, radius, half height). It feeds the attack table's range tests and the attack's lunge, turn and leap.

### `kh2_clear_target`

```c
int32_t kh2_clear_target(int32_t handle);
```

### `kh2_report_hit`

```c
int32_t kh2_report_hit(int32_t handle);
```

The host found that the current swing touched something: lets the combo chain go on.

### `kh2_hit_taken`

```c
int32_t kh2_hit_taken(int32_t handle, int32_t atkp, int32_t damage, float dir_x, float dir_y, float dir_z);
```

A hit taken: atkp = attack row id of the disc's atkp table (its reaction class decides flinch, push or blow), damage = HP taken (the host computes it), dir = from the attacker toward Sora. Returns 0 ignored (invincible / down), 1 guarded, 2 no flinch, 3 flinch, 4 blown, -1 unknown row.

### `kh2_set_hp`

```c
int32_t kh2_set_hp(int32_t handle, int32_t hp, int32_t max_hp);
```

### `kh2_revive`

```c
int32_t kh2_revive(int32_t handle, int32_t automatic);
```

Stands up from KH2_STATE_DOWN with full HP; auto != 0: stand up by itself after 180 ticks.

### `kh2_set_abilities`

```c
int32_t kh2_set_abilities(int32_t handle, uint64_t lo, uint64_t hi, uint8_t max_ground, uint8_t max_air, uint8_t max_finishers);
```

Ability bits 0..127 (bit 1 = Guard) and combo lengths; defaults Guard, 3, 3, 1.

### `kh2_set_loadout`

```c
int32_t kh2_set_loadout(int32_t handle, const uint16_t *ids, uint32_t count);
```

The equipped abilities (ability item ids, e.g. 82 Guard, 162 Combo Plus, 163 Air Combo Plus, 393 Finishing Plus, 394 Negative Combo, 395 Berserk Charge; growth abilities by their items). The block they give sets the attack table's bits, combo lengths and growth levels; Second Chance / Once More, Leaf Bracer and Berserk Charge apply. Replaces kh2_set_abilities/_growth.

### `kh2_get_loadout_block`

```c
int32_t kh2_get_loadout_block(int32_t handle, int32_t out[7]);
```

out[7]: ground combo, air combo, finishers, High Jump, Quick Run, Aerial Dodge, Glide.

### `kh2_set_high_jump`

```c
int32_t kh2_set_high_jump(int32_t handle, uint8_t level);
```

High Jump level 0..5.

### `kh2_set_growth`

```c
int32_t kh2_set_growth(int32_t handle, uint8_t high_jump, uint8_t quick_run, uint8_t aerial_dodge, uint8_t glide);
```

Growth abilities 0..5 (fmab rows): High Jump, Quick Run (Square while moving), Aerial Dodge (jump in the air), Glide (hold Square while falling).

### `kh2_set_pose_enabled`

```c
int32_t kh2_set_pose_enabled(int32_t handle, int32_t on);
```

Skeleton evaluation each tick (default on). Off: no bones, no attack volumes.

### `kh2_bone_count`

```c
int32_t kh2_bone_count(int32_t handle, int32_t which);
```

Skeletons: which = 0 body, 1 keyblade (hangs from body bone 178), 2 a drive form's second keyblade (bone 86; Valor, Master, Final).

### `kh2_get_bones`

```c
int32_t kh2_get_bones(int32_t handle, int32_t which, int32_t space, float *out, uint32_t max_bones);
```

space = 0: model space (body: relative to Sora's root, the body transform is pos + yaw; keyblade: relative to the hand bone); 1: world space. 16 floats per bone, column-major.

### `kh2_get_rest_bones`

```c
int32_t kh2_get_rest_bones(int32_t handle, int32_t which, float *out, uint32_t max_bones);
```

Rest pose (bind matrices, model space).

### `kh2_bone_parents`

```c
int32_t kh2_bone_parents(int32_t handle, int32_t which, int32_t *out, uint32_t max_bones);
```

### `kh2_get_attack_volumes`

```c
int32_t kh2_get_attack_volumes(int32_t handle, kh2_volume *out, uint32_t max);
```

The live attack capsules (world space). Returns the count written.

### `kh2_motion_tag`

```c
int32_t kh2_motion_tag(int32_t handle, char *buf, uint32_t len);
```

The playing motion's tag, e.g. "A300". Returns its length.

### `kh2_jump_height`

```c
float kh2_jump_height(int32_t handle);
```

Sora's jump height read from the disc (prty JumpHeight, or the High Jump row).

## Magic

kh2-rs magic_rules

### `kh2_cast`

```c
int32_t kh2_cast(int32_t handle, int32_t slot);
```

Asks for a cast of slot 0 Fire, 1 Blizzard, 2 Thunder, 3 Cure, 4 Magnet, 5 Reflect; tried for up to 30 ticks until the state and MP allow it. The host spawns the spell on KH2_EV_SPELL_FIRED.

### `kh2_set_magic_levels`

```c
int32_t kh2_set_magic_levels(int32_t handle, const uint8_t levels[6]);
```

### `kh2_set_mp`

```c
int32_t kh2_set_mp(int32_t handle, int32_t mp, int32_t max_mp);
```

- `KH2_EV_CAST_STARTED` = `1` — slot, cast_kind, motion, voice
- `KH2_EV_SPELL_FIRED` = `2` — frame trigger 17: MP paid, spawn the spell (charge: MP charge began)
- `KH2_EV_CAST_CANCELLED` = `3` — the motion changed before the trigger: no spell, no cost
- `KH2_EV_MP_RESTORED` = `4` — the MP charge ended
- `KH2_EV_FORM_CHANGED` = `5` — slot = new form (0 base), cast_kind = old form
- `KH2_EV_SPELL_ATTACK` = `6` — slot, stage, atkp/sub_id row, radius/height/life/speed, pos, dir
- `KH2_EV_HEALED` = `7` — amount, hp
- `KH2_EV_ITEM_USED` = `8` — slot = item id, amount = HP change, hp = MP change, charge = drive change (1/100 points)
- `KH2_EV_EXPERIENCE` = `9` — amount paid, hp = total
- `KH2_EV_BONUS` = `11` — slot = bonus id, amount = max HP added, hp = max MP added, charge = drive gauge added, sub_id / atkp = items
- `KH2_EV_LEVEL_UP` = `10      /* amount = the new level; rises over the previous lvup row: atkp = strength,`

### `kh2_event`

```c
typedef struct kh2_event {
    int32_t kind, slot, cast_kind, motion, voice, charge;
    int32_t stage, atkp, sub_id;  /* spell attack: stage and its atkp row (for kh2_damage) */
    float radius, height;         /* the stage's size (magi row) */
    float life, speed;            /* ticks the stage lasts (0: one shot), units per tick */
    float pos[3], dir[3];         /* where it starts and which way it goes */
    int32_t amount, hp;           /* healed */
} kh2_event;
```

### `kh2_damage`

```c
int32_t kh2_damage(int32_t handle, int32_t atkp, int32_t sub_id, int32_t defense, int32_t max_hp, int32_t weakness, int32_t general);
```

The damage of one of Sora's attack rows (sub_id -1: first variant) against a target's defense, max HP (type 5 rows) and element / general percents (100 neutral). -1: unknown row.

### `kh2_use_item`

```c
int32_t kh2_use_item(int32_t handle, int32_t item);
```

Uses an item (item id: 1 Potion, 2 Hi-Potion, 3 Ether, 4 Elixir, ...): the use motion plays and the effect lands 30 frames after its trigger (KH2_EV_ITEM_USED). The host keeps the inventory.

### `kh2_report_defeat`

```c
int32_t kh2_report_defeat(int32_t handle, int32_t lvpm_experience, int32_t enmp_experience);
```

A defeat: the lvpm row's Experience for the world level and the enemy's enmp Experience. Returns the EXP paid; KH2_EV_LEVEL_UP follows each level reached.

### `kh2_set_level`

```c
int32_t kh2_set_level(int32_t handle, int32_t level);
```

Level 1..99: strength, magic, defense from the disc's lvup row plus the Kingdom Key.

### `kh2_set_weapon_set`

```c
int32_t kh2_set_weapon_set(int32_t handle, int32_t set);
```

The weapon chosen at the start (0 Sword, 1 Shield, 2 Staff): the lvup ability column level-ups give.

### `kh2_give_bonus`

```c
int32_t kh2_give_bonus(int32_t handle, int32_t id);
```

Bonus reward `id` (bons rows, once per id): Sora's max HP / MP / drive gauge and items (KH2_EV_BONUS), the following members' max HP / MP (KH2_MEMBER_BONUS). 1 given, 0 given before, -1 error.

### `kh2_form_abilities`

```c
int32_t kh2_form_abilities(int32_t handle, int32_t form, uint16_t *out, uint32_t max);
```

Drive form `form` (1..5): its ability list at its level (fmlv grants + growth ability).

### `kh2_granted_abilities`

```c
int32_t kh2_granted_abilities(int32_t handle, uint16_t *out, uint32_t max);
```

Abilities given by level-ups, oldest first: writes up to max, returns the count (equip: kh2_set_loadout).

### `kh2_state_save`

```c
int32_t kh2_state_save(int32_t handle, uint8_t *out, uint32_t cap);
```

Save record ("KH2S" v2): level, EXP, HP/MP, AP, spell levels, weapon set, given and equipped abilities, drive gauge/forms/levels, position and yaw. Not saved: motion, combo, cast, an active form, enemies, party. save writes when cap is enough and returns the size; load returns 0, or -1 with nothing changed for a record that is not valid.

### `kh2_state_load`

```c
int32_t kh2_state_load(int32_t handle, const uint8_t *bytes, uint32_t len);
```

### `kh2_poll_event`

```c
int32_t kh2_poll_event(int32_t handle, kh2_event *out);
```

Takes the oldest event: 1 written, 0 none left.

## Drive forms

kh2-rs shared::drive::rules

### `kh2_drive_setup`

```c
int32_t kh2_drive_setup(int32_t handle, uint8_t points, uint8_t max_points, uint32_t owned_mask, const uint8_t *levels);
```

Gauge points / maximum, owned forms (bit n = form n, 1..4), form levels by form id (6 bytes, may be NULL).

### `kh2_drive_gain`

```c
int32_t kh2_drive_gain(int32_t handle, int32_t percent);
```

### `kh2_report_hit_atkp`

```c
int32_t kh2_report_hit_atkp(int32_t handle, int32_t atkp);
```

A landed hit with its atkp row: the combo latch plus the row's DriveDrain.

### `kh2_drive_enter`

```c
int32_t kh2_drive_enter(int32_t handle, int32_t form);
```

Enter form 1..4 (the Anti / Final roll may pick another): returns the form entered or -1. The body swaps when the change motion ends (KH2_EV_FORM_CHANGED); bones change with it.

### `kh2_drive_revert`

```c
int32_t kh2_drive_revert(int32_t handle);
```

### `kh2_drive_objects`

```c
int32_t kh2_drive_objects(int32_t handle, int32_t out[3]);
```

objentry ids of the form's body and hand 0 / hand 1 weapons (-1 none).

### `kh2_drive_set_second_keyblade`

```c
int32_t kh2_drive_set_second_keyblade(int32_t handle, int32_t item);
```

The keyblade item the second hand takes on the next entry (default 41, the Kingdom Key).

### `kh2_drive_outgoing_damage`

```c
int32_t kh2_drive_outgoing_damage(int32_t handle, int32_t atk_type, int32_t damage);
```

## Animation export

any slot, independent of what Sora plays; see docs/SKINNING.md

### `kh2_motion_desc`

```c
typedef struct kh2_motion_desc {
    float frame_start, frame_end, frame_return;
    float fps;              /* the animator steps fps / 60 frames per 60 Hz tick */
    int32_t has_root_motion;
    char tag[8];            /* e.g. "A300" */
} kh2_motion_desc;
```

### `kh2_motion_slot_count`

```c
int32_t kh2_motion_slot_count(int32_t handle, int32_t which);
```

### `kh2_motion_slot`

```c
int32_t kh2_motion_slot(int32_t handle, int32_t which, int32_t motion_id, int32_t variant);
```

variant: 0 battle + weapon, 1 battle, 2 field, 3 field + weapon (the game's fallback applies).

### `kh2_motion_info`

```c
int32_t kh2_motion_info(int32_t handle, int32_t which, int32_t slot, kh2_motion_desc *out);
```

### `kh2_sample_motion`

```c
int32_t kh2_sample_motion(int32_t handle, int32_t which, int32_t slot, float frame, float *out, uint32_t max_bones, float *root);
```

Bone matrices of `slot` at `frame` relative to the motion's root; root = (x, y, z, yaw) or NULL.

## Party members (kh2-rs party_rules + party_battle: field follow; with spawned enemies
 * kh2_party_tick runs the battle part: target choice of the member's style (0..3), action records,
 * attack motions, weapons, damage reported as kh2_enemy_event hits; enemies hit the members (HP from
 * the new game's plrp rows, status from lvup at the member's level), knock-out and the revive timer,
 * the danger test, emergency pass and HP/MP item handlers from the host's item stock; since 0.6 the
 * members' own BDX scripts run (kh2-rs friend_host): Specials (Fire, Thunder, Blade, Tornado ...) and
 * Cure switch the script's mode, its attack objects hit enemies, its heals heal; during a limit the
 * limit's motions play on the members' bodies. Not ported: equipment, the scripts' effects (reported).

### `kh2_member_state`

```c
typedef struct kh2_member_state {
    float pos[3];
    float yaw;
    int32_t member;       /* 0 Donald, 1 Goofy */
    int32_t motion_slot;
    float frame;
    int32_t hp, max_hp, mp, max_mp;
    int32_t ko;           /* 1 while knocked out (the revive timer runs) */
    int32_t level, style;
    int32_t strength, magic, defense;
} kh2_member_state;
```

- `KH2_MEMBER_HIT` = `1` — id = enemy id, atkp, damage, hp after
- `KH2_MEMBER_KO` = `2`
- `KH2_MEMBER_REVIVED` = `3` — hp after
- `KH2_MEMBER_ITEM` = `4` — id = item, atkp = target (0 Sora, 1 the member, 2 the other member)
- `KH2_MEMBER_LEVEL_UP` = `5` — hp = the new level, id = ability of the row (the members level with Sora's EXP in kh2_party_tick)
- `KH2_MEMBER_BONUS` = `6` — id = bonus id, damage = max HP added, hp = max MP added, atkp = item
- `KH2_MEMBER_SCRIPT_MODE` = `7` — the member's own script switched mode: text = its name ("cure", "blade", ...)
- `KH2_MEMBER_EFFECT` = `8` — the script started element id of the member's effect file
- `KH2_MEMBER_HEALED` = `9` — a script heal (Cure): atkp = target (0 Sora, 1 the member, 2 the other), damage = amount

### `kh2_member_event`

```c
typedef struct kh2_member_event {
    int32_t slot, kind, id, atkp, damage, hp;
    char text[32];
} kh2_member_event;
```

### `kh2_poll_member_event`

```c
int32_t kh2_poll_member_event(int32_t handle, kh2_member_event *out);
```

1 written, 0 none left.

### `kh2_party_set_level`

```c
int32_t kh2_party_set_level(int32_t handle, int32_t slot, int32_t level);
```

Level 1..99 (status = lvup row + the new game's bytes).

### `kh2_party_set_vitals`

```c
int32_t kh2_party_set_vitals(int32_t handle, int32_t slot, int32_t hp, int32_t max_hp, int32_t mp, int32_t max_mp);
```

HP/MP and maxima; HP 0 knocks out, HP > 0 on a KO'd member revives (scaled as the game does).

### `kh2_party_set_style`

```c
int32_t kh2_party_set_style(int32_t handle, int32_t slot, int32_t style);
```

Battle style 0..3 (setting byte 0).

### `kh2_party_set_items`

```c
int32_t kh2_party_set_items(int32_t handle, int32_t slot, const uint16_t *ids, const uint8_t *counts, uint32_t count);
```

The member's battle items: `count` ids with their counts (the HP/MP item handlers spend them).

### `kh2_party_add`

```c
int32_t kh2_party_add(int32_t handle, int32_t member);
```

Adds Donald (0) or Goofy (1) following Sora; returns the member's slot 0/1 or -1.

### `kh2_party_tick`

```c
int32_t kh2_party_tick(int32_t handle);
```

One tick of the followers (after kh2_tick).

### `kh2_party_state`

```c
int32_t kh2_party_state(int32_t handle, int32_t slot, kh2_member_state *out);
```

### `kh2_party_bones`

```c
int32_t kh2_party_bones(int32_t handle, int32_t slot, int32_t space, float *out, uint32_t max_bones);
```

## Limits (kh2-rs limit.rs rules + limit_script.rs: Sora's limit commands with Donald and Goofy)
 * The party is Sora plus the members who follow and stand. kh2_limit_start spends the MP, starts the
 * countdown (the command's Time, times Combination Boost) and the limit file's own script, whose
 * requests come back as limit events; the countdown and script run in kh2_party_tick, enemy hits during
 * the limit count. Not ported: the participants' own threads (the members' scripts play the attacks),
 * summon limits, the HUD; the limit's motions are reported, not played.

### `kh2_limit_state`

```c
typedef struct kh2_limit_state {
    int32_t command;      /* running command, 0 none */
    int32_t ticks_left;
    int32_t hits;
} kh2_limit_state;
```

- `KH2_LIMIT_STARTED` = `1` — a = command, b = time, text = file
- `KH2_LIMIT_ENDED` = `2` — a = command, b = hits
- `KH2_LIMIT_MOTION` = `3` — a = participant handle, b = LIMI motion id, blend
- `KH2_LIMIT_EFFECT` = `4` — a = participant handle, b = PAX element
- `KH2_LIMIT_NAMED` = `5` — text = event name
- `KH2_LIMIT_STOPPED` = `6` — the script thread ended; text = why

### `kh2_limit_event`

```c
typedef struct kh2_limit_event {
    int32_t kind, a, b;
    float blend;
    char text[32];
} kh2_limit_event;
```

### `kh2_limit_status`

```c
int32_t kh2_limit_status(int32_t handle, uint16_t *cmds, int32_t *status, uint32_t max);
```

Writes up to max command ids and status codes (0 available, 1 wrong world, 2 locked, 3 member absent, 4 ability missing, 5 no MP, 6 summon gauge, 7 a limit runs); returns the command count.

### `kh2_limit_start`

```c
int32_t kh2_limit_start(int32_t handle, int32_t cmd);
```

0, or minus the status code. A sub-command of the running limit goes to its script (event 23).

### `kh2_limit_set_world`

```c
int32_t kh2_limit_set_world(int32_t handle, int32_t world);
```

### `kh2_limit_state`

```c
int32_t kh2_limit_state(int32_t handle, kh2_limit_state *out);
```

### `kh2_poll_limit_event`

```c
int32_t kh2_poll_limit_event(int32_t handle, kh2_limit_event *out);
```

## Summons (kh2-rs summon.rs flow + drive_rules: Stitch, Genie, Chicken Little, Peter Pan)
 * The host says which charms are owned; kh2_summon spends the drive points, Donald and Goofy go
 * away, after the arrival wait the summon object appears at Sora's side with its own script
 * ("appear_skip"), the summon timer (gauge mode 3) drains with Summon Boost and feeds the summon
 * level; at 0 or on Dismiss its script plays "leave", then the companions come back. Ticked by
 * kh2_party_tick. Not ported: the summons' class AI (Stitch's moves and blast, Genie's records ...),
 * the arrival event, summon limits, Genie's Drive!.

- `KH2_SUMMON_DISMISS` = `0x13c`

### `kh2_summon_state`

```c
typedef struct kh2_summon_state {
    int32_t state;        /* 0 none, 1 arrival event, 2 out, 3 leaving */
    int32_t index;        /* 1-based sumn row */
    int32_t arriving;
    float timer, timer_total;
    int32_t segments, level, exp;
    int32_t object;       /* 1 when the object is out */
    float pos[3];
    float yaw;
    int32_t motion_slot;
    int32_t companions_away;
} kh2_summon_state;
```

### `kh2_summon_event`

```c
typedef struct kh2_summon_event {
    int32_t kind;         /* 1 started (value = command), 2 arrived (value = segments, timer, text = model),
                             3 script mode (text), 4 level up (value), 5 leaving (value = dismissed), 6 gone */
    int32_t value;
    float timer;
    char text[32];
} kh2_summon_event;
```

### `kh2_summon_set_owned`

```c
int32_t kh2_summon_set_owned(int32_t handle, const uint16_t *items, uint32_t count);
```

### `kh2_summon_status`

```c
int32_t kh2_summon_status(int32_t handle, uint16_t *cmds, int32_t *status, uint32_t max);
```

Writes up to max command ids and status codes (0 available, 1 not a summon, 2 charm not owned, 3 busy, 4 party missing, 5 in a form, 6 not enough drive points, 7 airborne); returns the count.

### `kh2_summon`

```c
int32_t kh2_summon(int32_t handle, int32_t cmd);
```

0, or minus the status code; KH2_SUMMON_DISMISS dismisses.

### `kh2_summon_state`

```c
int32_t kh2_summon_state(int32_t handle, kh2_summon_state *out);
```

### `kh2_summon_bones`

```c
int32_t kh2_summon_bones(int32_t handle, int32_t space, float *out, uint32_t max_bones);
```

### `kh2_poll_summon_event`

```c
int32_t kh2_poll_summon_event(int32_t handle, kh2_summon_event *out);
```

## Enemies

their own BDX scripts through kh2-rs's interpreter and script host

### `kh2_enemy_spawn`

```c
int32_t kh2_enemy_spawn(int32_t handle, const char *kind, float x, float y, float z, float yaw);
```

Spawns an enemy by model ("M_EX020" Shadow, "M_EX010" Soldier, "M_EX990" Dusk, ...) fighting Sora `handle`; returns its id (> 0) or -1.

### `kh2_area_spawn`

```c
int32_t kh2_area_spawn(int32_t handle, const char *area, int32_t program, int32_t with_disabled);
```

Spawns the enemies of an area's battle program (`program` = the btl program id, -1 every program; kh2-rs area_groups: the area file's spawn records with their spawn kinds and parameters). Records waiting for a signal are skipped unless with_disabled. Returns the number spawned (at most 256 enemies per Sora). The areas' signals, waves and triggers are not run: the host decides.

### `kh2_enemies_prune`

```c
int32_t kh2_enemies_prune(int32_t handle);
```

Drops the enemies that are gone (indices shift) and frees the enemy kinds no enemy uses (kinds load on demand at their first spawn). Returns the number dropped.

### `kh2_enemies_clear`

```c
int32_t kh2_enemies_clear(int32_t handle);
```

Removes every enemy of Sora `handle` and frees the kinds no other enemies use.

### `kh2_enemies_tick`

```c
int32_t kh2_enemies_tick(int32_t handle);
```

One tick of the enemies (after kh2_tick): scripts, movement, reactions, hits both ways.

### `kh2_enemy_count`

```c
int32_t kh2_enemy_count(int32_t handle);
```

### `kh2_enemy_state`

```c
typedef struct kh2_enemy_state {
    int32_t id;
    float pos[3];
    float yaw;
    int32_t hp, max_hp;
    int32_t state;        /* 0 idle (script), 1 hurt, 2 blown */
    int32_t motion_slot;
    float frame;
    int32_t grounded;
    float alpha;          /* the script's fade */
    int32_t enmp;
} kh2_enemy_state;
```

### `kh2_enemy_state`

```c
int32_t kh2_enemy_state(int32_t handle, int32_t index, kh2_enemy_state *out);
```

### `kh2_enemy_bones`

```c
int32_t kh2_enemy_bones(int32_t handle, int32_t index, int32_t space, float *out, uint32_t max_bones);
```

- `KH2_EN_HIT` = `1` — Sora hit it: atkp, damage, value = HP left
- `KH2_EN_HIT_SORA` = `2` — it hit Sora: atkp, damage, value = kh2_hit_taken result
- `KH2_EN_DEFEATED` = `3` — enmp, lvpm/enmp experience, value = EXP paid to Sora
- `KH2_EN_EFFECT` = `4` — value = element of its effect file
- `KH2_EN_DROP` = `5` — value = przt row, prizes per kind
- `KH2_EN_REMOVED` = `6`
- `KH2_EN_REACTION` = `7` — an accepted reaction command reached its script: atkp = command, value = handled

### `kh2_enemy_event`

```c
typedef struct kh2_enemy_event {
    int32_t kind, id, atkp, damage, value, enmp, lvpm_experience, enmp_experience;
    uint32_t prizes[9];
} kh2_enemy_event;
```

### `kh2_poll_enemy_event`

```c
int32_t kh2_poll_enemy_event(int32_t handle, kh2_enemy_event *out);
```

---- Reaction commands (kh2-rs reaction_cmd + reaction_parts: the objects' reaction parts touching Sora's body volume (the Soldier's command 34) and range trigger 20 offers, cmd row status, generic accept) The library fills slot 1 each kh2_enemies_tick; the host shows the prompt and calls accept on its button. The target script gets event 11 on the next tick (KH2_EN_REACTION); what it then asks of Sora (the paired motion from the enemy's set, KH2_STATE_REACTION, its velocity, a queued motion, the unhittable bit) runs on Sora's next ticks. Not ported: limits through this slot (Auto Limit), talk/save/shop rows. kh2_reaction_force_offer is kh2-rs's harness offer.

- `KH2_REACT_AVAILABLE` = `0`
- `KH2_REACT_BLOCKED` = `1`
- `KH2_REACT_TOO_FAR` = `2`
- `KH2_REACT_NO_MP` = `3`
- `KH2_REACT_NOT_PORTED` = `4`

### `kh2_reaction_offer`

```c
typedef struct kh2_reaction_offer {
    int32_t cmd;      /* cmd row id */
    int32_t from;     /* offering enemy id */
    int32_t status;   /* KH2_REACT_* */
    uint32_t message; /* the prompt's message id */
} kh2_reaction_offer;
```

### `kh2_reaction_offer`

```c
int32_t kh2_reaction_offer(int32_t handle, kh2_reaction_offer *out);
```

1 written, 0 nothing offered.

### `kh2_reaction_accept`

```c
int32_t kh2_reaction_accept(int32_t handle);
```

Returns the accepted command id, or -1 (nothing available offered).

### `kh2_reaction_force_offer`

```c
int32_t kh2_reaction_force_offer(int32_t handle, int32_t cmd);
```

Every living enemy offers `cmd` (0 stops).

## Field camera helper

kh2-rs camera_rules, oracle-tested there

- `KH2_CAM_RESET` = `1u` — R3: behind Sora
- `KH2_CAM_SELECT` = `2u` — look-around
- `KH2_CAM_MANUAL` = `4u` — no auto-camera (R2 held)

### `kh2_cam_input`

```c
typedef struct kh2_cam_input {
    float stick_x, stick_y; /* right stick, x right, y up */
    float zoom;             /* units added to the radius (mouse wheel) */
    uint32_t buttons;       /* KH2_CAM_* pressed this tick */
    float step;             /* 1 per 60 Hz tick (0 = 1), 2 per 30 Hz frame */
} kh2_cam_input;
```

### `kh2_cam_view`

```c
typedef struct kh2_cam_view {
    float pos[3], look[3];
    float fov;              /* horizontal, radians, for a 4:3 picture */
    float roll, yaw;
    int32_t mode;           /* 0 follow, 1 look-around */
} kh2_cam_view;
```

### `kh2_camera_tick`

```c
int32_t kh2_camera_tick(int32_t handle, const kh2_cam_input *input, kh2_cam_view *view);
```

One camera tick after kh2_tick: follows Sora, locks onto kh2_set_target's point, collides with the static collision. Use look - pos as the next kh2_inputs camera look direction.

### `kh2_camera_reset`

```c
int32_t kh2_camera_reset(int32_t handle, int32_t row);
```

New camera behind Sora with table row 0..3 (0 field, 1 indoor).

### `kh2_lock_on_pick`

```c
int32_t kh2_lock_on_pick(int32_t handle, const float *points, uint32_t count);
```

Index of the nearest of `count` xyz points within the lock-on range, or -1.
