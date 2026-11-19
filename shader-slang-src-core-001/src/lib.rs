//! Source archive consumed by the `shader-slang-src` build script.

/// Absolute path to this crate's shader-slang source archive.
pub const SOURCE_ARCHIVE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/source.tar");
