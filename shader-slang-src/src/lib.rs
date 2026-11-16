//! Build artifacts produced by this crate's `build.rs`: the shader-slang
//! compiler toolchain, either installed from a source build or downloaded as
//! the prebuilt release binaries (see [`SOURCE`]).
//!
//! The paths returned here point into this crate's `OUT_DIR`, where
//! `build.rs` installed the toolchain in the same layout as the shader-slang
//! release tarballs: `bin/` (binaries), `lib/` (static or shared libraries
//! and the standard-module tree), `include/` (headers) and `share/`
//! (documentation and data). They are stable for the lifetime of a single
//! build graph and are meant to be consumed from another crate's `build.rs`
//! (see the [`shader-slang-dev`](https://crates.io/crates/shader-slang-dev)
//! accessor crate).

use std::path::PathBuf;

/// The shader-slang release this crate builds (the `+<version>` build metadata
/// of this crate's version, e.g. `2026.21`). Emitted by `build.rs`.
pub const SLANG_VERSION: &str = env!("SHADER_SLANG_VERSION");

/// How the artifacts were obtained: `"source"` when `build.rs` compiled and
/// installed shader-slang from source, or `"download"` when `build.rs`
/// downloaded the prebuilt release binaries (enabled by setting
/// `SHADER_SLANG_SRC_DOWNLOAD_BINARIES` to `"true"` while building).
/// Emitted by `build.rs`.
pub const SOURCE: &str = env!("SHADER_SLANG_SOURCE");

/// Directory containing all installed artifacts: the `bin/`, `lib/`,
/// `include/` and `share/` directories plus the license and documentation
/// files.
pub fn artifact_root() -> PathBuf {
    PathBuf::from(env!("SHADER_SLANG_ARTIFACT_ROOT"))
}

/// Directory containing the built binaries (`slangc`, `slangd`, ...).
pub fn bin_dir() -> PathBuf {
    artifact_root().join("bin")
}

/// Directory containing the built static and shared libraries; suitable for a
/// `-L` linker flag or using as `cargo::rustc-link-search=[KIND=]<dir>` in cargo build script.
pub fn lib_dir() -> PathBuf {
    artifact_root().join("lib")
}

/// Directory containing the slang public headers (`slang.h`, ...); suitable
/// for an `-I` compiler flag.
pub fn include_dir() -> PathBuf {
    artifact_root().join("include")
}

/// Directory containing the installed `share/` data (`doc/slang/`
/// documentation, `slang/agent-skills/`).
pub fn share_dir() -> PathBuf {
    artifact_root().join("share")
}

/// Path to the `slangc` compiler executable.
pub fn slangc() -> PathBuf {
    binary("slangc")
}

/// Path to a named binary under [`bin_dir`] (the platform `.exe` suffix is
/// added automatically on Windows).
pub fn binary(name: &str) -> PathBuf {
    bin_dir().join(exe_name(name))
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// Name of the loose standard-module tree directory (e.g.
    /// `slang-standard-module-2026.21`).
    fn standard_module_dir_name() -> String {
        format!("slang-standard-module-{SLANG_VERSION}")
    }

    /// The standard-module trees present under the artifact root: the
    /// install puts the tree in `lib/`, and source builds additionally keep
    /// a copy in `bin/` (the statically linked slangc resolves the tree
    /// relative to its own executable).
    fn standard_module_dirs() -> Vec<PathBuf> {
        let name = standard_module_dir_name();
        let mut dirs = vec![lib_dir().join(&name)];
        if SOURCE == "source" || cfg!(windows) {
            dirs.push(bin_dir().join(&name));
        }
        dirs.retain(|dir| dir.is_dir());
        dirs
    }

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

    /// Recursively list every file under the artifact directories (`bin/`,
    /// `lib/`, `include/`), one per line. Makes a missing-file failure
    /// self-describing: the panic message shows exactly what the build
    /// actually installed.
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

    /// Path to the static library `name` under [`lib_dir`] (the platform
    /// prefix/extension is added automatically).
    fn static_lib_path(name: &str) -> PathBuf {
        lib_dir().join(if cfg!(windows) {
            format!("{name}.lib")
        } else {
            format!("lib{name}.a")
        })
    }

    /// The shared `slang-compiler` library under [`lib_dir`] that the release
    /// tarballs ship instead of the static libraries: `libslang-compiler.so`
    /// / `.dylib`, or the `slang-compiler.lib` import library on Windows.
    fn downloaded_compiler_lib() -> PathBuf {
        let dir = lib_dir();
        let exts: &[&str] = &["so", "dylib", "lib"];
        let Ok(entries) = fs::read_dir(&dir) else {
            panic!("directory is not readable: {}", dir.display());
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains("slang-compiler"))
                && path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| exts.contains(&e))
            {
                return path;
            }
        }
        panic!(
            "no shared `slang-compiler` library under {}\nfiles under {}:\n{}",
            dir.display(),
            artifact_root().display(),
            list_artifact_files()
        );
    }

    /// Find the shared plugin `needle` in `dir`: a file whose name contains
    /// `needle` and whose extension is one of `exts`.
    fn plugin_shared(dir: &Path, needle: &str, exts: &[&str]) -> Option<PathBuf> {
        let Ok(entries) = fs::read_dir(dir) else {
            return None;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let ext = path.extension().and_then(|e| e.to_str());
            if path.is_file() && name.contains(needle) && ext.is_some_and(|e| exts.contains(&e)) {
                return Some(path);
            }
        }
        None
    }

    #[test]
    fn directories_exist_and_are_readable() {
        assert_readable_dir(&artifact_root());
        assert_readable_dir(&bin_dir());
        assert_readable_dir(&lib_dir());
        assert_readable_dir(&include_dir());
        assert_readable_dir(&share_dir());
        // The installed headers must include the main C API header.
        assert_readable_file(&include_dir().join("slang.h"));
    }

    #[test]
    fn binaries_exist_and_are_readable() {
        assert_readable_file(&slangc());
        for name in ["slangd", "slang", "slangi"] {
            assert_readable_file(&binary(name));
        }
        // `slang-replay` is only produced by the source build; the release
        // tarballs do not ship it.
        if SOURCE == "source" {
            assert_readable_file(&binary("slang-replay"));
        }
    }

    #[test]
    fn standard_modules_exist_and_are_readable() {
        let dirs = standard_module_dirs();
        assert!(
            !dirs.is_empty(),
            "no standard-module tree under {}\nfiles under {}:\n{}",
            artifact_root().display(),
            artifact_root().display(),
            list_artifact_files()
        );
        // The tree must actually contain module files.
        let has_module = dirs
            .iter()
            .flat_map(|dir| list_artifact_files_under(dir))
            .any(|name| name.ends_with(".slang-module"));
        assert!(
            has_module,
            "no .slang-module files under {}",
            dirs[0].display()
        );
    }

    #[test]
    fn libraries_exist_and_are_readable() {
        if SOURCE == "source" {
            // The source build installs static libraries.
            for name in [
                "slang-compiler",
                "core",
                "compiler-core",
                "slang-rt",
                "slang-cpp-parser",
            ] {
                assert_readable_file(&static_lib_path(name));
            }
        } else {
            // The release tarballs ship shared libraries instead of static
            // ones.
            assert_readable_file(&downloaded_compiler_lib());
        }
        // The shared backend plugins the binaries dlopen at runtime.
        let plugin_dir = if cfg!(windows) { bin_dir() } else { lib_dir() };
        let exts: &[&str] = if cfg!(windows) {
            &["dll"]
        } else {
            &["so", "dylib"]
        };
        for plugin in ["slang-glslang", "slang-glsl-module"] {
            assert!(
                plugin_shared(&plugin_dir, plugin, exts).is_some(),
                "no `{plugin}` shared plugin in {}\nfiles under {}:\n{}",
                plugin_dir.display(),
                artifact_root().display(),
                list_artifact_files()
            );
        }
    }

    #[test]
    fn generic_binary_helper_resolves() {
        assert_eq!(binary("slangc"), slangc());
        assert_readable_file(&binary("slangc"));
    }

    #[test]
    fn build_provenance_matches_request() {
        // When SHADER_SLANG_SRC_DOWNLOAD_BINARIES changes, `cargo` re-runs
        // the build script and recompiles the crate (the script re-emits
        // the provenance as `SHADER_SLANG_SOURCE`), so the compile-time
        // `SOURCE` constant must always match the variable as seen by this
        // test process. A mismatch means the build did not follow the
        // request.
        let requested = std::env::var("SHADER_SLANG_SRC_DOWNLOAD_BINARIES");
        let expected = if requested.as_deref().is_ok_and(|v| v == "true") {
            "download"
        } else {
            "source"
        };
        assert_eq!(
            SOURCE,
            expected,
            "artifacts came from `{SOURCE}`, but SHADER_SLANG_SRC_DOWNLOAD_BINARIES={requested:?} requested `{expected}`"
        );
    }
}
