fn main() {
    // The `joduga` crate (dependency) already ran cmake and built
    // libjoduga_audio.so somewhere under target/<profile>/build/joduga-<hash>/out/lib/.
    // Tauri's resource check requires the file at ../external_libs/ relative to
    // this manifest dir.  Stage it there before calling tauri_build::build().
    stage_native_library();

    tauri_build::build();
}

/// Locate the cmake-built libjoduga_audio.so in the cargo build directory
/// and copy it to tauri-ui/external_libs/libjoduga_audio.so.0 so the Tauri
/// resource bundler can find it.
fn stage_native_library() {
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let external_libs = manifest_dir.join("..").join("external_libs");
    let dest = external_libs.join("libjoduga_audio.so.0");

    // If already staged (e.g. by CI pre-step), skip.
    if dest.exists() {
        println!("cargo:warning=libjoduga_audio.so.0 already staged");
        return;
    }

    fs::create_dir_all(&external_libs).ok();

    // OUT_DIR is something like target/<profile>/build/joduga-tauri-<hash>/out
    // We need to look in           target/<profile>/build/joduga-<hash>/out/lib/
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Walk up to the `build/` directory
    let build_dir: Option<&Path> = out_dir.ancestors().find(|p| {
        p.file_name()
            .map(|f| f.to_string_lossy() == "build")
            .unwrap_or(false)
    });

    if let Some(build_dir) = build_dir {
        if let Ok(entries) = fs::read_dir(build_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                // Match joduga-<hash> but NOT joduga-tauri-<hash>
                if name.starts_with("joduga-") && !name.contains("tauri") {
                    // Try the cmake install output first
                    let lib = entry.path().join("out").join("lib").join("libjoduga_audio.so");
                    if lib.exists() {
                        if let Ok(_) = fs::copy(&lib, &dest) {
                            println!("cargo:warning=Staged {:?} -> {:?}", lib, dest);
                            return;
                        }
                    }
                    // Try build/ subdirectory (some cmake configs)
                    let lib2 = entry.path().join("out").join("build").join("libjoduga_audio.so");
                    if lib2.exists() {
                        if let Ok(_) = fs::copy(&lib2, &dest) {
                            println!("cargo:warning=Staged {:?} -> {:?}", lib2, dest);
                            return;
                        }
                    }
                }
            }
        }
    }

    // Fallback: search the entire target directory tree
    let target_dir = out_dir.ancestors().find(|p| {
        p.file_name()
            .map(|f| f.to_string_lossy() == "target")
            .unwrap_or(false)
    });

    if let Some(target_dir) = target_dir {
        if let Some(lib) = find_file_recursive(target_dir, "libjoduga_audio.so") {
            if let Ok(_) = fs::copy(&lib, &dest) {
                println!("cargo:warning=Staged (fallback) {:?} -> {:?}", lib, dest);
                return;
            }
        }
    }

    println!("cargo:warning=Could not find libjoduga_audio.so to stage — Tauri resource bundling may fail");
}

fn find_file_recursive(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return None;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_name().map(|f| f == name).unwrap_or(false) {
            return Some(path);
        }
        if path.is_dir() {
            // Skip .git, node_modules
            let dirname = path.file_name().unwrap_or_default().to_string_lossy();
            if dirname.starts_with('.') || dirname == "node_modules" {
                continue;
            }
            if let Some(found) = find_file_recursive(&path, name) {
                return Some(found);
            }
        }
    }
    None
}
