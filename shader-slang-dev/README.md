# shader-slang-dev

Locates an installed [shader-slang](https://github.com/shader-slang/slang)
compiler toolchain (release `2026.21`, tag `v2026.21`) and exposes `PathBuf`
accessors for the installed binaries, libraries, headers and share
directory:

- `slangc()` — the compiler binary; `binary(name)` — any other binary under `bin_dir()`.
- `artifact_root()` — the install prefix the artifacts live in.
- `bin_dir()` — the directory holding the binaries.
- `lib_dir()` — the directory holding the static/shared libraries (for `-L` or `cargo::rustc-link-search=[KIND=]PATH`).
- `include_dir()` — the directory holding the headers (for `-I`).
- `share_dir()` — the directory holding the installed share data (documentation under `doc/slang/`, agent skills under `slang/agent-skills/`).
- `locate()` — where the artifacts were found (`Location::Home` / `System` / `OutDir`), `None` when nothing is found.
- `source()` — how the artifacts in use were obtained: `"home"`, `"system"`, `"source"`, `"download"` or `"none"`.
- `shader_slang_home()`, `common_locations()`, `is_install_root()` — the detection inputs (`$SHADER_SLANG_HOME`, the common per-OS/per-arch locations, the `bin/slangc` check).
- `SLANG_VERSION` — the shader-slang release pinned (`2026.21`); `SHADER_SLANG_HOME_ENV_VAR` — `"SHADER_SLANG_HOME"`; `SHADER_SLANG_SOURCE_URL` / `SHADER_SLANG_DIST_URL` — the URLs the build used (each `""` when the corresponding variable was unset or empty; an empty `SHADER_SLANG_SOURCE_URL` means the source came from a git clone).

The artifact root is resolved at runtime, in this order:

1. `$SHADER_SLANG_HOME` — an install prefix containing `bin/slangc`, whether
   it was built and installed from source or installed from a downloaded
   release tarball.
2. Common install locations per OS and CPU architecture (Linux: `/usr/local`,
   `/usr`, `/opt/slang`, `/opt/shader-slang`, `/opt/slang-<arch>`,
   `/opt/slang-linux-<arch>`, ..., `$HOME/.local`; macOS: additionally
   `/opt/homebrew`; Windows: `%ProgramFiles%\slang`, `C:\slang`,
   `C:\slang-windows-<arch>`, ...). A location only counts when it contains
   `bin/slangc`.
3. `$OUT_DIR/slang-artifact` — only when the `build` feature is enabled and
   the build script produced artifacts there.

With the single `build` feature (disabled by default), `build.rs` clones and
builds shader-slang from source — or fetches the `SHADER_SLANG_SOURCE_URL` /
`SHADER_SLANG_DIST_URL` tarballs — into `$OUT_DIR/slang-artifact` (skipping
the build when an installation is already available through (1) or (2)), the
optional build dependencies (`cmake`, `flate2`, `tar`, `ureq`, `url`) are
compiled, and two installation entry points become available:

- `install_artifacts(&Path)` — recursively copies the located artifact tree
  into the given install prefix, recreating symlinks with their original
  targets.
- the `shader-slang-dev-installer` binary:
  `cargo run --features build --bin shader-slang-dev-installer -- /opt/slang`

The libraries available depend on how the `$OUT_DIR` artifacts were
obtained: `source() == "source"` builds install the compiler and runtime as
static libraries (`slang-compiler`, `slang-rt`), while `source() ==
"download"` (prebuilt distribution tarball provided via
`SHADER_SLANG_DIST_URL`) ships shared libraries instead. Installs found via
`$SHADER_SLANG_HOME` or a common location are not classified by provenance;
the tests probe the artifact tree for whichever library kind is present.

See the workspace `README.md` for the full build details, the common-location
lists and usage examples.
