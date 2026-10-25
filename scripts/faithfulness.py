"""Evidence table for docs/FAITHFULNESS.md: for every kh2-rs source file libkh2 compiles (in place
with #[path], or as spans cut by the build scripts) and the kh2-rs crates it links, the original
functions the file cites (`FUN_xxxxxxxx`, `orig 0x...`) and their status in kh2-rs's function
ledger (`sheets/functions.json`, kh2-rs `kh2 ledger scan`): verified (an oracle test ran the
original in kh2-ee and compared), tested (a hand-written test names it), ported, read. Prints a
Markdown table and, per file, up to three oracle tests named by the ledger.

    python scripts/faithfulness.py > local/faithfulness-table.md

Needs ../kh2-rs next to this repository (read only).
"""
import json
import re
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KH2RS = ROOT.parent / 'kh2-rs'

# (kh2-rs path under crates/, how libkh2 uses it)
FILES = [
    ('kh2-game-core/src/motor.rs', 'in place'),
    ('kh2-game-core/src/world.rs', 'in place'),
    ('kh2-game-core/src/reaction_rules.rs', 'in place'),
    ('kh2-game-core/src/camera_rules.rs', 'in place'),
    ('kh2-game-core/src/magic_rules.rs', 'in place'),
    ('kh2-game-core/src/party_rules.rs', 'in place'),
    ('kh2-game-core/src/reaction_parts.rs', 'in place'),
    ('kh2-game-core/src/rng.rs', 'span'),
    ('kh2-game-core/src/shared/abilities.rs', 'span'),
    ('kh2-game-core/src/shared/drive.rs', 'span'),
    ('kh2-game-core/src/shared/party.rs', 'span'),
    ('kh2-game-core/src/shared/enemy.rs', 'in place'),
    ('kh2-game-core/src/shared/enemy_obj.rs', 'in place'),
    ('kh2-game-core/src/shared/reaction.rs', 'in place'),
    ('kh2-game-core/src/shared/struggle.rs', 'in place'),
    ('kh2-game-core/src/shared/prize.rs', 'in place'),
    ('kh2-game-core/src/shared/summon.rs', 'span'),
    ('kh2-game-core/src/progress.rs', 'span'),
    ('kh2-player/src/controller_rules.rs', 'in place'),
    ('kh2-player/src/player_reaction.rs', 'span'),
    ('kh2-player/src/prize.rs', 'span'),
    ('kh2-party/src/reaction_cmd.rs', 'span'),
    ('kh2-party/src/party_battle.rs', 'span'),
    ('kh2-party/src/items.rs', 'span'),
    ('kh2-party/src/friend_host.rs', 'in place'),
    ('kh2-party/src/limit.rs', 'span'),
    ('kh2-party/src/limit_visual.rs', 'span'),
    ('kh2-party/src/limit_script.rs', 'span'),
    ('kh2-party/src/summon.rs', 'span'),
    ('kh2-enemy/src/enemy_obj.rs', 'in place'),
    ('kh2-enemy/src/enemy_host.rs', 'in place'),
    ('kh2-enemy/src/enemy.rs', 'span'),
    ('kh2-event/src/struggle.rs', 'span'),
    ('kh2-event/src/npc.rs', 'span'),
    ('kh2-bdx/src', 'crate'),
    ('kh2-anim/src', 'crate'),
    ('kh2-tables/src', 'crate'),
]

ADDR = re.compile(r'FUN_00([0-9a-f]{6})|orig (?:ovl\d+ )?0x([0-9a-f]{6,7})\b')


def sources(p):
    p = KH2RS / 'crates' / p
    if p.is_dir():
        return sorted(x for x in p.rglob('*.rs'))
    return [p]


def main():
    sheet = json.loads((KH2RS / 'sheets' / 'functions.json').read_text(encoding='utf-8'))
    cols = sheet['columns']
    ia, ist, iev = cols.index('addr'), cols.index('status'), cols.index('evidence')
    rows = {r[ia]: (r[ist], r[iev]) for r in sheet['rows']}
    print('| kh2-rs file | use | cited | verified | tested | ported | read | other |')
    print('|---|---|---|---|---|---|---|---|')
    notes = []
    for f, use in FILES:
        text = ''.join(s.read_text(encoding='utf-8', errors='replace') for s in sources(f))
        addrs = {'0x' + (a or b).lstrip('0') for a, b in ADDR.findall(text)}
        c = Counter()
        oracle = []
        for a in sorted(addrs):
            st, ev = rows.get(a, ('unknown', ''))
            k = st if st in ('verified', 'tested', 'ported', 'read') else 'other'
            c[k] += 1
            if st == 'verified' and len(oracle) < 3:
                oracle.append(f'{a}: {ev}')
        print(f"| {f} | {use} | {len(addrs)} | {c['verified']} | {c['tested']} | {c['ported']} | {c['read']} | {c['other']} |")
        if oracle:
            notes.append((f, oracle))
    print()
    for f, o in notes:
        print(f'- {f}: ' + '; '.join(o))


if __name__ == '__main__':
    sys.exit(main())
