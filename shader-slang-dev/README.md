# shader-slang-dev

Ergonomic `PathBuf` accessors for the shader-slang binaries and libraries built
by [`shader-slang-src`](https://crates.io/crates/shader-slang-src):

- `slangc()`, `slangd()`, `slang()`, `slangi()`, `slang_replay()` — the binaries.
- `lib_dir()` — the directory holding the static/shared libraries (for `-L`).
- `include_dir()` — the directory holding the headers (for `-I`).

See the workspace `README.md` for a usage example.
