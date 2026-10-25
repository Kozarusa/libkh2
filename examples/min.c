/*
 * Minimal libkh2 host: a floor and a wall, Sora runs at the wall, jumps, attacks.
 * Prints one line per 10 ticks.
 *
 *   min <path to the USA disc image>     (or set KH2_ISO and pass nothing)
 */
#include <stdio.h>
#include <string.h>
#include "kh2.h"

static void quad(kh2_surface *t, const float a[3], const float b[3], const float c[3], const float d[3], uint32_t flags) {
    memset(t, 0, 2 * sizeof *t);
    memcpy(t[0].v[0], a, 12); memcpy(t[0].v[1], b, 12); memcpy(t[0].v[2], c, 12);
    memcpy(t[1].v[0], a, 12); memcpy(t[1].v[1], c, 12); memcpy(t[1].v[2], d, 12);
    t[0].flags = t[1].flags = flags;
}

int main(int argc, char **argv) {
    char err[256];
    if (kh2_init(argc > 1 ? argv[1] : NULL) != 0) {
        kh2_last_error(err, sizeof err);
        fprintf(stderr, "kh2_init: %s\n", err);
        return 1;
    }
    /* Floor 4000 x 4000 at y = 0 (normal up), a wall across x = 600 facing -x. */
    kh2_surface tris[4];
    const float s = 2000.0f;
    float f0[3] = {-s, 0, -s}, f1[3] = {-s, 0, s}, f2[3] = {s, 0, s}, f3[3] = {s, 0, -s};
    float w0[3] = {600, -100, -s}, w1[3] = {600, -100, s}, w2[3] = {600, 600, s}, w3[3] = {600, 600, -s};
    quad(&tris[0], f0, f1, f2, f3, 1);
    quad(&tris[2], w0, w1, w2, w3, KH2_SURF_WALL);
    printf("surfaces: %d\n", kh2_surfaces_load(tris, 4));

    int32_t h = kh2_create(0, 0, 0);
    printf("jump height from the disc: %.1f\n", kh2_jump_height(h));
    kh2_state st;
    char tag[16];
    for (int t = 0; t < 240; t++) {
        kh2_inputs in = {0};
        in.cam_look_x = 1.0f; /* camera looks along +x */
        if (t < 100) in.stick_y = 1.0f;          /* run forward, into the wall */
        if (t >= 110 && t < 150) in.buttons |= KH2_BTN_JUMP; /* a held jump */
        if (t >= 180 && (t % 6) == 0) in.buttons |= KH2_BTN_ATTACK; /* mash attack */
        kh2_tick(h, &in, &st);
        if (st.attack_active) kh2_report_hit(h); /* pretend every swing connects */
        if (t % 10 == 0) {
            kh2_motion_tag(h, tag, sizeof tag);
            printf("t=%3d pos=(%7.1f %6.1f %7.1f) yaw=%5.2f state=%d motion=%3d %-5s frame=%5.1f combo=%d attack=%d\n",
                   t, st.pos[0], st.pos[1], st.pos[2], st.yaw, st.state, st.motion_id, tag, st.frame, st.combo, st.attack_active);
        }
    }
    int n = kh2_bone_count(h, 0);
    printf("body bones: %d, keyblade bones: %d\n", n, kh2_bone_count(h, 1));
    kh2_delete(h);
    kh2_terminate();
    return 0;
}
