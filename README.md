# shader-slang-dev

Rust crates for building the [shader-slang](https://github.com/shader-slang/slang)
compiler toolchain (`slangc` and friends) from source and locating the built
artifacts from another crate's `build.rs`.

- **`shader-slang-src`** builds the shader-slang binaries and libraries from
  source with static linking enabled. The shader-slang source is tracked as a
  git submodule pinned to a release tag; the crate version carries the slang
  version as `+<version>` build metadata (e.g. `0.1.0+2026.21` builds
  shader-slang `2026.21`).
- **`shader-slang-dev`** depends on `shader-slang-src` and exposes `PathBuf`
  accessors for the built binaries, libraries and headers.

## Usage

```toml
[build-dependencies]
shader-slang-dev = "0.1.0"
```

```rust
fn main() {
    let slangc = shader_slang_dev::slangc();
    // Compile shaders with the built slangc:
    //   std::process::Command::new(slangc).arg(...).status().unwrap();
    // Link against the built libraries with:
    //   shader_slang_dev::lib_dir()   // -> pass as `-L <dir>`
    // Compile against the headers with:
    //   shader_slang_dev::include_dir() // -> pass as `-I <dir>`
}
```

## How it builds

`shader-slang-src`'s `build.rs` configures and builds shader-slang with CMake
using the same flags as candle's `build-install-slang.sh`
(`-DSLANG_LIB_TYPE=STATIC`, tests/examples/rhi/gfx disabled), then collects the
produced binaries into `<out>/bin/`, the static/shared libraries into
`<out>/lib/`, and the public headers into `<out>/include/`.

The build requires `git`, `cmake` and a C/C++ toolchain. Linux is fully
supported; macOS and Windows support is best-effort.

The shader-slang source is a git submodule at `slang/`. When the submodule is
not available (a `crates.io` tarball or a cargo *git* dependency, neither of
which carries submodule contents), `build.rs` clones the pinned release tag
instead.
