//! Extracts the pure `rules` module of kh2-rs `kh2-game-core/src/shared/drive.rs` (the rest of
//! that file is Bevy and disc glue) into OUT_DIR, so `drive_rules` compiles kh2-rs's own text.
//! The module is found by its `pub mod rules {` line and ends at its matching brace (braces in
//! comments and string literals are skipped).

use std::path::Path;

/// The index just past the brace that closes the one opened before `from` (depth 1 at `from`).
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

/// kh2-rs `shared/abilities.rs` without its two non-pure ties: the `KINGDOM_KEY` import (its value,
/// the Kingdom Key's item id, is restated) and the methods over `progress::Progress` (each removed
/// with its doc comment, brace-matched).
fn abilities() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-game-core/src/shared/abilities.rs");
    println!("cargo:rerun-if-changed={}", src.display());
    let mut text = std::fs::read_to_string(&src).expect("kh2-rs abilities.rs");
    text = text.replace("use crate::data::KINGDOM_KEY;", "const KINGDOM_KEY: u16 = 41;").replace("//!", "//");
    let mut from = 0;
    while let Some(rel) = text[from..].find("progress::Progress") {
        let at = from + rel;
        let line_start = text[..at].rfind(char::from(10u8)).map_or(0, |i| i + 1);
        if text[line_start..at].trim_start().starts_with("//") {
            from = at + 1;
            continue;
        }
        assert!(text[line_start..at].contains("fn "), "a non-signature use of Progress in abilities.rs");
        let mut start = line_start;
        loop {
            let prev = text[..start.saturating_sub(1)].rfind(char::from(10u8)).map_or(0, |i| i + 1);
            if start == 0 || !text[prev..start].trim_start().starts_with("///") {
                break;
            }
            start = prev;
        }
        let open = at + text[at..].find('{').unwrap() + 1;
        let end = close(text.as_bytes(), open);
        text.replace_range(start..end, "");
        from = start;
    }
    if let Some(t) = text.find("#[cfg(test)]") {
        text.truncate(t);
    }
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("abilities.rs");
    std::fs::write(out, text).unwrap();
}

/// The text from `start` (a line marker, kept) to the end of the first `fn` item named `last_fn`
/// after it (brace-matched).
fn span(text: &str, start: &str, last_fn: &str) -> String {
    let a = text.find(start).unwrap_or_else(|| panic!("{start:?} not found"));
    let f = a + text[a..].find(last_fn).unwrap_or_else(|| panic!("{last_fn:?} not found"));
    let open = f + text[f..].find('{').unwrap() + 1;
    text[a..close(text.as_bytes(), open)].to_string()
}

/// kh2-rs's item effect (`kh2-party/src/items.rs`: `Vitals`, `item_effect`, oracle-tested there
/// against FUN_001a12a0) and the defeat payout (`kh2-game-core/src/progress.rs`: `defeat_payout`):
/// the pure spans of two Bevy files.
fn items() {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates");
    let items = base.join("kh2-party/src/items.rs");
    let progress = base.join("kh2-game-core/src/progress.rs");
    println!("cargo:rerun-if-changed={}", items.display());
    println!("cargo:rerun-if-changed={}", progress.display());
    let it = std::fs::read_to_string(&items).expect("items.rs");
    let pr = std::fs::read_to_string(&progress).expect("progress.rs");
    let mut out = span(&it, "/// The parts of a status record", "pub fn item_effect");
    out.push(char::from(10u8));
    out.push_str(&span(&pr, "pub fn defeat_payout", "pub fn defeat_payout"));
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("items.rs"), out).unwrap();
}

/// kh2-rs `kh2-party/src/party_battle.rs` (the party members' battle rules) without its one line that
/// re-exports two names of a Bevy module (`shared::party`), which nothing in the file uses.
fn party_battle() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-party/src/party_battle.rs");
    println!("cargo:rerun-if-changed={}", src.display());
    let text = std::fs::read_to_string(&src).expect("party_battle.rs");
    let line = "pub use crate::shared::party::{hp_change, HpChange};";
    assert!(text.contains(line), "party_battle.rs changed its re-export line");
    let text = text.replace(line, "").replace("//!", "//");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("party_battle.rs"), text).unwrap();
}

/// kh2-rs `kh2-game-core/src/rng.rs` (the game's generator, shared by the party members' choices)
/// with `word_of`, which reads a state file through kh2-rs's JSON reader, answering nothing (no
/// state file in libkh2: the generator starts from the game's boot seed).
fn rng() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-game-core/src/rng.rs");
    println!("cargo:rerun-if-changed={}", src.display());
    let mut text = std::fs::read_to_string(&src).expect("rng.rs");
    let sig = "fn word_of(text: &str) -> Option<u64> {";
    let a = text.find(sig).expect("rng.rs word_of");
    let end = close(text.as_bytes(), a + sig.len());
    text.replace_range(a..end, "fn word_of(_text: &str) -> Option<u64> {
    None
}");
    let text = text.replace("//!", "//");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("rng.rs"), text).unwrap();
}

/// kh2-rs `kh2-party/src/reaction_cmd.rs`'s pure span (the offer read from range trigger 20, the
/// `cmd` row fields, the availability status, the native gate), from its `TRIGGER` constant to
/// `window_at`; the one name it takes from a Bevy module (`shared::reaction::EXECUTE_EVENT`, 0x7f)
/// is restated, and the test-only disc scan is dropped.
fn reaction_cmd() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-party/src/reaction_cmd.rs");
    println!("cargo:rerun-if-changed={}", src.display());
    let text = std::fs::read_to_string(&src).expect("reaction_cmd.rs");
    let mut out = span(&text, "/// Range trigger id of the offer.", "pub fn window_at");
    let scan = out.find("#[cfg(test)]").expect("reaction_cmd.rs scan_mset");
    let open = scan + out[scan..].find('{').unwrap() + 1;
    let end = close(out.as_bytes(), open);
    out.replace_range(scan..end, "");
    let name = "crate::shared::reaction::EXECUTE_EVENT";
    assert!(out.contains(name), "reaction_cmd.rs changed its EXECUTE_EVENT use");
    let mut out = out.replace(name, "EXECUTE_EVENT");
    // The rules record, and the native offer of an object's reaction parts (FUN_0019cb70).
    out.push_str("\n\n");
    out.push_str(&span(&text, "/// The commands and message ids the offer needs", "pub struct ReactionRules"));
    out.push_str("\n\n");
    out.push_str(&span(&text, "impl ReactionRules {", "pub fn cmd"));
    out.push_str("\n}\n\n");
    out.push_str(&span(&text, "/// A reaction part with this id offers", "pub fn part_offer"));
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("reaction_cmd.rs"), out).unwrap();
}

/// kh2-rs `shared/party.rs`'s pure span: `HpChange` and `hp_change` (FUN_00186238, the party HP method).
fn party_hp() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-game-core/src/shared/party.rs");
    println!("cargo:rerun-if-changed={}", src.display());
    let text = std::fs::read_to_string(&src).expect("shared/party.rs");
    let out = span(&text, "/// What `FUN_00186238`", "pub fn hp_change");
    std::fs::write(Path::new(&std::env::var("OUT_DIR").unwrap()).join("party_hp.rs"), out).unwrap();
}

fn main() {
    party_hp();
    reaction_cmd();
    rng();
    party_battle();
    items();
    abilities();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../kh2-rs/crates/kh2-game-core/src/shared/drive.rs");
    println!("cargo:rerun-if-changed={}", src.display());
    let text = std::fs::read_to_string(&src).expect("kh2-rs drive.rs");
    let start = text.find("pub mod rules {").expect("`pub mod rules {` in drive.rs");
    let body_start = start + "pub mod rules {".len();
    let b = text.as_bytes();
    let (mut depth, mut i) = (1i32, body_start);
    while i < b.len() && depth > 0 {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
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
            b'\'' if b.get(i + 2) == Some(&b'\'') => i += 2,
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    assert_eq!(depth, 0, "unbalanced braces in drive.rs rules");
    let body = &text[body_start..i - 1];
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).join("drive_rules.rs");
    std::fs::write(out, body).unwrap();
}
