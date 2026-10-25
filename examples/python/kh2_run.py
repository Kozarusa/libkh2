"""A short libkh2 run through Python ctypes: Sora, Donald and Goofy against a Soldier on a flat floor.

    python examples/python/kh2_run.py [path to kh2.dll / libkh2.so] [disc image]

The library defaults to dist/libkh2/bin or target/release; the disc image to KH2_ISO. Prints a line
every second of game time (60 ticks) and the enemy and member events as they come.
"""
import ctypes as C
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def find_lib(arg):
    if arg:
        return arg
    names = ["kh2.dll", "libkh2.so", "libkh2.dylib"]
    for d in [ROOT / "dist" / "libkh2" / "bin", ROOT / "target" / "release"]:
        for n in names:
            if (d / n).exists():
                return str(d / n)
    sys.exit("kh2 library not found: build it (cargo build --release -p libkh2) or pass its path")


class Surface(C.Structure):
    _fields_ = [("v", (C.c_float * 3) * 3), ("flags", C.c_uint32), ("group", C.c_uint8), ("material", C.c_uint8), ("_pad", C.c_uint16)]


class Inputs(C.Structure):
    _fields_ = [("stick_x", C.c_float), ("stick_y", C.c_float), ("cam_look_x", C.c_float), ("cam_look_z", C.c_float), ("buttons", C.c_uint32)]


I32, F32 = C.c_int32, C.c_float
STATE_FIELDS = [("pos", F32 * 3), ("vel", F32 * 3), ("yaw", F32)] + [(n, I32) for n in "grounded state motion_slot motion_id".split()] + [("frame", F32), ("gframe", F32)]
STATE_FIELDS += [(n, I32) for n in "motion_finished blending combo attack_active attack_id attack_group combo_window finisher_window weapon_out ground_material invulnerable hp max_hp mp max_mp".split()]
STATE_FIELDS += [("mp_charge", F32), ("form", I32), ("drive_points", I32), ("drive_fraction", I32), ("form_time", F32), ("form_time_total", F32)]
STATE_FIELDS += [(n, I32) for n in "level exp strength magic defense".split()]


class State(C.Structure):
    _fields_ = STATE_FIELDS


class EnemyState(C.Structure):
    _fields_ = [("id", I32), ("pos", F32 * 3), ("yaw", F32), ("hp", I32), ("max_hp", I32), ("state", I32), ("motion_slot", I32), ("frame", F32), ("grounded", I32), ("alpha", F32), ("enmp", I32)]


class EnemyEvent(C.Structure):
    _fields_ = [(n, I32) for n in "kind id atkp damage value enmp lvpm_experience enmp_experience".split()] + [("prizes", C.c_uint32 * 9)]


class MemberState(C.Structure):
    _fields_ = [("pos", F32 * 3), ("yaw", F32), ("member", I32), ("motion_slot", I32), ("frame", F32)] + [(n, I32) for n in "hp max_hp mp max_mp ko level style strength magic defense".split()]


class MemberEvent(C.Structure):
    _fields_ = [(n, I32) for n in "slot kind id atkp damage hp".split()]


ATTACK = 1
EVENT = {1: "Sora hit", 2: "hit Sora", 3: "defeated", 4: "effect", 5: "drop", 6: "removed", 7: "reaction"}
MEMBER = {1: "hit", 2: "knocked out", 3: "revived", 4: "item", 5: "level up", 6: "bonus"}


def main():
    lib = C.CDLL(find_lib(sys.argv[1] if len(sys.argv) > 1 else None))
    iso = sys.argv[2] if len(sys.argv) > 2 else os.environ.get("KH2_ISO")
    if not iso:
        sys.exit("pass the disc image or set KH2_ISO")
    v = lib.kh2_version()
    print(f"libkh2 {v >> 16}.{(v >> 8) & 0xff}.{v & 0xff}")
    if lib.kh2_init(iso.encode()) != 0:
        buf = C.create_string_buffer(256)
        lib.kh2_last_error(buf, 256)
        sys.exit(f"kh2_init failed: {buf.value.decode()}")
    s = 3000.0
    floor = (Surface * 2)()
    for k, tri in enumerate([((-s, 0, -s), (-s, 0, s), (s, 0, s)), ((-s, 0, -s), (s, 0, s), (s, 0, -s))]):
        for i in range(3):
            for j in range(3):
                floor[k].v[i][j] = tri[i][j]
        floor[k].flags = 1
    lib.kh2_surfaces_load(floor, 2)
    lib.kh2_create.argtypes = [F32, F32, F32]
    lib.kh2_enemy_spawn.argtypes = [I32, C.c_char_p, F32, F32, F32, F32]
    h = lib.kh2_create(0.0, 0.0, 0.0)
    lib.kh2_party_add(h, 0)
    lib.kh2_party_add(h, 1)
    soldier = lib.kh2_enemy_spawn(h, b"M_EX010", 0.0, 0.0, 400.0, 3.14159)
    st, es, me = State(), EnemyState(), MemberState()
    ev, mev = EnemyEvent(), MemberEvent()
    for t in range(600):
        lib.kh2_enemy_state(h, 0, C.byref(es))
        dx, dz = es.pos[0] - st.pos[0], es.pos[2] - st.pos[2]
        d = max((dx * dx + dz * dz) ** 0.5, 1e-3)
        lib.kh2_set_target.argtypes = [I32, F32, F32, F32, F32, F32]
        if es.hp > 0:
            lib.kh2_set_target(h, es.pos[0], es.pos[1] + 80.0, es.pos[2], 40.0, 80.0)
        inp = Inputs(dx / d if d > 150 else 0.0, dz / d if d > 150 else 0.0, 0.0, 1.0, ATTACK if t % 6 == 0 else 0)
        lib.kh2_tick(h, C.byref(inp), C.byref(st))
        lib.kh2_party_tick(h)
        lib.kh2_enemies_tick(h)
        while lib.kh2_poll_enemy_event(h, C.byref(ev)) == 1:
            print(f"  tick {t:3}: {EVENT.get(ev.kind, ev.kind)} enemy {ev.id} atkp {ev.atkp} damage {ev.damage} value {ev.value}")
        while lib.kh2_poll_member_event(h, C.byref(mev)) == 1:
            print(f"  tick {t:3}: member {mev.slot} {MEMBER.get(mev.kind, mev.kind)} id {mev.id} damage {mev.damage} hp {mev.hp}")
        if t % 60 == 0:
            lib.kh2_party_state(h, 0, C.byref(me))
            print(f"t={t // 60}s Sora ({st.pos[0]:.0f}, {st.pos[2]:.0f}) state {st.state} combo {st.combo} HP {st.hp}/{st.max_hp} LV {st.level} | Soldier {soldier} HP {es.hp}/{es.max_hp} | Donald HP {me.hp}/{me.max_hp}")
    lib.kh2_terminate()


if __name__ == "__main__":
    main()
