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
        (Some(b), Some(s)) if s < b => text[a..s + 1].to_string(),
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

fn main() {
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
