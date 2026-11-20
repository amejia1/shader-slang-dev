//! Locate an installed shader-slang toolchain, or build the vendored source.
//!
//! # Locating an installation (default)
//!
//! The accessors resolve the artifact root at runtime from:
//!
//! 1. `$SHADER_SLANG_HOME` — an install prefix containing `bin/slangc`.
//!    Authoritative: when set, it is used as-is.
//! 2. Common per-OS install locations ([`common_locations`]). A location only
//!    counts when it is a complete install ([`is_complete_install_root`]).
//!
//! [`locate`] returns `None` when nothing is found and [`artifact_root`]
//! panics with the searched locations. [`source`] reports the selected origin.
//!
//! The crate does not pin a shader-slang release; it locates whatever is
//! installed. Run `slangc -version` to determine the version in use.
//!
//! # Vendored build (`vendored` feature)
//!
//! With `--features vendored`, the `shader-slang-src` dependency reassembles
//! its source-partition crates, builds shader-slang, and installs it into that
//! dependency's `$OUT_DIR/slang-artifact`. Only these vendored artifacts are
//! used: `$SHADER_SLANG_HOME` and common locations are deliberately ignored.
//!
//! [`install_artifacts`] copies the selected tree (symlinks preserved) to an
//! install prefix, and the `shader-slang-dev-installer` binary wraps it:
//!
//! ```sh
//! cargo run --features vendored --bin shader-slang-dev-installer -- /opt/slang
//! ```
//!
//! Typical use is from another crate's `build.rs`:
//!
//! ```no_run
//! let slangc = shader_slang_dev::slangc();
//! // Compile shaders with the located slangc:
//! //   std::process::Command::new(&slangc).arg("shader.slang").status().unwrap();
//! // Link against the located libraries, or compile against the headers:
//! let _lib_dir = shader_slang_dev::lib_dir();      // `-L <dir>` or `cargo::rustc-link-search=[KIND=]<dir>` in cargo build script.
//! let _include_dir = shader_slang_dev::include_dir(); // `-I <dir>`
//! ```

mod detect;
#[cfg(feature = "vendored")]
mod install;
#[cfg(feature = "vendored")]
pub use install::install_artifacts;

pub use detect::Location;

use std::path::{Path, PathBuf};

/// Name of the environment variable that points at a shader-slang install
/// prefix (a directory containing `bin/slangc`), whether it was built and
/// installed from source or installed from a downloaded tarball.
pub const SHADER_SLANG_HOME_ENV_VAR: &str = detect::HOME_ENV_VAR;

/// Locate the shader-slang artifacts. With `vendored`, only the artifacts
/// from `shader-slang-src` are eligible. Otherwise, search
/// `$SHADER_SLANG_HOME` and then [`common_locations`].
pub fn locate() -> Option<Location> {
    #[cfg(feature = "vendored")]
    {
        let root = shader_slang_src::artifact_root();
        detect::is_complete_install_root(&root).then_some(Location::Vendored(root))
    }
    #[cfg(not(feature = "vendored"))]
    {
        detect::find(detect::home_env().as_deref())
    }
}

/// The `SHADER_SLANG_HOME` install prefix, when set to a non-empty value.
pub fn shader_slang_home() -> Option<PathBuf> {
    detect::home_env()
}

/// Common install locations searched after `$SHADER_SLANG_HOME` when the
/// `vendored` feature is disabled, per operating system and CPU architecture.
pub fn common_locations() -> Vec<PathBuf> {
    detect::common_locations()
}

/// Whether `path` looks like a shader-slang install prefix at all (it
/// contains `bin/slangc`, with the platform `.exe` suffix on Windows).
pub fn is_install_root(path: &Path) -> bool {
    detect::is_install_root(path)
}

/// Whether `path` is a complete shader-slang install: an install prefix
/// ([`is_install_root`]) that also carries a versioned standard-module tree
/// `slang-standard-module-<version>` (any release) under `lib/` or `bin/`.
/// Common locations and the `$OUT_DIR` build are only accepted when this
/// holds. Detection is version-agnostic; use `slangc -version` to determine
/// the release in use.
pub fn is_complete_install_root(path: &Path) -> bool {
    detect::is_complete_install_root(path)
}

/// How the artifacts in use were obtained: `"home"`, `"system"`,
/// `"vendored"`, or `"none"` when no installation was found.
pub fn source() -> &'static str {
    match locate() {
        Some(Location::Home(_)) => "home",
        Some(Location::System(_)) => "system",
        Some(Location::Vendored(_)) => "vendored",
        None => "none",
    }
}

/// Directory containing all installed artifacts: the `bin/`, `lib/`,
/// `include/` and `share/` directories plus the license and documentation
/// files.
///
/// Panics when no shader-slang installation is found; the panic message lists
/// the searched locations.
pub fn artifact_root() -> PathBuf {
    locate().map(Location::into_path).unwrap_or_else(|| {
        #[cfg(feature = "vendored")]
        panic!("shader-slang-src did not produce a complete vendored installation");
        #[cfg(not(feature = "vendored"))]
        {
            let mut searched = vec![format!("${SHADER_SLANG_HOME_ENV_VAR}")];
            searched.extend(
                detect::common_locations()
                    .iter()
                    .map(|p| p.display().to_string()),
            );
            panic!(
                "no shader-slang installation found (searched: {})\n\
                 set ${SHADER_SLANG_HOME_ENV_VAR} to an install prefix containing bin/slangc, \
                 install shader-slang into a common location, or enable the \
                 `vendored` feature to build the vendored source",
                searched.join(", ")
            )
        }
    })
}

/// Directory containing the binaries (`slangc`, `slangd`, ...).
pub fn bin_dir() -> PathBuf {
    artifact_root().join("bin")
}

/// Directory containing the static and shared libraries; suitable for a
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
    bin_dir().join(detect::exe_name(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    /// The standard-module trees present under the artifact root: the
    /// install puts the tree in `lib/`, and source builds additionally keep
    /// a copy in `bin/` (the statically linked slangc resolves the tree
    /// relative to its own executable). Any release's tree is accepted.
    fn standard_module_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();
        for base in [lib_dir(), bin_dir()] {
            let Ok(entries) = fs::read_dir(&base) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("slang-standard-module-")
                {
                    dirs.push(path);
                }
            }
        }
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
    /// self-describing: the panic message shows exactly what the located
    /// install contains.
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

    /// The shared `slang-compiler` library under [`lib_dir`] that the
    /// distribution tarballs ship instead of the static libraries:
    /// `libslang-compiler.so` / `.dylib`, or the `slang-compiler.lib` import
    /// library on Windows.
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
    fn artifact_root_is_a_valid_install() {
        let root = artifact_root();
        assert!(
            is_install_root(&root),
            "located artifact root {} has no bin/slangc",
            root.display()
        );
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
        // Source builds install the compiler and runtime as static
        // libraries; distribution tarballs (and installs made from them)
        // ship the shared compiler library instead. Probe which one the
        // located install provides.
        let static_libs = ["slang-compiler", "slang-rt"].map(static_lib_path);
        if static_libs.iter().all(|path| path.is_file()) {
            for path in &static_libs {
                assert_readable_file(path);
            }
        } else {
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
    fn source_matches_located_artifacts() {
        let source = source();
        match locate().expect("tests require a located shader-slang installation") {
            Location::Home(_) => assert_eq!(source, "home"),
            Location::System(_) => assert_eq!(source, "system"),
            Location::Vendored(_) => assert_eq!(source, "vendored"),
        }
    }
}
