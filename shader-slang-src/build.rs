use cmake::Config as CmakeConfig;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use url::Url;

/// Upstream shader-slang repository. Used to fetch the pinned `external/`
/// submodule checkouts when the source tarball does not carry them (GitHub
/// source archives never do).
const SLANG_REPO_URL: &str = "https://github.com/shader-slang/slang.git";

/// The shader-slang release this crate builds (tag `v2026.21`).
const SLANG_VERSION: &str = "2026.21";

/// Environment variable holding the URL of the shader-slang source tarball.
/// Accepts a valid `https` or `file` URL in the WHATWG URL standard; unset
/// or empty means [`DEFAULT_SOURCE_URL`].
const SOURCE_URL_ENV_VAR: &str = "SHADER_SLANG_SOURCE_URL";

/// Default source tarball (used when [`SOURCE_URL_ENV_VAR`] is unset or
/// empty): the GitHub source archive of the `v2026.21` tag.
const DEFAULT_SOURCE_URL: &str =
    "https://github.com/shader-slang/slang/archive/refs/tags/v2026.21.tar.gz";

/// Environment variable holding the URL of a prebuilt shader-slang
/// distribution tarball. Only checked for being set to a non-empty string;
/// the value must then be a valid `https` or `file` URL in the WHATWG URL
/// standard. When set, the tarball is fetched and extracted under `OUT_DIR`
/// and the extracted directory serves as the installed toolchain, skipping
/// the source build.
const DIST_URL_ENV_VAR: &str = "SHADER_SLANG_DIST_URL";

/// Plugins `slangc` dloads at runtime (glslang for SPIR-V via GLSL,
/// glsl-module for GLSL source); each must be present as a shared library
/// (a DLL on Windows) among the installed artifacts.
const SLANG_PLUGINS: &[&str] = &["slang-glslang", "slang-glsl-module"];

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
const LAYOUT_MARKER: &str = "3";

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set"));

    let source_url = source_url();
    let dist_url = dist_url();
    let source = if dist_url.is_some() {
        "download"
    } else {
        "source"
    };

    let build_dir = out_dir.join("build");
    let artifact_root = out_dir.join(ARTIFACT_ROOT_DIR);
    let bin_dir = artifact_root.join("bin");
    let lib_dir = artifact_root.join("lib");

    // Make the provenance visible in the build output (shown every time this
    // script runs, i.e. whenever the provenance or the inputs change).
    println!(
        "cargo:warning=shader-slang-src: installing slang {SLANG_VERSION} from {}",
        match &dist_url {
            Some(url) => format!("the distribution tarball at {url}"),
            None => format!("the source tarball at {source_url}"),
        }
    );

    // Artifacts installed in a previous run belong to the provenance that
    // run used. When the provenance changes (SHADER_SLANG_DIST_URL was set
    // or unset), remove the stale artifact root so the set provided below
    // always matches the current provenance.
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
        // A fresh build or download starts from an empty artifact root, so
        // the result contains exactly the files the `cmake --install` run
        // (or the distribution tarball) provides.
        if artifact_root.exists() {
            fs::remove_dir_all(&artifact_root).unwrap_or_else(|e| {
                panic!(
                    "failed to remove stale artifact root {}: {e}",
                    artifact_root.display()
                )
            });
        }
        if let Some(dist_url) = &dist_url {
            install_dist(&out_dir, &artifact_root, &bin_dir, &lib_dir, dist_url);
        } else {
            let slang_src = fetch_slang_source(&out_dir, &source_url);
            build_slang(&slang_src, &build_dir, &artifact_root, &bin_dir, &lib_dir);
        }
    }
    fs::write(&provenance_file, source)
        .unwrap_or_else(|e| panic!("failed to write {}: {e}", provenance_file.display()));

    // Expose the artifact root and the URL configuration to the library.
    println!(
        "cargo::rustc-env=SHADER_SLANG_ARTIFACT_ROOT={}",
        artifact_root.display()
    );
    println!(
        "cargo::rustc-env=SHADER_SLANG_SOURCE_URL={}",
        source_url.as_str()
    );
    println!(
        "cargo::rustc-env=SHADER_SLANG_DIST_URL={}",
        dist_url.as_ref().map_or("", |url| url.as_str())
    );
    // Re-run if this script changes.
    println!("cargo::rerun-if-changed=build.rs");
    // Re-run when the source or distribution URL changes.
    println!("cargo::rerun-if-env-changed={SOURCE_URL_ENV_VAR}");
    println!("cargo::rerun-if-env-changed={DIST_URL_ENV_VAR}");
}

/// Read `SHADER_SLANG_SOURCE_URL`, falling back to [`DEFAULT_SOURCE_URL`]
/// when the variable is unset or empty, and parse it as a WHATWG URL
/// restricted to the `https` and `file` schemes.
fn source_url() -> Url {
    let raw = env::var(SOURCE_URL_ENV_VAR).unwrap_or_default();
    let value = if raw.is_empty() {
        DEFAULT_SOURCE_URL
    } else {
        raw.as_str()
    };
    parse_url(SOURCE_URL_ENV_VAR, value)
}

/// Read `SHADER_SLANG_DIST_URL`: `None` when the variable is unset or set to
/// the empty string, otherwise the parsed URL (restricted to the `https`
/// and `file` schemes).
fn dist_url() -> Option<Url> {
    match env::var(DIST_URL_ENV_VAR) {
        Ok(value) if !value.is_empty() => Some(parse_url(DIST_URL_ENV_VAR, &value)),
        _ => None,
    }
}

/// Parse `value` as a WHATWG URL and require the `https` or `file` scheme.
fn parse_url(env_var: &str, value: &str) -> Url {
    let url =
        Url::parse(value).unwrap_or_else(|e| panic!("{env_var} is not a valid URL ({value}): {e}"));
    match url.scheme() {
        "https" | "file" => url,
        scheme => {
            panic!("{env_var} must use the `https` or `file` scheme, got `{scheme}`: {value}")
        }
    }
}

/// Fetch the tarball at `url` into `dir` — downloading `https` URLs and
/// copying `file` URLs — and return the local tarball path.
fn fetch_tarball(url: &Url, dir: &Path, env_var: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap_or_else(|e| panic!("failed to create {}: {e}", dir.display()));
    let name = url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
        .filter(|name| !name.is_empty())
        .unwrap_or("archive.tar.gz");
    let tarball = dir.join(name);
    match url.scheme() {
        "https" => {
            let response = ureq::get(url.as_str())
                .header("user-agent", "shader-slang-src")
                .call()
                .unwrap_or_else(|e| panic!("failed to download {url} ({env_var}): {e}"));
            let mut file = fs::File::create(&tarball)
                .unwrap_or_else(|e| panic!("failed to create {}: {e}", tarball.display()));
            std::io::copy(&mut response.into_body().into_reader(), &mut file)
                .unwrap_or_else(|e| panic!("failed to write {}: {e}", tarball.display()));
        }
        "file" => {
            let path = url
                .to_file_path()
                .unwrap_or_else(|_| panic!("{env_var} is not a valid file URL: {url}"));
            fs::copy(&path, &tarball).unwrap_or_else(|e| {
                panic!(
                    "failed to copy {} to {} ({env_var}): {e}",
                    path.display(),
                    tarball.display()
                )
            });
        }
        scheme => unreachable!("scheme `{scheme}` was rejected by parse_url"),
    }
    tarball
}

/// Extract the gzipped tarball at `tarball` into `dest` (recreated when it
/// already exists) and return the directory holding the archive's root
/// entries: the archive's single top-level directory when it has exactly
/// one, `dest` itself otherwise.
///
/// `Archive::unpack` is used rather than a per-entry `unpack_in` loop: it
/// creates the destination directory first (a `unpack_in` call on a
/// non-existent destination canonicalizes it and fails) and unpacks the
/// directory entries after the files, so restrictive directory permissions
/// cannot break the extraction.
fn extract_tarball(tarball: &Path, dest: &Path) -> PathBuf {
    if dest.exists() {
        fs::remove_dir_all(dest)
            .unwrap_or_else(|e| panic!("failed to remove {}: {e}", dest.display()));
    }
    let file = fs::File::open(tarball)
        .unwrap_or_else(|e| panic!("failed to open {}: {e}", tarball.display()));
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    archive
        .unpack(dest)
        .unwrap_or_else(|e| panic!("failed to unpack {}: {e}", tarball.display()));
    let entries: Vec<_> = fs::read_dir(dest)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", dest.display()))
        .flatten()
        .collect();
    if entries.len() == 1 && entries[0].path().is_dir() {
        entries[0].path()
    } else {
        dest.to_path_buf()
    }
}

/// Fetch the shader-slang source tarball at `source_url` into `out_dir`,
/// extract it under `out_dir` and return the extracted source tree (the
/// directory containing `CMakeLists.txt`).
fn fetch_slang_source(out_dir: &Path, source_url: &Url) -> PathBuf {
    let tarball = fetch_tarball(source_url, &out_dir.join("source"), SOURCE_URL_ENV_VAR);
    let root = extract_tarball(&tarball, &out_dir.join("source-extracted"));
    if !root.join("CMakeLists.txt").is_file() {
        panic!(
            "the source tarball at {source_url} has no CMakeLists.txt under {}",
            root.display()
        );
    }
    init_slang_submodules(&root);
    root
}

/// Populate the `external/` submodule checkouts of an extracted source
/// tarball. GitHub source archives do not carry submodule contents, so the
/// tarball alone is not enough to build: reconstruct just enough git
/// metadata to resolve the pinned gitlinks — `git init`, fetch the release
/// tag from the upstream repository and load its tree into the index —
/// then run `git submodule update --init --recursive`. Skipped when the
/// extracted tree already carries the submodule contents (e.g. a
/// pre-packed tarball).
fn init_slang_submodules(slang_src: &Path) {
    if submodules_present(slang_src) {
        return;
    }
    let tag = format!("v{SLANG_VERSION}");
    run_git(slang_src, &["init", "-q"]);
    run_git(slang_src, &["remote", "add", "origin", SLANG_REPO_URL]);
    run_git(slang_src, &["fetch", "-q", "--depth", "1", "origin", &tag]);
    // Load the fetched tree into the index (the worktree is left untouched)
    // so the pinned gitlink SHAs are visible to `git submodule update`.
    run_git(slang_src, &["reset", "-q", "FETCH_HEAD"]);
    run_git(
        slang_src,
        &[
            "submodule",
            "update",
            "--init",
            "--recursive",
            "--depth",
            "1",
        ],
    );
}

/// Whether every submodule path listed in `.gitmodules` is already a
/// non-empty directory (i.e. the source already carries the checkouts).
/// A missing `.gitmodules` means there is nothing to fetch.
fn submodules_present(slang_src: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(slang_src.join(".gitmodules")) else {
        return true;
    };
    let mut paths = Vec::new();
    let mut in_submodule = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_submodule = line.starts_with("[submodule");
        } else if in_submodule && line.starts_with("path") {
            if let Some((_, value)) = line.split_once('=') {
                paths.push(value.trim().to_string());
            }
        }
    }
    !paths.is_empty()
        && paths.iter().all(|path| {
            fs::read_dir(slang_src.join(path))
                .map(|mut entries| entries.next().is_some())
                .unwrap_or(false)
        })
}

/// Run `git <args>` in `dir`, panicking on failure.
///
/// `core.longpaths=true` lets Git for Windows check out long file names: the
/// extracted source tree lives under the deep `$OUT_DIR` path, and some
/// slang submodules carry files whose full paths would exceed the default
/// Windows MAX_PATH limit. The option is a no-op on other platforms.
fn run_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-c")
        .arg("core.longpaths=true")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("failed to run `git` (required to fetch the slang submodules)");
    if !status.success() {
        panic!("`git {}` failed in {}", args.join(" "), dir.display());
    }
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
    let module_dir_name = format!("slang-standard-module-{SLANG_VERSION}");
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

/// Append the platform executable suffix when needed.
fn exe(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
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
    // Pin the version from this crate's pinned slang release. Slang
    // otherwise derives it from git tags, which the fetched tree (and
    // shallow checkouts) do not carry; without a tag it falls back to
    // 0.0.0 and the versioned plugin file names would not match what the
    // crate promises.
    cmake_config
        .define("SLANG_VERSION_NUMERIC", SLANG_VERSION)
        .define("SLANG_VERSION_FULL", SLANG_VERSION);
    // Build the default target (everything) instead of the crate's default
    // `install` target: the toolchain is installed into `artifact_root`
    // below via `cmake --install`.
    cmake_config.no_build_target(true);

    run_cmake_build(&mut cmake_config);

    // Install the built toolchain into the artifact root, stripping the
    // installed binaries. This is the same step the shader-slang release
    // pipeline runs before packing its release tarballs, so the source
    // build and the distribution tarball provide the same layout.
    run_cmake_install(build_dir, artifact_root);

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
    smoke_check_slangc(bin_dir);

    // Mark the artifact set complete only once every step above succeeded.
    write_layout_marker(artifact_root);
}

/// Fetch the prebuilt distribution tarball at `dist_url` into `out_dir`,
/// extract it under `out_dir` and move the extracted tree into the artifact
/// root, so the extracted directory serves as the location of the
/// shader-slang binaries exactly as if they had been built from source and
/// installed.
fn install_dist(
    out_dir: &Path,
    artifact_root: &Path,
    bin_dir: &Path,
    lib_dir: &Path,
    dist_url: &Url,
) {
    let tarball = fetch_tarball(dist_url, &out_dir.join("dist"), DIST_URL_ENV_VAR);
    let extracted = extract_tarball(&tarball, &out_dir.join("dist-extracted"));
    if artifact_root.exists() {
        fs::remove_dir_all(artifact_root).unwrap_or_else(|e| {
            panic!(
                "failed to remove stale artifact root {}: {e}",
                artifact_root.display()
            )
        });
    }
    fs::rename(&extracted, artifact_root).unwrap_or_else(|e| {
        panic!(
            "failed to move {} into {}: {e}",
            extracted.display(),
            artifact_root.display()
        )
    });

    // The distribution tarball's root entries must be exactly the artifact
    // layout (`bin/`, `lib/`, `include/`, `share/`, ...).
    for (dir, label) in [
        (bin_dir, "bin"),
        (lib_dir, "lib"),
        (&artifact_root.join("include"), "include"),
        (&artifact_root.join("share"), "share"),
    ] {
        if !dir.is_dir() {
            panic!("the distribution tarball at {dist_url} has no {label}/ directory");
        }
    }

    // Fail loudly here instead of at test time if the tarball lacks a
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
                "the distribution tarball at {dist_url} does not contain the `{plugin}` plugin in {}",
                plugin_dir.display()
            );
        }
    }

    // Show exactly what this download installed (see `list_artifacts`).
    list_artifacts(artifact_root);

    // Smoke check: the installed slangc must run.
    smoke_check_slangc(bin_dir);

    // Mark the artifact set complete only once every step above succeeded.
    write_layout_marker(artifact_root);
}

/// Smoke check: the installed `slangc` must run.
fn smoke_check_slangc(bin_dir: &Path) {
    let status = Command::new(bin_dir.join(exe("slangc")))
        .arg("-version")
        .status()
        .expect("failed to run installed slangc");
    if !status.success() {
        panic!("installed slangc failed to run (`slangc -version` exited non-zero)");
    }
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
/// it when the compiler is a shared library (a distribution tarball). A
/// source build links the compiler statically into slangc, and the runtime
/// then resolves the tree relative to the loaded compiler code -- i.e. the
/// slangc executable -- so the tree must sit in `bin/` as well for
/// `import slang.<module>` to work.
fn copy_standard_modules_to_bin(artifact_root: &Path, bin_dir: &Path, lib_dir: &Path) {
    let module_dir_name = format!("slang-standard-module-{SLANG_VERSION}");
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
