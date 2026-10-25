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
#define KH2_VERSION_MAJOR 0
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
KH2_API uint32_t kh2_version(void);
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
KH2_API void kh2_delete(int32_t handle);
/* Warps Sora (velocity and jump cleared). */
KH2_API int32_t kh2_set_position(int32_t handle, float x, float y, float z, float yaw);

/* One 60 Hz logic tick. `state` may be NULL. */
KH2_API int32_t kh2_tick(int32_t handle, const kh2_inputs *inputs, kh2_state *state);
/* Two ticks with the same input: one 30 Hz field frame. */
KH2_API int32_t kh2_tick_field(int32_t handle, const kh2_inputs *inputs, kh2_state *state);
KH2_API int32_t kh2_get_state(int32_t handle, kh2_state *state);

/* The host's lock-on target point (an enemy's target-shape centre, radius, half height). It feeds
 * the attack table's range tests and the attack's lunge, turn and leap. */
KH2_API int32_t kh2_set_target(int32_t handle, float x, float y, float z, float radius, float half_height);
KH2_API int32_t kh2_clear_target(int32_t handle);
/* The host found that the current swing touched something: lets the combo chain go on. */
KH2_API int32_t kh2_report_hit(int32_t handle);

/* A hit taken: atkp = attack row id of the disc's atkp table (its reaction class decides flinch,
 * push or blow), damage = HP taken (the host computes it), dir = from the attacker toward Sora.
 * Returns 0 ignored (invincible / down), 1 guarded, 2 no flinch, 3 flinch, 4 blown, -1 unknown row. */
KH2_API int32_t kh2_hit_taken(int32_t handle, int32_t atkp, int32_t damage, float dir_x, float dir_y, float dir_z);
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
KH2_API int32_t kh2_set_magic_levels(int32_t handle, const uint8_t levels[6]);
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
#define KH2_EV_LEVEL_UP 10      /* amount = the new level */
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
/* Takes the oldest event: 1 written, 0 none left. */
KH2_API int32_t kh2_poll_event(int32_t handle, kh2_event *out);

/* ---- Drive forms (kh2-rs shared::drive::rules) ---- */
/* Gauge points / maximum, owned forms (bit n = form n, 1..4), form levels by form id (6 bytes, may be NULL). */
KH2_API int32_t kh2_drive_setup(int32_t handle, uint8_t points, uint8_t max_points, uint32_t owned_mask, const uint8_t *levels);
KH2_API int32_t kh2_drive_gain(int32_t handle, int32_t percent);
/* A landed hit with its atkp row: the combo latch plus the row's DriveDrain. */
KH2_API int32_t kh2_report_hit_atkp(int32_t handle, int32_t atkp);
/* Enter form 1..4 (the Anti / Final roll may pick another): returns the form entered or -1. The
 * body swaps when the change motion ends (KH2_EV_FORM_CHANGED); bones change with it. */
KH2_API int32_t kh2_drive_enter(int32_t handle, int32_t form);
KH2_API int32_t kh2_drive_revert(int32_t handle);
/* objentry ids of the form's body and hand 0 / hand 1 weapons (-1 none). */
KH2_API int32_t kh2_drive_objects(int32_t handle, int32_t out[3]);
/* The keyblade item the second hand takes on the next entry (default 41, the Kingdom Key). */
KH2_API int32_t kh2_drive_set_second_keyblade(int32_t handle, int32_t item);
KH2_API int32_t kh2_drive_outgoing_damage(int32_t handle, int32_t atk_type, int32_t damage);

/* ---- Animation export (any slot, independent of what Sora plays; see docs/SKINNING.md) ---- */
typedef struct kh2_motion_desc {
    float frame_start, frame_end, frame_return;
    float fps;              /* the animator steps fps / 60 frames per 60 Hz tick */
    int32_t has_root_motion;
    char tag[8];            /* e.g. "A300" */
} kh2_motion_desc;
KH2_API int32_t kh2_motion_slot_count(int32_t handle, int32_t which);
/* variant: 0 battle + weapon, 1 battle, 2 field, 3 field + weapon (the game's fallback applies). */
KH2_API int32_t kh2_motion_slot(int32_t handle, int32_t which, int32_t motion_id, int32_t variant);
KH2_API int32_t kh2_motion_info(int32_t handle, int32_t which, int32_t slot, kh2_motion_desc *out);
/* Bone matrices of `slot` at `frame` relative to the motion's root; root = (x, y, z, yaw) or NULL. */
KH2_API int32_t kh2_sample_motion(int32_t handle, int32_t which, int32_t slot, float frame, float *out, uint32_t max_bones, float *root);

/* ---- Party members (kh2-rs party_rules: field following only) ---- */
typedef struct kh2_member_state {
    float pos[3];
    float yaw;
    int32_t member;       /* 0 Donald, 1 Goofy */
    int32_t motion_slot;
    float frame;
} kh2_member_state;
/* Adds Donald (0) or Goofy (1) following Sora; returns the member's slot 0/1 or -1. */
KH2_API int32_t kh2_party_add(int32_t handle, int32_t member);
/* One tick of the followers (after kh2_tick). */
KH2_API int32_t kh2_party_tick(int32_t handle);
KH2_API int32_t kh2_party_state(int32_t handle, int32_t slot, kh2_member_state *out);
KH2_API int32_t kh2_party_bones(int32_t handle, int32_t slot, int32_t space, float *out, uint32_t max_bones);

/* ---- Enemies (their own BDX scripts through kh2-rs's interpreter and script host) ---- */
/* Spawns an enemy by model ("M_EX020" Shadow, "M_EX010" Soldier, "M_EX990" Dusk, ...) fighting
 * Sora `handle`; returns its id (> 0) or -1. */
KH2_API int32_t kh2_enemy_spawn(int32_t handle, const char *kind, float x, float y, float z, float yaw);
/* One tick of the enemies (after kh2_tick): scripts, movement, reactions, hits both ways. */
KH2_API int32_t kh2_enemies_tick(int32_t handle);
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
} kh2_enemy_state;
KH2_API int32_t kh2_enemy_state(int32_t handle, int32_t index, kh2_enemy_state *out);
KH2_API int32_t kh2_enemy_bones(int32_t handle, int32_t index, int32_t space, float *out, uint32_t max_bones);
#define KH2_EN_HIT 1        /* Sora hit it: atkp, damage, value = HP left */
#define KH2_EN_HIT_SORA 2   /* it hit Sora: atkp, damage, value = kh2_hit_taken result */
#define KH2_EN_DEFEATED 3   /* enmp, lvpm/enmp experience, value = EXP paid to Sora */
#define KH2_EN_EFFECT 4     /* value = element of its effect file */
#define KH2_EN_DROP 5       /* value = przt row, prizes per kind */
#define KH2_EN_REMOVED 6
typedef struct kh2_enemy_event {
    int32_t kind, id, atkp, damage, value, enmp, lvpm_experience, enmp_experience;
    uint32_t prizes[9];
} kh2_enemy_event;
KH2_API int32_t kh2_poll_enemy_event(int32_t handle, kh2_enemy_event *out);

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
