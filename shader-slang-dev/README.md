# shader-slang-dev

Ergonomic `PathBuf` accessors for the shader-slang binaries and libraries built
by [`shader-slang-src`](https://crates.io/crates/shader-slang-src):

- `slangc()`, `slangd()`, `slang()`, `slangi()`, `slang_replay()` — the binaries.
- `lib_dir()` — the directory holding the static/shared libraries (for `-L` or `cargo::rustc-link-search=[KIND=]PATH`).
- `include_dir()` — the directory holding the headers (for `-I`).

Availability of the static libraries and `slang_replay()` depends on how the
artifacts were obtained: `SOURCE == "source"` builds ship static libraries
and `slang_replay`, while `SOURCE == "download"` (prebuilt release
tarballs) ship shared libraries instead.

See the workspace `README.md` for a usage example.
