use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let zvec_lib_dir = manifest_dir
        .parent()
        .expect("membench repo parent")
        .join("symbiotic-memory/vendor/zvec-rust/vendor/lib");
    let zvec_lib = zvec_lib_dir.join("libzvec_c_api.dylib");

    println!("cargo:rerun-if-changed={}", zvec_lib.display());
    if zvec_lib.exists() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", zvec_lib_dir.display());
        println!(
            "cargo:rustc-link-arg-bins=-Wl,-rpath,{}",
            zvec_lib_dir.display()
        );
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
}
