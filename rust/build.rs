/// Build script to compile the C++ audio engine.
/// This uses cmake to build the C++ library and link it to Rust.
use std::env;
use std::path::PathBuf;

fn main() {
    // Get the workspace root (two levels up from rust/)
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap();

    // Build the C++ library using CMake
    let dst = cmake::Config::new(workspace_root).define("CMAKE_BUILD_TYPE", "Release").build();

    // Tell cargo to link the static library
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=joduga_audio");

    // Static C++ library needs the C++ standard library at link time.
    // Use CARGO_CFG_TARGET_OS (runtime env) instead of #[cfg(target_os)]
    // so the detection is always correct regardless of host/target combo.
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match target_os.as_str() {
        "linux" => {
            println!("cargo:rustc-link-lib=stdc++");
            println!("cargo:rustc-link-lib=pthread");
            println!("cargo:rustc-link-lib=rt");
        }
        "macos" => {
            println!("cargo:rustc-link-lib=c++");
            println!("cargo:rustc-link-lib=framework=CoreFoundation");
        }
        "windows" => {
            println!("cargo:rustc-link-lib=winmm");
        }
        _ => {}
    }

    // Rerun if CMakeLists.txt or any C++ file changes
    println!("cargo:rerun-if-changed=../CMakeLists.txt");
    println!("cargo:rerun-if-changed=../cpp/");
}
