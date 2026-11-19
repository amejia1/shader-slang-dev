//! Installing the located shader-slang artifacts (feature `vendored` only).

use std::fs;
use std::io;
use std::path::Path;

use crate::{detect, locate};

/// Recursively copy the located shader-slang artifact tree — the one
/// [`crate::artifact_root`] resolves (the vendored build when that feature is
/// enabled, otherwise `$SHADER_SLANG_HOME` or a common install location)
/// — into `dest`, which is created when missing. Existing files under `dest`
/// are overwritten.
///
/// Symlinks are recreated as symlinks with their original target (on Unix;
/// on Windows they are recreated when the process may create symlinks and
/// otherwise copied as their target, directories recursively).
///
/// Returns an error when no installation is found, or when the located root
/// does not look like an install (no `bin/slangc`).
pub fn install_artifacts(dest: &Path) -> io::Result<()> {
    let Some(location) = locate() else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "no shader-slang installation found; set ${} to an install prefix \
                 containing bin/slangc, install shader-slang into a common location, \
                 or enable the `build` feature",
                detect::HOME_ENV_VAR
            ),
        ));
    };
    let src = location.path();
    if !detect::is_install_root(src) {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{} (from {}) has no bin/slangc and is not a shader-slang install",
                src.display(),
                location.kind()
            ),
        ));
    }
    copy_tree(src, dest)
}

/// Recursively copy `src` into `dst`, recreating symlinks and overwriting
/// existing files.
fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = fs::symlink_metadata(&from)?;
        if meta.file_type().is_symlink() {
            copy_symlink(&from, &to)?;
        } else if meta.is_dir() {
            copy_tree(&from, &to)?;
        } else {
            remove_existing(&to)?;
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Remove whatever exists at `path` (file, symlink or directory).
fn remove_existing(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() || meta.is_file() {
                fs::remove_file(path)
            } else {
                fs::remove_dir_all(path)
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Recreate the symlink at `src` as `dst`, preserving the link target.
#[cfg(unix)]
fn copy_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    let target = fs::read_link(src)?;
    remove_existing(dst)?;
    std::os::unix::fs::symlink(&target, dst)
}

/// Recreate the symlink at `src` as `dst`. Windows requires a privilege to
/// create symlinks, so when that fails the link target is copied instead
/// (directories recursively).
#[cfg(windows)]
fn copy_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    let target = fs::read_link(src)?;
    remove_existing(dst)?;
    let followed = fs::metadata(src).ok();
    let link_result = match &followed {
        Some(meta) if meta.is_dir() => std::os::windows::fs::symlink_dir(&target, dst),
        Some(_) => std::os::windows::fs::symlink_file(&target, dst),
        None => Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("broken symlink {}", src.display()),
        )),
    };
    match link_result {
        Ok(()) => Ok(()),
        Err(_) => copy_followed(src, dst),
    }
}

/// Copy the symlink target's contents (fallback for platforms without
/// symlink support).
#[cfg(not(any(unix, windows)))]
fn copy_symlink(src: &Path, dst: &Path) -> io::Result<()> {
    remove_existing(dst)?;
    copy_followed(src, dst)
}

/// Copy what `src` points at (following the link) into `dst`.
#[cfg(not(unix))]
fn copy_followed(src: &Path, dst: &Path) -> io::Result<()> {
    if fs::metadata(src).map(|m| m.is_dir()).unwrap_or(false) {
        copy_tree(&fs::canonicalize(src)?, dst)
    } else {
        fs::copy(src, dst).map(|_| ())
    }
}
