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
 * The library keeps one global state (like libsm64) guarded by a mutex.
 */
#ifndef KH2_H
#define KH2_H

#include <stdint.h>

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
/* High Jump level 0..5. */
KH2_API int32_t kh2_set_high_jump(int32_t handle, uint8_t level);
/* Growth abilities 0..5 (fmab rows): High Jump, Quick Run (Square while moving), Aerial Dodge
 * (jump in the air), Glide (hold Square while falling). */
KH2_API int32_t kh2_set_growth(int32_t handle, uint8_t high_jump, uint8_t quick_run, uint8_t aerial_dodge, uint8_t glide);
/* Skeleton evaluation each tick (default on). Off: no bones, no attack volumes. */
KH2_API int32_t kh2_set_pose_enabled(int32_t handle, int32_t on);

/* Skeletons: which = 0 body, 1 keyblade (hangs from body bone 178). */
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

#ifdef __cplusplus
}
#endif

#endif /* KH2_H */
