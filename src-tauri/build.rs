use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    build_eocc_hook_binary();
    tauri_build::build()
}

/// Build the standalone eocc-hook binary from eocc-core and place it in OUT_DIR
/// so that setup.rs can embed it via `include_bytes!`.
///
/// Uses a separate `--target-dir` to avoid deadlocking with the parent cargo
/// process that is compiling this crate. This is the documented approach:
/// <https://doc.rust-lang.org/cargo/reference/build-scripts.html>
fn build_eocc_hook_binary() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

    let eocc_core_manifest = manifest_dir
        .join("crates")
        .join("eocc-core")
        .join("Cargo.toml");

    // Separate target directory avoids cargo build-lock deadlock
    let hook_target_dir = out_dir.join("eocc-hook-build");

    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let profile_dir = if profile == "release" {
        "release"
    } else {
        "debug"
    };

    let mut cmd = Command::new(&cargo);
    cmd.arg("build")
        .arg("--manifest-path")
        .arg(&eocc_core_manifest)
        .arg("--bin")
        .arg("eocc-hook")
        .arg("--features")
        .arg("headless")
        .arg("--target-dir")
        .arg(&hook_target_dir);

    if profile == "release" {
        cmd.arg("--release");
    }

    let status = cmd
        .status()
        .expect("Failed to invoke cargo for eocc-hook build");

    if !status.success() {
        panic!("Failed to build eocc-hook binary");
    }

    let binary_name = if cfg!(target_os = "windows") {
        "eocc-hook.exe"
    } else {
        "eocc-hook"
    };

    let src = hook_target_dir.join(profile_dir).join(binary_name);
    let dst = out_dir.join(binary_name);

    std::fs::copy(&src, &dst).unwrap_or_else(|e| {
        panic!(
            "Failed to copy eocc-hook binary from {:?} to {:?}: {}",
            src, dst, e
        );
    });

    // Rebuild when hook source or core library changes
    println!("cargo:rerun-if-changed=crates/eocc-core/src/bin/eocc-hook.rs");
    println!("cargo:rerun-if-changed=crates/eocc-core/src/hook_state.rs");
    println!("cargo:rerun-if-changed=crates/eocc-core/src/lib.rs");
}
