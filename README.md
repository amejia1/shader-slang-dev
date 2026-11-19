# shader-slang-dev

A Rust crate that locates an installed [shader-slang](https://github.com/shader-slang/slang)
compiler toolchain (`slangc` and friends) and exposes `PathBuf` accessors for
the binaries, libraries, headers and share directory. With the single
`build` feature it can additionally build the toolchain from source or from
the downloaded release tarballs into `$OUT_DIR`, and install the artifacts
into a prefix (`install_artifacts`, or the `shader-slang-dev-installer`
binary).

- **`shader-slang-dev`** locates the shader-slang `2026.21` toolchain and
  exposes `PathBuf` accessors for the installed binaries, libraries, headers
  and share directory.

## Locating an installation

The artifact root is resolved at runtime, in this order:

1. **`$SHADER_SLANG_HOME`** — an install prefix containing `bin/slangc`,
   whether it was built and installed from source or installed from a
   downloaded release tarball. Authoritative: when set, it is used as-is:
   ```sh
   export SHADER_SLANG_HOME=/opt/slang-2026.21-linux-x86_64
   ```
2. **Common install locations**, per OS and CPU architecture (see
   `shader_slang_dev::common_locations()`). A location only counts when it
   provides the pinned release `2026.21` — `bin/slangc` plus the versioned
   standard-module tree `lib/slang-standard-module-2026.21` (see
   `shader_slang_dev::is_pinned_install_root()`) — so an install of a
   *different* slang release (e.g. a distro package) is not picked up:
   - Linux: `/usr/local`, `/usr`, `/opt/slang`, `/opt/shader-slang`,
     `/opt/slang-<arch>`, `/opt/slang-linux-<arch>` (the same two under
     `/usr/local`), `$HOME/.local`, `$HOME/slang`, `$HOME/slang-linux-<arch>`
   - macOS: `/usr/local`, `/opt/homebrew`, `/opt/slang`, `/opt/shader-slang`,
     `/opt/slang-<arch>`, `/opt/slang-macos-<arch>` (the same two under
     `/usr/local`), `$HOME/.local`, `$HOME/slang`, `$HOME/slang-macos-<arch>`
   - Windows: `%ProgramFiles%\slang`, `%ProgramFiles%\shader-slang` (and the
     `(x86)` / `ProgramW6432` variants), `%LOCALAPPDATA%\slang`,
     `%LOCALAPPDATA%\Programs\slang`, `C:\slang`, `C:\slang-<arch>`,
     `C:\slang-windows-<arch>`, `%USERPROFILE%\slang`,
     `%USERPROFILE%\slang-windows-<arch>`
3. **`$OUT_DIR/slang-artifact`** — only when the `build` feature is enabled
   and the build script produced artifacts there.

`shader_slang_dev::locate()` returns where the artifacts were found,
`shader_slang_dev::source()` reports `"home"`, `"system"`, `"source"`,
`"download"` or `"none"`, and `shader_slang_dev::artifact_root()` panics with
the list of searched locations when nothing is found.

## Usage

```toml
[build-dependencies]
shader-slang-dev = "1"
```
```rust
let slangc = shader_slang_dev::slangc();
// Compile shaders with the located slangc:
//   std::process::Command::new(slangc).arg(...).status().unwrap();
// Link against the located libraries with:
//   shader_slang_dev::lib_dir()   // -> pass as `-L <dir>` or as
//  `cargo::rustc-link-search=[KIND=]<dir>` in cargo build script.
// Compile against the headers with:
//   shader_slang_dev::include_dir() // -> pass as `-I <dir>`
```

## Versioning

The crate is versioned with plain semver (`1.0.0`); it does not carry the
slang release as `+<version>` build metadata. The shader-slang release it
pins is `2026.21`, tag `v2026.21` (cloned by the `build` feature's default
source path; see below).

## Building from source or tarballs: the `build` feature

The single `build` feature — **disabled by default** — gates all support for
producing shader-slang artifacts:

```toml
[build-dependencies]
shader-slang-dev = { version = "1", features = ["build"] }
```

When enabled, the optional build dependencies (`cmake`, `flate2`, `tar`,
`ureq`, `url`) are compiled, `build.rs` obtains the shader-slang source into
`$OUT_DIR` in one of two ways, and the `install_artifacts` function plus the
`shader-slang-dev-installer` binary become available:

- **Default (both URL variables unset or empty):** a shallow git clone of
  `https://github.com/shader-slang/slang.git` at tag `v2026.21` with
  `--recurse-submodules --shallow-submodules`, placed under `$OUT_DIR`.
- **`SHADER_SLANG_SOURCE_URL` set to a non-empty `https` or `file` URL** (in
  the WHATWG URL standard): the tarball at that URL is downloaded (`https`)
  or copied (`file`) into `$OUT_DIR` and extracted there. The tarball is
  expected to already contain the `external/` submodule checkouts (plain
  GitHub source archives do not, and are rejected); e.g. pack a fully
  checked-out tree with `git archive`.

From the source tree, `build.rs` configures and builds shader-slang through
the `cmake` crate (which drives the system `cmake` binary) using the same
flags as candle's `build-install-slang.sh` (`-DSLANG_LIB_TYPE=STATIC`,
tests/examples/rhi/gfx disabled), then installs the toolchain into the
artifact root (`<out_dir>/slang-artifact/`) with
`cmake --install <build_dir> --prefix <artifact_root> --strip`. The install
tree has the same layout as the shader-slang release tarballs:

- `bin/` — the binaries (`slangc`, `slangd`, `slang`, `slangi`), plus a
  copy of the standard-module tree: the statically linked `slangc`
  resolves the loose standard modules relative to its own executable, so
  the tree must sit next to the binary.
- `lib/` — the static libraries, the shared backend plugins (which `slangc`
  dlopens at runtime through the binaries' `$ORIGIN/../lib` RUNPATH), the
  standard-module tree, and the CMake package files.
- `include/` — the public headers.
- `share/` — the documentation (`doc/slang/`) and the agent skills
  (`slang/agent-skills/`).
- the license and third-party notice files at the root.

If an installation is already available through `$SHADER_SLANG_HOME` or a
common location (see above), the build is **skipped** — the lookup order
prefers those over `$OUT_DIR` anyway, so building would be wasted work.

Changing `SHADER_SLANG_SOURCE_URL` or `SHADER_SLANG_DIST_URL` re-runs the
build script (`cargo::rerun-if-env-changed`), so the build follows the new
value.

The build requires `git`, `cmake` (>= 3.20, for `cmake --install --strip`)
and a C/C++ toolchain. Linux is fully supported; macOS and Windows support
is best-effort. On Windows the clone runs with `core.longpaths=true` so the
submodule checkouts under the deep `$OUT_DIR` path succeed.

### Using prebuilt binaries (`SHADER_SLANG_DIST_URL`)

Setting the environment variable `SHADER_SLANG_DIST_URL` to a non-empty
`https` or `file` URL of a shader-slang distribution tarball makes
`build.rs` fetch that tarball into `$OUT_DIR` (download for `https`, copy
for `file`), extract it under `$OUT_DIR`, and use the extracted directory
as the artifact root — the location of the shader-slang binaries, exactly
as if they had been built from source and installed — instead of building
from source:

```sh
SHADER_SLANG_DIST_URL=https://github.com/shader-slang/slang/releases/download/v2026.21/slang-2026.21-linux-x86_64.tar.gz cargo build --features build
```

An unset or empty variable (the default) builds from source. Changing
`SHADER_SLANG_DIST_URL` re-runs the build script
(`cargo::rerun-if-env-changed`) and re-provides the matching artifacts
(the previously installed set is removed when the provenance changes).

The two provenances differ in what they provide:

- The source build installs **static** libraries; the distribution tarballs
  ship **shared** libraries (and import `.lib` files on Windows) instead,
  plus the prebuilt `slang-llvm` backend plugin and `libgfx`.

For `$OUT_DIR` artifacts, `shader_slang_dev::source()` tells the two apart
(`"source"` vs `"download"`); artifacts found via `$SHADER_SLANG_HOME` or a
common location report `"home"` / `"system"`. The URLs the build used are
exposed as the `shader_slang_dev::SHADER_SLANG_SOURCE_URL` and
`shader_slang_dev::SHADER_SLANG_DIST_URL` constants (each `""` when the
corresponding variable was unset or empty; an empty
`SHADER_SLANG_SOURCE_URL` means the source came from a git clone).

### Installing the artifacts: `install_artifacts` / `shader-slang-dev-installer`

With the `build` feature, `shader_slang_dev::install_artifacts` recursively
copies the located artifact tree (the one `artifact_root()` resolves — from
`$SHADER_SLANG_HOME`, a common location, or the `$OUT_DIR` build) into a
given install prefix, recreating symlinks with their original targets:

```rust,no_run
shader_slang_dev::install_artifacts(std::path::Path::new("/opt/slang")).unwrap();
```

The `shader-slang-dev-installer` binary (built only with the `build`
feature) does the same from the command line:

```sh
cargo run --features build --bin shader-slang-dev-installer -- /opt/slang
# then point the crate at it:
export SHADER_SLANG_HOME=/opt/slang
```
