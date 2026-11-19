//! Reassemble the vendored source partitions and build shader-slang.

#[path = "src/build_main.rs"]
mod build_main;
#[path = "src/slang_version.rs"]
mod slang_version;
#[path = "src/sources.rs"]
mod sources;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/build_main.rs");
    println!("cargo::rerun-if-changed=src/slang_version.rs");
    println!("cargo::rerun-if-changed=src/sources.rs");
    build_main::main(sources::SOURCE_ARCHIVES, slang_version::SLANG_VERSION);
}
