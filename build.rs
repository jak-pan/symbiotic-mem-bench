use std::path::{Path, PathBuf};

fn main() {
    // The adapter links zvec's C API dynamically. zvec-sys emits an rpath from
    // its own build script, but a build script's link args do not propagate to
    // a downstream binary, so `membench` itself must carry the rpath — without
    // it the CLI links fine and then dies at startup with a dyld error.
    println!("cargo:rerun-if-env-changed=ZVEC_LIB_DIR");
    if let Some(dir) = zvec_lib_dir() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{}", dir.display());
    }

    // Bake the current git commit into the binary so the server can report the
    // exact code it was built from (refreshed whenever HEAD changes).
    println!("cargo:rerun-if-changed=.git/HEAD");
    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_SHA={sha}");

    compile_contract_protos();
}

/// Locate the directory holding zvec's shared library, mirroring zvec-sys's own
/// resolution order:
///
/// 1. `ZVEC_LIB_DIR` — the documented override.
/// 2. A sibling `symbiotic-memory` checkout, for co-development.
/// 3. The vendored copy inside the *pinned* cargo git checkout, which is what a
///    clean clone actually builds against.
///
/// Returns `None` when the library is nowhere to be found — a plain (non-adapter)
/// build does not link it, so that is not an error.
fn zvec_lib_dir() -> Option<PathBuf> {
    let vendored = |root: &Path| root.join("vendor/zvec-rust/vendor/lib");

    if let Ok(dir) = std::env::var("ZVEC_LIB_DIR") {
        let dir = PathBuf::from(dir);
        if has_zvec_lib(&dir) {
            return Some(dir);
        }
    }

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    if let Some(parent) = manifest_dir.parent() {
        let sibling = vendored(&parent.join("symbiotic-memory"));
        println!("cargo:rerun-if-changed={}", sibling.display());
        if has_zvec_lib(&sibling) {
            return Some(sibling);
        }
    }

    // ~/.cargo/git/checkouts/symbiotic-memory-<hash>/<short-rev>/vendor/...
    let cargo_home = std::env::var("CARGO_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".cargo"))
        })?;
    let checkouts = std::fs::read_dir(cargo_home.join("git/checkouts")).ok()?;
    for repo in checkouts.flatten() {
        if !repo
            .file_name()
            .to_string_lossy()
            .starts_with("symbiotic-memory-")
        {
            continue;
        }
        let Ok(revisions) = std::fs::read_dir(repo.path()) else {
            continue;
        };
        for revision in revisions.flatten() {
            let candidate = vendored(&revision.path());
            if has_zvec_lib(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn has_zvec_lib(dir: &Path) -> bool {
    ["libzvec_c_api.dylib", "libzvec_c_api.so"]
        .iter()
        .any(|name| dir.join(name).is_file())
}

/// Compiles the bench-owned contract schemas (proto/CONTRACTS.md) into
/// `membench::proto::*` via prost. protoc comes from protoc-bin-vendored so a
/// clean clone needs no system protobuf install.
fn compile_contract_protos() {
    let protoc = protoc_bin_vendored::protoc_bin_path().expect("vendored protoc");
    // SAFETY: build scripts are single-threaded at this point and no reader of
    // PROTOC runs concurrently; prost-build reads it when spawning protoc below.
    unsafe { std::env::set_var("PROTOC", protoc) };
    let files = [
        "proto/membench/trace/v1/trace.proto",
        "proto/membench/manifest/v1/manifest.proto",
        "proto/membench/scorecard/v1/scorecard.proto",
    ];
    for file in files {
        println!("cargo:rerun-if-changed={file}");
    }
    prost_build::Config::new()
        .compile_protos(&files, &["proto"])
        .expect("contract proto compilation failed");
}
