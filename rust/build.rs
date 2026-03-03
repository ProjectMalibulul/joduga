/// Build script to compile the C++ audio engine.
/// This uses cmake to build the C++ library and link it to Rust.
use std::env;
use std::path::PathBuf;

fn main() {
    // Get the workspace root (two levels up from rust/)
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir.parent().unwrap();

    // Build the C++ library using CMake
    let dst = cmake::Config::new(workspace_root)
        .define("CMAKE_BUILD_TYPE", "Release")
        .build();

    // Tell cargo to link the static library
    println!("cargo:rustc-link-search=native={}/lib", dst.display());
    println!("cargo:rustc-link-lib=static=joduga_audio");

    // Static C++ library needs the C++ standard library at link time
    #[cfg(target_os = "linux")]
    {
        println!("cargo:rustc-link-lib=stdc++");
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=rt");
    }

    #[cfg(target_os = "macos")]
    {
        println!("cargo:rustc-link-lib=c++");
    }

    #[cfg(target_os = "windows")]
    {
        println!("cargo:rustc-link-lib=winmm");
    }

    // Rerun if CMakeLists.txt or any C++ file changes
    println!("cargo:rerun-if-changed=../CMakeLists.txt");
    println!("cargo:rerun-if-changed=../cpp/");
}
