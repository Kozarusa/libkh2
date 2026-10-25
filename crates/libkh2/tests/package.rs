//! The release package (`scripts/package.ps1 -Zip`: `dist/libkh2` and `dist/libkh2.zip`) holds no
//! game data and no machine paths. Skipped (with a note) when the package was not built.
//!
//! Game data: file types of the disc (ISO, BAR / MDLX / MSET / BDX / ARD, the archive's IDX / IMG)
//! by name and by content (the ISO 9660 `CD001` mark, a BAR head `BAR\x01`). Machine paths: a
//! Windows user folder (`X:\Users\...`), `/home/<name>/`, `/Users/<name>/`.

use std::path::{Path, PathBuf};

const DISC_TYPES: &[&str] = &["iso", "bar", "mdlx", "mset", "bdx", "ard", "idx", "img", "bin", "a.us", "coct", "pax"];

fn files(d: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn disc_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    DISC_TYPES.iter().any(|t| n.ends_with(&format!(".{t}")))
}

/// The machine paths in `b` (as text runs), first few.
fn machine_paths(b: &[u8]) -> Vec<String> {
    let mut found = Vec::new();
    let low: Vec<u8> = b.iter().map(|c| c.to_ascii_lowercase()).collect();
    for (i, w) in low.windows(9).enumerate() {
        let hit = (w[0].is_ascii_alphabetic() && &w[1..9] == b":\\users\\") || w.starts_with(b"/home/") || (w.starts_with(b"/users/") && low.get(i + 7).is_some_and(|c| c.is_ascii_alphanumeric()));
        if hit {
            let end = (i + 60).min(b.len());
            found.push(String::from_utf8_lossy(&b[i..end]).into_owned());
            if found.len() >= 3 {
                break;
            }
        }
    }
    found
}

/// The entry names of a zip's central directory.
fn zip_names(z: &[u8]) -> Vec<String> {
    let Some(eocd) = (0..z.len().saturating_sub(21)).rev().find(|&i| z[i..].starts_with(b"PK\x05\x06")) else { return Vec::new() };
    let u16_at = |o: usize| u16::from_le_bytes([z[o], z[o + 1]]) as usize;
    let u32_at = |o: usize| u32::from_le_bytes([z[o], z[o + 1], z[o + 2], z[o + 3]]) as usize;
    let (n, mut p) = (u16_at(eocd + 10), u32_at(eocd + 16));
    let mut names = Vec::new();
    for _ in 0..n {
        if !z[p..].starts_with(b"PK\x01\x02") {
            break;
        }
        let (nl, xl, cl) = (u16_at(p + 28), u16_at(p + 30), u16_at(p + 32));
        names.push(String::from_utf8_lossy(&z[p + 46..p + 46 + nl]).into_owned());
        p += 46 + nl + xl + cl;
    }
    names
}

#[test]
fn the_package_holds_no_game_data_and_no_machine_paths() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist");
    let dir = root.join("libkh2");
    if !dir.is_dir() {
        eprintln!("skipped: no dist/libkh2 (run scripts/package.ps1 -Zip)");
        return;
    }
    let mut all = Vec::new();
    files(&dir, &mut all);
    let mut bad = Vec::new();
    for f in &all {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let b = std::fs::read(f).unwrap();
        if disc_name(&name) {
            bad.push(format!("{name}: a disc file type"));
        }
        // An ISO's volume descriptor (sector 16; the DLL's own ISO reader holds the mark as a literal).
        if b.get(0x8001..0x8006) == Some(b"CD001") || b.starts_with(b"BAR\x01") {
            bad.push(format!("{name}: disc content (ISO mark or BAR head)"));
        }
        for p in machine_paths(&b) {
            bad.push(format!("{name}: machine path {p:?}"));
        }
    }
    let zip = root.join("libkh2.zip");
    if let Ok(z) = std::fs::read(&zip) {
        let names = zip_names(&z);
        assert_eq!(names.iter().filter(|n| !n.ends_with('/') && !n.ends_with('\\')).count(), all.len(), "the zip holds the folder's files: {names:?}");
        for n in names.iter().filter(|n| disc_name(n)) {
            bad.push(format!("zip entry {n}: a disc file type"));
        }
    }
    eprintln!("package: {} files checked", all.len());
    assert!(bad.is_empty(), "{bad:#?}");
}
