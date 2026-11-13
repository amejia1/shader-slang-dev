//! Build artifacts produced by this crate's `build.rs`: the shader-slang
//! compiler toolchain, built from source with static linking enabled.
//!
//! The paths returned here point into this crate's `OUT_DIR`, which `build.rs`
//! populated with a `bin/` (binaries), `lib/` (static/shared libraries) and
//! `include/` (headers) layout. They are stable for the lifetime of a single
//! build graph and are meant to be consumed from another crate's `build.rs`
//! (see the [`shader-slang-dev`](https://crates.io/crates/shader-slang-dev)
//! accessor crate).

use std::path::PathBuf;

/// The shader-slang release this crate builds (the `+<version>` build metadata
/// of this crate's version, e.g. `2026.21`). Emitted by `build.rs`.
pub const SLANG_VERSION: &str = env!("SHADER_SLANG_VERSION");

/// Directory containing all built artifacts (`bin/`, `lib/`, `include/`).
pub fn artifact_root() -> PathBuf {
    PathBuf::from(env!("SHADER_SLANG_ARTIFACT_ROOT"))
}

/// Directory containing the built binaries (`slangc`, `slangd`, ...).
pub fn bin_dir() -> PathBuf {
    artifact_root().join("bin")
}

/// Directory containing the built static and shared libraries; suitable for a
/// `-L` linker flag.
pub fn lib_dir() -> PathBuf {
    artifact_root().join("lib")
}

/// Directory containing the slang public headers (`slang.h`, ...); suitable
/// for an `-I` compiler flag.
pub fn include_dir() -> PathBuf {
    artifact_root().join("include")
}

/// Path to the `slangc` compiler executable.
pub fn slangc() -> PathBuf {
    binary("slangc")
}

/// Path to the `slangd` language server executable.
pub fn slangd() -> PathBuf {
    binary("slangd")
}

/// Path to the `slang` interpreter executable.
pub fn slang() -> PathBuf {
    binary("slang")
}

/// Path to the `slangi` interpreter executable.
pub fn slangi() -> PathBuf {
    binary("slangi")
}

/// Path to the `slang-replay` executable.
pub fn slang_replay() -> PathBuf {
    binary("slang-replay")
}

/// Path to a named binary under [`bin_dir`] (the platform `.exe` suffix is
/// added automatically on Windows).
pub fn binary(name: &str) -> PathBuf {
    bin_dir().join(exe_name(name))
}

/// Path to a named library under [`lib_dir`]. Use [`lib_dir`] to enumerate the
/// exact set of libraries a given build produced.
pub fn library(name: &str) -> PathBuf {
    lib_dir().join(name)
}

/// Path to the static `slang-compiler` library (the core compiler API).
pub fn lib_slang_compiler() -> PathBuf {
    static_lib("slang-compiler")
}

/// Path to the static `core` library.
pub fn lib_core() -> PathBuf {
    static_lib("core")
}

/// Path to the static `compiler-core` library.
pub fn lib_compiler_core() -> PathBuf {
    static_lib("compiler-core")
}

/// Path to the static `slang-rt` runtime library.
pub fn lib_slang_rt() -> PathBuf {
    static_lib("slang-rt")
}

/// Path to the static `slang-cpp-parser` library.
pub fn lib_slang_cpp_parser() -> PathBuf {
    static_lib("slang-cpp-parser")
}

/// Directory holding the loose standard-module files
/// (`slang-standard-module-<version>/slang/...`, `.../experimental/...`),
/// resolved at runtime relative to the slang binaries; required for
/// `import slang.<module>`.
pub fn standard_module_dir() -> PathBuf {
    bin_dir().join(format!("slang-standard-module-{SLANG_VERSION}"))
}

/// Path to the shared `glslang` backend plugin the binaries dlopen at runtime.
pub fn lib_slang_glslang() -> PathBuf {
    shared_lib("slang-glslang")
}

/// Path to the shared `glsl-module` backend plugin.
pub fn lib_slang_glsl_module() -> PathBuf {
    shared_lib("slang-glsl-module")
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

fn static_lib(name: &str) -> PathBuf {
    lib_dir().join(if cfg!(windows) {
        format!("{name}.lib")
    } else {
        format!("lib{name}.a")
    })
}

/// Slang version-suffixes the runtime-loaded module file names on
/// non-Windows platforms only, and uses the `.dylib` extension on macOS
/// (see slang's `source/slang-glslang/CMakeLists.txt`).
fn shared_lib(name: &str) -> PathBuf {
    lib_dir().join(if cfg!(windows) {
        format!("{name}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{name}-{SLANG_VERSION}.dylib")
    } else {
        format!("lib{name}-{SLANG_VERSION}.so")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Assert that `path` is an existing, readable directory.
    fn assert_readable_dir(path: &Path) {
        assert!(
            path.is_dir(),
            "expected an existing directory, got: {}",
            path.display()
        );
        fs::read_dir(path)
            .unwrap_or_else(|e| panic!("directory is not readable ({}): {e}", path.display()));
    }

    /// Recursively collect the file names under `dir` (sorted).
    fn list_artifact_files_under(dir: &Path) -> Vec<String> {
        let mut files = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(next) = stack.pop() {
            let Ok(entries) = fs::read_dir(&next) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    files.push(
                        path.file_name()
                            .expect("file has no name")
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        files.sort();
        files
    }

    /// Recursively list every file under the collected artifact directories
    /// (`bin/`, `lib/`, `include/`), one per line. Makes a missing-file
    /// failure self-describing: the panic message shows exactly what the
    /// build actually produced.
    fn list_artifact_files() -> String {
        let mut all = Vec::new();
        for dir in [bin_dir(), lib_dir(), include_dir()] {
            all.extend(list_artifact_files_under(&dir));
        }
        all.sort();
        all.iter().map(|n| format!("  {n}\n")).collect()
    }

    /// Assert that `path` is an existing, readable file.
    fn assert_readable_file(path: &Path) {
        if !path.is_file() {
            panic!(
                "expected an existing file, got: {}\nfiles under {}:\n{}",
                path.display(),
                artifact_root().display(),
                list_artifact_files()
            );
        }
        fs::File::open(path)
            .unwrap_or_else(|e| panic!("file is not readable ({}): {e}", path.display()));
    }

    #[test]
    fn directories_exist_and_are_readable() {
        assert_readable_dir(&artifact_root());
        assert_readable_dir(&bin_dir());
        assert_readable_dir(&lib_dir());
        assert_readable_dir(&include_dir());
        assert_readable_dir(&standard_module_dir());
        // The standard-module tree must actually contain module files.
        let has_module = list_artifact_files_under(&standard_module_dir())
            .iter()
            .any(|name| name.ends_with(".slang-module"));
        assert!(
            has_module,
            "no .slang-module files under {}",
            standard_module_dir().display()
        );
    }

    #[test]
    fn binaries_exist_and_are_readable() {
        assert_readable_file(&slangc());
        assert_readable_file(&slangd());
        assert_readable_file(&slang());
        assert_readable_file(&slangi());
        assert_readable_file(&slang_replay());
    }

    #[test]
    fn libraries_exist_and_are_readable() {
        assert_readable_file(&lib_slang_compiler());
        assert_readable_file(&lib_core());
        assert_readable_file(&lib_compiler_core());
        assert_readable_file(&lib_slang_rt());
        assert_readable_file(&lib_slang_cpp_parser());
        assert_readable_file(&lib_slang_glslang());
        assert_readable_file(&lib_slang_glsl_module());
    }

    #[test]
    fn generic_binary_and_library_helpers_resolve() {
        assert_readable_file(&binary("slangc"));

        let compiler_lib = lib_slang_compiler();
        let name = compiler_lib
            .file_name()
            .expect("library path has no file name")
            .to_string_lossy()
            .into_owned();
        assert_eq!(library(&name), compiler_lib);
        assert_readable_file(&library(&name));
    }
}
