# shader-slang-src

Builds the [shader-slang](https://github.com/shader-slang/slang) compiler
toolchain (`slangc` and friends) from source with static linking enabled, and
exposes the location of the built binaries, libraries and headers.

The shader-slang source is vendored as a git submodule at `slang/`, pinned to a
release tag. The crate version encodes that release as `+<version>` build
metadata: `0.1.0+2026.21` builds shader-slang `2026.21`.

See the workspace `README.md` for usage; most consumers should depend on
`shader-slang-dev` instead, which wraps this crate with ergonomic `PathBuf`
accessors.
