#!/bin/sh
# Lists what changed in ../kh2-rs since the commit libkh2 was checked against (KH2RS_COMMIT), in
# the files libkh2 compiles in place and the files it transcribes. Read-only: only `git diff`.
# Exit 1 when any of them changed. Run from the repository root.
set -e
pinned=$(tr -d '\r\n' < KH2RS_COMMIT)
kh2rs=../kh2-rs
compiled="crates/kh2-game-core/src/motor.rs crates/kh2-game-core/src/world.rs crates/kh2-game-core/src/reaction_rules.rs crates/kh2-game-core/src/camera_rules.rs crates/kh2-game-core/src/magic_rules.rs crates/kh2-game-core/src/shared/drive.rs crates/kh2-player/src/controller_rules.rs"
compiled="$compiled crates/kh2-game-core/src/shared/abilities.rs crates/kh2-party/src/items.rs crates/kh2-game-core/src/progress.rs crates/kh2-game-core/src/party_rules.rs crates/kh2-enemy/src/enemy_host.rs crates/kh2-enemy/src/enemy_obj.rs crates/kh2-enemy/src/kinds crates/kh2-game-core/src/shared/enemy.rs crates/kh2-game-core/src/shared/enemy_obj.rs crates/kh2-game-core/src/shared/reaction.rs crates/kh2-party/src/party_battle.rs crates/kh2-game-core/src/rng.rs crates/kh2-party/src/reaction_cmd.rs crates/kh2-game-core/src/shared/party.rs"
transcribed="crates/kh2-player/src/sora.rs crates/kh2-game-core/src/skel.rs crates/kh2-game-core/src/camera.rs crates/kh2-player/src/drive.rs crates/kh2-player/src/magic.rs crates/kh2-game-core/src/combat.rs crates/kh2-game-core/src/shared/sora.rs crates/kh2-party/src/party.rs crates/kh2-enemy/src/enemy_ai.rs crates/kh2-enemy/src/enemy.rs crates/kh2-player/src/magic.rs"
deps="crates/kh2-bdx crates/kh2-anim crates/kh2-map crates/kh2-model crates/kh2-tables crates/kh2-archive crates/kh2-bar crates/kh2-iso crates/kh2-ps2 crates/kh2-sheets sheets"
head=$(git -C "$kh2rs" rev-parse HEAD)
echo "kh2-rs pinned $pinned, now $head"
status=0
for group in compiled transcribed deps; do
    eval files=\$$group
    out=$(git -C "$kh2rs" diff --stat "$pinned" -- $files; git -C "$kh2rs" status --short -- $files)
    if [ -n "$out" ]; then
        echo "== $group: changed"
        echo "$out"
        status=1
    else
        echo "== $group: unchanged"
    fi
done
if [ $status = 1 ]; then
    echo "Drift: rebuild, run the tests (with KH2_ISO), review the transcribed files, then update KH2RS_COMMIT."
fi
exit $status
