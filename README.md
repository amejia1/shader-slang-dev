# shader-slang-dev

Rust crates for building the [shader-slang](https://github.com/shader-slang/slang)
compiler toolchain (`slangc` and friends) from source and locating the built
artifacts from another crate's `build.rs`:

- **`shader-slang-src`** builds the shader-slang `2026.21` toolchain from
  source with static linking enabled and installs it in the same layout as
  the shader-slang release tarballs. The shader-slang source is not vendored
  in this repository (a submodule would make the crate too large to
  publish); `build.rs` fetches it at build time as a tarball from
  `SHADER_SLANG_SOURCE_URL` (default: the GitHub source archive of tag
  `v2026.21`).
- **`shader-slang-dev`** depends on `shader-slang-src` and exposes `PathBuf`
  accessors for the installed binaries, libraries, headers and share
  directory.

## Usage

```toml
[build-dependencies]
shader-slang-dev = "1"
```

```rust
fn main() {
    let slangc = shader_slang_dev::slangc();
    // Compile shaders with the built slangc:
    //   std::process::Command::new(slangc).arg(...).status().unwrap();
    // Link against the built libraries with:
    //   shader_slang_dev::lib_dir()   // -> pass as `-L <dir>` or as
    //  `cargo::rustc-link-search=[KIND=]<dir>` in cargo build script.
    // Compile against the headers with:
    //   shader_slang_dev::include_dir() // -> pass as `-I <dir>`
}
```

## Versioning

`shader-slang-src` is versioned with plain semver (`1.0.0`); the crate no
longer carries the slang release as `+<version>` build metadata. The
shader-slang release it builds is `2026.21`, pinned by the default value of
the `SHADER_SLANG_SOURCE_URL` environment variable (the GitHub source
archive of tag `v2026.21`). To build a different slang release, point
`SHADER_SLANG_SOURCE_URL` at that release's source tarball.

`shader-slang-dev` depends on `shader-slang-src = "1"` (caret), so a fresh
dependency resolution picks the latest published `1.x` of
`shader-slang-src` — just depend on `shader-slang-dev` and you get the
newest published release. Existing `Cargo.lock` files keep their locked
`shader-slang-src` version until you run `cargo update`.

## How it builds

`shader-slang-src`'s `build.rs` fetches the shader-slang source tarball from
`SHADER_SLANG_SOURCE_URL` — a valid `https` or `file` URL in the WHATWG URL
standard — into `$OUT_DIR` (downloading `https` URLs, copying `file` URLs),
extracts it under `$OUT_DIR`, and configures and builds the extracted tree
through the `cmake` crate (which drives the system `cmake` binary) using the
same flags as candle's `build-install-slang.sh` (`-DSLANG_LIB_TYPE=STATIC`,
tests/examples/rhi/gfx disabled). It then installs the toolchain into the
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

The build requires `git`, `cmake` (>= 3.20, for `cmake --install --strip`)
and a C/C++ toolchain. Linux is fully supported; macOS and Windows support
is best-effort.

The default source URL points at the GitHub source archive of tag
`v2026.21`
(`https://github.com/shader-slang/slang/archive/refs/tags/v2026.21.tar.gz`).
GitHub source archives do not carry the `external/` submodule checkouts
that slang vendors, so after extracting the tarball `build.rs` fetches the
pinned submodules of the `v2026.21` tag with `git` (`git init` + fetch of
the tag + `git submodule update --init --recursive`) before configuring.
A `SHADER_SLANG_SOURCE_URL` that points at a tarball which already includes
the `external/` checkouts (e.g. a pre-packed tarball) skips that step.

Changing `SHADER_SLANG_SOURCE_URL` re-runs the build script
(`cargo::rerun-if-env-changed`), so the build follows the new URL.

### Using prebuilt binaries

Setting the environment variable `SHADER_SLANG_DIST_URL` to a non-empty
`https` or `file` URL of a shader-slang distribution tarball makes
`build.rs` fetch that tarball into `$OUT_DIR` (download for `https`, copy
for `file`), extract it under `$OUT_DIR`, and use the extracted directory
as the artifact root — the location of the shader-slang binaries, exactly
as if they had been built from source and installed — instead of building
from source:

```sh
SHADER_SLANG_DIST_URL=https://github.com/shader-slang/slang/releases/download/v2026.21/slang-2026.21-linux-x86_64.tar.gz cargo build
```

An unset or empty variable (the default) builds from source. Changing
`SHADER_SLANG_DIST_URL` re-runs the build script
(`cargo::rerun-if-env-changed`) and re-provides the matching artifacts
(the previously installed set is removed when the provenance changes).

The two provenances differ in what they provide:

- The source build installs **static** libraries; the distribution tarballs
  ship **shared** libraries (and import `.lib` files on Windows) instead,
  plus the prebuilt `slang-llvm` backend plugin and `libgfx`.

The `shader-slang-src::SOURCE` constant (`"source"` or `"download"`) tells
the two apart. The URLs the build used are exposed as the
`shader-slang-src::SHADER_SLANG_SOURCE_URL` and
`shader-slang-src::SHADER_SLANG_DIST_URL` constants
(`SHADER_SLANG_DIST_URL` is `""` when the variable was unset or empty).
