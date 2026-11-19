//! Integration test for installing the vendored artifact tree.

#![cfg(feature = "vendored")]

use std::fs;

#[test]
fn install_artifacts_copies_vendored_tree() {
    let dir = std::env::temp_dir().join(format!("shader-slang-dev-install-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let dest = dir.join("install-dest");

    shader_slang_dev::install_artifacts(&dest).unwrap();

    assert_eq!(
        fs::read(dest.join("bin").join(if cfg!(windows) {
            "slangc.exe"
        } else {
            "slangc"
        }))
        .unwrap(),
        fs::read(shader_slang_dev::slangc()).unwrap()
    );
    assert!(dest.join("include").join("slang.h").is_file());
    assert!(dest.join("lib").is_dir());
    assert!(dest.join("share").is_dir());

    fs::remove_dir_all(&dir).unwrap();
}
