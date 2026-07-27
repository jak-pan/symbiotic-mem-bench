use std::path::{Path, PathBuf};

const ZVEC_VERIFICATION_MARKER: &str = ".membench-zvec-verified";

fn main() {
    // The adapter links zvec's C API dynamically. zvec-sys emits an rpath from
    // its own build script, but a build script's link args do not propagate to
    // a downstream binary, so `membench` itself must carry the rpath — without
    // it the CLI links fine and then dies at startup with a dyld error.
    println!("cargo:rerun-if-env-changed=ZVEC_LIB_DIR");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-changed=.symbiotic-memory-pin");
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

/// Accept only a package explicitly prepared and verified by
/// `scripts/prepare-adapter-zvec.sh`. A plain non-adapter build does not link
/// zvec, so it may omit `ZVEC_LIB_DIR`; an adapter build may not.
fn zvec_lib_dir() -> Option<PathBuf> {
    let adapter_enabled = std::env::var_os("CARGO_FEATURE_SYMBIOTIC_MEMORY_ADAPTER").is_some();
    let Some(configured) = std::env::var_os("ZVEC_LIB_DIR") else {
        if adapter_enabled {
            panic!("adapter builds require ZVEC_LIB_DIR from scripts/prepare-adapter-zvec.sh");
        }
        return None;
    };
    let configured = PathBuf::from(configured);
    let dir = configured.canonicalize().unwrap_or_else(|error| {
        panic!(
            "ZVEC_LIB_DIR {} is not an accessible verified package: {error}",
            configured.display()
        )
    });
    if !has_zvec_lib(&dir) {
        panic!(
            "ZVEC_LIB_DIR {} does not contain libzvec_c_api",
            dir.display()
        );
    }

    let target = std::env::var("TARGET").expect("Cargo must set TARGET for build scripts");
    let pin = include_str!(".symbiotic-memory-pin").trim();
    let expected_marker = format!("symbiotic_memory_pin={pin}\ntarget={target}\n");
    let marker_path = dir.join(ZVEC_VERIFICATION_MARKER);
    println!("cargo:rerun-if-changed={}", marker_path.display());
    let marker = std::fs::read_to_string(&marker_path).unwrap_or_else(|error| {
        panic!(
            "ZVEC_LIB_DIR {} lacks verification marker {}: {error}; use scripts/prepare-adapter-zvec.sh",
            dir.display(),
            marker_path.display()
        )
    });
    if marker != expected_marker {
        panic!(
            "ZVEC_LIB_DIR {} verification marker does not match pin {} and target {}",
            dir.display(),
            pin,
            target
        );
    }
    Some(dir)
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
