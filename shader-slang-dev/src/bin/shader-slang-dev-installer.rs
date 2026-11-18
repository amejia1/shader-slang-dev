//! `shader-slang-dev-installer` — install the located shader-slang artifacts
//! into a prefix, from the command line.
//!
//! The artifacts installed are the ones the `shader-slang-dev` crate locates:
//! `$SHADER_SLANG_HOME`, a common install location, or the `$OUT_DIR` build
//! produced with the `build` feature (this binary requires that feature).
//! The tree is copied recursively and symlinks are preserved.
//!
//! Usage:
//!
//! ```sh
//! cargo run --features build --bin shader-slang-dev-installer -- /opt/slang
//! # or, when installed on PATH:
//! shader-slang-dev-installer /opt/slang
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "usage: shader-slang-dev-installer <dest-dir>

Recursively copies the located shader-slang artifacts ($SHADER_SLANG_HOME, a
common install location, or the $OUT_DIR build with the `build` feature)
into <dest-dir>, preserving symlinks.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.len() != 1 {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    }
    let dest = PathBuf::from(&args[0]);
    match shader_slang_dev::install_artifacts(&dest) {
        Ok(()) => {
            println!(
                "installed shader-slang {} artifacts from {} to {}",
                shader_slang_dev::SLANG_VERSION,
                shader_slang_dev::artifact_root().display(),
                dest.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("shader-slang-dev-installer: {e}");
            ExitCode::FAILURE
        }
    }
}
