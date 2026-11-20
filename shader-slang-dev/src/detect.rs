//! Detection of an existing shader-slang installation.
//!
//! This module is `std`-only. Vendored artifacts bypass this lookup and come
//! directly from the `shader-slang-src` dependency.
//!
//! Lookup order (see [`find`]):
//!
//! 1. `$SHADER_SLANG_HOME` — an install prefix containing `bin/slangc`,
//!    whether it was built and installed from source or extracted from a
//!    downloaded release tarball. Authoritative: when set and non-empty it
//!    is used as-is, without further validation.
//! 2. Common per-OS install locations (see [`common_locations`]), including
//!    architecture-specific directories matching the release-tarball naming
//!    (`slang-<os>-<arch>`). A location only counts when
//!    [`is_complete_install_root`] holds for it: `bin/slangc` plus a versioned
//!    standard-module tree (any release) under `lib/` or `bin/`.
//! 3. An optional vendored root — used by `find_with` tests and by callers
//!    that explicitly provide one. The library's `vendored` feature instead
//!    bypasses home and common locations entirely.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Environment variable pointing at a shader-slang install prefix (a
/// directory containing `bin/slangc`, `lib/`, ...). Whether the install was
/// built from source or extracted from a downloaded tarball does not matter.
pub const HOME_ENV_VAR: &str = "SHADER_SLANG_HOME";

/// Where a located artifact root came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// From the `SHADER_SLANG_HOME` environment variable.
    Home(PathBuf),
    /// From one of the [`common_locations`].
    System(PathBuf),
    /// Built from the `shader-slang-src` source partitions.
    Vendored(PathBuf),
}

// The build script uses only some of these methods depending on the `build`
// feature; silence dead-code for the rest.
#[allow(dead_code)]
impl Location {
    /// The artifact root path.
    pub fn path(&self) -> &Path {
        match self {
            Location::Home(p) | Location::System(p) | Location::Vendored(p) => p,
        }
    }

    /// Human-readable name of where the artifacts came from.
    pub fn kind(&self) -> &'static str {
        match self {
            Location::Home(_) => "SHADER_SLANG_HOME",
            Location::System(_) => "a common install location",
            Location::Vendored(_) => "shader-slang-src",
        }
    }

    /// Consume into the artifact root path.
    pub fn into_path(self) -> PathBuf {
        match self {
            Location::Home(p) | Location::System(p) | Location::Vendored(p) => p,
        }
    }
}

/// Append the platform executable suffix when needed.
pub fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Whether `root` looks like a shader-slang install prefix at all: it
/// contains `bin/slangc` (with the platform `.exe` suffix on Windows).
pub fn is_install_root(root: &Path) -> bool {
    root.join("bin").join(exe_name("slangc")).is_file()
}

/// Whether `root` is a complete shader-slang install: an install prefix
/// ([`is_install_root`]) that also carries a versioned standard-module tree
/// `slang-standard-module-<version>` (any release) under `lib/` or `bin/` —
/// one level into `lib/` covers multiarch layouts such as
/// `/usr/lib/x86_64-linux-gnu/`.
///
/// Detection is version-agnostic: the crate locates whatever shader-slang
/// release is installed. Run `slangc -version` to determine which release is
/// in use. Common and vendored locations are only accepted when this holds, so
/// a prefix with a stray `bin/slangc` but no standard modules is skipped.
pub fn is_complete_install_root(root: &Path) -> bool {
    if !is_install_root(root) {
        return false;
    }
    if has_standard_module_tree(&root.join("lib")) || has_standard_module_tree(&root.join("bin")) {
        return true;
    }
    if let Ok(entries) = fs::read_dir(root.join("lib")) {
        for entry in entries.flatten() {
            if has_standard_module_tree(&entry.path()) {
                return true;
            }
        }
    }
    false
}

/// Whether `dir` directly contains a standard-module tree directory
/// (`slang-standard-module-<version>`) of any shader-slang release.
fn has_standard_module_tree(dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry.path().is_dir()
            && entry
                .file_name()
                .to_string_lossy()
                .starts_with("slang-standard-module-")
    })
}

/// The `SHADER_SLANG_HOME` value as a path, or `None` when unset or empty.
pub fn home_env() -> Option<PathBuf> {
    env::var_os(HOME_ENV_VAR)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// The user's home directory (`$HOME`, falling back to `$USERPROFILE`).
fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

/// Short OS name used in the release-tarball directory naming
/// (`slang-<version>-<os>-<arch>`): `linux`, `macos` or `windows`.
pub fn os_name() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    }
}

/// Short CPU architecture name used in the release-tarball directory naming
/// (`slang-<version>-<os>-<arch>`).
pub fn arch() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "arm") {
        "arm"
    } else if cfg!(target_arch = "riscv64") {
        "riscv64"
    } else {
        "unknown"
    }
}

/// Common locations a shader-slang installation is found in, per operating
/// system, including architecture-specific directories matching the
/// release-tarball naming (`slang-<os>-<arch>` — e.g. where a release tarball
/// like `slang-<version>-linux-x86_64.tar.gz` is extracted in place under `/opt`).
///
/// A location only counts when [`is_complete_install_root`] holds for it, so
/// unrelated prefixes (`/usr`, `/usr/local` on a machine without a slang
/// install, prefixes with no standard modules, ...) are skipped.
pub fn common_locations() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let os = os_name();
    let arch = arch();
    if cfg!(target_os = "linux") {
        dirs.extend(["/usr/local", "/usr", "/opt/slang", "/opt/shader-slang"].map(PathBuf::from));
    } else if cfg!(target_os = "macos") {
        // Homebrew prefixes: /opt/homebrew on Apple Silicon, /usr/local on
        // Intel (both listed); `brew install slang` puts slangc in bin/.
        dirs.extend(
            [
                "/usr/local",
                "/opt/homebrew",
                "/opt/slang",
                "/opt/shader-slang",
            ]
            .map(PathBuf::from),
        );
    } else if cfg!(target_os = "windows") {
        for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Some(value) = env::var_os(var) {
                let base = PathBuf::from(value);
                dirs.push(base.join("slang"));
                dirs.push(base.join("shader-slang"));
            }
        }
        if let Some(value) = env::var_os("LOCALAPPDATA") {
            let base = PathBuf::from(value);
            dirs.push(base.join("slang"));
            dirs.push(base.join("Programs").join("slang"));
        }
        dirs.push(PathBuf::from(r"C:\slang"));
    }
    // Architecture-specific extraction locations of the release tarballs.
    if cfg!(target_os = "windows") {
        dirs.push(PathBuf::from(format!(r"C:\slang-{arch}")));
        dirs.push(PathBuf::from(format!(r"C:\slang-windows-{arch}")));
    } else {
        for base in ["/opt", "/usr/local"] {
            dirs.push(PathBuf::from(base).join(format!("slang-{os}-{arch}")));
            dirs.push(PathBuf::from(base).join(format!("slang-{arch}")));
        }
    }
    if let Some(home) = home_dir() {
        dirs.push(home.join(".local"));
        dirs.push(home.join("slang"));
        dirs.push(home.join(format!("slang-{os}-{arch}")));
    }
    dirs
}

/// The lookup itself, with the candidate lists passed in so tests can
/// substitute their own: `SHADER_SLANG_HOME` first (authoritative, used
/// as-is), then `common`, then `out_dir` (each `common`/`out_dir` entry only
/// counts when [`is_complete_install_root`] holds for it).
#[cfg(any(test, not(feature = "vendored")))]
pub fn find_with(
    home: Option<&Path>,
    common: &[PathBuf],
    out_dir: Option<&Path>,
) -> Option<Location> {
    if let Some(home) = home {
        return Some(Location::Home(home.to_path_buf()));
    }
    for dir in common {
        if is_complete_install_root(dir) {
            return Some(Location::System(dir.clone()));
        }
    }
    if let Some(out_dir) = out_dir {
        if is_complete_install_root(out_dir) {
            return Some(Location::Vendored(out_dir.to_path_buf()));
        }
    }
    None
}

/// Locate installed shader-slang artifacts: `$SHADER_SLANG_HOME`, then the
/// [`common_locations`].
#[cfg(not(feature = "vendored"))]
pub fn find(home: Option<&Path>) -> Option<Location> {
    find_with(home, &common_locations(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir = env::temp_dir().join(format!(
            "shader-slang-dev-detect-{}-{tag}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A minimal install prefix: just `bin/slangc`.
    fn fake_install(root: &Path) {
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join(exe_name("slangc")), b"fake slangc").unwrap();
    }

    /// A complete install prefix: `bin/slangc` plus a versioned
    /// standard-module tree under `lib/`.
    fn fake_complete_install(root: &Path) {
        fake_install(root);
        let tree = root.join("lib").join("slang-standard-module-2026.21");
        fs::create_dir_all(&tree).unwrap();
        fs::write(tree.join("core.slang-module"), b"module;").unwrap();
    }

    #[test]
    fn exe_name_adds_suffix_only_on_windows() {
        if cfg!(windows) {
            assert_eq!(exe_name("slangc"), "slangc.exe");
        } else {
            assert_eq!(exe_name("slangc"), "slangc");
        }
    }

    #[test]
    fn install_root_requires_bin_slangc() {
        let dir = temp_root("is-install-root");
        let root = dir.join("root");
        fs::create_dir_all(&root).unwrap();
        assert!(!is_install_root(&root));
        fake_install(&root);
        assert!(is_install_root(&root));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn complete_root_requires_a_standard_module_tree() {
        let dir = temp_root("is-complete-root");
        let root = dir.join("root");
        fs::create_dir_all(&root).unwrap();
        assert!(!is_complete_install_root(&root));
        // bin/slangc alone is not enough.
        fake_install(&root);
        assert!(is_install_root(&root));
        assert!(!is_complete_install_root(&root));
        // A standard-module tree of any release makes it a complete install.
        let tree = root.join("lib").join("slang-standard-module-2026.21");
        fs::create_dir_all(&tree).unwrap();
        assert!(is_complete_install_root(&root));
        // A different release's tree counts too: detection is version-agnostic.
        let other = dir.join("other-release");
        fake_install(&other);
        let other_tree = other.join("lib").join("slang-standard-module-2027.5");
        fs::create_dir_all(&other_tree).unwrap();
        assert!(is_complete_install_root(&other));
        // A multiarch lib/<triple>/ tree.
        let multiarch = dir.join("multiarch");
        fake_install(&multiarch);
        let tree = multiarch
            .join("lib")
            .join("x86_64-linux-gnu")
            .join("slang-standard-module-2026.21");
        fs::create_dir_all(&tree).unwrap();
        assert!(is_complete_install_root(&multiarch));
        // A tree under bin/.
        let in_bin = dir.join("bin-tree");
        fake_install(&in_bin);
        let tree = in_bin.join("bin").join("slang-standard-module-2026.21");
        fs::create_dir_all(&tree).unwrap();
        assert!(is_complete_install_root(&in_bin));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn find_prefers_home_then_complete_common_then_out_dir() {
        let dir = temp_root("find-order");
        let home = dir.join("home");
        let common = dir.join("common");
        let out = dir.join("out");
        let empty = dir.join("empty");
        let bin_only = dir.join("bin-only");
        fs::create_dir_all(&empty).unwrap();
        fake_complete_install(&home);
        fake_complete_install(&common);
        fake_complete_install(&out);
        fake_install(&bin_only);

        // SHADER_SLANG_HOME is authoritative: used as-is, even unvalidated.
        let bogus = dir.join("bogus");
        fs::create_dir_all(&bogus).unwrap();
        assert_eq!(
            find_with(Some(&bogus), std::slice::from_ref(&common), Some(&out)),
            Some(Location::Home(bogus))
        );
        // Then the first complete common location (a bin/slangc-only prefix
        // is skipped).
        let commons = [empty.clone(), bin_only.clone(), common.clone()];
        assert_eq!(
            find_with(None, &commons, Some(&out)),
            Some(Location::System(common))
        );
        // Then $OUT_DIR.
        assert_eq!(
            find_with(None, std::slice::from_ref(&empty), Some(&out)),
            Some(Location::Vendored(out))
        );
        // Nothing valid anywhere.
        assert!(find_with(None, std::slice::from_ref(&bin_only), Some(&empty)).is_none());
        assert!(find_with(None, &[], None).is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn common_locations_are_os_and_arch_specific() {
        let locs = common_locations();
        assert!(!locs.is_empty());
        let os = os_name();
        let arch = arch();
        if cfg!(target_os = "linux") {
            assert!(locs.contains(&PathBuf::from("/usr/local")));
            assert!(locs.contains(&PathBuf::from("/opt/slang")));
        } else if cfg!(target_os = "macos") {
            assert!(locs.contains(&PathBuf::from("/opt/homebrew")));
            assert!(locs.contains(&PathBuf::from("/opt/slang")));
        } else if cfg!(target_os = "windows") {
            assert!(locs.contains(&PathBuf::from(r"C:\slang")));
            assert!(locs.contains(&PathBuf::from(format!(r"C:\slang-{arch}"))));
        }
        if !cfg!(target_os = "windows") && os != "unknown" {
            assert!(locs.contains(&PathBuf::from(format!("/opt/slang-{os}-{arch}"))));
        }
    }
}
