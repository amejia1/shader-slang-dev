use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Upstream shader-slang repository.
const SLANG_REPO_URL: &str = "https://github.com/shader-slang/slang.git";

/// Programs built by slang; each is copied when the build produced it.
/// `slangc` is the compiler most consumers invoke; the rest are convenience.
const SLANG_BINARIES: &[&str] = &["slangc", "slangd", "slangi", "slang", "slang-replay"];

/// Library file extensions copied into the artifact `lib/` directory.
const LIBRARY_EXTENSIONS: &[&str] = &["a", "lib", "so", "dylib", "dll"];

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set"));
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is not set"));

    // The slang release this crate builds comes from its `+<version>` build
    // metadata (e.g. `0.1.0+2026.21` -> `2026.21`, tag `v2026.21`).
    let slang_tag = format!("v{}", slang_version());

    let bin_dir = out_dir.join("bin");
    let lib_dir = out_dir.join("lib");
    let include_dir = out_dir.join("include");

    // Building slang from source is expensive, so skip it when the artifacts
    // already exist. Cargo already re-runs this script only when needed, but
    // this guard also makes repeated runs cheap.
    if !bin_dir.join(exe("slangc")).is_file() {
        let slang_src = resolve_slang_source(&manifest_dir, &out_dir, &slang_tag);
        build_slang(&slang_src, &out_dir, &bin_dir, &lib_dir, &include_dir);
    }

    // Expose the artifact root to the library so it can locate the binaries
    // and libraries it produced.
    println!(
        "cargo:rustc-env=SHADER_SLANG_ARTIFACT_ROOT={}",
        out_dir.display()
    );

    println!("cargo:rustc-env=SHADER_SLANG_VERSION={}", slang_version());
    // Re-run if the vendored submodule or this script changes.
    println!("cargo:rerun-if-changed=slang");
    println!("cargo:rerun-if-changed=build.rs");
}

/// The slang version this crate builds, taken from the `+<version>` build
/// metadata of the crate's own version (`0.1.0+2026.21` -> `2026.21`).
fn slang_version() -> &'static str {
    let full = env!("CARGO_PKG_VERSION");
    match full.split_once('+') {
        Some((_, version)) => version,
        None => panic!(
            "shader-slang-src version `{}` must encode the slang version as \
             `+<version>` build metadata (e.g. `0.1.0+2026.21`)",
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

/// Configure and build slang, then collect the binaries, libraries and headers
/// into `bin_dir`, `lib_dir` and `include_dir`.
fn build_slang(
    slang_src: &Path,
    out_dir: &Path,
    bin_dir: &Path,
    lib_dir: &Path,
    include_dir: &Path,
) {
    for tool in ["git", "cmake"] {
        if Command::new(tool).arg("--version").output().is_err() {
            panic!("required tool not found on PATH: {tool}");
        }
    }

    let build_dir = out_dir.join("build");
    let jobs = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .to_string();

    // Configure with the same flags as candle's build-install-slang.sh:
    // static compiler library, lean build (no tests/examples/rhi/gfx). The
    // glslang backend plugin is explicitly requested because `slangc` dlopens
    // it at runtime; without it the toolchain is incomplete.
    let status = Command::new("cmake")
        .arg("-S")
        .arg(slang_src)
        .arg("-B")
        .arg(&build_dir)
        .arg("-DCMAKE_BUILD_TYPE=Release")
        .arg("-DSLANG_LIB_TYPE=STATIC")
        .arg("-DSLANG_ENABLE_TESTS=OFF")
        .arg("-DSLANG_ENABLE_EXAMPLES=OFF")
        .arg("-DSLANG_ENABLE_SLANG_RHI=OFF")
        .arg("-DSLANG_ENABLE_GFX=OFF")
        .arg("-DSLANG_ENABLE_SLANG_GLSLANG=ON")
        // Pin the version from this crate's own metadata. Slang otherwise
        // derives it from git tags, which shallow checkouts (as used by CI)
        // do not carry; without a tag it falls back to 0.0.0 and the
        // versioned plugin file names would not match what the crate
        // promises.
        .arg(format!("-DSLANG_VERSION_NUMERIC={}", slang_version()))
        .arg(format!("-DSLANG_VERSION_FULL={}", slang_version()))
        .status()
        .expect("failed to run cmake");
    if !status.success() {
        panic!("cmake configure of slang failed");
    }

    let run_build = || -> std::process::ExitStatus {
        let mut build = Command::new("cmake");
        build
            .arg("--build")
            .arg(&build_dir)
            .arg("--config")
            .arg("Release");
        // `--parallel` is not meaningful for the Visual Studio generator; the
        // best-effort Windows path omits it and lets MSBuild pick the default.
        if !cfg!(windows) {
            build.arg("--parallel").arg(&jobs);
        }
        build.status().expect("failed to run cmake --build")
    };

    let status = run_build();
    // The dxc sub-build (an old LLVM snapshot with its own makefiles) has
    // parallel-build races observed on macOS (duplicate ar invocations,
    // missing tablegen headers); an incremental retry resumes where the
    // failed run left off and recovers from them.
    let status = if status.success() { status } else { run_build() };
    if !status.success() {
        panic!("cmake build of slang failed");
    }

    // Locate the directory CMake emitted the executables into. Do not search
    // the whole tree for `slangc`: CMake also leaves stub files named `slangc`
    // next to per-target build rules, and picking one of those would make the
    // sibling `lib/` lookup below point at an empty directory.
    let build_bin_dir = find_bin_dir(&build_dir).unwrap_or_else(|| {
        panic!(
            "slang build did not produce a `slangc` binary under {}",
            build_dir.display()
        )
    });

    // Collect the binaries.
    fs::create_dir_all(bin_dir).expect("failed to create bin dir");
    for name in SLANG_BINARIES {
        if let Some(path) = find_file(&build_bin_dir, &exe(name)) {
            copy(&path, &bin_dir.join(exe(name)));
        }
    }
    if !bin_dir.join(exe("slangc")).is_file() {
        panic!(
            "slang build did not produce a `slangc` binary under {}",
            build_bin_dir.display()
        );
    }

    // Collect the static and shared libraries into a single directory so it
    // can be passed to a `-L` flag. The shared libraries are the downstream
    // compiler backends the binaries dlopen at runtime; the binaries carry a
    // `$ORIGIN/../lib` RUNPATH, which is exactly this directory.
    fs::create_dir_all(lib_dir).expect("failed to create lib dir");
    copy_libraries(&build_dir, &build_bin_dir, lib_dir);

    // On Windows the shared plugins (and the other runtime DLLs, e.g.
    // dxcompiler) are emitted beside the executables; their import .lib
    // files were collected into lib/ above. slangc resolves them through the
    // default loader search, which starts in the executable's directory, so
    // they must sit in the artifact bin/ as well.
    if cfg!(windows) {
        let Ok(entries) = fs::read_dir(&build_bin_dir) else {
            panic!(
                "failed to read build bin dir {}",
                build_bin_dir.display()
            );
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if has_ext(&path, &["dll"]) {
                copy(&path, &bin_dir.join(path.file_name().unwrap()));
            }
        }
    }

    // The runtime-loaded plugins are the backends `slangc` needs (glslang
    // for SPIR-V-via-GLSL, glsl-module for GLSL source). On Windows the
    // plugin is a .dll in bin/ (the import .lib in lib/ does not count);
    // on other platforms it is a shared library in lib/. Fail loudly here
    // instead of at test time if a build layout change moved one somewhere
    // the copies above did not reach.
    for plugin in ["slang-glslang", "slang-glsl-module"] {
        let dest_dir = if cfg!(windows) { bin_dir } else { lib_dir };
        let exts: &[&str] = if cfg!(windows) {
            &["dll"]
        } else {
            &["so", "dylib"]
        };
        if !has_plugin_shared(dest_dir, plugin, exts) {
            // Sweep the build tree for a layout change.
            for path in find_files(&build_dir, exts) {
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                if name.starts_with(plugin) {
                    copy(&path, &dest_dir.join(name));
                }
            }
        }
        if !has_plugin_shared(dest_dir, plugin, exts) {
            panic!(
                "slang build did not produce the `{plugin}` plugin library in {}",
                dest_dir.display()
            );
        }
    }

    // Copy the public headers (`slang.h`, ...) for consumers that compile
    // against the slang C API and need an `-I` directory.
    let slang_include = slang_src.join("include");
    fs::create_dir_all(include_dir).expect("failed to create include dir");
    if slang_include.is_dir() {
        for entry in fs::read_dir(&slang_include).expect("failed to read slang include dir") {
            let path = entry.expect("failed to read slang include entry").path();
            if path.is_file() {
                copy(&path, &include_dir.join(path.file_name().unwrap()));
            }
        }
    }

    // The loose standard-module tree (`slang-standard-module-<version>/`) is
    // resolved at runtime relative to the slang binary's directory, so it
    // must sit next to the collected binaries for `import slang.<module>` to
    // work. It is always produced as part of a slangc build.
    let module_dir_name = format!("slang-standard-module-{}", slang_version());
    let found = find_dir(&build_dir, &module_dir_name).unwrap_or_else(|| {
        panic!(
            "slang build did not produce a `{module_dir_name}` directory under {}",
            build_dir.display()
        )
    });
    copy_dir_tree(&found, &bin_dir.join(&module_dir_name));

    // Show exactly what this build collected (see `list_out_dir`).
    list_out_dir(out_dir, bin_dir, lib_dir, include_dir);

    // Smoke check: the installed slangc must run.
    let status = Command::new(bin_dir.join(exe("slangc")))
        .arg("-version")
        .status()
        .expect("failed to run built slangc");
    if !status.success() {
        panic!("built slangc failed to run (`slangc -version` exited non-zero)");
    }
}

/// Find the CMake output directory holding the `slangc` executable.
///
/// CMake layouts vary by generator: single-config generators emit into
/// `<build>/Release/bin` when slang overrides the output directories, while
/// multi-config generators (Visual Studio, Ninja Multi-Config) use
/// `<build>/<config>/bin`. A packaged install tree named
/// `slang-<version>-<platform>/bin` may also be present.
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

    // Packaged install tree, e.g. `<build>/slang-2026.21-linux-x86_64/bin`.
    if let Ok(entries) = fs::read_dir(build_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let bin = path.join("bin");
            if bin.join(exe("slangc")).is_file() {
                return Some(bin);
            }
        }
    }

    // Last resort: the first executable named `slangc` anywhere in the tree.
    find_file(build_dir, &exe("slangc")).and_then(|path| path.parent().map(Path::to_path_buf))
}

/// Copy the static and shared libraries produced by the build into `lib_dir`.
fn copy_libraries(build_dir: &Path, build_bin_dir: &Path, lib_dir: &Path) {
    // The common layout keeps `lib/` next to `bin/`.
    let mut copied_any = false;
    if let Some(parent) = build_bin_dir.parent() {
        let sibling = parent.join("lib");
        if sibling.is_dir() {
            copy_dir_files(&sibling, lib_dir);
            copied_any = true;
        }
    }

    // Best-effort fallback for layouts where the libraries are not a sibling
    // of the binaries.
    if !copied_any {
        for path in find_files(build_dir, LIBRARY_EXTENSIONS) {
            copy(&path, &lib_dir.join(path.file_name().unwrap()));
        }
    }
}

/// Copy every regular file in `src_dir` with a library extension into `dst_dir`.
fn copy_dir_files(src_dir: &Path, dst_dir: &Path) {
    if let Ok(entries) = fs::read_dir(src_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && has_ext(&path, LIBRARY_EXTENSIONS) {
                copy(&path, &dst_dir.join(path.file_name().unwrap()));
            }
        }
    }
}

/// Whether `dir` contains the shared plugin `needle`: a file whose name
/// starts with `needle` and whose extension is one of `exts` (the platform's
/// shared-library extensions). Import libraries (.lib on Windows) never
/// count: they are link inputs, not the runtime-loaded plugin.
fn has_plugin_shared(dir: &Path, needle: &str, exts: &[&str]) -> bool {
    let Ok(entries) = fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(needle) && has_ext(&path, exts) {
            return true;
        }
    }
    false
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

/// Print the complete recursive file list of the collected artifacts.
///
/// Cargo records build-script stdout in
/// `target/<profile>/build/shader-slang-src-*/output`, so a later
/// "missing file" failure can be compared against exactly what this build
/// collected. The CMake tree and the source clone are not artifacts and are
/// not listed.
fn list_out_dir(out_dir: &Path, bin_dir: &Path, lib_dir: &Path, include_dir: &Path) {
    println!(
        "shader-slang-src: collected files under {}:",
        out_dir.display()
    );
    for dir in [bin_dir, lib_dir, include_dir] {
        for path in collect_files(dir) {
            println!("  {}", path.display());
        }
    }
}

/// Recursively search `dir` for a directory with the exact `name`.
fn find_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some(name) {
                return Some(path);
            }
            if let Some(found) = find_dir(&path, name) {
                return Some(found);
            }
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

/// Recursively collect every file under `dir` whose extension is in `exts`.
fn find_files(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(find_files(&path, exts));
        } else if has_ext(&path, exts) {
            out.push(path);
        }
    }
    out
}

fn has_ext(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.contains(&e))
}

fn copy(src: &Path, dst: &Path) {
    fs::copy(src, dst)
        .unwrap_or_else(|e| panic!("failed to copy {} to {}: {e}", src.display(), dst.display()));
}
