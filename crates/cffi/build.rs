use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/lib.rs");

    let crate_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let out_dir = PathBuf::from(&crate_dir).join("include");

    std::fs::create_dir_all(&out_dir).expect("failed to create include directory");

    let header_path = out_dir.join("tailwindcss_oxide.h");

    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_language(cbindgen::Language::C)
        .with_include_guard("TAILWINDCSS_OXIDE_H")
        .generate()
        .expect("failed to generate C header with cbindgen")
        .write_to_file(&header_path);
}
