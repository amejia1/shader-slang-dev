//! Integration tests for the runtime installation detection.
//!
//! `SHADER_SLANG_HOME` is process-global, so the env-mutating assertions live
//! in a single test (guarded by a lock) to keep the test binary race-free.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "shader-slang-dev-detection-{}-{tag}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn fake_install(root: &Path) {
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(
        bin.join(if cfg!(windows) {
            "slangc.exe"
        } else {
            "slangc"
        }),
        b"fake slangc",
    )
    .unwrap();
}

fn slangc_name() -> &'static str {
    if cfg!(windows) {
        "slangc.exe"
    } else {
        "slangc"
    }
}

#[test]
fn shader_slang_home_env_var_is_used_first() {
    let _guard = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("home-env");
    let home = dir.join("slang-home");
    fake_install(&home);

    std::env::set_var(shader_slang_dev::SHADER_SLANG_HOME_ENV_VAR, &home);
    assert_eq!(shader_slang_dev::shader_slang_home(), Some(home.clone()));
    assert_eq!(shader_slang_dev::artifact_root(), home);
    assert_eq!(shader_slang_dev::source(), "home");
    assert_eq!(
        shader_slang_dev::slangc(),
        home.join("bin").join(slangc_name())
    );
    assert_eq!(
        shader_slang_dev::locate(),
        Some(shader_slang_dev::Location::Home(home.clone()))
    );

    std::env::remove_var(shader_slang_dev::SHADER_SLANG_HOME_ENV_VAR);
    assert!(shader_slang_dev::shader_slang_home().is_none());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn is_install_root_checks_bin_slangc() {
    let dir = temp_dir("is-install-root");
    assert!(!shader_slang_dev::is_install_root(&dir));
    fake_install(&dir);
    assert!(shader_slang_dev::is_install_root(&dir));
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn common_locations_are_os_and_arch_aware() {
    let locs = shader_slang_dev::common_locations();
    assert!(!locs.is_empty());
    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "other-arch"
    };
    if cfg!(target_os = "linux") {
        assert!(locs.contains(&PathBuf::from("/usr/local")));
        assert!(locs.contains(&PathBuf::from("/opt/slang")));
        assert!(locs
            .iter()
            .any(|p| p.to_string_lossy().contains(&format!("slang-linux-{arch}"))));
    } else if cfg!(target_os = "macos") {
        assert!(locs.contains(&PathBuf::from("/opt/homebrew")));
        assert!(locs
            .iter()
            .any(|p| p.to_string_lossy().contains(&format!("slang-macos-{arch}"))));
    } else if cfg!(target_os = "windows") {
        assert!(locs.iter().any(|p| p
            .to_string_lossy()
            .contains(&format!("slang-windows-{arch}"))));
    }
}
