//! Extracts the pure `rules` module of kh2-rs `kh2-game-core/src/shared/drive.rs` (the rest of
//! that file is Bevy and disc glue) into OUT_DIR, so `drive_rules` compiles kh2-rs's own text.
//! The module is found by its `pub mod rules {` line and ends at its matching brace (braces in
//! comments and string literals are skipped).

use std::path::Path;

fn main() {
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
