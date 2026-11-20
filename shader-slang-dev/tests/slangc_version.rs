//! Integration test: run the located `slangc` and check that it reports a
//! version. The crate does not pin a shader-slang release, so the version in
//! use is whatever the located `slangc` reports via `slangc -version`.

use std::process::Command;

#[test]
fn slangc_reports_a_version() {
    let slangc = shader_slang_dev::slangc();
    assert!(slangc.is_file(), "slangc not found at {}", slangc.display());

    let output = Command::new(&slangc)
        .arg("-version")
        .output()
        .unwrap_or_else(|e| panic!("failed to run `{} -version`: {e}", slangc.display()));

    assert!(
        output.status.success(),
        "`{} -version` failed with {:?}",
        slangc.display(),
        output.status
    );

    // `slangc -version` prints the release version (to stderr).
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let version = format!("{stdout}{stderr}");
    let version = version.trim();
    assert!(
        !version.is_empty() && version.chars().any(|c| c.is_ascii_digit()),
        "`{} -version` did not report a version.\nstdout: {stdout}\nstderr: {stderr}",
        slangc.display(),
    );
}
