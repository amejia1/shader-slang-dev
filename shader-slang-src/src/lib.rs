//! Build artifacts produced from the vendored shader-slang source partitions.

use std::path::{Path, PathBuf};

/// Root of the installed shader-slang artifacts in this crate's `$OUT_DIR`.
///
/// The directory contains `bin/`, `lib/`, `include/`, and `share/`. It is
/// available after this crate's build script has completed.
pub fn artifact_root() -> PathBuf {
    Path::new(env!("OUT_DIR")).join("slang-artifact")
}
