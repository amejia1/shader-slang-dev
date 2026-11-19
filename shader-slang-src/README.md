# shader-slang-src

Builds the vendored shader-slang `2026.21` source and exposes the installed
artifact root through `shader_slang_src::artifact_root()`.

The `shader-slang-src-*` dependencies contain the complete upstream source,
including all recursively cloned submodules, as source archives. This crate's
build script extracts those archives over one source tree, builds shader-slang
with CMake, and installs the toolchain into `$OUT_DIR/slang-artifact`.

Use the `vendored` feature of `shader-slang-dev` instead of depending on this
crate directly when runtime artifact discovery and path accessors are needed.
