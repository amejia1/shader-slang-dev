# shader-slang-dev

Rust crates for building the [shader-slang](https://github.com/shader-slang/slang)
compiler toolchain (`slangc` and friends) from source and locating the built
artifacts from another crate's `build.rs`.

- **`shader-slang-src`** builds the shader-slang binaries and libraries from
  source with static linking enabled. The shader-slang source is tracked as a
  git submodule pinned to a release tag; the crate version carries the slang
  version as `+<version>` build metadata (e.g. `1.0.0+2026.21` builds
  shader-slang `2026.21`).
- **`shader-slang-dev`** depends on `shader-slang-src` and exposes `PathBuf`
  accessors for the built binaries, libraries and headers.

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

`shader-slang-src` records the slang release it builds in two places:

- **Build metadata** (`+<slang-version>`) — the exact slang release:
  `1.0.0+2026.21` builds slang `2026.21` (tag `v2026.21`); `build.rs`
  reads it from the crate version.
- **Patch number** — a release counter, bumped once per new slang source
  revision: `1.0.0+2026.21`, `1.0.1+2026.22`, `1.0.2+2026.22.1`, ...

Cargo ignores build metadata when matching version requirements, so the
patch number is what dependency resolution sees. Every published
`shader-slang-src` version must therefore have a **unique patch number**
(never republish a numeric version with different build metadata).

### Using the latest slang release

`shader-slang-dev` depends on `shader-slang-src = "1"` (caret), so a fresh
dependency resolution picks the latest published `1.x` of
`shader-slang-src` — just depend on `shader-slang-dev` and you get the
newest slang release. Existing `Cargo.lock` files keep their locked
`shader-slang-src` version until you run `cargo update`.

### Pinning a specific slang release

To build a specific slang release, add an exact pin on `shader-slang-src`
alongside the `shader-slang-dev` dependency; it is intersected with the
caret requirement above:

```toml
[build-dependencies]
shader-slang-dev = "1"
shader-slang-src = "=1.0.1" # always build slang 2026.22
```

The pin targets the numeric version (build metadata is ignored in version
requirements). If the pin conflicts with the range `shader-slang-dev`
declares for `shader-slang-src`, cargo fails with a clear resolution error.

## How it builds

`shader-slang-src`'s `build.rs` configures and builds shader-slang through
the `cmake` crate (which drives the system `cmake` binary) using the same
flags as candle's `build-install-slang.sh` (`-DSLANG_LIB_TYPE=STATIC`,
tests/examples/rhi/gfx disabled), then collects the produced binaries into
`<out>/bin/`, the static/shared libraries into `<out>/lib/`, and the public
headers into `<out>/include/`.

The build requires `git`, `cmake` and a C/C++ toolchain. Linux is fully
supported; macOS and Windows support is best-effort.

The shader-slang source is a git submodule at `slang/`. When the submodule is
not available (a `crates.io` tarball or a cargo *git* dependency, neither of
which carries submodule contents), `build.rs` clones the pinned release tag
instead.
