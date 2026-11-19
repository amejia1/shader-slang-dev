# shader-slang-dev

Rust crates for locating an installed [shader-slang](https://github.com/shader-slang/slang)
`2026.21` toolchain or building that release entirely from vendored source.

- **`shader-slang-dev`** exposes `PathBuf` accessors for `slangc`, other
  binaries, libraries, headers, and share data. Its optional `vendored`
  feature selects artifacts built from the vendored source.
- **`shader-slang-src`** reassembles and builds the complete source, including
  all submodules, and installs it into its `$OUT_DIR/slang-artifact`.
- **`shader-slang-src-*`** are the source-partition crates. Every partition
  packages below the crates.io 10 MiB limit. The aggregator depends on all of
  them with the same exact crate version.

## Locating an installation (default)

Without features, `shader-slang-dev` only locates an existing installation at
runtime, in this order:

1. **`$SHADER_SLANG_HOME`** — an install prefix containing `bin/slangc`.
   Authoritative: when set, it is used as-is:
   ```sh
   export SHADER_SLANG_HOME=/opt/slang-2026.21-linux-x86_64
   ```
2. **Common install locations**, per OS and CPU architecture (see
   `shader_slang_dev::common_locations()`). A location counts only when it
   provides `bin/slangc` and the versioned standard-module tree
   `slang-standard-module-2026.21` under `lib/` or `bin/`:
   - Linux: `/usr/local`, `/usr`, `/opt/slang`, `/opt/shader-slang`,
     `/opt/slang-<arch>`, `/opt/slang-linux-<arch>` (the same two under
     `/usr/local`), `$HOME/.local`, `$HOME/slang`, `$HOME/slang-linux-<arch>`.
   - macOS: `/usr/local`, `/opt/homebrew`, `/opt/slang`, `/opt/shader-slang`,
     `/opt/slang-<arch>`, `/opt/slang-macos-<arch>` (the same two under
     `/usr/local`), `$HOME/.local`, `$HOME/slang`, `$HOME/slang-macos-<arch>`.
   - Windows: `%ProgramFiles%\slang`, `%ProgramFiles%\shader-slang` (and the
     `(x86)` / `ProgramW6432` variants), `%LOCALAPPDATA%\slang`,
     `%LOCALAPPDATA%\Programs\slang`, `C:\slang`, `C:\slang-<arch>`,
     `C:\slang-windows-<arch>`, `%USERPROFILE%\slang`,
     `%USERPROFILE%\slang-windows-<arch>`.

`locate()` returns `Location::Home`, `Location::System`, or `None`;
`source()` reports `"home"`, `"system"`, or `"none"`. `artifact_root()`
panics with the searched locations when no installation is found.

To use a release tarball, download and extract it yourself, then set
`SHADER_SLANG_HOME` or extract it into a common location. No distribution URL
is needed or read by the crates.

## Usage

```toml
[build-dependencies]
shader-slang-dev = "1"
```

```rust
let slangc = shader_slang_dev::slangc();
// Compile shaders with the located slangc:
//   std::process::Command::new(slangc).arg("shader.slang").status().unwrap();
// Link against the located libraries:
let _lib_dir = shader_slang_dev::lib_dir();
// Compile against the headers:
let _include_dir = shader_slang_dev::include_dir();
```

## Building vendored source

Enable `vendored` to build the pinned source instead of locating an installed
toolchain:

```toml
[build-dependencies]
shader-slang-dev = { version = "1", features = ["vendored"] }
```

`shader-slang-src` extracts every `source.tar` partition over one tree, checks
that all submodule directories are populated, then configures and builds with
CMake. The source is already present in the crates: no Git clone, source URL,
or distribution URL is used. **`SHADER_SLANG_HOME` and common locations are
ignored in this mode.** `locate()` returns `Location::Vendored` and `source()`
returns `"vendored"`.

The CMake configuration uses a static compiler library, disables tests,
examples, RHI, gfx, and the downloadable LLVM backend, and enables the glslang
backend. The pinned numeric and full versions are supplied explicitly because
Git metadata is absent. The build requires CMake >= 3.20 and a C/C++
toolchain; Git is not required.

The installed layout matches the release tarballs:

- `bin/` — `slangc`, `slangd`, `slang`, `slangi`, and a copy of the standard
  modules needed by the statically linked compiler.
- `lib/` — static libraries, shared runtime plugins, standard modules, and
  CMake package files.
- `include/` — public headers.
- `share/` — documentation and agent skills.
- license and third-party notice files at the root.

Vendored builds install static compiler/runtime libraries. A manually
installed release tarball generally provides shared libraries instead.

## Installing the selected artifacts

With `vendored`, `install_artifacts(&Path)` recursively copies the vendored
artifact tree, preserving symlinks. The installer binary wraps it:

```sh
cargo run --features vendored --bin shader-slang-dev-installer -- /opt/slang
# The installation can then be used without the feature:
export SHADER_SLANG_HOME=/opt/slang
```

## Updating the vendored release

All Rust crates share `workspace.package.version` in the root `Cargo.toml`
(initially `1.0.0`). Change that single value before packaging a new set.
`shader-slang-src` requires every partition at that exact version.

Run the POSIX shell generator with a release tag:

```sh
devops/package-shader-slang-source.sh v2026.21
```

The script:

1. Makes a shallow clone of the tag and recursively shallow-clones every
   submodule (Git with `core.longpaths=true` supports deep Windows paths).
2. Deletes all `.git` directories and files.
3. Creates `shader-slang-src-core-*` and `shader-slang-src-external-*` crates,
   recursively splitting directories above 9 MiB and then best-fit packing the
   measured items into shared source archives.
4. Regenerates the aggregator's exact-version dependencies and source list,
   and synchronizes both crates' generated `SLANG_VERSION` constants.
5. Runs `cargo package` for every partition and rejects any `.crate` above
   10 MiB.

For a previously prepared checkout, set
`SHADER_SLANG_SOURCE_CHECKOUT=/path/to/slang`; the script still removes its
Git metadata. Without that variable, its temporary clone lives under
`target/source-packaging/`.

Validate the script with:

```sh
shellcheck -s dash devops/package-shader-slang-source.sh
```

Publish partition crates before `shader-slang-src`, and publish
`shader-slang-src` before `shader-slang-dev`, so registry dependencies resolve
at every step.
