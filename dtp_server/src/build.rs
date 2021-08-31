use std::env;
use std::path::{Path};

fn main() {
    if cfg!(feature = "interface") {
        // find the library and link rust to it
        println!("cargo:rustc-link-lib=dylib=solution");
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let solution_dir = format!("{}/demo", manifest_dir);
        println!("cargo:rustc-link-search=native={}", solution_dir);
    }
}

