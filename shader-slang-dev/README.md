# shader-slang-dev

Builds the [shader-slang](https://github.com/shader-slang/slang) compiler
toolchain (release `2026.21`, tag `v2026.21`) from source with static
linking enabled, installs it in the same layout as the release tarballs,
and exposes `PathBuf` accessors for the installed binaries, libraries,
headers and share directory:

- `slangc()` — the compiler binary; `binary(name)` — any other binary under `bin_dir()`.
- `artifact_root()` — the install prefix the artifacts live in.
- `bin_dir()` — the directory holding the binaries.
- `lib_dir()` — the directory holding the static/shared libraries (for `-L` or `cargo::rustc-link-search=[KIND=]PATH`).
- `include_dir()` — the directory holding the headers (for `-I`).
- `share_dir()` — the directory holding the installed share data (documentation under `doc/slang/`, agent skills under `slang/agent-skills/`).
- `SLANG_VERSION` — the shader-slang release in use (`2026.21`); `SOURCE` — how the artifacts were obtained (`"source"` or `"download"`); `SHADER_SLANG_SOURCE_URL` / `SHADER_SLANG_DIST_URL` — the URLs the build used (each `""` when the corresponding variable was unset; an empty `SHADER_SLANG_SOURCE_URL` means the source came from a git clone).

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
directory as the installed toolchain, skipping the source build.

The libraries available depend on how the artifacts were obtained:
`SOURCE == "source"` builds install the compiler and runtime as static
libraries (`slang-compiler`, `slang-rt`), while `SOURCE == "download"`
(prebuilt distribution tarball provided via `SHADER_SLANG_DIST_URL`) ship
shared libraries instead.

See the workspace `README.md` for a usage example and build details.
