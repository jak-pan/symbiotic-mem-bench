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
}
