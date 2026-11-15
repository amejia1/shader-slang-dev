use cmake::Config as CmakeConfig;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Upstream shader-slang repository.
const SLANG_REPO_URL: &str = "https://github.com/shader-slang/slang.git";

/// Environment variable that switches the artifact provenance: when set to
/// exactly `"true"` (case-sensitive), prebuilt shader-slang release binaries
/// are downloaded from GitHub instead of built from source.
const DOWNLOAD_ENV_VAR: &str = "SHADER_SLANG_SRC_DOWNLOAD_BINARIES";

/// Programs built by slang; `slangc` is the compiler most consumers invoke;
/// the rest are convenience. The install covers all of them except
/// `slang-replay` (not marked `INSTALL` by slang), which is copied from the
/// build tree.
const SLANG_BINARIES: &[&str] = &["slangc", "slangd", "slangi", "slang", "slang-replay"];

/// Plugins `slangc` dloads at runtime (glslang for SPIR-V via GLSL,
/// glsl-module for GLSL source); each must be present as a shared library
/// (a DLL on Windows) among the installed artifacts.
const SLANG_PLUGINS: &[&str] = &["slang-glslang", "slang-glsl-module"];

/// Library file extensions copied into the artifact `lib/` directory.
const LIBRARY_EXTENSIONS: &[&str] = &["a", "lib", "so", "dylib", "dll"];

/// Directory name under `OUT_DIR` holding the installed toolchain (the
/// artifact root). Its layout matches the shader-slang release tarballs:
/// `bin/`, `lib/`, `include/`, `share/` plus the license and documentation
/// files.
const ARTIFACT_ROOT_DIR: &str = "slang-artifact";

/// Content of the marker file (`.shader-slang-layout`) written into the
/// artifact root once a complete artifact set has been produced. Bump it
/// whenever the expected artifact layout changes, so artifacts installed by
/// an older revision of this script fail the completeness guard and are
/// re-installed instead of being reused.
const LAYOUT_MARKER: &str = "1";

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set"));
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"));

    // The slang release this crate builds comes from its `+<version>` build
    // metadata (e.g. `1.0.0+2026.21` -> `2026.21`, tag `v2026.21`).
    let slang_tag = format!("v{}", slang_version());

    let build_dir = out_dir.join("build");
    let artifact_root = out_dir.join(ARTIFACT_ROOT_DIR);
    let bin_dir = artifact_root.join("bin");
    let lib_dir = artifact_root.join("lib");

    // When SHADER_SLANG_SRC_DOWNLOAD_BINARIES is set to exactly "true"
    // (case-sensitive), download the prebuilt shader-slang release binaries
    // from GitHub instead of building them from source.
    let download_binaries = env::var(DOWNLOAD_ENV_VAR).is_ok_and(|value| value == "true");
    let source = if download_binaries {
        "download"
    } else {
        "source"
    };

    // Artifacts installed in a previous run belong to the provenance that
    // run used. When the provenance changes (the environment variable was
    // set or unset), remove the stale artifact root so the set provided
    // below always matches the current provenance.
    let provenance_file = out_dir.join(".slang-artifact-source");
    let same_provenance = fs::read_to_string(&provenance_file)
        .map(|previous| previous.trim() == source)
        .unwrap_or(false);
    if !same_provenance && artifact_root.exists() {
        fs::remove_dir_all(&artifact_root).unwrap_or_else(|e| {
            panic!(
                "failed to remove stale artifact root {}: {e}",
                artifact_root.display()
            )
        });
    }

    // Producing the slang toolchain is expensive, so skip it when the
    // artifacts already exist. Cargo already re-runs this script only when
    // needed, but this guard also makes repeated runs cheap.
    if !artifacts_complete(&artifact_root, &bin_dir, &lib_dir) {
        if download_binaries {
            download_slang(&out_dir, &artifact_root, &bin_dir, &lib_dir);
        } else {
            let slang_src = resolve_slang_source(&manifest_dir, &out_dir, &slang_tag);
            build_slang(&slang_src, &build_dir, &artifact_root, &bin_dir, &lib_dir);
        }
    }
    fs::write(&provenance_file, source)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", provenance_file.display()));

    // Expose the artifact root to the library so it can locate the binaries
    // and libraries it installed.
    println!(
        "cargo::rustc-env=SHADER_SLANG_ARTIFACT_ROOT={}",
        artifact_root.display()
    );
    println!("cargo::rustc-env=SHADER_SLANG_VERSION={}", slang_version());
    println!("cargo::rustc-env=SHADER_SLANG_SOURCE={}", source);
    // Re-run if the vendored submodule or this script changes.
    println!("cargo::rerun-if-changed=slang");
    println!("cargo::rerun-if-changed=build.rs");
    // Re-run when the download/build provenance switches.
    println!("cargo::rerun-if-env-changed=SHADER_SLANG_SRC_DOWNLOAD_BINARIES");
}

/// Whether the artifact set installed under `artifact_root` is complete
/// enough to skip the build: the layout marker (which invalidates artifacts
/// produced by an older revision of this script), the `slangc` executable,
/// the headers, every runtime plugin library, and the standard-module tree
/// must all be present.
fn artifacts_complete(artifact_root: &Path, bin_dir: &Path, lib_dir: &Path) -> bool {
    if !artifact_root.is_dir() {
        return false;
    }
    let marker_ok = fs::read_to_string(artifact_root.join(".shader-slang-layout"))
        .is_ok_and(|contents| contents.trim() == LAYOUT_MARKER);
    if !marker_ok {
        return false;
    }
    if !bin_dir.join(exe("slangc")).is_file() {
        return false;
    }
    if !artifact_root.join("include").is_dir() {
        return false;
    }
    // The standard-module tree is installed into `lib/`; source builds keep
    // an additional copy in `bin/` for the statically linked slangc.
    let module_dir_name = format!("slang-standard-module-{}", slang_version());
    if !lib_dir.join(&module_dir_name).is_dir() && !bin_dir.join(&module_dir_name).is_dir() {
        return false;
    }
    let plugin_dir = if cfg!(windows) { bin_dir } else { lib_dir };
    let exts: &[&str] = if cfg!(windows) {
        &["dll"]
    } else {
        &["so", "dylib"]
    };
    for plugin in SLANG_PLUGINS {
        if !has_plugin_shared(plugin_dir, plugin, exts) {
            return false;
        }
    }
    true
}

/// The slang version this crate builds, taken from the `+<version>` build
/// metadata of the crate's own version (`1.0.0+2026.21` -> `2026.21`).
fn slang_version() -> &'static str {
    let full = env!("CARGO_PKG_VERSION");
    match full.split_once('+') {
        Some((_, version)) => version,
        None => panic!(
            "shader-slang-src version `{}` must encode the slang version as \
             `+<version>` build metadata (e.g. `1.0.0+2026.21`)",
            full
        ),
    }
}

/// Append the platform executable suffix when needed.
fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

/// Return the directory containing the slang source tree to build.
///
/// The preferred source is the git submodule vendored at `slang/`. When it is
/// not present (a crates.io tarball or a cargo git dependency, neither of
/// which carries submodule contents), the pinned release tag is cloned into
/// the build output directory instead.
fn resolve_slang_source(manifest_dir: &Path, out_dir: &Path, tag: &str) -> PathBuf {
    let submodule = manifest_dir.join("slang");
    if submodule.join("CMakeLists.txt").is_file() {
        init_slang_submodules(&submodule);
        return submodule;
    }

    let clone_dir = out_dir.join("slang-src");
    clone_slang(tag, &clone_dir);
    clone_dir
}

/// Initialize the submodules slang itself vendors under `external/` (glslang,
/// spirv-tools, ...). Idempotent and cheap when already initialized.
fn init_slang_submodules(slang_dir: &Path) {
    let status = Command::new("git")
        .args([
            "submodule",
            "update",
            "--init",
            "--recursive",
            "--depth",
            "1",
        ])
        .current_dir(slang_dir)
        .status()
        .expect("failed to run `git submodule update`");
    if !status.success() {
        panic!(
            "failed to initialize slang submodules in {}",
            slang_dir.display()
        );
    }
}

/// Clone the slang release tag (shallow, with its own submodules) into `dir`,
/// reusing a previous clone when present.
fn clone_slang(tag: &str, dir: &Path) {
    if dir.join("CMakeLists.txt").is_file() {
        return;
    }
    if dir.exists() {
        fs::remove_dir_all(dir).expect("failed to remove incomplete slang clone");
    }
    let status = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            "--branch",
            tag,
            "--recurse-submodules",
            "--shallow-submodules",
            SLANG_REPO_URL,
        ])
        .arg(dir)
        .status()
        .expect("failed to run `git clone`");
    if !status.success() {
        panic!("failed to clone {SLANG_REPO_URL} at tag {tag}");
    }
    if !dir.join("CMakeLists.txt").is_file() {
        panic!("slang clone at {} is missing CMakeLists.txt", dir.display());
    }
}

/// Configure and build slang, then install it into `artifact_root` with
/// `cmake --install` so the installed layout matches the shader-slang
/// release tarballs.
fn build_slang(
    slang_src: &Path,
    build_dir: &Path,
    artifact_root: &Path,
    bin_dir: &Path,
    lib_dir: &Path,
) {
    if Command::new("git").arg("--version").output().is_err() {
        panic!("required tool not found on PATH: git");
    }

    // Configure and build through the `cmake` crate, which drives the system
    // `cmake` binary (failing clearly if it is not installed) and uses
    // `<out_dir>/build` as its build directory, i.e. exactly `build_dir`
    // above. The flags match candle's build-install-slang.sh: static
    // compiler library, lean build (no tests/examples/rhi/gfx). The glslang
    // backend plugin is explicitly requested because `slangc` dlopens it at
    // runtime; without it the toolchain is incomplete.
    let mut cmake_config = CmakeConfig::new(slang_src);
    cmake_config
        .out_dir(build_dir.parent().expect("build dir has no parent"))
        // Pin the build type to Release regardless of Rust's own profile
        // (without this, the crate would build a Debug slang when the script
        // runs under `cargo build`/`cargo test` in dev profile).
        .profile("Release")
        .define("SLANG_LIB_TYPE", "STATIC")
        // The release pipeline only ships the `slang-llvm` backend plugin
        // when it builds LLVM from a system toolchain; the default
        // FETCH_BINARY flavor downloads the release archive for it and its
        // copy step is not part of the default build, which breaks
        // `cmake --install`. Disable it: the source build does not provide
        // the `slang-llvm` plugin (the release tarballs do).
        .define("SLANG_SLANG_LLVM_FLAVOR", "DISABLE")
        .define("SLANG_ENABLE_TESTS", "OFF")
        .define("SLANG_ENABLE_EXAMPLES", "OFF")
        .define("SLANG_ENABLE_SLANG_RHI", "OFF")
        .define("SLANG_ENABLE_GFX", "OFF")
        .define("SLANG_ENABLE_SLANG_GLSLANG", "ON");
    // Pin the version from this crate's own metadata. Slang otherwise
    // derives it from git tags, which shallow checkouts (as used by CI)
    // do not carry; without a tag it falls back to 0.0.0 and the
    // versioned plugin file names would not match what the crate
    // promises.
    cmake_config
        .define("SLANG_VERSION_NUMERIC", slang_version())
        .define("SLANG_VERSION_FULL", slang_version());
    // Build the default target (everything) instead of the crate's default
    // `install` target: the toolchain is installed into `artifact_root`
    // below via `cmake --install`.
    cmake_config.no_build_target(true);

    run_cmake_build(&mut cmake_config);

    // Install the built toolchain into the artifact root, stripping the
    // installed binaries. This is the same step the shader-slang release
    // pipeline runs before packing its release tarballs, so the source
    // build and the download provide the same layout.
    run_cmake_install(build_dir, artifact_root);

    // The install covers only the targets slang marks `INSTALL`; copy in
    // the remaining build outputs the artifact has always provided.
    supplement_from_build_tree(build_dir, artifact_root, bin_dir, lib_dir);

    // Normalize the header location to the release layout (`include/`).
    collect_headers(artifact_root);

    // Make the loose standard modules resolvable by the statically linked
    // slangc.
    copy_standard_modules_to_bin(artifact_root, bin_dir, lib_dir);

    // Fail loudly here instead of at test time if the install lacks a
    // runtime plugin.
    let plugin_dir = if cfg!(windows) { bin_dir } else { lib_dir };
    let exts: &[&str] = if cfg!(windows) {
        &["dll"]
    } else {
        &["so", "dylib"]
    };
    for plugin in SLANG_PLUGINS {
        if !has_plugin_shared(plugin_dir, plugin, exts) {
            panic!(
                "slang install did not produce the `{plugin}` plugin library in {}",
                plugin_dir.display()
            );
        }
    }

    // Show exactly what this build installed (see `list_artifacts`).
    list_artifacts(artifact_root);

    // Smoke check: the installed slangc must run.
    let status = Command::new(bin_dir.join(exe("slangc")))
        .arg("-version")
        .status()
        .expect("failed to run installed slangc");
    if !status.success() {
        panic!("installed slangc failed to run (`slangc -version` exited non-zero)");
    }

    // Mark the artifact set complete only once every step above succeeded.
    write_layout_marker(artifact_root);
}

/// Write the layout marker file into `artifact_root` (see `LAYOUT_MARKER`).
fn write_layout_marker(artifact_root: &Path) {
    fs::write(artifact_root.join(".shader-slang-layout"), LAYOUT_MARKER).unwrap_or_else(|e| {
        panic!(
            "failed to write layout marker in {}: {e}",
            artifact_root.display()
        )
    });
}

/// Run the `cmake` crate's configure+build, retrying once on failure.
///
/// The dxc sub-build (an old LLVM snapshot with its own makefiles) has
/// parallel-build races observed on macOS (duplicate ar invocations,
/// missing tablegen headers); an incremental retry resumes where the
/// failed run left off and recovers from them. The `cmake` crate panics on
/// a failed command, so the first attempt is run inside `catch_unwind` to
/// allow the retry.
fn run_cmake_build(cmake_config: &mut CmakeConfig) {
    let first_attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cmake_config.build();
    }));
    if first_attempt.is_err() {
        cmake_config.build();
    }
}

/// Install the built toolchain: `cmake --install <build_dir> --prefix
/// <artifact_root> --strip`.
fn run_cmake_install(build_dir: &Path, artifact_root: &Path) {
    let output = Command::new("cmake")
        .arg("--install")
        .arg(build_dir)
        .arg("--prefix")
        .arg(artifact_root)
        .arg("--strip")
        .output()
        .unwrap_or_else(|e| panic!("failed to run `cmake --install`: {e}"));
    if !output.status.success() {
        panic!(
            "cmake --install {} --prefix {} --strip failed:\n{}",
            build_dir.display(),
            artifact_root.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

/// `cmake --install` installs only the targets slang marks with `INSTALL`.
/// Copy the remaining build outputs the artifact has always provided into
/// the artifact root: the static libraries the compiler's static archive
/// depends on (`core`, `compiler-core`, ...), which consumers link against
/// via `lib_dir()`, and the `slang-replay` tool.
fn supplement_from_build_tree(
    build_dir: &Path,
    artifact_root: &Path,
    bin_dir: &Path,
    lib_dir: &Path,
) {
    let build_bin_dir = find_bin_dir(build_dir).unwrap_or_else(|| {
        panic!(
            "slang build did not produce a `slangc` binary under {}",
            build_dir.display()
        )
    });

    // The executables the install did not produce.
    for name in SLANG_BINARIES {
        let dest = bin_dir.join(exe(name));
        if !dest.is_file() {
            if let Some(src) = find_file(&build_bin_dir, &exe(name)) {
                copy(&src, &dest);
            }
        }
    }

    // The libraries the install did not produce, from the build `lib/`
    // directory (the sibling of the build `bin/` directory).
    if let Some(parent) = build_bin_dir.parent() {
        let build_lib_dir = parent.join("lib");
        if build_lib_dir.is_dir() {
            let Ok(entries) = fs::read_dir(&build_lib_dir) else {
                panic!("failed to read build lib dir {}", build_lib_dir.display());
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() && has_ext(&path, LIBRARY_EXTENSIONS) {
                    let dest = lib_dir.join(path.file_name().unwrap());
                    if !dest.exists() {
                        copy(&path, &dest);
                    }
                }
            }
        }
    }
    // The install must have produced the core artifacts; anything missing
    // here is a broken layout, not a missing build output.
    if !bin_dir.join(exe("slangc")).is_file() {
        panic!(
            "artifact {} has no `slangc` after supplementing from {}",
            artifact_root.display(),
            build_dir.display()
        );
    }
}

/// Find the CMake output directory holding the `slangc` executable.
///
/// CMake layouts vary by generator: single-config generators emit into
/// `<build>/Release/bin` when slang overrides the output directories, while
/// multi-config generators (Visual Studio, Ninja Multi-Config) use
/// `<build>/<config>/bin`. Do not search the whole tree for `slangc`:
/// CMake also leaves stub files named `slangc` next to per-target build
/// rules, and picking one of those would make the sibling `lib/` lookup
/// below point at an empty directory.
fn find_bin_dir(build_dir: &Path) -> Option<PathBuf> {
    let candidates = [
        build_dir.join("Release").join("bin"),
        build_dir.join("bin"),
        build_dir.join("Release"),
        build_dir.to_path_buf(),
    ];
    for candidate in candidates {
        if candidate.join(exe("slangc")).is_file() {
            return Some(candidate);
        }
    }

    // Last resort: the first executable named `slangc` anywhere in the tree.
    find_file(build_dir, &exe("slangc")).and_then(|path| path.parent().map(Path::to_path_buf))
}

/// The pinned slang release installs the public headers at the install
/// prefix root, while the release tarballs keep them under `include/`. Move
/// the root-level headers into `include/` so both provenances expose the
/// same layout (and [`crate::include_dir`] resolves identically).
fn collect_headers(artifact_root: &Path) {
    let include_dir = artifact_root.join("include");
    let entries = fs::read_dir(artifact_root)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", artifact_root.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("h") {
            fs::create_dir_all(&include_dir).expect("failed to create include dir");
            fs::rename(&path, include_dir.join(path.file_name().unwrap()))
                .unwrap_or_else(|e| panic!("failed to move {} into include/: {e}", path.display()));
        }
    }
}

/// Copy the installed loose standard-module tree from `lib/` into `bin/`.
///
/// The release layout keeps the tree under `lib/`, where the runtime finds
/// it when the compiler is a shared library (the downloaded release). A
/// source build links the compiler statically into slangc, and the runtime
/// then resolves the tree relative to the loaded compiler code -- i.e. the
/// slangc executable -- so the tree must sit in `bin/` as well for
/// `import slang.<module>` to work.
fn copy_standard_modules_to_bin(artifact_root: &Path, bin_dir: &Path, lib_dir: &Path) {
    let module_dir_name = format!("slang-standard-module-{}", slang_version());
    let src = lib_dir.join(&module_dir_name);
    let dst = bin_dir.join(&module_dir_name);
    if src.is_dir() && !dst.is_dir() {
        copy_dir_tree(&src, &dst);
    }
    // The tree must exist after the copy: every slangc build produces it.
    if !dst.is_dir() {
        panic!(
            "slang install did not produce a `{module_dir_name}` directory under {}",
            artifact_root.display()
        );
    }
}

/// Download the prebuilt shader-slang release for the target platform from
/// the GitHub release corresponding to this crate's `+<version>` metadata
/// (e.g. `1.0.0+2026.21` -> tag `v2026.21`), and unpack it into
/// `artifact_root`. The release tarball's root entries are exactly the
/// artifact layout (`bin/`, `lib/`, `include/`, `share/`, ...), so the
/// download provides the same layout the source install does.
fn download_slang(out_dir: &Path, artifact_root: &Path, bin_dir: &Path, lib_dir: &Path) {
    let (os, arch) = download_target();
    let file_name = format!("slang-{}-{}-{}.tar.gz", slang_version(), os, arch);
    let url = format!(
        "https://github.com/shader-slang/slang/releases/download/v{}/{file_name}",
        slang_version()
    );

    let tarball = out_dir.join(&file_name);
    let response = ureq::get(&url)
        .header("user-agent", "shader-slang-src")
        .call()
        .unwrap_or_else(|e| panic!("failed to download {url}: {e}"));
    let mut file = fs::File::create(&tarball)
        .unwrap_or_else(|e| panic!("failed to create {}: {e}", tarball.display()));
    std::io::copy(&mut response.into_body().into_reader(), &mut file)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", tarball.display()));

    // Unpack the release tarball directly into `artifact_root`: its root
    // entries are exactly the `bin/`, `lib/`, `include/`, `share/` artifact
    // directories plus the license and documentation files. The `tar` crate
    // recreates the symlinks (e.g. `libslang-compiler.so` ->
    // `libslang-compiler.so.0.2026.21`) that a file copy would not.
    let file = fs::File::open(&tarball)
        .unwrap_or_else(|e| panic!("failed to open {}: {e}", tarball.display()));
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let entries = archive
        .entries()
        .unwrap_or_else(|e| panic!("failed to read entries of {file_name}: {e}"));
    for entry in entries {
        let mut entry = entry.unwrap_or_else(|e| panic!("failed to read a {file_name} entry: {e}"));
        entry
            .unpack_in(artifact_root)
            .unwrap_or_else(|e| panic!("failed to unpack {file_name}: {e}"));
    }
    for (dir, label) in [
        (bin_dir, "bin"),
        (lib_dir, "lib"),
        (&artifact_root.join("include"), "include"),
        (&artifact_root.join("share"), "share"),
    ] {
        if !dir.is_dir() {
            panic!("downloaded {file_name} has no {label}/ directory");
        }
    }

    // Fail loudly here instead of at test time if the release lacks a
    // runtime plugin.
    let plugin_dir = if os == "windows" { bin_dir } else { lib_dir };
    let exts: &[&str] = if os == "windows" {
        &["dll"]
    } else {
        &["so", "dylib"]
    };
    for plugin in SLANG_PLUGINS {
        if !has_plugin_shared(plugin_dir, plugin, exts) {
            panic!(
                "downloaded {file_name} does not contain the `{plugin}` plugin in {}",
                plugin_dir.display()
            );
        }
    }

    // Show exactly what this download installed (see `list_artifacts`).
    list_artifacts(artifact_root);

    // Smoke check: the downloaded slangc must run.
    let status = Command::new(bin_dir.join(exe("slangc")))
        .arg("-version")
        .status()
        .expect("failed to run downloaded slangc");
    if !status.success() {
        panic!("downloaded slangc failed to run (`slangc -version` exited non-zero)");
    }

    // Mark the artifact set complete only once every step above succeeded.
    write_layout_marker(artifact_root);
}

/// The target operating system and CPU architecture this build produces
/// artifacts for, as shipped in the shader-slang release tarballs. Read from
/// the `CARGO_CFG_TARGET_*` variables cargo sets for build scripts. Errors
/// (rather than silently falling back to a source build) for platforms or
/// architectures the release does not cover.
fn download_target() -> (&'static str, &'static str) {
    let os = env::var("CARGO_CFG_TARGET_OS")
        .unwrap_or_else(|_| panic!("CARGO_CFG_TARGET_OS is not set"));
    let arch = env::var("CARGO_CFG_TARGET_ARCH")
        .unwrap_or_else(|_| panic!("CARGO_CFG_TARGET_ARCH is not set"));
    let os = match os.as_str() {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        other => panic!(
            "prebuilt shader-slang binaries are not available for the target \
             operating system `{other}`; supported: linux, macos, windows"
        ),
    };
    let arch = match arch.as_str() {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => panic!(
            "prebuilt shader-slang binaries are not available for the target \
             CPU architecture `{other}`; supported: x86_64, aarch64"
        ),
    };
    (os, arch)
}

/// Whether `dir` contains the shared plugin `needle`: a file whose name
/// contains `needle` (non-Windows names carry the `lib` prefix) and whose
/// extension is one of `exts` (the platform's shared-library extensions).
/// Import libraries (.lib on Windows) never count: they are link inputs, not
/// the runtime-loaded plugin.
fn has_plugin_shared(dir: &Path, needle: &str, exts: &[&str]) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.contains(needle) && has_ext(&path, exts) {
            return true;
        }
    }
    false
}

fn has_ext(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.contains(&e))
}

/// Recursively collect every file under `dir`, sorted.
fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(entries) = fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Print the file list of the installed artifact root.
///
/// The `share/` tree is large (it carries the full documentation), so it is
/// listed one level only. Cargo records build-script stdout in
/// `target/<profile>/build/shader-slang-src-*/output`, so a later
/// "missing file" failure can be compared against exactly what this build
/// installed.
fn list_artifacts(artifact_root: &Path) {
    println!(
        "shader-slang-src: installed files under {}:",
        artifact_root.display()
    );
    for dir in ["bin", "lib", "include"] {
        for path in collect_files(&artifact_root.join(dir)) {
            println!("  {}", path.display());
        }
    }
    // Top level: the license/documentation files, and the `share/` children.
    let Ok(entries) = fs::read_dir(artifact_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            println!("  {}", path.display());
        } else if path.file_name().and_then(|n| n.to_str()) == Some("share") {
            let Ok(children) = fs::read_dir(&path) else {
                continue;
            };
            for child in children.flatten() {
                println!("  {}", child.path().display());
            }
        }
    }
}

/// Recursively search `dir` for a file with the exact `name`.
fn find_file(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Some(path);
        }
    }
    None
}

/// Recursively copy the tree rooted at `src` into `dst` (existing files are
/// overwritten).
fn copy_dir_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("failed to create directory");
    let entries = fs::read_dir(src).expect("failed to read directory");
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            copy_dir_tree(&path, &dst.join(entry.file_name()));
        } else {
            copy(&path, &dst.join(path.file_name().unwrap()));
        }
    }
}

fn copy(src: &Path, dst: &Path) {
    fs::copy(src, dst)
        .unwrap_or_else(|e| panic!("failed to copy {} to {}: {e}", src.display(), dst.display()));
}
