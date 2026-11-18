//! Build script for `shader-slang-dev`.
//!
//! Without the `build` feature this script only registers its inputs: the
//! library locates an existing shader-slang installation at runtime
//! (`$SHADER_SLANG_HOME`, then the common install locations — see
//! `src/detect.rs`).
//!
//! With the `build` feature it additionally builds shader-slang from source
//! (or fetches the `SHADER_SLANG_SOURCE_URL` / `SHADER_SLANG_DIST_URL`
//! tarballs) into `$OUT_DIR/slang-artifact` — unless an installation is
//! already available outside `$OUT_DIR`, in which case the build is skipped
//! (the library's lookup order prefers those locations over `$OUT_DIR`
//! anyway). The implementation lives in `src/build_main.rs` so it can use
//! the optional build dependencies, which are only compiled with the feature.

// Without the `build` feature the build script only needs
// `detect::HOME_ENV_VAR`; the rest of the shared module is used by the
// library (and by `build_main` with the feature on).
#[cfg(not(feature = "build"))]
#[path = "src/detect.rs"]
#[allow(dead_code)]
mod detect;
#[cfg(feature = "build")]
#[path = "src/detect.rs"]
mod detect;

#[cfg(feature = "build")]
#[path = "src/build_main.rs"]
mod build_main;

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-changed=src/detect.rs");
    println!("cargo::rerun-if-env-changed={}", detect::HOME_ENV_VAR);
    #[cfg(feature = "build")]
    build_main::main();
}
