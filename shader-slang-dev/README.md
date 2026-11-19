# shader-slang-dev

Locates an installed [shader-slang](https://github.com/shader-slang/slang)
`2026.21` toolchain, or builds the vendored source with the `vendored` feature.

Accessors include:

- `slangc()` and `binary(name)` for executables.
- `artifact_root()`, `bin_dir()`, `lib_dir()`, `include_dir()`, and `share_dir()`.
- `locate()` and `source()` for the selected origin.
- `shader_slang_home()`, `common_locations()`, `is_install_root()`, and
  `is_pinned_install_root()` for installed-toolchain discovery.
- `SLANG_VERSION` and `SHADER_SLANG_HOME_ENV_VAR` constants.

## Default: installed toolchain

The runtime lookup checks `$SHADER_SLANG_HOME` first, then common OS- and
architecture-specific install locations. `SHADER_SLANG_HOME` is authoritative;
common locations must contain `bin/slangc` and the pinned release's versioned
standard-module tree. `locate()` returns `Location::Home` or
`Location::System`, and `source()` returns `"home"` or `"system"`.

Download and extract a release tarball yourself to use prebuilt artifacts,
then set `SHADER_SLANG_HOME` to its root. The crate does not download source
or distribution tarballs.

## `vendored`: build the included source

```toml
[build-dependencies]
shader-slang-dev = { version = "1", features = ["vendored"] }
```

The `shader-slang-src` dependency reassembles the complete source from its
`shader-slang-src-*` partition crates, builds it with CMake, and installs it
under its `$OUT_DIR/slang-artifact`. No clone or download occurs. This mode
uses only `Location::Vendored`; `SHADER_SLANG_HOME` and common locations are
ignored, and `source()` returns `"vendored"`.

CMake >= 3.20 and a C/C++ toolchain are required. The build installs static
compiler/runtime libraries and shared backend plugins. Manually installed
release tarballs usually contain shared compiler libraries instead.

`install_artifacts(&Path)` and the `shader-slang-dev-installer` binary are
available with `vendored` and copy the vendored tree, preserving symlinks:

```sh
cargo run --features vendored --bin shader-slang-dev-installer -- /opt/slang
```

See the workspace `README.md` for the complete location lists, build layout,
and source-update instructions.
