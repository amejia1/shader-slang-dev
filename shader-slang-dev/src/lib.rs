//! Locate the shader-slang binaries and libraries built by
//! [`shader_slang_src`].
//!
//! This crate is the ergonomic accessor over `shader-slang-src`: it pulls in
//! the source-build crate as a regular dependency and exposes `PathBuf`
//! accessors for the `slangc` binary, the other binaries, and the library and
//! include directories.
//!
//! Typical use is from another crate's `build.rs`:
//!
//! ```no_run
//! use std::process::Command;
//!
//! fn main() {
//!     let slangc = shader_slang_dev::slangc();
//!     // Compile shaders with the built slangc.
//!     let status = Command::new(&slangc)
//!         .arg("shader.slang")
//!         .arg("-target")
//!         .arg("spirv")
//!         .status()
//!         .unwrap();
//!     assert!(status.success());
//!
//!     // Link against the built libraries, or compile against the headers:
//!     let _lib_dir = shader_slang_dev::lib_dir();      // `-L <dir>`
//!     let _include_dir = shader_slang_dev::include_dir(); // `-I <dir>`
//! }
//! ```

use std::path::PathBuf;

/// The shader-slang release built by the underlying `shader-slang-src` crate.
pub use shader_slang_src::SLANG_VERSION;

/// Directory containing all built artifacts (`bin/`, `lib/`, `include/`).
pub fn artifact_root() -> PathBuf {
    shader_slang_src::artifact_root()
}

/// Directory containing the built binaries (`slangc`, `slangd`, ...).
pub fn bin_dir() -> PathBuf {
    shader_slang_src::bin_dir()
}

/// Directory containing the built static/shared libraries; use as an `-L` flag.
pub fn lib_dir() -> PathBuf {
    shader_slang_src::lib_dir()
}

/// Directory containing the slang headers; use as an `-I` flag.
pub fn include_dir() -> PathBuf {
    shader_slang_src::include_dir()
}

/// Path to the `slangc` compiler executable.
pub fn slangc() -> PathBuf {
    shader_slang_src::slangc()
}

/// Path to the `slangd` language server executable.
pub fn slangd() -> PathBuf {
    shader_slang_src::slangd()
}

/// Path to the `slang` interpreter executable.
pub fn slang() -> PathBuf {
    shader_slang_src::slang()
}

/// Path to the `slangi` interpreter executable.
pub fn slangi() -> PathBuf {
    shader_slang_src::slangi()
}

/// Path to the `slang-replay` executable.
pub fn slang_replay() -> PathBuf {
    shader_slang_src::slang_replay()
}

/// Path to a named binary under [`bin_dir`] (the platform `.exe` suffix is
/// added automatically on Windows).
pub fn binary(name: &str) -> PathBuf {
    shader_slang_src::binary(name)
}

/// Path to a named library under [`lib_dir`]. Use [`lib_dir`] to enumerate the
/// exact set of libraries a given build produced.
pub fn library(name: &str) -> PathBuf {
    shader_slang_src::library(name)
}

/// Path to the static `slang-compiler` library (the core compiler API).
pub fn lib_slang_compiler() -> PathBuf {
    shader_slang_src::lib_slang_compiler()
}

/// Path to the static `core` library.
pub fn lib_core() -> PathBuf {
    shader_slang_src::lib_core()
}

/// Path to the static `compiler-core` library.
pub fn lib_compiler_core() -> PathBuf {
    shader_slang_src::lib_compiler_core()
}

/// Path to the static `slang-rt` runtime library.
pub fn lib_slang_rt() -> PathBuf {
    shader_slang_src::lib_slang_rt()
}

/// Path to the static `slang-cpp-parser` library.
pub fn lib_slang_cpp_parser() -> PathBuf {
    shader_slang_src::lib_slang_cpp_parser()
}

/// Path to the shared `glslang` backend plugin the binaries dlopen at runtime.
pub fn lib_slang_glslang() -> PathBuf {
    shader_slang_src::lib_slang_glslang()
}

/// Path to the shared `glsl-module` backend plugin.
pub fn lib_slang_glsl_module() -> PathBuf {
    shader_slang_src::lib_slang_glsl_module()
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

    /// Recursively list every file under the collected artifact directories
    /// (`bin/`, `lib/`, `include/`), one per line. Makes a missing-file
    /// failure self-describing: the panic message shows exactly what the
    /// build actually produced.
    fn list_artifact_files() -> String {
        let mut files = Vec::new();
        let mut stack = vec![bin_dir(), lib_dir(), include_dir()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        files.sort();
        files
            .iter()
            .map(|p| format!("  {}\n", p.display()))
            .collect()
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
