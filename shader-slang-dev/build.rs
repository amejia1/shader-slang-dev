//! Build script for `shader-slang-dev`.
//!
//! Building shader-slang itself is owned by the optional `shader-slang-src`
//! dependency. Without `vendored`, only environment discovery is registered.

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    #[cfg(not(feature = "vendored"))]
    println!("cargo::rerun-if-env-changed=SHADER_SLANG_HOME");
}
