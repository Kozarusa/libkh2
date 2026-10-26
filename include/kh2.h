/*
 * libkh2: Sora's controller from Kingdom Hearts II as an embeddable library (libsm64-style).
 *
 * The host owns the world and the drawing; libkh2 owns Sora. Call kh2_init once with the path of
 * the user's own disc image, load collision triangles, create a Sora, then call kh2_tick once per
 * 60 Hz logic tick (or kh2_tick_field per 30 Hz frame) with the input and read back the state.
 *
 * Units: kh2 units (centimetres), right-handed, Y up. Sora's yaw 0 faces +Z; forward is
 * (sin yaw, 0, cos yaw). Matrices are 16 floats, column-major.
 *
 * Every function returns -1 (or a negative value) on error; kh2_last_error explains it.
 * The library keeps one global state (like libsm64) guarded by a mutex: every call is safe from any
 * thread and calls are serialised (no parallel ticks). Pointers passed in are only read during the
 * call. No call panics or aborts through the ABI: internal errors return -1 with a message.
 */
#ifndef KH2_H
#define KH2_H

#include <stdint.h>

/* The version this header describes; kh2_version() returns the library's (same packing). */
#define KH2_VERSION_MAJOR 2
#define KH2_VERSION_MINOR 4
#define KH2_VERSION_PATCH 0
#define KH2_VERSION ((KH2_VERSION_MAJOR << 16) | (KH2_VERSION_MINOR << 8) | KH2_VERSION_PATCH)

#ifdef _WIN32
#define KH2_API __declspec(dllimport)
#else
#define KH2_API
#endif

#ifdef __cplusplus
extern "C" {
#endif

/* Surface attribute: the COCT PARTY_COLLIDE bit. A wall-like triangle (normal more than 60 degrees
 * from up) stops Sora when it carries it (and its low nibble is not 4, bit 0x100000 clear). Floors
 * and ceilings are found by their normal whatever the flags. */
#define KH2_SURF_WALL 0x10u

/* One collision triangle. The face normal (v[1]-v[0]) x (v[2]-v[0]) must point to the side Sora
 * stands on: up for floors, toward the walkable side for walls (walls are one-sided). */
typedef struct kh2_surface {
    float v[3][3];
    uint32_t flags;    /* COCT attribute word, e.g. KH2_SURF_WALL for a solid wall */
    uint8_t group;     /* mesh group 0..63, switched by kh2_set_collision */
    uint8_t material;  /* ground material (footstep kind), reported back in kh2_state */
    uint16_t _pad;
} kh2_surface;

#define KH2_BTN_ATTACK 1u /* X: attack */
#define KH2_BTN_JUMP   2u /* Circle: jump (hold for the full height) */
#define KH2_BTN_GUARD  4u /* Square: guard */

typedef struct kh2_inputs {
    float stick_x, stick_y;       /* left stick: x right, y forward (away from the camera), length 0..1 */
    float cam_look_x, cam_look_z; /* the camera's horizontal look direction in world space */
    uint32_t buttons;             /* KH2_BTN_* held this tick; presses are derived from the edges */
} kh2_inputs;

/* kh2_state.state */
#define KH2_STATE_GROUND 0
#define KH2_STATE_AIR 1
#define KH2_STATE_ATTACK 2
#define KH2_STATE_AIR_ATTACK 3
#define KH2_STATE_GUARD 4
#define KH2_STATE_QUICK_RUN 5
#define KH2_STATE_GLIDE 6
#define KH2_STATE_HURT 7
#define KH2_STATE_BLOWN 8
#define KH2_STATE_DOWN 9      /* knocked out: kh2_revive */
#define KH2_STATE_COUNTER 10  /* the guard repelled a hit */
#define KH2_STATE_MAGIC 11    /* a cast motion plays */
#define KH2_STATE_FORM_CHANGE 12 /* a drive change motion plays */
#define KH2_STATE_REACTION 13    /* a reaction command's paired motion plays (no input) */

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

/* A live attack capsule in world units: segment a-b, radius. */
typedef struct kh2_volume {
    float a[3];
    float b[3];
    float radius;
    int32_t attack_id;
    int32_t group;
} kh2_volume;

/* Opens the disc image (NULL: the KH2_ISO environment variable) and loads Sora's model, motion
 * set, keyblade, attack table and movement parameters. Nothing is copied out of the disc. */
KH2_API int32_t kh2_init(const char *iso_path);
/* The library version, (major << 16) | (minor << 8) | patch; compare with KH2_VERSION. */
KH2_API uint32_t kh2_version(void);
/* Frees every Sora, HUD, audio engine and the disc; kh2_init may be called again. Safe without kh2_init. */
KH2_API void kh2_terminate(void);
/* The last error message; returns its full length. */
KH2_API uint32_t kh2_last_error(char *buf, uint32_t len);

/* Replaces the static collision. Returns the triangle count kept (degenerate ones are dropped). */
KH2_API int32_t kh2_surfaces_load(const kh2_surface *tris, uint32_t count);
/* Replaces the static collision with an area's own collision from the disc, e.g. "tt00". */
KH2_API int32_t kh2_surfaces_load_map(const char *area);
/* Switches mesh group `group` (0..63) solid or not. */
KH2_API void kh2_set_collision(uint32_t group, int32_t solid);

/* Creates a Sora at (x, y, z); settles on a floor within a step below. Returns a handle. */
KH2_API int32_t kh2_create(float x, float y, float z);
/* Frees Sora `handle` and everything attached to it (party, enemies, HUD, limit, summon); the handle becomes invalid. */
KH2_API void kh2_delete(int32_t handle);
/* Warps Sora (velocity and jump cleared). */
KH2_API int32_t kh2_set_position(int32_t handle, float x, float y, float z, float yaw);

/* One 60 Hz logic tick. `state` may be NULL. */
KH2_API int32_t kh2_tick(int32_t handle, const kh2_inputs *inputs, kh2_state *state);
/* Two ticks with the same input: one 30 Hz field frame. */
KH2_API int32_t kh2_tick_field(int32_t handle, const kh2_inputs *inputs, kh2_state *state);
/* Writes Sora's state (position cm, velocity cm/tick, yaw rad, state, motion) to `*state` (caller-owned). 0, or -1 for a bad handle / NULL. */
KH2_API int32_t kh2_get_state(int32_t handle, kh2_state *state);

/* The host's lock-on target point (an enemy's target-shape centre, radius, half height). It feeds
 * the attack table's range tests and the attack's lunge, turn and leap. */
KH2_API int32_t kh2_set_target(int32_t handle, float x, float y, float z, float radius, float half_height);
/* Clears the lock-on target. 0, or -1 for a bad handle. */
KH2_API int32_t kh2_clear_target(int32_t handle);
/* The host found that the current swing touched something: lets the combo chain go on. */
KH2_API int32_t kh2_report_hit(int32_t handle);

/* A hit taken: atkp = attack row id of the disc's atkp table (its reaction class decides flinch,
 * push or blow), damage = HP taken (the host computes it), dir = from the attacker toward Sora.
 * Returns 0 ignored (invincible / down), 1 guarded, 2 no flinch, 3 flinch, 4 blown, -1 unknown row. */
KH2_API int32_t kh2_hit_taken(int32_t handle, int32_t atkp, int32_t damage, float dir_x, float dir_y, float dir_z);
/* Sets HP and maximum HP (clamped to 0..=max). 0, or -1 for a bad handle. */
KH2_API int32_t kh2_set_hp(int32_t handle, int32_t hp, int32_t max_hp);
/* Stands up from KH2_STATE_DOWN with full HP; auto != 0: stand up by itself after 180 ticks. */
KH2_API int32_t kh2_revive(int32_t handle, int32_t automatic);

/* Ability bits 0..127 (bit 1 = Guard) and combo lengths; defaults Guard, 3, 3, 1. */
KH2_API int32_t kh2_set_abilities(int32_t handle, uint64_t lo, uint64_t hi, uint8_t max_ground, uint8_t max_air, uint8_t max_finishers);
/* The equipped abilities (ability item ids, e.g. 82 Guard, 162 Combo Plus, 163 Air Combo Plus,
 * 393 Finishing Plus, 394 Negative Combo, 395 Berserk Charge; growth abilities by their items).
 * The block they give sets the attack table's bits, combo lengths and growth levels; Second
 * Chance / Once More, Leaf Bracer and Berserk Charge apply. Replaces kh2_set_abilities/_growth. */
KH2_API int32_t kh2_set_loadout(int32_t handle, const uint16_t *ids, uint32_t count);
/* out[7]: ground combo, air combo, finishers, High Jump, Quick Run, Aerial Dodge, Glide. */
KH2_API int32_t kh2_get_loadout_block(int32_t handle, int32_t out[7]);
/* High Jump level 0..5. */
KH2_API int32_t kh2_set_high_jump(int32_t handle, uint8_t level);
/* Growth abilities 0..5 (fmab rows): High Jump, Quick Run (Square while moving), Aerial Dodge
 * (jump in the air), Glide (hold Square while falling). */
KH2_API int32_t kh2_set_growth(int32_t handle, uint8_t high_jump, uint8_t quick_run, uint8_t aerial_dodge, uint8_t glide);
/* Final Mix's Dodge Roll level (0..5, growth group 4): moving + a Square tap rolls (motion A180,
 * AirSlideStarTime of invincibility); held past 10 ticks it becomes the Quick Run when learned. */
KH2_API int32_t kh2_set_dodge_roll(int32_t handle, uint8_t level);
/* The difficulty: 0 Beginner, 1 Standard (default), 2 Proud, 3 Critical (the Final Mix damage routine's
 * step: party members take 50 / - / 200 / 200 % and deal 150 / - / 100 / 125 %). Process-wide; -1 above 3. */
KH2_API int32_t kh2_set_difficulty(int32_t difficulty);
/* Skeleton evaluation each tick (default on). Off: no bones, no attack volumes. */
KH2_API int32_t kh2_set_pose_enabled(int32_t handle, int32_t on);

/* Skeletons: which = 0 body, 1 keyblade (hangs from body bone 178), 2 a drive form's second
 * keyblade (bone 86; Valor, Master, Final). */
KH2_API int32_t kh2_bone_count(int32_t handle, int32_t which);
/* space = 0: model space (body: relative to Sora's root, the body transform is pos + yaw;
 * keyblade: relative to the hand bone); 1: world space. 16 floats per bone, column-major. */
KH2_API int32_t kh2_get_bones(int32_t handle, int32_t which, int32_t space, float *out, uint32_t max_bones);
/* Rest pose (bind matrices, model space). */
KH2_API int32_t kh2_get_rest_bones(int32_t handle, int32_t which, float *out, uint32_t max_bones);
/* Writes the parent index of each bone (-1 for the root) of body `which` (0 Sora, 1 keyblade, 2 second keyblade) to `out` (caller-owned, `max_bones` long). Returns the bone count, or -1. */
KH2_API int32_t kh2_bone_parents(int32_t handle, int32_t which, int32_t *out, uint32_t max_bones);

/* The live attack capsules (world space). Returns the count written. */
KH2_API int32_t kh2_get_attack_volumes(int32_t handle, kh2_volume *out, uint32_t max);
/* The playing motion's tag, e.g. "A300". Returns its length. */
KH2_API int32_t kh2_motion_tag(int32_t handle, char *buf, uint32_t len);
/* Sora's jump height read from the disc (prty JumpHeight, or the High Jump row). */
KH2_API float kh2_jump_height(int32_t handle);

/* ---- Magic (kh2-rs magic_rules) ---- */
/* Asks for a cast of slot 0 Fire, 1 Blizzard, 2 Thunder, 3 Cure, 4 Magnet, 5 Reflect; tried for up
 * to 30 ticks until the state and MP allow it. The host spawns the spell on KH2_EV_SPELL_FIRED. */
KH2_API int32_t kh2_cast(int32_t handle, int32_t slot);
/* Sets the six magic levels (Fire, Blizzard, Thunder, Cure, Magnet, Reflect; 0 = not owned, 1..3). 0, or -1. */
KH2_API int32_t kh2_set_magic_levels(int32_t handle, const uint8_t levels[6]);
/* Sets MP and maximum MP (clamped to 0..=max). 0, or -1. */
KH2_API int32_t kh2_set_mp(int32_t handle, int32_t mp, int32_t max_mp);

#define KH2_EV_CAST_STARTED 1   /* slot, cast_kind, motion, voice */
#define KH2_EV_SPELL_FIRED 2    /* frame trigger 17: MP paid, spawn the spell (charge: MP charge began) */
#define KH2_EV_CAST_CANCELLED 3 /* the motion changed before the trigger: no spell, no cost */
#define KH2_EV_MP_RESTORED 4    /* the MP charge ended */
#define KH2_EV_FORM_CHANGED 5   /* slot = new form (0 base), cast_kind = old form */
#define KH2_EV_SPELL_ATTACK 6   /* slot, stage, atkp/sub_id row, radius/height/life/speed, pos, dir */
#define KH2_EV_HEALED 7         /* amount, hp */
#define KH2_EV_ITEM_USED 8      /* slot = item id, amount = HP change, hp = MP change, charge = drive change (1/100 points) */
#define KH2_EV_EXPERIENCE 9     /* amount paid, hp = total */
#define KH2_EV_BONUS 11         /* slot = bonus id, amount = max HP added, hp = max MP added, charge = drive gauge added, sub_id / atkp = items */
#define KH2_EV_LEVEL_UP 10      /* amount = the new level; rises over the previous lvup row: atkp = strength,
                                   stage = magic, slot = defense, charge = AP; sub_id = ability given (0 none) */
typedef struct kh2_event {
    int32_t kind, slot, cast_kind, motion, voice, charge;
    int32_t stage, atkp, sub_id;  /* spell attack: stage and its atkp row (for kh2_damage) */
    float radius, height;         /* the stage's size (magi row) */
    float life, speed;            /* ticks the stage lasts (0: one shot), units per tick */
    float pos[3], dir[3];         /* where it starts and which way it goes */
    int32_t amount, hp;           /* healed */
} kh2_event;
/* The damage of one of Sora's attack rows (sub_id -1: first variant) against a target's defense,
 * max HP (type 5 rows) and element / general percents (100 neutral). -1: unknown row. */
KH2_API int32_t kh2_damage(int32_t handle, int32_t atkp, int32_t sub_id, int32_t defense, int32_t max_hp, int32_t weakness, int32_t general);
/* Uses an item (item id: 1 Potion, 2 Hi-Potion, 3 Ether, 4 Elixir, ...): the use motion plays and
 * the effect lands 30 frames after its trigger (KH2_EV_ITEM_USED). The host keeps the inventory. */
KH2_API int32_t kh2_use_item(int32_t handle, int32_t item);
/* A defeat: the lvpm row's Experience for the world level and the enemy's enmp Experience. Returns
 * the EXP paid; KH2_EV_LEVEL_UP follows each level reached. */
KH2_API int32_t kh2_report_defeat(int32_t handle, int32_t lvpm_experience, int32_t enmp_experience);
/* Level 1..99: strength, magic, defense from the disc's lvup row plus the Kingdom Key. */
KH2_API int32_t kh2_set_level(int32_t handle, int32_t level);
/* The weapon chosen at the start (0 Sword, 1 Shield, 2 Staff): the lvup ability column level-ups give. */
KH2_API int32_t kh2_set_weapon_set(int32_t handle, int32_t set);
/* Bonus reward `id` (bons rows, once per id): Sora's max HP / MP / drive gauge and items (KH2_EV_BONUS),
 * the following members' max HP / MP (KH2_MEMBER_BONUS). 1 given, 0 given before, -1 error. */
KH2_API int32_t kh2_give_bonus(int32_t handle, int32_t id);
/* ---- Items and equipment (kh2-rs GameData::sora_status / member_status: item_stat rows) ----
 * The inventory is the host's; the library describes items and applies equipment: every equipped
 * item's item_stat row adds attack, magic, defense and AP, scales the resistances, and its ability
 * joins the loadout. Sora's equipment is saved in the record. */
typedef struct kh2_item_info {
    uint16_t id;
    uint8_t kind;         /* item.Type: 2 keyblade, 3 staff, 4 shield, 19 ability ... */
    uint8_t ap;
    uint16_t command, name, description; /* cmd row, message ids */
    uint16_t amount;      /* Flag1 | Rank << 8 */
    uint16_t shop_buy, shop_sell;
    uint8_t attack, magic, defense;
    uint8_t resist[7];    /* physical, fire, blizzard, thunder, dark, light, general (percent) */
    uint16_t ability;     /* the ability the item carries, 0 none */
} kh2_item_info_t;
/* 0, or -1 for no such item. */
KH2_API int32_t kh2_item_info(int32_t id, kh2_item_info_t *out);
/* Sora's equipment (item ids: weapon, armor, accessories; up to 16). Default: the Kingdom Key. */
KH2_API int32_t kh2_set_equipment(int32_t handle, const uint16_t *ids, uint32_t count);
/* Sets party member `slot`'s equipment to `count` item ids (`ids` caller-owned); its status is rebuilt. 0, or -1. */
KH2_API int32_t kh2_party_set_equipment(int32_t handle, int32_t slot, const uint16_t *ids, uint32_t count);
/* Drive form `form` (1..5): its ability list at its level (fmlv grants + growth ability). */
KH2_API int32_t kh2_form_abilities(int32_t handle, int32_t form, uint16_t *out, uint32_t max);
/* Abilities given by level-ups, oldest first: writes up to max, returns the count (equip: kh2_set_loadout). */
KH2_API int32_t kh2_granted_abilities(int32_t handle, uint16_t *out, uint32_t max);
/* Save record ("KH2S" v3): level, EXP, HP/MP, AP, spell levels, weapon set, given and equipped
 * abilities, drive gauge/forms/levels, position and yaw. Not saved: motion, combo, cast, an active
 * form, enemies, party. save writes when cap is enough and returns the size; load returns 0, or -1
 * with nothing changed for a record that is not valid. */
KH2_API int32_t kh2_state_save(int32_t handle, uint8_t *out, uint32_t cap);
/* Restores a record written by kh2_state_save (`bytes`, `len`, caller-owned; format 4 on Final Mix). 0, or -1 for a damaged or older record. */
KH2_API int32_t kh2_state_load(int32_t handle, const uint8_t *bytes, uint32_t len);
/* Takes the oldest event: 1 written, 0 none left. */
KH2_API int32_t kh2_poll_event(int32_t handle, kh2_event *out);

/* ---- Drive forms (kh2-rs shared::drive::rules) ---- */
/* Gauge points / maximum, owned forms (bit n = form n; Final Mix: 1 Valor, 2 Wisdom, 3 Limit, 4 Master,
 * 5 Final, 6 Anti is never owned), form levels by form id (7 bytes, may be NULL). */
KH2_API int32_t kh2_drive_setup(int32_t handle, uint8_t points, uint8_t max_points, uint32_t owned_mask, const uint8_t *levels);
/* Adds `percent` of one drive point to the gauge (100 = one point), outside a form. 0, or -1. */
KH2_API int32_t kh2_drive_gain(int32_t handle, int32_t percent);
/* A landed hit with its atkp row: the combo latch plus the row's DriveDrain. */
KH2_API int32_t kh2_report_hit_atkp(int32_t handle, int32_t atkp);
/* Enter form 1..4 (the Anti / Final roll may pick another): returns the form entered or -1. The
 * body swaps when the change motion ends (KH2_EV_FORM_CHANGED); bones change with it. */
KH2_API int32_t kh2_drive_enter(int32_t handle, int32_t form);
/* Reverts to base Sora as the Revert command does: 0, or -1 when refused (Anti form in battle, a change running) or a bad handle. */
KH2_API int32_t kh2_drive_revert(int32_t handle);
/* objentry ids of the form's body and hand 0 / hand 1 weapons (-1 none). */
KH2_API int32_t kh2_drive_objects(int32_t handle, int32_t out[3]);
/* The keyblade item the second hand takes on the next entry (default 41, the Kingdom Key). */
KH2_API int32_t kh2_drive_set_second_keyblade(int32_t handle, int32_t item);
/* The damage Sora deals of attack type `atk_type` after the current form's rule (Anti's factor). Returns the damage, or -1. */
KH2_API int32_t kh2_drive_outgoing_damage(int32_t handle, int32_t atk_type, int32_t damage);

/* ---- Animation export (any slot, independent of what Sora plays; see docs/SKINNING.md) ---- */
typedef struct kh2_motion_desc {
    float frame_start, frame_end, frame_return;
    float fps;              /* the animator steps fps / 60 frames per 60 Hz tick */
    int32_t has_root_motion;
    char tag[8];            /* e.g. "A300" */
} kh2_motion_desc;
/* The number of motion slots of body `which` (0 Sora, 1 keyblade, 2 second keyblade), or -1. */
KH2_API int32_t kh2_motion_slot_count(int32_t handle, int32_t which);
/* variant: 0 battle + weapon, 1 battle, 2 field, 3 field + weapon (the game's fallback applies). */
KH2_API int32_t kh2_motion_slot(int32_t handle, int32_t which, int32_t motion_id, int32_t variant);
/* Writes slot `slot`'s frame range, rate and tag (body `which`) to `*out` (caller-owned). 0, or -1 for an empty slot. */
KH2_API int32_t kh2_motion_info(int32_t handle, int32_t which, int32_t slot, kh2_motion_desc *out);
/* Bone matrices of `slot` at `frame` relative to the motion's root; root = (x, y, z, yaw) or NULL. */
KH2_API int32_t kh2_sample_motion(int32_t handle, int32_t which, int32_t slot, float frame, float *out, uint32_t max_bones, float *root);

/* ---- Party members (kh2-rs party_rules + party_battle: field follow; with spawned enemies
 * kh2_party_tick runs the battle part: target choice of the member's style (0..3), action records,
 * attack motions, weapons, damage reported as kh2_enemy_event hits; enemies hit the members (HP from
 * the new game's plrp rows, status from lvup at the member's level), knock-out and the revive timer,
 * the danger test, emergency pass and HP/MP item handlers from the host's item stock; since 0.6 the
 * members' own BDX scripts run (kh2-rs friend_host): Specials (Fire, Thunder, Blade, Tornado ...) and
 * Cure switch the script's mode, its attack objects hit enemies, its heals heal; during a limit the
 * limit's motions play on the members' bodies. Not ported: equipment, the scripts' effects (reported). ---- */
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
#define KH2_MEMBER_HIT 1      /* id = enemy id, atkp, damage, hp after */
#define KH2_MEMBER_KO 2
#define KH2_MEMBER_REVIVED 3  /* hp after */
#define KH2_MEMBER_ITEM 4     /* id = item, atkp = target (0 Sora, 1 the member, 2 the other member) */
#define KH2_MEMBER_LEVEL_UP 5 /* hp = the new level, id = ability of the row (the members level with Sora's EXP in kh2_party_tick) */
#define KH2_MEMBER_BONUS 6    /* id = bonus id, damage = max HP added, hp = max MP added, atkp = item */
#define KH2_MEMBER_SCRIPT_MODE 7 /* the member's own script switched mode: text = its name ("cure", "blade", ...) */
#define KH2_MEMBER_EFFECT 8   /* the script started element id of the member's effect file */
#define KH2_MEMBER_HEALED 9   /* a script heal (Cure): atkp = target (0 Sora, 1 the member, 2 the other), damage = amount */
typedef struct kh2_member_event {
    int32_t slot, kind, id, atkp, damage, hp;
    char text[32];
} kh2_member_event;
/* 1 written, 0 none left. */
KH2_API int32_t kh2_poll_member_event(int32_t handle, kh2_member_event *out);
/* Level 1..99 (status = lvup row + the new game's bytes). */
KH2_API int32_t kh2_party_set_level(int32_t handle, int32_t slot, int32_t level);
/* HP/MP and maxima; HP 0 knocks out, HP > 0 on a KO'd member revives (scaled as the game does). */
KH2_API int32_t kh2_party_set_vitals(int32_t handle, int32_t slot, int32_t hp, int32_t max_hp, int32_t mp, int32_t max_mp);
/* Battle style 0..3 (setting byte 0). */
KH2_API int32_t kh2_party_set_style(int32_t handle, int32_t slot, int32_t style);
/* The member's battle items: `count` ids with their counts (the HP/MP item handlers spend them). */
KH2_API int32_t kh2_party_set_items(int32_t handle, int32_t slot, const uint16_t *ids, const uint8_t *counts, uint32_t count);
/* Adds Donald (0) or Goofy (1) following Sora; returns the member's slot 0/1 or -1. */
KH2_API int32_t kh2_party_add(int32_t handle, int32_t member);
/* One tick of the followers (after kh2_tick). */
KH2_API int32_t kh2_party_tick(int32_t handle);
/* Writes party member `slot`'s state (position cm, HP, MP, motion) to `*out`. 0, or -1. */
KH2_API int32_t kh2_party_state(int32_t handle, int32_t slot, kh2_member_state *out);
/* Writes party member `slot`'s bone matrices (16 floats each, `space` 0 model / 1 world) to `out` (caller-owned, `max_bones` matrices). Returns the bone count, or -1. */
KH2_API int32_t kh2_party_bones(int32_t handle, int32_t slot, int32_t space, float *out, uint32_t max_bones);

/* ---- Limits (kh2-rs limit.rs rules + limit_script.rs: Sora's limit commands with Donald and Goofy)
 * The party is Sora plus the members who follow and stand. kh2_limit_start spends the MP, starts the
 * countdown (the command's Time, times Combination Boost) and the limit file's own script, whose
 * requests come back as limit events; the countdown and script run in kh2_party_tick, enemy hits during
 * the limit count. Not ported: the participants' own threads (the members' scripts play the attacks),
 * summon limits, the HUD; the limit's motions are reported, not played. ---- */
typedef struct kh2_limit_state {
    int32_t command;      /* running command, 0 none */
    int32_t ticks_left;
    int32_t hits;
} kh2_limit_state_t;
#define KH2_LIMIT_STARTED 1   /* a = command, b = time, text = file */
#define KH2_LIMIT_ENDED 2     /* a = command, b = hits */
#define KH2_LIMIT_MOTION 3    /* a = participant handle, b = LIMI motion id, blend */
#define KH2_LIMIT_EFFECT 4    /* a = participant handle, b = PAX element */
#define KH2_LIMIT_NAMED 5     /* text = event name */
#define KH2_LIMIT_STOPPED 6   /* the script thread ended; text = why */
typedef struct kh2_limit_event {
    int32_t kind, a, b;
    float blend;
    char text[32];
} kh2_limit_event;
/* Writes up to max command ids and status codes (0 available, 1 wrong world, 2 locked, 3 member
 * absent, 4 ability missing, 5 no MP, 6 summon gauge, 7 a limit runs); returns the command count. */
KH2_API int32_t kh2_limit_status(int32_t handle, uint16_t *cmds, int32_t *status, uint32_t max);
/* 0, or minus the status code. A sub-command of the running limit goes to its script (event 23). */
KH2_API int32_t kh2_limit_start(int32_t handle, int32_t cmd);
/* The world id the limit rows are filtered by (0: only world-free rows). 0, or -1. */
KH2_API int32_t kh2_limit_set_world(int32_t handle, int32_t world);
/* Writes the running limit's state (kh2_limit_state_t) to `*out` (caller-owned). 0, or -1. */
KH2_API int32_t kh2_limit_state(int32_t handle, kh2_limit_state_t *out);
/* Pops the oldest limit event into `*out`: 1 written, 0 none, -1 bad handle. */
KH2_API int32_t kh2_poll_limit_event(int32_t handle, kh2_limit_event *out);

/* ---- Summons (kh2-rs summon.rs flow + drive_rules: Stitch, Genie, Chicken Little, Peter Pan)
 * The host says which charms are owned; kh2_summon spends the drive points, Donald and Goofy go
 * away, after the arrival wait the summon object appears at Sora's side with its own script
 * ("appear_skip"), the summon timer (gauge mode 3) drains with Summon Boost and feeds the summon
 * level; at 0 or on Dismiss its script plays "leave", then the companions come back. Ticked by
 * kh2_party_tick. Not ported: the summons' class AI (Stitch's moves and blast, Genie's records ...),
 * the arrival event, summon limits, Genie's Drive!. ---- */
#define KH2_SUMMON_DISMISS 0x13c
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
} kh2_summon_state_t;
typedef struct kh2_summon_event {
    int32_t kind;         /* 1 started (value = command), 2 arrived (value = segments, timer, text = model),
                             3 script mode (text), 4 level up (value), 5 leaving (value = dismissed), 6 gone,
                             7 class record (value = index), 8 blast hit (value = enemy id), 9 MP refill,
                             10 heal (value = Sora's HP), 11 potion */
    int32_t value;
    float timer;
    char text[32];
} kh2_summon_event;
/* The Summon Effects option: 1 plays the arrival as an event. */
KH2_API int32_t kh2_summon_set_arrival_event(int32_t handle, int32_t on);
/* Sets the owned summon charms (`count` item ids, caller-owned). 0, or -1. */
KH2_API int32_t kh2_summon_set_owned(int32_t handle, const uint16_t *items, uint32_t count);
/* Writes up to max command ids and status codes (0 available, 1 not a summon, 2 charm not owned,
 * 3 busy, 4 party missing, 5 in a form, 6 not enough drive points, 7 airborne); returns the count. */
KH2_API int32_t kh2_summon_status(int32_t handle, uint16_t *cmds, int32_t *status, uint32_t max);
/* 0, or minus the status code; KH2_SUMMON_DISMISS dismisses. */
KH2_API int32_t kh2_summon(int32_t handle, int32_t cmd);
/* Writes the summon's state (who, position cm, gauge) to `*out`. 0, or -1. */
KH2_API int32_t kh2_summon_state(int32_t handle, kh2_summon_state_t *out);
/* Writes the summon's bone matrices (16 floats each, `space` 0 model / 1 world) to `out` (`max_bones` matrices). Returns the count, or -1. */
KH2_API int32_t kh2_summon_bones(int32_t handle, int32_t space, float *out, uint32_t max_bones);
/* Pops the oldest summon event into `*out`: 1 written, 0 none, -1 bad handle. */
KH2_API int32_t kh2_poll_summon_event(int32_t handle, kh2_summon_event *out);

/* ---- Enemies (their own BDX scripts through kh2-rs's interpreter and script host) ---- */
/* Spawns an enemy by model ("M_EX020" Shadow, "M_EX010" Soldier, "M_EX990" Dusk, ...) fighting
 * Sora `handle`; returns its id (> 0) or -1. */
KH2_API int32_t kh2_enemy_spawn(int32_t handle, const char *kind, float x, float y, float z, float yaw);
/* Spawns the enemies of an area's battle program (`program` = the btl program id, -1 every program;
 * kh2-rs area_groups: the area file's spawn records with their spawn kinds and parameters).
 * Records waiting for a signal are skipped unless with_disabled. Returns the number spawned (at most
 * 256 enemies per Sora). The areas' signals, waves and triggers are not run: the host decides. */
KH2_API int32_t kh2_area_spawn(int32_t handle, const char *area, int32_t program, int32_t with_disabled);
/* Runs an area's battle program the way the game sets it up (kh2-rs spawn_waves: area_groups compiled
 * from source): the members created at load appear now; each kh2_enemies_tick the kind-2 groups create
 * their waves while Sora stands in their volumes, kind-6 groups their members, the group records count
 * their enemies and raise global events 3 (a record's enemies gone) and 2 (all gone), the signals
 * enable / disable groups on the events. Returns the number created now. */
KH2_API int32_t kh2_area_run(int32_t handle, const char *area, int32_t program);
/* A global event (id, argument; -1 any) for the area run's signals (e.g. 0x9b of the field objects). */
KH2_API int32_t kh2_area_raise_event(int32_t handle, int32_t event, int32_t arg);
/* Takes the events the groups raised: up to max (id, argument) pairs into out; returns the count. */
KH2_API int32_t kh2_area_take_events(int32_t handle, int32_t *out, uint32_t max);
/* Ends the area run (its groups and waves); its enemies stay (kh2_enemies_clear removes them). 0, or -1. */
KH2_API int32_t kh2_area_end(int32_t handle);

/* The mission manager an area run's program starts (opcode 0x15; kh2-rs struggle.rs compiled from
 * source): the mission file's counters, clock, layouts and script; every reaction a fighter takes
 * knocks orbs out of its counter into the pool, the first fighter in reach takes one back; the
 * script sends the result (global events 159 won / 160 lost, also in kh2_area_take_events). Ticked
 * in kh2_enemies_tick. The Struggle matches are tt04 programs 182 (N_EX500_BTL), 183 (N_EX600_BTL) and 184 (Seifer). */
typedef struct kh2_mission {
    int32_t running;       /* 1 while the manager exists */
    int32_t counters[3];   /* host 4.29: 0 the player's orbs, 1 the opponent's */
    int32_t clock_frames;  /* the time shown, 60 a second */
    int32_t clock_down;    /* 1: counts down */
    int32_t state;         /* 0 wait to show, 1 running, 2..4 the end */
    int32_t event_playing; /* a layout event plays (the groups wait) */
    int32_t stopped;
    int32_t end_cause;     /* 0 not yet, 1 player's counter full, 2 opponent's, 3 clock, 4 knock-out */
    int32_t result;        /* 0 none, 1 won (159), 2 lost (160) */
    int32_t ended;         /* the manager ended */
    int32_t game_over;     /* it ended with the game-over path */
    int32_t menu;          /* the last step asked for the result menu (record 0x1a) */
    uint32_t orbs;         /* orbs in the pool */
    uint32_t id;           /* the mission id */
} kh2_mission;
/* Returns 1 and fills out when a manager exists, 0 when none. */
KH2_API int32_t kh2_mission_state(int32_t handle, kh2_mission *out);
/* Up to max orbs as (x, y, z cm, kind) 4 floats each (kind 0 the player's, 2 the opponent's). */
KH2_API int32_t kh2_mission_orbs(int32_t handle, float *out, uint32_t max);
/* Takes up to max sounds as (number, has place, x, y, z) 5 floats each (the last 256 are kept). */
KH2_API int32_t kh2_mission_take_sounds(int32_t handle, float *out, uint32_t max);

/* Talks (kh2-rs npc.rs talk_step): the host owns its townspeople and their dialogue. When Sora
 * interacts with one, kh2_talk_begin stops Sora and turns him to it, turns it to him (unless
 * keep_facing), starts the talk camera (kh2_camera_tick reports mode 4) from its type-0x13 record
 * of obj/<model>.mdlx, locks Sora's control and fills the request (the talk id back, the target's
 * motion 0x32). kh2_talk_end releases him and the camera warps back. bones: NULL (the model's rest
 * pose) or bone_count column-major 4x4 matrices of its pose in model space. */
typedef struct kh2_talk_target {
    float pos[3];
    float yaw;
    int32_t part;        /* the talk part of the offer */
    int32_t keep_facing; /* script array bit 4 */
    int32_t own_frame;   /* script array bit 6: the camera in the target's frame */
    int32_t examine;     /* Execute 0x1e: no camera, no turn, no motion */
    uint32_t talk_id;
} kh2_talk_target;
typedef struct kh2_talk_request {
    uint32_t talk_id;
    int32_t examine;
    float target_yaw;
    float sora_yaw;
    int32_t target_motion; /* 0x32, -1 none */
    int32_t camera;        /* 1: the talk camera started */
    float look[3];
    float dist;
    float height;
} kh2_talk_request;
/* Starts a talk with the townsperson `model` (NULL or a name) at `*target`, with its bones (NULL or 16 x `bone_count` floats): Sora is locked until kh2_talk_end and the camera cuts to the talk view; the request is written to `*out` (may be NULL). 0, or -1. */
KH2_API int32_t kh2_talk_begin(int32_t handle, const char *model, const kh2_talk_target *target, const float *bones, uint32_t bone_count, kh2_talk_request *out);
/* Ends the talk: Sora's control and the field camera return. 0, or -1. */
KH2_API int32_t kh2_talk_end(int32_t handle);

/* Sound and effect cues (docs/SOUND-AND-EFFECTS.md): what the original plays or starts, from the
 * motions' frame triggers of Sora, the members, the enemies and the summon, the scripts' effect
 * starts and the mission's numbered sounds. The library plays nothing; the host does. Collected by
 * kh2_tick, kh2_party_tick and kh2_enemies_tick; at most 1024 wait. */
#define KH2_CUE_SOUND 1         /* trigger 8: params[0] bank, [1] index (sound bank * 1000 + index; bank 500: the owner's own) */
#define KH2_CUE_VOICE 2         /* triggers 13 / 14: params[0] voice / line, [1] priority (enemies) */
#define KH2_CUE_EFFECT 3        /* trigger 1: params[0] element of the owner's effect file, bound to the owner */
#define KH2_CUE_FOOTSTEP 4      /* trigger 2: params[0] footstep kind (the floor material picks the sample) */
#define KH2_CUE_DUST 5          /* trigger 19: params[0] dust kind */
#define KH2_CUE_SCRIPT_EFFECT 6 /* a script started element params[0] (enemies; model "limit": the limit's own) */
#define KH2_CUE_SOUND_NUMBER 7  /* a numbered sound: params[0] | params[1] << 16 (mission start / end / fail, orbs) */
typedef struct kh2_cue {
    int32_t kind;          /* KH2_CUE_* */
    int32_t owner;         /* 0 Sora, 1 member (owner_id = follower slot), 2 enemy (its id), 3 summon */
    int32_t owner_id;
    int16_t params[4];
    int16_t frame;         /* the motion frame of the trigger */
    int16_t _pad;
    float pos[3];          /* the owner's position (cm) */
    char model[16];        /* the owner's model: its effect / sound file is obj/<model>.apdx */
} kh2_cue;
#define KH2_CUE_HIT 9           /* a landed hit: params[0] the row's EnemyReaction (hit sound class), [1] the row; model the attacker's, pos the victim's */
#define KH2_CUE_COLLISION_GROUP 8 /* a script switched collision group params[0] on (params[1] 1) or off: show / hide that mesh group */
/* Takes the oldest cue: 1 when one was taken, 0 when none waits. */
KH2_API int32_t kh2_poll_cue(int32_t handle, kh2_cue *out);

/* An area's set-up for the host (kh2-rs kh2_ard run_area, orig 0x181148: the map, btl and evt
 * programs, then the entrance test): every object its spawn groups create (townspeople, field
 * objects, save points, chests, enemies, the player's start 0x236) with model, place, talk id and
 * the command its reaction part offers; the collision mask the objects' scripts switch (hosts
 * 1.46 / 1.47: the library applies the switches its scripts make and cues them as
 * KH2_CUE_COLLISION_GROUP); camera type; music. The host spawns and draws the townspeople and
 * field objects itself and starts talks with kh2_talk_begin (talk_id, part_command 0x1f talk). */
typedef struct kh2_area_info {
    uint32_t object_count;
    uint32_t group_count;
    uint64_t collision_mask;
    uint32_t camera_type;
    uint16_t music[2];     /* field, battle; 0xffff none */
    int32_t has_player;
    float player_pos[3];
    float player_yaw;
} kh2_area_info;
typedef struct kh2_area_object {
    uint32_t object;       /* 00objentry id */
    int32_t object_type;   /* 4 enemy, 5 townsperson, ... */
    char model[32];
    int32_t script;        /* 0 map, 1 btl, 2 evt */
    int32_t program;
    char group[8];         /* spawn group tag */
    float pos[3];
    float yaw;
    uint32_t talk_id;
    uint32_t part_command;
    int32_t spawn_kind;
    uint32_t serial;
} kh2_area_object;
/* Returns the object count (fills out when not NULL). */
KH2_API int32_t kh2_area_setup(const char *area, int32_t map, int32_t btl, int32_t evt, int32_t entrance, kh2_area_info *out);
/* Object index of the last kh2_area_setup; -1 out of range. */
KH2_API int32_t kh2_area_get_object(int32_t index, kh2_area_object *out);

/* Runtime meshes (docs/MESHES.md): an actor's body from its own model file, skinned on the CPU by
 * its current bones, with its decoded textures, so a host draws without an exporter. owner 0 Sora
 * (part 0 body, 1 keyblade, 2 second keyblade), 1 a member (part = follower slot), 2 an enemy
 * (part = index), 3 the summon. space 1 world (cm), 0 model. Prepare, then copy. */
typedef struct kh2_mesh_info {
    uint32_t vertex_count;
    uint32_t index_count;     /* triangle list */
    uint32_t submesh_count;
    uint32_t _pad;
} kh2_mesh_info;
typedef struct kh2_submesh {
    uint32_t first_index;
    uint32_t index_count;
    int32_t texture;          /* global texture id, -1 none */
    int32_t transparent;      /* 1: alpha-blended, draw after the opaque ones */
} kh2_submesh;
/* Skins the current pose on the CPU: `owner` 0 Sora (`part` 0 body, 1 keyblade, 2 second keyblade), 1 a member slot, 2 an enemy index, 3 the summon; `space` 0 model, 1 world. Writes the sizes to `*out` (may be NULL); read the arrays with kh2_mesh_get. 0, or -1. */
KH2_API int32_t kh2_mesh_prepare(int32_t handle, int32_t owner, int32_t part, int32_t space, kh2_mesh_info *out);
/* positions 3, normals 3, uvs 2, colors 4 (1.0 = neutral 0x80) floats a vertex; any pointer NULL;
 * -1 when a capacity is smaller than the mesh. */
KH2_API int32_t kh2_mesh_get(float *positions, float *normals, float *uvs, float *colors, uint32_t max_vertices, uint32_t *indices, uint32_t max_indices, kh2_submesh *subs, uint32_t max_subs);
/* Texture ids stay valid for the library's life (the texture keeps its model loaded). */
KH2_API int32_t kh2_texture_info(int32_t id, uint32_t *width, uint32_t *height);
/* RGBA8 row-major, straight alpha (PS2 0..0x80 mapped to 0..255); returns the byte count. */
KH2_API int32_t kh2_texture_get(int32_t id, uint8_t *out, uint32_t cap);

/* The field HUD as a 2D draw list (docs/HUD.md): the command menu (Attack, Magic with the spells,
 * Items from the host's list, Drive with the owned forms) and the reaction capsule, from the
 * game's art and font, on the game's 512 x 448 screen (y down). Load once, then each tick after
 * kh2_enemies_tick: kh2_hud_input (menu buttons), kh2_hud_tick, kh2_hud_draw_list. A chosen spell
 * is cast, Attack presses attack, the reaction command is accepted; items and forms come out of
 * kh2_hud_poll_event for the host to use. Not drawn yet: gauges, party panels, target gauge,
 * lock-on reticle, banners, INFORMATION bar. */
#define KH2_HUD_UP 1u
#define KH2_HUD_DOWN 2u
#define KH2_HUD_CONFIRM 4u
#define KH2_HUD_BACK 8u
#define KH2_HUD_REACTION 16u
typedef struct kh2_hud_sprite {
    int32_t texture;     /* kh2_hud_texture_info / kh2_hud_texture_get */
    int32_t blend;       /* 0 alpha, 1 additive, 2 subtractive */
    int32_t nearest;     /* 1: nearest filtering */
    int32_t text;        /* 1: a glyph or icon of the font atlas */
    float pos[8];        /* corners TL, TR, BL, BR: x, y */
    float uv[8];         /* texel coordinates of the corners */
    uint8_t color[16];   /* RGBA per corner, 0x80 = 1.0: texel * colour / 128 */
} kh2_hud_sprite;
typedef struct kh2_hud_event {
    int32_t kind;        /* 1 attack, 2 magic (value slot), 3 item (value id), 4 drive (value form), 5 reaction (value command) */
    int32_t value;
} kh2_hud_event;
/* Loads the field HUD of world code `world` (e.g. "tt"; the disc's field2d and fonts) for Sora `handle`. 0, or -1 when the files are missing. */
KH2_API int32_t kh2_hud_load(int32_t handle, const char *world);
/* The menu buttons of this tick (KH2_HUD_* bits: up, down, confirm, back). 0, or -1. */
KH2_API int32_t kh2_hud_input(int32_t handle, uint32_t buttons);
/* The Items page: `count` item ids and counts (caller-owned). 0, or -1. */
KH2_API int32_t kh2_hud_set_items(int32_t handle, const uint16_t *ids, const uint8_t *counts, uint32_t count);
/* Returns the draw list's sprite count. */
KH2_API int32_t kh2_hud_tick(int32_t handle);
/* The area-name card of "tt04"-style area names (the place table's name for 120 frames): 1 shown, 0 no name. */
KH2_API int32_t kh2_hud_area_card(int32_t handle, const char *area);
/* Shows system message `message` on the INFORMATION bar (a mission's objective, its header +6): 1 shown,
 * 0 no such message, -1 no HUD. The bar waits 180 ticks, scrolls the text out and closes. */
KH2_API int32_t kh2_hud_info(int32_t handle, uint32_t message);
/* Opens the message window of `class` (0 event message, 1 top banner, 2 top, 3 bottom) with system
 * message `message` (0 closes it): it opens, types the text, shows the prompt and waits. 1 shown, 0 no
 * such message / class, -1 no HUD. */
KH2_API int32_t kh2_hud_message(int32_t handle, uint32_t message, int32_t class_);
/* The message window's button: 1 it was waiting and closes, 0 not waiting, -1 no HUD. */
KH2_API int32_t kh2_hud_message_confirm(int32_t handle);
/* Copies up to `max` sprites of the last kh2_hud_tick to `out` (caller-owned; 512 x 448 HUD units, y down). Returns the number in the list (may exceed `max`), or -1. */
KH2_API int32_t kh2_hud_draw_list(int32_t handle, kh2_hud_sprite *out, uint32_t max);
/* Pops the oldest menu choice into `*out`: 1 written, 0 none, -1. */
KH2_API int32_t kh2_hud_poll_event(int32_t handle, kh2_hud_event *out);
/* Returns the HUD texture count; fills width / height of texture id when id >= 0. */
KH2_API int32_t kh2_hud_texture_info(int32_t handle, int32_t id, uint32_t *width, uint32_t *height);
/* Copies texture `id`'s RGBA8 pixels (row-major from the top, straight alpha; width x height x 4 bytes from kh2_hud_texture_info) to `out` (`cap` bytes). Returns the byte count, or -1. Textures can be added by later calls (party faces): query kh2_hud_texture_info for new ids. */
KH2_API int32_t kh2_hud_texture_get(int32_t handle, int32_t id, uint8_t *out, uint32_t cap);

/* Moving collision objects (like libsm64's surface objects): triangles in the object's own space
 * placed with a transform the host changes; Sora standing on one rides with it (its translation
 * and the yaw of its rotation) on his next kh2_tick. The collision world is rebuilt when an object
 * is created, moved or deleted (cost: the whole world's triangles). */
typedef struct kh2_object_transform {
    float pos[3];  /* cm */
    float rot[3];  /* Euler radians, applied X, then Y, then Z */
} kh2_object_transform;
/* Returns the object id (> 0). transform NULL: the origin. */
KH2_API int32_t kh2_surface_object_create(const kh2_surface *tris, uint32_t count, const kh2_object_transform *transform);
/* Moves collision object `id` to `*transform` (position cm, yaw rad); Sora standing on it moves along. 0, or -1. */
KH2_API int32_t kh2_surface_object_move(int32_t id, const kh2_object_transform *transform);
/* Removes collision object `id`. 0, or -1. */
KH2_API int32_t kh2_surface_object_delete(int32_t id);

/* The audio engine (like libsm64's sm64_audio_tick): Sora's sound cues (and his party's, enemies',
 * summon's, the mission's) played through kh2-rs's software sound driver (the game's SE sequencer
 * on a software SPU2, tables from the disc's LIBSSL.IRX), rendered as 16-bit stereo PCM at 48 kHz.
 * Init once (returns 48000), then feed the audio device: kh2_audio_tick(h, out, frames) writes
 * 2 * frames interleaved samples. Voices, footsteps and music are not played. */
KH2_API int32_t kh2_audio_init(void);
/* Plays the cues waiting and renders `frames` stereo frames (16-bit, 48 kHz, interleaved) into `out` (caller-owned, 2 x `frames` samples). 0, or -1 without kh2_audio_init. */
KH2_API int32_t kh2_audio_tick(int32_t handle, int16_t *out, uint32_t frames);
/* The listener for volume and pan (position cm, forward, up; usually the camera's). Without one every
 * sound plays centred at full level. */
KH2_API int32_t kh2_audio_set_listener(const float *pos, const float *forward, const float *up);
/* Plays area music bgm/musicNNN.bgm (number; kh2_area_info.music names an area's), negative stops. */
KH2_API int32_t kh2_audio_play_bgm(int32_t number);
/* The area of the voices (its battle voice bank) and footsteps (its sound info record), e.g. "tt04". */
KH2_API int32_t kh2_audio_set_area(const char *area);
/* kh2_cam_view.mode: 0 follow, 1 look-around, 2 lock-on (a kh2_set_target point), 4 talk,
 * 5 overhead, 8 always-behind. */
/* Drops the enemies that are gone (indices shift) and frees the enemy kinds no enemy uses (kinds load
 * on demand at their first spawn). Returns the number dropped. */
KH2_API int32_t kh2_enemies_prune(int32_t handle);
/* Removes every enemy of Sora `handle` and frees the kinds no other enemies use. */
KH2_API int32_t kh2_enemies_clear(int32_t handle);
/* The mission counters the scripts read (host 4.29; Struggle: slot 0 Sora's orbs, slot 1 the opponent's);
 * on = 0 removes the mission. The host keeps them (docs/STRUGGLE.md). */
KH2_API int32_t kh2_enemies_set_mission(int32_t handle, int32_t on, int32_t c0, int32_t c1, int32_t c2);
/* One tick of the enemies (after kh2_tick): scripts, movement, reactions, hits both ways. */
KH2_API int32_t kh2_enemies_tick(int32_t handle);
/* The number of enemies of Sora `handle` (living and dying), or -1. */
KH2_API int32_t kh2_enemy_count(int32_t handle);
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
} kh2_enemy_state_t;
/* Writes enemy `index`'s state (id, model, position cm, yaw, HP, motion) to `*out`. 0, or -1 for a bad index. */
KH2_API int32_t kh2_enemy_state(int32_t handle, int32_t index, kh2_enemy_state_t *out);
/* Writes enemy `index`'s bone matrices (16 floats each, `space` 0 model / 1 world) to `out` (`max_bones` matrices). Returns the count, or -1. */
KH2_API int32_t kh2_enemy_bones(int32_t handle, int32_t index, int32_t space, float *out, uint32_t max_bones);
#define KH2_EN_HIT 1        /* Sora hit it: atkp, damage, value = HP left */
#define KH2_EN_HIT_SORA 2   /* it hit Sora: atkp, damage, value = kh2_hit_taken result */
#define KH2_EN_DEFEATED 3   /* enmp, lvpm/enmp experience, value = EXP paid to Sora */
#define KH2_EN_EFFECT 4     /* value = element of its effect file */
#define KH2_EN_DROP 5       /* value = przt row, prizes per kind */
#define KH2_EN_REMOVED 6
#define KH2_EN_REACTION 7   /* an accepted reaction command reached its script: atkp = command, value = handled */
#define KH2_EN_CLASH 8      /* its attack window clashed with Reflect's guard (no hit on Sora): atkp */
typedef struct kh2_enemy_event {
    int32_t kind, id, atkp, damage, value, enmp, lvpm_experience, enmp_experience;
    uint32_t prizes[9];
} kh2_enemy_event;
/* Pops the oldest enemy event (KH2_EN_*) into `*out`: 1 written, 0 none, -1 bad handle. */
KH2_API int32_t kh2_poll_enemy_event(int32_t handle, kh2_enemy_event *out);

/* ---- Reaction commands (kh2-rs reaction_cmd + reaction_parts: the objects' reaction parts touching
 * Sora's body volume (the Soldier's command 34) and range trigger 20 offers, cmd row status, generic accept)
 * The library fills slot 1 each kh2_enemies_tick; the host shows the prompt and calls accept on its
 * button. The target script gets event 11 on the next tick (KH2_EN_REACTION); what it then asks of Sora
 * (the paired motion from the enemy's set, KH2_STATE_REACTION, its velocity, a queued motion, the
 * unhittable bit) runs on Sora's next ticks. Not ported: limits through this slot (Auto Limit),
 * talk/save/shop rows. kh2_reaction_force_offer is kh2-rs's harness offer. */
#define KH2_REACT_AVAILABLE 0
#define KH2_REACT_BLOCKED 1
#define KH2_REACT_TOO_FAR 2
#define KH2_REACT_NO_MP 3
#define KH2_REACT_NOT_PORTED 4
typedef struct kh2_reaction_offer {
    int32_t cmd;      /* cmd row id */
    int32_t from;     /* offering enemy id */
    int32_t status;   /* KH2_REACT_* */
    uint32_t message; /* the prompt's message id */
} kh2_reaction_offer_t;
/* 1 written, 0 nothing offered. */
KH2_API int32_t kh2_reaction_offer(int32_t handle, kh2_reaction_offer_t *out);
/* Returns the accepted command id, or -1 (nothing available offered). */
KH2_API int32_t kh2_reaction_accept(int32_t handle);
/* Every living enemy offers `cmd` (0 stops). */
KH2_API int32_t kh2_reaction_force_offer(int32_t handle, int32_t cmd);

/* ---- Field camera helper (kh2-rs camera_rules, oracle-tested there) ---- */
#define KH2_CAM_RESET 1u   /* R3: behind Sora */
#define KH2_CAM_SELECT 2u  /* look-around */
#define KH2_CAM_MANUAL 4u  /* no auto-camera (R2 held) */

typedef struct kh2_cam_input {
    float stick_x, stick_y; /* right stick, x right, y up */
    float zoom;             /* units added to the radius (mouse wheel) */
    uint32_t buttons;       /* KH2_CAM_* pressed this tick */
    float step;             /* 1 per 60 Hz tick (0 = 1), 2 per 30 Hz frame */
} kh2_cam_input;

typedef struct kh2_cam_view {
    float pos[3], look[3];
    float fov;              /* horizontal, radians, for a 4:3 picture */
    float roll, yaw;
    int32_t mode;           /* 0 follow, 1 look-around */
} kh2_cam_view;

/* One camera tick after kh2_tick: follows Sora, locks onto kh2_set_target's point, collides with
 * the static collision. Use look - pos as the next kh2_inputs camera look direction. */
KH2_API int32_t kh2_camera_tick(int32_t handle, const kh2_cam_input *input, kh2_cam_view *view);
/* New camera behind Sora with table row 0..3 (0 field, 1 indoor). */
KH2_API int32_t kh2_camera_reset(int32_t handle, int32_t row);
/* Index of the nearest of `count` xyz points within the lock-on range, or -1. */
KH2_API int32_t kh2_lock_on_pick(int32_t handle, const float *points, uint32_t count);

#ifdef __cplusplus
}
#endif

#endif /* KH2_H */
