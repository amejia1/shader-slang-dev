//! Source reassembly and CMake build implementation for `shader-slang-src`.

use cmake::Config as CmakeConfig;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Plugins `slangc` loads at runtime (glslang for SPIR-V via GLSL,
/// glsl-module for GLSL source).
const SLANG_PLUGINS: &[&str] = &["slang-glslang", "slang-glsl-module"];

/// Bump when the expected installed artifact layout changes.
const LAYOUT_MARKER: &str = "3";

pub fn main(source_archives: &[&str], slang_version: &str) {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is not set"));
    let source_dir = out_dir.join("slang-src");
    let build_dir = out_dir.join("build");
    let artifact_root = out_dir.join("slang-artifact");
    let bin_dir = artifact_root.join("bin");
    let lib_dir = artifact_root.join("lib");

    println!("shader-slang-src: building vendored slang {slang_version}");
    if !artifacts_complete(&artifact_root, &bin_dir, &lib_dir, slang_version) {
        if artifact_root.exists() {
            fs::remove_dir_all(&artifact_root).unwrap_or_else(|e| {
                panic!(
                    "failed to remove stale artifact root {}: {e}",
                    artifact_root.display()
                )
            });
        }
        reassemble_sources(source_archives, &source_dir);
        build_slang(
            &source_dir,
            &build_dir,
            &artifact_root,
            &bin_dir,
            &lib_dir,
            slang_version,
        );
    }
}

/// Extract every source partition over one tree. Each archive contains paths
/// relative to the shader-slang repository root.
fn reassemble_sources(source_archives: &[&str], source_dir: &Path) {
    if source_dir.exists() {
        fs::remove_dir_all(source_dir).unwrap_or_else(|e| {
            panic!(
                "failed to remove old source tree {}: {e}",
                source_dir.display()
            )
        });
    }
    fs::create_dir_all(source_dir).unwrap_or_else(|e| {
        panic!(
            "failed to create source directory {}: {e}",
            source_dir.display()
        )
    });
    for archive in source_archives {
        println!("cargo::rerun-if-changed={archive}");
        let file = fs::File::open(archive)
            .unwrap_or_else(|e| panic!("failed to open source archive {archive}: {e}"));
        tar::Archive::new(file)
            .unpack(source_dir)
            .unwrap_or_else(|e| panic!("failed to unpack source archive {archive}: {e}"));
    }
    if !source_dir.join("CMakeLists.txt").is_file() {
        panic!(
            "reassembled source tree {} has no CMakeLists.txt",
            source_dir.display()
        );
    }
    if !submodules_present(source_dir) {
        panic!(
            "reassembled source tree {} does not contain every submodule checkout",
            source_dir.display()
        );
    }
}

/// Whether every submodule path listed in `.gitmodules` is a non-empty
/// directory. A missing `.gitmodules` means there is nothing to check.
fn submodules_present(source_dir: &Path) -> bool {
    let Ok(contents) = fs::read_to_string(source_dir.join(".gitmodules")) else {
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
            fs::read_dir(source_dir.join(path))
                .map(|mut entries| entries.next().is_some())
                .unwrap_or(false)
        })
}

/// Whether the installed artifact set is complete enough to skip rebuilding.
fn artifacts_complete(
    artifact_root: &Path,
    bin_dir: &Path,
    lib_dir: &Path,
    slang_version: &str,
) -> bool {
    if !artifact_root.is_dir() {
        return false;
    }
    let marker_ok = fs::read_to_string(artifact_root.join(".shader-slang-layout"))
        .is_ok_and(|contents| contents.trim() == LAYOUT_MARKER);
    if !marker_ok || !bin_dir.join(exe_name("slangc")).is_file() {
        return false;
    }
    if !artifact_root.join("include").is_dir() {
        return false;
    }
    // Source builds keep an additional standard-module copy in `bin/` for
    // the statically linked slangc.
    let module_dir_name = format!("slang-standard-module-{slang_version}");
    if !lib_dir.join(&module_dir_name).is_dir() && !bin_dir.join(&module_dir_name).is_dir() {
        return false;
    }
    let plugin_dir = if cfg!(windows) { bin_dir } else { lib_dir };
    let exts: &[&str] = if cfg!(windows) {
        &["dll"]
    } else {
        &["so", "dylib"]
    };
    SLANG_PLUGINS
        .iter()
        .all(|plugin| has_plugin_shared(plugin_dir, plugin, exts))
}

/// Configure and build slang, then install it with the release-tarball layout.
fn build_slang(
    source_dir: &Path,
    build_dir: &Path,
    artifact_root: &Path,
    bin_dir: &Path,
    lib_dir: &Path,
    slang_version: &str,
) {
    let mut cmake_config = CmakeConfig::new(source_dir);
    cmake_config
        .out_dir(build_dir.parent().expect("build dir has no parent"))
        // Always build Release slang, regardless of the Rust profile.
        .profile("Release")
        .define("SLANG_LIB_TYPE", "STATIC")
        // The default FETCH_BINARY flavor downloads slang-llvm and does not
        // install it as part of the default build.
        .define("SLANG_SLANG_LLVM_FLAVOR", "DISABLE")
        .define("SLANG_ENABLE_TESTS", "OFF")
        .define("SLANG_ENABLE_EXAMPLES", "OFF")
        .define("SLANG_ENABLE_SLANG_RHI", "OFF")
        .define("SLANG_ENABLE_GFX", "OFF")
        .define("SLANG_ENABLE_SLANG_GLSLANG", "ON")
        // Git metadata is deliberately absent from the vendored sources, so
        // provide the release values that CMake otherwise derives from tags.
        .define("SLANG_VERSION_NUMERIC", slang_version)
        .define("SLANG_VERSION_FULL", slang_version)
        .no_build_target(true);

    run_cmake_build(&mut cmake_config);
    run_cmake_install(build_dir, artifact_root);
    collect_headers(artifact_root);
    copy_standard_modules_to_bin(artifact_root, bin_dir, lib_dir, slang_version);

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

    list_artifacts(artifact_root);
    smoke_check_slangc(bin_dir);
    write_layout_marker(artifact_root);
}

fn smoke_check_slangc(bin_dir: &Path) {
    let status = Command::new(bin_dir.join(exe_name("slangc")))
        .arg("-version")
        .status()
        .expect("failed to run installed slangc");
    if !status.success() {
        panic!("installed slangc failed to run (`slangc -version` exited non-zero)");
    }
}

fn write_layout_marker(artifact_root: &Path) {
    fs::write(artifact_root.join(".shader-slang-layout"), LAYOUT_MARKER).unwrap_or_else(|e| {
        panic!(
            "failed to write layout marker in {}: {e}",
            artifact_root.display()
        )
    });
}

/// Retry once to recover from known parallel-build races in the old DXC
/// sub-build on macOS. The second attempt resumes the incremental build.
fn run_cmake_build(cmake_config: &mut CmakeConfig) {
    let first_attempt = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cmake_config.build();
    }));
    if first_attempt.is_err() {
        cmake_config.build();
    }
}

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

/// Move root-level headers to the release layout's `include/` directory.
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

/// Statically linked slangc resolves standard modules next to its executable.
fn copy_standard_modules_to_bin(
    artifact_root: &Path,
    bin_dir: &Path,
    lib_dir: &Path,
    slang_version: &str,
) {
    let module_dir_name = format!("slang-standard-module-{slang_version}");
    let src = lib_dir.join(&module_dir_name);
    let dst = bin_dir.join(&module_dir_name);
    if src.is_dir() && !dst.is_dir() {
        copy_dir_tree(&src, &dst);
    }
    if !dst.is_dir() {
        panic!(
            "slang install did not produce a `{module_dir_name}` directory under {}",
            artifact_root.display()
        );
    }
}

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

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}
