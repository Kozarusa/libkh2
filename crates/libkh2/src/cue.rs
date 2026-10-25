//! Sound and effect cues for the host (round 9): what the original plays or starts from a motion's
//! frame triggers, as kh2-rs's `audio::motion_sounds` / `motion_voices`, `audio` footsteps and
//! `fx::motion_effects` read them: trigger 8 a sound of the object's own bank (bank, index),
//! 13 / 14 a voice line (line, priority), 1 an element of the object's own effect file, 2 a
//! footstep sound (kind), 19 footstep dust (kind). The library plays nothing; the cue says who,
//! what and where, and the host plays it (docs/SOUND-AND-EFFECTS.md maps the numbers to files).

use glam::Vec3;
use kh2_anim::triggers::frame;

use crate::anim::Anim;

/// Who a cue belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Owner {
    Sora,
    /// A party member (slot in the follower list).
    Member(u32),
    /// An enemy (its id).
    Enemy(u32),
    /// A summon object (its slot).
    Summon(u32),
}

impl Owner {
    pub fn code(self) -> (i32, i32) {
        match self {
            Owner::Sora => (0, 0),
            Owner::Member(k) => (1, k as i32),
            Owner::Enemy(k) => (2, k as i32),
            Owner::Summon(k) => (3, k as i32),
        }
    }
}

/// What a cue asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Frame trigger 8: sound `index` of bank `bank` of the owner's effect file (`obj/<model>.apdx`).
    Sound = 1,
    /// Frame triggers 13 / 14: voice line `line` (priority for enemies).
    Voice = 2,
    /// Frame trigger 1: element `id` of the owner's effect file, bound to the owner.
    Effect = 3,
    /// Frame trigger 2: a footstep of `kind` (the floor's material picks the sample).
    Footstep = 4,
    /// Frame trigger 19: footstep dust of `kind`.
    Dust = 5,
    /// A script started element `params[0]` of the owner's effect file (enemy host effect calls,
    /// the limit script's 2.59 / 2.50: `params[1]` the limit's `who` word low half).
    ScriptEffect = 6,
    /// An interface or object sound by its number (`params[0..2]` = number low / high 16 bits): the
    /// mission's start / end / failure sounds and the orbs' bounce and pickup.
    SoundNumber = 7,
    /// A script switched collision group `params[0]` on (`params[1]` 1) or off (0): the library's
    /// collision follows; the host shows or hides that mesh group.
    CollisionGroup = 8,
}

/// One cue.
#[derive(Clone, Debug, PartialEq)]
pub struct Cue {
    pub kind: Kind,
    pub owner: Owner,
    /// The owner's model (`P_EX100`, `M_EX010`, ...): its effect file is `obj/<model>.apdx`.
    pub model: String,
    /// The trigger's parameters (up to four).
    pub params: [i16; 4],
    /// The owner's position (cm).
    pub pos: Vec3,
    /// The motion frame of the trigger.
    pub frame: i16,
}

/// The cues of the frame triggers `anim`'s last tick crossed (kh2-rs's `t.crossed(prev, now)` over
/// the motion's global frames).
pub fn of_anim(anim: &Anim, owner: Owner, model: &str, pos: Vec3) -> Vec<Cue> {
    let (Some(t), Some(m)) = (anim.triggers(), anim.motion()) else { return Vec::new() };
    if t.frames.is_empty() || anim.frame == anim.prev_frame {
        return Vec::new();
    }
    let g = |f: f32| f * crate::anim::LOGIC_HZ / m.fps.max(1.0);
    let mut out = Vec::new();
    for ft in t.crossed(g(anim.prev_frame), g(anim.frame)) {
        let kind = match ft.id {
            frame::SOUND_EFFECT => Kind::Sound,
            frame::VOICE | frame::VOICE2 => Kind::Voice,
            frame::EFFECT_CASTER => Kind::Effect,
            frame::FOOTSTEP_SOUND => Kind::Footstep,
            frame::FOOTSTEP_EFFECT => Kind::Dust,
            _ => continue,
        };
        let mut params = [0i16; 4];
        for (d, s) in params.iter_mut().zip(ft.params.iter()) {
            *d = *s;
        }
        out.push(Cue { kind, owner, model: model.to_string(), params, pos, frame: ft.frame });
    }
    out
}
