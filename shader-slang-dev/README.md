# shader-slang-dev

Ergonomic `PathBuf` accessors for the shader-slang toolchain installed by
[`shader-slang-src`](https://crates.io/crates/shader-slang-src):

- `slangc()` — the compiler binary; `binary(name)` — any other binary under `bin_dir()`.
- `artifact_root()` — the install prefix the artifacts live in.
- `bin_dir()` — the directory holding the binaries.
- `lib_dir()` — the directory holding the static/shared libraries (for `-L` or `cargo::rustc-link-search=[KIND=]PATH`).
- `include_dir()` — the directory holding the headers (for `-I`).
- `share_dir()` — the directory holding the installed share data (documentation under `doc/slang/`, agent skills under `slang/agent-skills/`).
- `SLANG_VERSION` — the shader-slang release in use (`2026.21`); `SOURCE` — how the artifacts were obtained (`"source"` or `"download"`); `SHADER_SLANG_SOURCE_URL` / `SHADER_SLANG_DIST_URL` — the URLs the build used (`SHADER_SLANG_DIST_URL` is `""` when the variable was unset).

The libraries available depend on how the artifacts were obtained:
`SOURCE == "source"` builds install the compiler and runtime as static
libraries (`slang-compiler`, `slang-rt`), while `SOURCE == "download"`
(prebuilt distribution tarball provided via `SHADER_SLANG_DIST_URL`) ship
shared libraries instead.

See the workspace `README.md` for a usage example.
