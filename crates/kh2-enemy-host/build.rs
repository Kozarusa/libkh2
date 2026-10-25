//! Lists kh2-rs's enemy kind files (`kh2-enemy/src/kinds/*.rs`, one `Kind` constant each) into
//! `$OUT_DIR/kinds.rs`, as kh2-enemy's own build script does, so `KINDS` is kh2-rs's list.

use std::fmt::Write as _;
use std::path::Path;

fn main() {
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
