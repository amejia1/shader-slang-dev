use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Upstream shader-slang repository.
const SLANG_REPO_URL: &str = "https://github.com/shader-slang/slang.git";

/// Programs built by slang; each is copied when the build produced it.
/// `slangc` is the compiler most consumers invoke; the rest are convenience.
const SLANG_BINARIES: &[&str] = &["slangc", "slangd", "slangi", "slang", "slang-replay"];

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
    // static compiler library, lean build (no tests/examples/rhi/gfx).
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
        .status()
        .expect("failed to run cmake");
    if !status.success() {
        panic!("cmake configure of slang failed");
    }

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
    let status = build.status().expect("failed to run cmake --build");
    if !status.success() {
        panic!("cmake build of slang failed");
    }

    // Collect the binaries.
    fs::create_dir_all(bin_dir).expect("failed to create bin dir");
    for name in SLANG_BINARIES {
        if let Some(path) = find_file(&build_dir, &exe(name)) {
            copy(&path, &bin_dir.join(exe(name)));
        }
    }
    if !bin_dir.join(exe("slangc")).is_file() {
        panic!(
            "slang build did not produce a `slangc` binary under {}",
            build_dir.display()
        );
    }

    // Collect the static and shared libraries into a single directory so it
    // can be passed to a `-L` flag. The shared libraries are the downstream
    // compiler backends the binaries dlopen at runtime; the binaries carry a
    // `$ORIGIN/../lib` RUNPATH, which is exactly this directory.
    fs::create_dir_all(lib_dir).expect("failed to create lib dir");
    let slangc_path =
        find_file(&build_dir, &exe("slangc")).expect("slangc binary vanished from the build tree");
    let build_bin = slangc_path.parent().unwrap().to_path_buf();
    let build_lib = build_bin.parent().unwrap().join("lib");
    let mut copied_any = false;
    if let Ok(entries) = fs::read_dir(&build_lib) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && has_ext(&path, &["a", "lib", "so", "dylib", "dll"]) {
                copy(&path, &lib_dir.join(path.file_name().unwrap()));
                copied_any = true;
            }
        }
    }
    // Best-effort fallback for layouts where the libraries are not a sibling
    // of the binaries (e.g. multi-config generators on Windows).
    if !copied_any {
        for path in find_files(&build_dir, &["a", "lib", "so", "dylib", "dll"]) {
            copy(&path, &lib_dir.join(path.file_name().unwrap()));
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

    // Smoke check: the installed slangc must run.
    let status = Command::new(bin_dir.join(exe("slangc")))
        .arg("-version")
        .status()
        .expect("failed to run built slangc");
    if !status.success() {
        panic!("built slangc failed to run (`slangc -version` exited non-zero)");
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
