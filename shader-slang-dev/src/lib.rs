//! Locate a shader-slang installation (the `slangc` compiler toolchain and
//! friends) and, with the `build` feature, build and install it.
//!
//! # Locating an installation (default)
//!
//! The accessors below resolve the artifact root at runtime, in this order:
//!
//! 1. `$SHADER_SLANG_HOME` — an install prefix containing `bin/slangc`,
//!    whether it was built and installed from source or installed from a
//!    downloaded release tarball ([`SHADER_SLANG_HOME_ENV_VAR`]).
//!    Authoritative: when set, it is used as-is.
//! 2. Common per-OS install locations ([`common_locations`]), including
//!    architecture-specific directories matching the release-tarball naming
//!    (`slang-<os>-<arch>`). A location only counts when it provides the
//!    pinned release ([`is_pinned_install_root`]: `bin/slangc` plus the
//!    versioned standard-module tree), so an install of a different slang
//!    release is not picked up.
//! 3. `$OUT_DIR/slang-artifact` — only when the `build` feature is enabled
//!    and the build script produced artifacts there.
//!
//! [`locate`] returns `None` when nothing is found, [`source`] reports how
//! the artifacts in use were obtained, and [`artifact_root`] panics with the
//! searched locations when nothing is found.
//!
//! # Building from source or tarballs (`build` feature)
//!
//! With `--features build`, `build.rs` clones and builds shader-slang from
//! source (or fetches the `SHADER_SLANG_SOURCE_URL` / `SHADER_SLANG_DIST_URL`
//! tarballs) into `$OUT_DIR/slang-artifact` — unless an installation is
//! already available through (1) or (2) above, in which case the build is
//! skipped. [`install_artifacts`] then copies the located artifact tree
//! (symlinks preserved) to a given install prefix, and the
//! `shader-slang-dev-installer` binary wraps it:
//!
//! ```sh
//! cargo run --features build --bin shader-slang-dev-installer -- /opt/slang
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
#[cfg(feature = "build")]
mod install;
#[cfg(feature = "build")]
pub use install::install_artifacts;

pub use detect::Location;

use std::path::{Path, PathBuf};

/// The shader-slang release this crate pins (`2026.21`, tag `v2026.21`).
pub const SLANG_VERSION: &str = detect::SLANG_VERSION;

/// Name of the environment variable that points at a shader-slang install
/// prefix (a directory containing `bin/slangc`), whether it was built and
/// installed from source or installed from a downloaded tarball.
pub const SHADER_SLANG_HOME_ENV_VAR: &str = detect::HOME_ENV_VAR;

/// The URL of the shader-slang source tarball the build used (a valid
/// `https` or `file` URL), or the empty string when the
/// `SHADER_SLANG_SOURCE_URL` environment variable was unset or empty — in
/// that case a build with the `build` feature cloned the `v2026.21` tag of
/// the upstream repository with all its submodules instead. With the `build`
/// feature disabled no build happened and this is simply the variable's
/// value as seen when the crate was compiled. Emitted by `build.rs`.
pub const SHADER_SLANG_SOURCE_URL: &str = match option_env!("SHADER_SLANG_SOURCE_URL") {
    Some(value) => value,
    None => "",
};

/// The URL of the prebuilt distribution tarball the build used, or the empty
/// string when the `SHADER_SLANG_DIST_URL` environment variable was unset or
/// empty (a valid `https` or `file` URL otherwise). With the `build` feature
/// disabled no build happened and this is simply the variable's value as
/// seen when the crate was compiled. Emitted by `build.rs`.
pub const SHADER_SLANG_DIST_URL: &str = match option_env!("SHADER_SLANG_DIST_URL") {
    Some(value) => value,
    None => "",
};

/// Locate the shader-slang artifacts: `$SHADER_SLANG_HOME`, then the
/// [`common_locations`], then `$OUT_DIR/slang-artifact` (the last only when
/// the `build` feature is enabled). Returns `None` when nothing is found.
pub fn locate() -> Option<Location> {
    detect::find(
        detect::home_env().as_deref(),
        out_dir_artifacts().as_deref(),
    )
}

/// The `SHADER_SLANG_HOME` install prefix, when set to a non-empty value.
pub fn shader_slang_home() -> Option<PathBuf> {
    detect::home_env()
}

/// Common install locations searched after `$SHADER_SLANG_HOME` (and before
/// `$OUT_DIR`), per operating system and CPU architecture.
pub fn common_locations() -> Vec<PathBuf> {
    detect::common_locations()
}

/// Whether `path` looks like a shader-slang install prefix at all (it
/// contains `bin/slangc`, with the platform `.exe` suffix on Windows).
pub fn is_install_root(path: &Path) -> bool {
    detect::is_install_root(path)
}

/// Whether `path` provides the pinned shader-slang release ([`SLANG_VERSION`]):
/// an install prefix ([`is_install_root`]) that also carries the versioned
/// standard-module tree `slang-standard-module-<SLANG_VERSION>` under `lib/`
/// or `bin/`. Common locations and the `$OUT_DIR` build are only accepted
/// when this holds.
pub fn is_pinned_install_root(path: &Path) -> bool {
    detect::is_pinned_install_root(path)
}

/// How the artifacts in use were obtained: `"home"` (`SHADER_SLANG_HOME`),
/// `"system"` (a common install location), `"source"` or `"download"` (the
/// `$OUT_DIR` artifacts built from source or fetched from the
/// `SHADER_SLANG_DIST_URL` tarball — reachable only with the `build`
/// feature), or `"none"` when no installation was found.
pub fn source() -> &'static str {
    match locate() {
        Some(Location::Home(_)) => "home",
        Some(Location::System(_)) => "system",
        Some(Location::OutDir(_)) => {
            if SHADER_SLANG_DIST_URL.is_empty() {
                "source"
            } else {
                "download"
            }
        }
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
        let mut searched = vec![format!("${SHADER_SLANG_HOME_ENV_VAR}")];
        searched.extend(
            detect::common_locations()
                .iter()
                .map(|p| p.display().to_string()),
        );
        if cfg!(feature = "build") {
            searched.push(format!("$OUT_DIR/{}", detect::ARTIFACT_ROOT_DIR));
        }
        panic!(
            "no shader-slang installation found (searched: {})\n\
             set ${SHADER_SLANG_HOME_ENV_VAR} to an install prefix containing bin/slangc, \
             install shader-slang {SLANG_VERSION} into a common location, or enable the \
             `build` feature to build it into $OUT_DIR",
            searched.join(", ")
        )
    })
}

/// `$OUT_DIR/slang-artifact`, but only when the `build` feature is enabled
/// (the build script only produces artifacts there with the feature on).
fn out_dir_artifacts() -> Option<PathBuf> {
    if cfg!(feature = "build") {
        option_env!("OUT_DIR").map(|dir| PathBuf::from(dir).join(detect::ARTIFACT_ROOT_DIR))
    } else {
        None
    }
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
        let mut dirs = vec![lib_dir().join(&name), bin_dir().join(&name)];
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
            Location::OutDir(_) => {
                let expected = if SHADER_SLANG_DIST_URL.is_empty() {
                    "source"
                } else {
                    "download"
                };
                assert_eq!(
                    source, expected,
                    "artifacts are in $OUT_DIR, but SHADER_SLANG_DIST_URL={SHADER_SLANG_DIST_URL:?} implies `{expected}`"
                );
            }
        }
    }

    #[test]
    fn source_url_constant_matches_request() {
        // Unset or empty SHADER_SLANG_SOURCE_URL must compile to the empty
        // string (a git clone was used); a set value must be an https or
        // file URL.
        let requested = std::env::var("SHADER_SLANG_SOURCE_URL").unwrap_or_default();
        if requested.is_empty() {
            assert_eq!(SHADER_SLANG_SOURCE_URL, "");
        } else {
            assert!(
                SHADER_SLANG_SOURCE_URL.starts_with("https://")
                    || SHADER_SLANG_SOURCE_URL.starts_with("file://"),
                "SHADER_SLANG_SOURCE_URL={SHADER_SLANG_SOURCE_URL:?} is not an https or file URL"
            );
        }
    }

    #[test]
    fn dist_url_constant_matches_request() {
        // Unset or empty SHADER_SLANG_DIST_URL must compile to the empty
        // string; a set value must be an https or file URL.
        let requested = std::env::var("SHADER_SLANG_DIST_URL").unwrap_or_default();
        if requested.is_empty() {
            assert_eq!(SHADER_SLANG_DIST_URL, "");
        } else {
            assert!(
                SHADER_SLANG_DIST_URL.starts_with("https://")
                    || SHADER_SLANG_DIST_URL.starts_with("file://"),
                "SHADER_SLANG_DIST_URL={SHADER_SLANG_DIST_URL:?} is not an https or file URL"
            );
        }
    }
}
