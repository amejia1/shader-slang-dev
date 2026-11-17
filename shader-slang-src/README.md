# shader-slang-src

Builds the [shader-slang](https://github.com/shader-slang/slang) compiler
toolchain (release `2026.21`, tag `v2026.21`) from source with static
linking enabled, installs it in the same layout as the release tarballs,
and exposes the location of the installed binaries, libraries, headers and
share directory.

The shader-slang source is not vendored in this crate (a git submodule
would make the crate too large to publish). By default `build.rs` clones
tag `v2026.21` of the upstream repository with all its submodules into
`$OUT_DIR` and builds from the clone. Alternatively, setting the
`SHADER_SLANG_SOURCE_URL` environment variable to a non-empty `https` or
`file` URL (in the WHATWG URL standard) makes `build.rs` fetch that source
tarball into `$OUT_DIR` instead — downloading `https` URLs and copying
`file` URLs — and extract it there; the tarball is expected to already
contain the `external/` submodule checkouts (plain GitHub source archives
do not, and are rejected).

Setting the environment variable `SHADER_SLANG_DIST_URL` to a non-empty
`https` or `file` URL of a prebuilt shader-slang distribution tarball makes
`build.rs` fetch and extract that tarball instead and use the extracted
directory as the installed toolchain, skipping the source build. See the
workspace `README.md` for details and the differences between the two
artifact sets (`shader-slang-src::SOURCE`).

See the workspace `README.md` for usage; most consumers should depend on
`shader-slang-dev` instead, which wraps this crate with ergonomic `PathBuf`
accessors.
