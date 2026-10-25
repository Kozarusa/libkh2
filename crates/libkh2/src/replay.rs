//! Scripted input recordings and their traces, for regression tests and the replay example.
//!
//! A script is text, one step per line: `<ticks> <buttons> <stick_x> <stick_y> [cam_x cam_z]`.
//! `buttons` is any of `a` (attack), `j` (jump), `g` (guard) or `-`. Lines starting with `#` are
//! comments. Example: `30 - 0 1` runs forward for 30 ticks, `1 a 0 0` presses attack.

use std::fmt::Write;

use crate::sora::{Input, Sora};
use crate::World;

/// One step of a script.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub ticks: u32,
    pub input: Input,
}

pub fn parse(script: &str) -> Result<Vec<Step>, String> {
    let mut out = Vec::new();
    for (n, line) in script.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize, d: f32| -> Result<f32, String> { f.get(i).map_or(Ok(d), |v| v.parse().map_err(|_| format!("line {}: {v:?} is not a number", n + 1))) };
        let ticks = f.first().ok_or("empty")?.parse().map_err(|_| format!("line {}: bad tick count", n + 1))?;
        let b = f.get(1).copied().unwrap_or("-");
        let input = Input { stick: [num(2, 0.0)?, num(3, 0.0)?], cam_look: [num(4, 0.0)?, num(5, 1.0)?], attack: b.contains('a'), jump: b.contains('j'), guard: b.contains('g') };
        out.push(Step { ticks, input });
    }
    Ok(out)
}

/// Runs a script and returns the trace: one line per tick with the state, motion, frame,
/// position, yaw and combo. Floats are printed to 0.01, so tiny float drift does not change it.
pub fn run(sora: &mut Sora, world: &World, steps: &[Step]) -> String {
    let mut t = String::new();
    let mut tick = 0;
    for s in steps {
        for _ in 0..s.ticks {
            sora.tick(world, &s.input);
            if !sora.attack_triggers().is_empty() {
                sora.connected = true;
            }
            let p = sora.motor.pos;
            let _ = writeln!(t, "{tick} {} {} {:.2} {:.2} {:.2} {:.2} {:.2} {}", sora.state.code(), sora.body.tag(), sora.body.frame, p.x, p.y, p.z, sora.motor.yaw, sora.hits);
            tick += 1;
        }
    }
    t
}

/// FNV-1a of a trace (what the golden files keep for disc runs: no game values).
pub fn digest(trace: &str) -> u64 {
    trace.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3))
}
