//! Integration test: run the `slangc` binary built by this crate and check
//! that it reports the expected shader-slang release.

use std::process::Command;

#[test]
fn slangc_reports_expected_version() {
    let slangc = shader_slang_src::slangc();
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

    // `slangc -version` prints the release to stderr.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(shader_slang_src::SLANG_VERSION),
        "`{} -version` did not report expected version `{}`.\nstdout: {stdout}\nstderr: {stderr}",
        slangc.display(),
        shader_slang_src::SLANG_VERSION,
    );
}
