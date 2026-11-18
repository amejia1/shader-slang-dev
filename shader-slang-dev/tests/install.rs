//! Integration tests for `install_artifacts` (only compiled with the `build`
//! feature).
//!
//! `SHADER_SLANG_HOME` is process-global, so the env-mutating tests take a
//! lock to stay race-free.

#![cfg(feature = "build")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "shader-slang-dev-install-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn slangc_name() -> &'static str {
    if cfg!(windows) {
        "slangc.exe"
    } else {
        "slangc"
    }
}

/// A fake artifact root: `bin/slangc`, a nested `lib/` tree, and (on Unix)
/// a file symlink and a directory symlink.
fn fake_artifact_root(root: &Path) {
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(root.join("bin").join(slangc_name()), b"fake slangc\n").unwrap();
    fs::create_dir_all(root.join("lib").join("sub")).unwrap();
    fs::write(root.join("lib").join("libslang-compiler.so"), b"lib").unwrap();
    fs::write(root.join("lib").join("sub").join("deep.txt"), b"deep").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            "libslang-compiler.so",
            root.join("lib").join("libslang-link.so"),
        )
        .unwrap();
        fs::create_dir_all(root.join("share-real")).unwrap();
        std::os::unix::fs::symlink("../share-real", root.join("share")).unwrap();
    }
}

#[test]
fn install_artifacts_copies_tree_and_preserves_symlinks() {
    let _guard = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("copy");
    let src = dir.join("artifact-root");
    fake_artifact_root(&src);

    std::env::set_var(shader_slang_dev::SHADER_SLANG_HOME_ENV_VAR, &src);
    let dest = dir.join("install-dest");
    shader_slang_dev::install_artifacts(&dest).unwrap();

    // Files are copied recursively with their contents.
    assert_eq!(
        fs::read(dest.join("bin").join(slangc_name())).unwrap(),
        b"fake slangc\n"
    );
    assert_eq!(
        fs::read(dest.join("lib").join("libslang-compiler.so")).unwrap(),
        b"lib"
    );
    assert_eq!(
        fs::read(dest.join("lib").join("sub").join("deep.txt")).unwrap(),
        b"deep"
    );
    // Symlinks are recreated as symlinks with the original target.
    #[cfg(unix)]
    {
        let link = dest.join("lib").join("libslang-link.so");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "{link:?} was not recreated as a symlink"
        );
        assert_eq!(
            fs::read_link(&link).unwrap(),
            Path::new("libslang-compiler.so")
        );
        let share = dest.join("share");
        assert!(fs::symlink_metadata(&share)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_link(&share).unwrap(), Path::new("../share-real"));
        assert!(dest.join("share-real").is_dir());
    }

    std::env::remove_var(shader_slang_dev::SHADER_SLANG_HOME_ENV_VAR);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn install_artifacts_rejects_a_root_without_slangc() {
    let _guard = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("invalid");
    let bogus = dir.join("bogus");
    fs::create_dir_all(&bogus).unwrap();

    std::env::set_var(shader_slang_dev::SHADER_SLANG_HOME_ENV_VAR, &bogus);
    let err = shader_slang_dev::install_artifacts(&dir.join("dest")).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

    std::env::remove_var(shader_slang_dev::SHADER_SLANG_HOME_ENV_VAR);
    let _ = fs::remove_dir_all(&dir);
}
