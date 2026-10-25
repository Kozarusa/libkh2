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

/// One tick's state as a line (everything the controller decides: state, motion, frame,
/// position, velocity, yaw, combo, HP, MP, form, drive points, invincibility).
pub fn snapshot(s: &Sora) -> String {
    let (p, v) = (s.motor.pos, s.motor.vel);
    format!(
        "{} {} {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} {:.2} {} {} {} {} {} {}",
        s.state.code(),
        s.body.tag(),
        s.body.frame,
        p.x,
        p.y,
        p.z,
        v.x,
        v.z,
        s.motor.yaw,
        s.hits,
        s.hp,
        s.mp,
        s.drive.form,
        s.drive.points.whole,
        (s.invuln > 0.0) as u8
    )
}

/// One tick's enemies and party members as a line: each enemy's id, HP, state, motion slot, frame,
/// position and yaw; each member's position, yaw and motion slot.
pub fn battle_snapshot(en: &crate::enemy::Enemies, party: &[crate::party::Follower]) -> String {
    let mut t = String::new();
    for e in &en.list {
        let p = e.pos();
        let _ = write!(t, "e{} {} {:?} {:?} {:.2} {:.2} {:.2} {:.2} {:.2};", e.id, e.hp, e.state, e.anim.slot, e.anim.frame, p.x, p.y, p.z, e.obj.motor.yaw);
    }
    for f in party {
        let p = f.motor.pos;
        let _ = write!(t, "m{} {:.2} {:.2} {:.2} {:.2} {:?};", f.member, p.x, p.y, p.z, f.motor.yaw, f.body.slot);
    }
    t
}

/// Per-tick digests of a scenario: one `tick digest` line per tick (digests only, so a golden file
/// built on the disc carries no game values).
pub fn digests(lines: &[String]) -> String {
    lines.iter().enumerate().map(|(i, l)| format!("{i} {:016x}\n", digest(l))).collect()
}

/// Compares per-tick digests with a golden file: `Ok(true)` equal, `Ok(false)` written (missing or
/// `KH2_BLESS` set), `Err` the first tick that differs and its snapshot.
pub fn check_golden(path: &std::path::Path, lines: &[String]) -> Result<bool, String> {
    let now = digests(lines);
    if std::env::var_os("KH2_BLESS").is_some() || !path.exists() {
        if let Some(d) = path.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        std::fs::write(path, &now).map_err(|e| e.to_string())?;
        return Ok(false);
    }
    let old = std::fs::read_to_string(path).map_err(|e| e.to_string())?.replace(['\r'], "");
    match old.lines().zip(now.lines()).position(|(a, b)| a != b) {
        None if old.lines().count() == now.lines().count() => Ok(true),
        None => Err(format!("{}: length {} -> {}", path.display(), old.lines().count(), now.lines().count())),
        Some(t) => Err(format!("{}: first difference at tick {t}: now {:?}", path.display(), lines[t])),
    }
}
