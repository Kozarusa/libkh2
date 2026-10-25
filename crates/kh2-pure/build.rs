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

fn main() {
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
