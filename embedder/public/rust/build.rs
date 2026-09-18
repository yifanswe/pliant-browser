use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=PLIANT_CHROMIUM_LIB_DIR");
    println!("cargo:rustc-link-lib=framework=PliantContent");
    if let Some(directory) = env::var_os("PLIANT_CHROMIUM_LIB_DIR") {
        let directory = PathBuf::from(directory)
            .canonicalize()
            .expect("PLIANT_CHROMIUM_LIB_DIR must be an existing directory");
        println!("cargo:rustc-link-search=framework={}", directory.display());
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
    } else {
        // `cargo check` can type-check the real Rust API without pretending
        // that a native runtime exists. Executables still require the library.
        println!(
            "cargo:warning=Set PLIANT_CHROMIUM_LIB_DIR before linking or running a native host."
        );
    }
}
