use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=native/host.h");
    println!("cargo:rerun-if-changed=native/host.mm");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let object = out.join("host.o");
    let archive = out.join("libpliant_browser_host.a");
    run(
        Command::new("xcrun")
            .args([
                "--sdk",
                "macosx",
                "clang++",
                "-std=c++17",
                "-fobjc-arc",
                "-fmodules",
                "-mmacosx-version-min=11.0",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-c",
                "native/host.mm",
                "-o",
            ])
            .arg(&object),
        "compile AppKit host",
    );
    run(
        Command::new("xcrun")
            .args(["--sdk", "macosx", "ar", "rcs"])
            .arg(&archive)
            .arg(&object),
        "archive AppKit host",
    );

    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=pliant_browser_host");
    println!("cargo:rustc-link-lib=c++");
    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-arg-bin=pliant-browser=-Wl,-rpath,@executable_path/../Frameworks");
}

fn run(command: &mut Command, description: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to {description}: {error}"));
    assert!(status.success(), "failed to {description}: {status}");
}
