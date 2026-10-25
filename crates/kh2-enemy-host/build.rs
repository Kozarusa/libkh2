//! Lists kh2-rs's enemy kind files (`kh2-enemy/src/kinds/*.rs`, one `Kind` constant each) into
//! `$OUT_DIR/kinds.rs`, as kh2-enemy's own build script does, so `KINDS` is kh2-rs's list.

use std::fmt::Write as _;
use std::path::Path;

/// The index just past the brace that closes the one opened before `from` (depth 1 at `from`);
/// comments and string literals are skipped.
fn close(b: &[u8], from: usize) -> usize {
    let (mut depth, mut i) = (1i32, from);
    while i < b.len() && depth > 0 {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != 10 {
                    i += 1;
                }
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    if b[i] == 0x5c {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    i
}

/// The text from `start` (kept) to the end of the first item named by `last` after it: a block
/// (brace-matched) or a `;`-ended item, whichever its declaration has first.
fn span(text: &str, start: &str, last: &str) -> String {
    let a = text.find(start).unwrap_or_else(|| panic!("{start:?} not found"));
    let f = a + text[a..].find(last).unwrap_or_else(|| panic!("{last:?} not found"));
    let brace = text[f..].find('{').map(|x| f + x);
    let semi = text[f..].find(';').map(|x| f + x);
    match (brace, semi) {
        (Some(b), Some(s)) if s < b && !last.contains("fn ") => text[a..s + 1].to_string(),
        (Some(b), _) => text[a..close(text.as_bytes(), b + 1)].to_string(),
        (None, Some(s)) => text[a..s + 1].to_string(),
        _ => panic!("{last:?} has no body"),
    }
}

/// kh2-rs's limit rules (`kh2-party/src/limit.rs`: rows, usable / available, Auto Limit, the task's
/// events, hit count, timer), `limit_visual.rs`'s two request records and `limit_script.rs`'s script
/// thread (`ScriptRun` and its host), cut from their Bevy files: the spans named below, the Bevy
/// systems and the `Progress` method left out.
fn limits() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-party/src");
    let read = |f: &str| {
        let p = base.join(f);
        println!("cargo:rerun-if-changed={}", p.display());
        std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("{f}"))
    };
    let l = read("limit.rs");
    let mut out = String::new();
    for (a, b) in [
        ("/// Item type of an ability", "pub fn row_for_command"),
        ("/// The ability item of Auto Limit", "pub fn auto_limit_pick"),
        ("/// The Execute 81 commands that no", "pub fn sub_commands"),
        ("/// Sent when a limit starts.", "pub struct LimitStarted"),
        ("/// Entry kinds of a limit's resource BAR", "pub fn sub_command_event"),
        ("/// The limit task (`DAT_01d5d6b0`", "pub struct LimitTask"),
        ("/// The running limit's hit count", "pub struct LimitHits"),
        ("impl LimitHits {", "pub fn add"),
        ("/// How many of the enemy hits of a frame", "pub fn counted_hits"),
        ("/// The whole timer ticks one frame removes", "pub fn timer_ticks"),
    ] {
        out.push_str(&span(&l, a, b));
        if a == "impl LimitHits {" {
            out.push_str("\n}");
        }
        out.push_str("\n\n");
    }
    let v = read("limit_visual.rs");
    let mut vis = span(&v, "/// One call of host 2.47 as the app plays it.", "pub struct LimitMotion");
    vis.push_str("\n\n");
    vis.push_str(&span(&v, "/// One call of host 2.59 as the app plays it", "pub struct LimitEffect"));
    let s = read("limit_script.rs");
    let script = span(&s, "/// The stand-in for the limit task object", "pub fn script_of");
    let dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(Path::new(&dir).join("limit.rs"), out).unwrap();
    std::fs::write(Path::new(&dir).join("limit_visual.rs"), vis).unwrap();
    std::fs::write(Path::new(&dir).join("limit_script.rs"), script).unwrap();
}

/// kh2-rs `kh2-player/src/player_reaction.rs`'s pure span: the paired motions of a motion set
/// (`Paired`, `paired_entries`, `sub_name`, `RELEASE_AFTER`).
fn player_reaction() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-player/src/player_reaction.rs");
    println!("cargo:rerun-if-changed={}", p.display());
    let s = std::fs::read_to_string(&p).expect("player_reaction.rs");
    let mut out = span(&s, "/// Ticks Sora holds the last frame of a paired motion", "const RELEASE_AFTER");
    out.push_str("\n\n");
    out.push_str(&span(&s, "/// One paired sub-motion", "pub type Paired"));
    out.push_str("\n\n");
    out.push_str(&span(&s, "/// The paired player motions of a motion set", "pub fn paired_entries"));
    out.push_str("\n\n");
    out.push_str(&span(&s, "/// The sub-motion name for a player model byte", "pub fn sub_name"));
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("player_reaction.rs"), out).unwrap();
}

/// The two small pieces `friend_host.rs` names outside itself: `party.rs`'s `SpecialOn` and
/// `fx.rs`'s `hit_rotation`.
fn friend_bits() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates");
    let read = |f: &str| {
        let p = base.join(f);
        println!("cargo:rerun-if-changed={}", p.display());
        std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("{f}"))
    };
    let dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(Path::new(&dir).join("special_on.rs"), span(&read("kh2-party/src/party.rs"), "/// Whom a special's target record names", "pub enum SpecialOn")).unwrap();
    std::fs::write(Path::new(&dir).join("hit_rotation.rs"), span(&read("kh2-game-core/src/fx.rs"), "pub fn hit_rotation", "pub fn hit_rotation")).unwrap();
}

/// kh2-rs `kh2-party/src/summon.rs`'s pure modules: from `pub mod cmd {` to the end of `pub mod rules`
/// (the command ids, motions, the summon AI object `ai`, Stitch's, Genie's and the scripted classes,
/// the rules); the Bevy systems after them are left out.
fn summon_rules() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-party/src/summon.rs");
    println!("cargo:rerun-if-changed={}", p.display());
    let s = std::fs::read_to_string(&p).expect("summon.rs");
    let a = s.find("pub mod cmd {").expect("summon.rs cmd");
    let r = s.find("pub mod rules {").expect("summon.rs rules");
    let end = close(s.as_bytes(), r + "pub mod rules {".len());
    let mut doc_start = a;
    // Keep the doc comment lines above `pub mod cmd`.
    while let Some(prev) = s[..doc_start.saturating_sub(1)].rfind('\n') {
        if s[prev + 1..doc_start].trim_start().starts_with("///") {
            doc_start = prev + 1;
        } else {
            break;
        }
    }
    let mut text = s[doc_start..end].to_string();
    // FUN_002f6320's locomotion choice (a free function after the modules).
    text.push_str("\n\n");
    text.push_str(&span(&s, "fn locomotion_want", "fn locomotion_want").replacen("fn locomotion_want", "pub fn locomotion_want", 1));
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("summon_rules.rs"), text).unwrap();
    // `shared/summon.rs`'s two pure names the modules use: `KidOp` and `GENIE_NEO`.
    let q = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-game-core/src/shared/summon.rs");
    println!("cargo:rerun-if-changed={}", q.display());
    let t = std::fs::read_to_string(&q).expect("shared/summon.rs");
    let mut out = span(&t, "pub enum KidOp", "pub enum KidOp");
    let k = t[..t.find("pub enum KidOp").unwrap()].rfind("#[derive").unwrap();
    out = format!("{}{}", &t[k..t.find("pub enum KidOp").unwrap()], out);
    out.push(char::from(10u8));
    out.push_str(&span(&t, "pub const GENIE_NEO", "pub const GENIE_NEO"));
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("shared_summon.rs"), out).unwrap();
}

fn main() {
    summon_rules();
    friend_bits();
    player_reaction();
    limits();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-enemy/src/kinds");
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut stems: Vec<String> = std::fs::read_dir(&dir)
        .expect("kh2-enemy/src/kinds")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_string))
        .collect();
    stems.sort();
    let mut out = String::from("pub mod kinds {\n");
    for s in &stems {
        let path = dir.join(format!("{s}.rs"));
        println!("cargo:rerun-if-changed={}", path.display());
        let _ = writeln!(out, "    #[path = {:?}]\n    pub mod {s};", path.to_string_lossy().replace('\\', "/"));
    }
    out.push_str("}\n\n/// Every enemy kind of kh2-rs (its `src/kinds/` files, by file name).\npub static KINDS: &[&crate::shared::enemy::Kind] = &[");
    for (i, s) in stems.iter().enumerate() {
        let _ = write!(out, "{}&kinds::{s}::KIND", if i == 0 { "" } else { ", " });
    }
    out.push_str("];\n");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("kinds.rs"), out).unwrap();
}
