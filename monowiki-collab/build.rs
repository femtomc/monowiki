//! Build script to compile the editor UI before embedding.
//!
//! Runs `bun run build` in the editor directory if the dist is missing or stale.

use std::path::Path;
use std::process::Command;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let editor_dir = Path::new(&manifest_dir).join("../editor");
    let dist_dir = editor_dir.join("dist");
    let src_dir = editor_dir.join("src");

    // Rerun if editor source changes
    println!("cargo:rerun-if-changed={}", src_dir.display());
    println!("cargo:rerun-if-changed={}", editor_dir.join("index.html").display());
    println!("cargo:rerun-if-changed={}", editor_dir.join("package.json").display());

    // Check if dist exists and has files
    let needs_build = !dist_dir.exists()
        || !dist_dir.join("index.html").exists()
        || is_src_newer_than_dist(&src_dir, &dist_dir);

    if needs_build {
        println!("cargo:warning=Building editor UI with bun...");

        let status = Command::new("bun")
            .args(["run", "build"])
            .current_dir(&editor_dir)
            .status()
            .expect("Failed to run bun. Is bun installed?");

        if !status.success() {
            panic!("Editor build failed. Run `bun run build` in /editor manually.");
        }
    }
}

/// Check if any source file is newer than the dist directory.
fn is_src_newer_than_dist(src_dir: &Path, dist_dir: &Path) -> bool {
    let dist_mtime = match dist_dir.metadata().and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return true,
    };

    fn check_dir(dir: &Path, reference: std::time::SystemTime) -> bool {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(meta) = path.metadata() {
                    if let Ok(mtime) = meta.modified() {
                        if mtime > reference {
                            return true;
                        }
                    }
                }
                if path.is_dir() && check_dir(&path, reference) {
                    return true;
                }
            }
        }
        false
    }

    check_dir(src_dir, dist_mtime)
}
