use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=ZVEC_LIB_DIR");
    if let Some(dir) = zvec_lib_dir() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", dir.display());
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{}", dir.display());
    }

    let sha = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=GIT_SHA={sha}");
}

fn zvec_lib_dir() -> Option<PathBuf> {
    let vendored = |root: &Path| root.join("vendor/zvec-rust/vendor/lib");
    if let Ok(dir) = std::env::var("ZVEC_LIB_DIR") {
        let dir = PathBuf::from(dir);
        if has_zvec_lib(&dir) {
            return Some(dir);
        }
    }

    let cargo_home = std::env::var("CARGO_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|home| PathBuf::from(home).join(".cargo"))
        })?;
    for repo in std::fs::read_dir(cargo_home.join("git/checkouts"))
        .ok()?
        .flatten()
    {
        if !repo
            .file_name()
            .to_string_lossy()
            .starts_with("symbiotic-memory-")
        {
            continue;
        }
        for revision in std::fs::read_dir(repo.path()).ok()?.flatten() {
            let candidate = vendored(&revision.path());
            if has_zvec_lib(&candidate) {
                return Some(candidate);
            }
        }
    }

    // Sibling fallback is for deliberate co-development only. Exact-pin
    // release gates find Cargo's locked checkout above.
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").ok()?);
    let repo_root = manifest_dir.parent()?.parent()?;
    if let Some(parent) = repo_root.parent() {
        let sibling = vendored(&parent.join("symbiotic-memory"));
        println!("cargo:rerun-if-changed={}", sibling.display());
        if has_zvec_lib(&sibling) {
            return Some(sibling);
        }
    }
    None
}

fn has_zvec_lib(dir: &Path) -> bool {
    ["libzvec_c_api.dylib", "libzvec_c_api.so"]
        .iter()
        .any(|name| dir.join(name).is_file())
}
