//! kh2-rs's enemy script host without Bevy.
//!
//! The enemies run under the game's own BDX scripts through `kh2_bdx` (pure, its interpreter is
//! checked against the original in kh2-rs). The script talks to the world through
//! `kh2-enemy/src/enemy_host.rs`'s `BevyHost`, which answers every host call from the scripted
//! object (`shared/enemy_obj.rs`'s `Obj`) and an `Env` of the world. Those files, `shared/enemy.rs`,
//! `shared/reaction.rs`, `kh2-enemy/src/enemy_obj.rs` and the enemy kind files are compiled here
//! from kh2-rs's source. The few crate paths they name outside themselves are provided below:
//! the pure modules of kh2-game-core (from `libkh2-pure`) and small stand-ins for the parts of
//! kh2-rs that are Bevy systems (the trace, the voice queue, the story flags, `Stats`, `Rig`).

pub use kh2_game_core::{magic_rules, motor, party_battle, party_rules, reaction_rules, rng, world};

/// kh2-rs `camera::rules` (the summon classes read its angle helpers).
pub mod camera {
    pub use kh2_game_core::camera_rules as rules;
}

/// kh2-rs `party::SpecialOn` (span by `build.rs`).
pub mod party {
    use bevy::prelude::*;
    include!(concat!(env!("OUT_DIR"), "/special_on.rs"));
}

/// kh2-rs `fx::hit_rotation` (span by `build.rs`).
pub mod fx {
    use bevy::prelude::*;
    include!(concat!(env!("OUT_DIR"), "/hit_rotation.rs"));
}

/// The ECS id stand-in the compiled files carry (`bevy::prelude::Entity` of the shim).
pub use bevy::prelude::Entity;

/// KH2 units are centimetres; kh2-rs's Bevy scene units are metres.
pub const SCALE: f32 = 0.01;

/// kh2-rs `data::Stats` (the status block a hit reads), the same fields.
pub mod data {
    #[derive(Clone, Copy, Debug, Default, PartialEq)]
    pub struct Stats {
        pub level: i32,
        pub max_hp: i32,
        pub strength: i32,
        pub magic: i32,
        pub defense: i32,
        pub ap: i32,
        pub max_mp: i32,
        pub max_taken: i32,
        pub min_taken: i32,
        pub max_dealt: i32,
        pub min_dealt: i32,
        pub weakness: [i32; 7],
    }

    /// kh2-rs `data::GameData`, the one table the friend host reads (`atkp`, for an attack's combo group).
    #[derive(Default)]
    pub struct GameData {
        pub atkp: Vec<kh2_tables::tables::Atkp>,
    }
}

/// kh2-rs `skel::Rig` without the effect file name: what the host reads of a model.
pub mod skel {
    pub struct Rig {
        pub model: kh2_model::Model,
        pub mset: kh2_anim::Mset,
    }
}

/// The enemy voice requests of host 1.262 (kh2-rs `audio::queue_enemy_voice`): queued for the
/// library to hand to the host as events.
pub mod audio {
    use std::sync::Mutex;

    #[derive(Clone, Debug, PartialEq)]
    pub struct EnemyVoiceCall {
        pub tag: String,
        pub line: i32,
        pub priority: i32,
        pub at: bevy::prelude::Vec3,
    }

    pub static VOICES: Mutex<Vec<EnemyVoiceCall>> = Mutex::new(Vec::new());

    pub fn queue_enemy_voice(c: EnemyVoiceCall) {
        VOICES.lock().unwrap_or_else(|e| e.into_inner()).push(c);
    }

    pub fn take_voices() -> Vec<EnemyVoiceCall> {
        std::mem::take(&mut *VOICES.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// kh2-rs `progress::shared_story`: no story is installed in libkh2, so every flag reads clear (the
/// host's own answer for that case: "a new game").
pub mod progress {
    pub struct Story;

    impl Story {
        pub fn set(&mut self, _id: u16) {}
        pub fn test(&self, _id: u16) -> bool {
            false
        }
    }

    pub struct StoryCell(std::sync::Mutex<Story>);

    impl StoryCell {
        pub fn lock(&self) -> std::sync::MutexGuard<'_, Story> {
            self.0.lock().unwrap_or_else(|e| e.into_inner())
        }
    }

    pub fn shared_story() -> Option<&'static StoryCell> {
        None
    }
}

pub mod shared {
    #[path = "../../../../../kh2-rs/crates/kh2-game-core/src/shared/enemy.rs"]
    pub mod enemy;
    #[path = "../../../../../kh2-rs/crates/kh2-game-core/src/shared/enemy_obj.rs"]
    pub mod enemy_obj;
    #[path = "../../../../../kh2-rs/crates/kh2-game-core/src/shared/reaction.rs"]
    #[allow(dead_code)]
    pub mod reaction;
    /// `shared/struggle.rs`: the mission manager's state (header, layouts, counters, clock, the end).
    #[path = "../../../../../kh2-rs/crates/kh2-game-core/src/shared/struggle.rs"]
    #[allow(dead_code, clippy::all)]
    pub mod struggle;
    /// `shared/prize.rs`: the prize records and constants the orbs use.
    #[path = "../../../../../kh2-rs/crates/kh2-game-core/src/shared/prize.rs"]
    #[allow(dead_code, clippy::all)]
    pub mod prize;
    /// `shared/summon.rs`'s `KidOp` and `GENIE_NEO` (span by `build.rs`).
    pub mod summon {
        use bevy::prelude::*;
        include!(concat!(env!("OUT_DIR"), "/shared_summon.rs"));
    }
}

#[path = "../../../../kh2-rs/crates/kh2-enemy/src/enemy_obj.rs"]
pub mod enemy_obj;

/// kh2-rs `enemy_ai::AiTrace` without the log output: it counts the host calls by class, so the
/// library can report which calls a script made that the host does not serve.
pub mod enemy_ai {
    use std::collections::BTreeMap;

    use kh2_bdx::{Call, Value};

    use crate::enemy_host::{call_id, St};

    #[derive(Default)]
    pub struct AiTrace {
        pub level: u8,
        pub tick: u64,
        pub calls: BTreeMap<(u16, u16), (St, u64)>,
        pub notes: Vec<String>,
    }

    impl AiTrace {
        pub fn line(&self, _tag: &str, _msg: &str) {}

        pub fn note(&mut self, _tag: &str, msg: String) {
            if !self.notes.contains(&msg) {
                self.notes.push(msg);
            }
        }

        pub fn record(&mut self, _tag: &str, c: &Call, _v: &Value, st: St, _why: &str) {
            let e = self.calls.entry(call_id(c)).or_insert((st, 0));
            e.1 += 1;
        }

        /// The calls of class `Missing` seen so far (group, index, count).
        pub fn missing(&self) -> Vec<(u16, u16, u64)> {
            self.calls.iter().filter(|(_, (st, _))| *st == St::Missing).map(|(k, (_, n))| (k.0, k.1, *n)).collect()
        }
    }
}

#[path = "../../../../kh2-rs/crates/kh2-enemy/src/enemy_host.rs"]
#[allow(dead_code, clippy::all)]
pub mod enemy_host;

include!(concat!(env!("OUT_DIR"), "/kinds.rs"));

/// kh2-rs `kh2-party/src/friend_host.rs`: the party members' script host (the enemy host plus host group 10).
#[path = "../../../../kh2-rs/crates/kh2-party/src/friend_host.rs"]
#[allow(dead_code, unused_imports, clippy::all)]
pub mod friend_host;

/// kh2-rs `kh2-player/src/player_reaction.rs`'s paired motion reader (span by `build.rs`).
#[allow(dead_code, clippy::all)]
pub mod player_reaction {
    use std::collections::HashMap;
    use std::sync::Arc;

    use kh2_anim::{Slot, Triggers};
    include!(concat!(env!("OUT_DIR"), "/player_reaction.rs"));
    /// `RELEASE_AFTER` for the library.
    pub const RELEASE_TICKS: u32 = RELEASE_AFTER;
}

/// kh2-rs `kh2-party/src/limit.rs`'s rules (spans by `build.rs`).
#[allow(dead_code, unused_imports, clippy::all)]
pub mod limit {
    use bevy::prelude::*;
    use kh2_tables::player;
    use kh2_tables::tables as t;
    include!(concat!(env!("OUT_DIR"), "/limit.rs"));
}

/// kh2-rs `limit_visual.rs`'s request records.
#[allow(dead_code, clippy::all)]
pub mod limit_visual {
    use bevy::prelude::*;
    include!(concat!(env!("OUT_DIR"), "/limit_visual.rs"));
}

/// kh2-rs `limit_script.rs`: the limit BAR's own script thread and its host.
#[allow(dead_code, unused_imports, clippy::all)]
pub mod limit_script {
    use std::collections::BTreeMap;

    use bevy::prelude::*;
    use kh2_bdx::{Call, Host, HostTable, Program, Runtime, Value};

    use crate::limit::{LimitArg, LimitReceiver, LimitScriptEvent, LimitStarted, LimitTask};
    include!(concat!(env!("OUT_DIR"), "/limit_script.rs"));
}

/// kh2-rs `assets::Disc` for the compiled area code: the archive behind a lock, `read` answering a file.
pub mod assets {
    pub struct Disc(pub std::sync::Mutex<kh2_archive::Archive>);

    impl Disc {
        pub fn read(&self, name: &str) -> Option<Vec<u8>> {
            self.0.lock().ok()?.read(name).ok()
        }
    }
}

/// kh2-rs `enemy.rs`'s area battle groups (span by `build.rs`).
#[allow(dead_code, unused_imports, clippy::all)]
pub mod area {
    use std::collections::HashMap;
    use std::sync::Arc;

    use bevy::prelude::*;

    use crate::assets::Disc;
    use crate::shared::enemy::Kind;
    use crate::world::World;
    use crate::{KINDS, SCALE};
    include!(concat!(env!("OUT_DIR"), "/area_groups.rs"));
}

/// kh2-rs `summon.rs`'s pure modules (command ids, the summon AI object, Stitch's / Genie's / the scripted
/// classes, the rules), cut by `build.rs`.
#[allow(dead_code, unused_imports, clippy::all)]
pub mod summon {
    use bevy::prelude::*;
    include!(concat!(env!("OUT_DIR"), "/summon_rules.rs"));
}

/// kh2-rs `kh2-player/src/prize.rs`'s prize rules (span by `build.rs`): what the orbs run on.
#[allow(dead_code, unused_imports, clippy::all)]
pub mod prize {
    use bevy::prelude::*;

    use crate::world::World;
    pub use crate::shared::prize::{Consts, KindEntry, Phase, Prize, KINDS};
    include!(concat!(env!("OUT_DIR"), "/prize.rs"));
}

/// kh2-rs `kh2-event/src/struggle.rs`'s mission rules and the mission script's host (spans by `build.rs`).
#[allow(dead_code, unused_imports, clippy::all)]
pub mod struggle {
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use bevy::prelude::*;
    use kh2_bdx::{Call, Host, Program, Runtime, Value};

    use crate::prize::{self, Consts, Phase};
    use crate::reaction_rules as rules;
    use crate::shared::struggle::exe_bytes;
    pub use crate::shared::struggle::{exe_word, parse_header, Active, Clock, Counter, EV_CLOCK_END, EV_COUNTER_EMPTY, EV_COUNTER_FULL, EV_PLAYER_KO, EV_PLAYER_LOST, EV_PLAYER_WON, Element, Finish, GameOverExits, GameOverRequest, GlobalEvent, Layout, Mission, MissionStart, Orb};
    include!(concat!(env!("OUT_DIR"), "/struggle.rs"));
}
