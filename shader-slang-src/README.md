# shader-slang-src

Builds the [shader-slang](https://github.com/shader-slang/slang) compiler
toolchain (`slangc` and friends) from source with static linking enabled, and
exposes the location of the built binaries, libraries and headers.

The shader-slang source is vendored as a git submodule at `slang/`, pinned to a
release tag. The crate version encodes that release as `+<version>` build
metadata: `1.0.0+2026.21` builds shader-slang `2026.21`.

Setting the environment variable `SHADER_SLANG_SRC_DOWNLOAD_BINARIES` to
`true` (case-sensitive) makes `build.rs` download and extract the prebuilt
shader-slang release binaries from the corresponding GitHub release instead
of building them from source; any other value (or an unset variable) builds
from source. See the workspace `README.md` for details and the differences
between the two artifact sets (`shader-slang-src::SOURCE`).

See the workspace `README.md` for usage; most consumers should depend on
`shader-slang-dev` instead, which wraps this crate with ergonomic `PathBuf`
accessors.
