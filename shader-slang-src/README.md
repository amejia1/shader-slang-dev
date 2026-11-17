# shader-slang-src

Builds the [shader-slang](https://github.com/shader-slang/slang) compiler
toolchain (release `2026.21`, tag `v2026.21`) from source with static
linking enabled, installs it in the same layout as the release tarballs,
and exposes the location of the installed binaries, libraries, headers and
share directory.

The shader-slang source is not vendored in this crate (a git submodule
would make the crate too large to publish). Instead, `build.rs` fetches the
source tarball from the `SHADER_SLANG_SOURCE_URL` environment variable — a
valid `https` or `file` URL in the WHATWG URL standard; unset or empty
defaults to the GitHub source archive of tag `v2026.21`,
`https://github.com/shader-slang/slang/archive/refs/tags/v2026.21.tar.gz`
— downloads or copies it into `$OUT_DIR`, extracts it there, and builds
from the extracted tree.

Setting the environment variable `SHADER_SLANG_DIST_URL` to a non-empty
`https` or `file` URL of a prebuilt shader-slang distribution tarball makes
`build.rs` fetch and extract that tarball instead and use the extracted
directory as the installed toolchain, skipping the source build. See the
workspace `README.md` for details and the differences between the two
artifact sets (`shader-slang-src::SOURCE`).

See the workspace `README.md` for usage; most consumers should depend on
`shader-slang-dev` instead, which wraps this crate with ergonomic `PathBuf`
accessors.
