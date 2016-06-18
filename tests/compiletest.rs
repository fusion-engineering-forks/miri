extern crate compiletest_rs as compiletest;

use std::path::{PathBuf, Path};
use std::io::Write;

fn run_mode(dir: &'static str, mode: &'static str, sysroot: &str) {
    // Disable rustc's new error fomatting. It breaks these tests.
    std::env::remove_var("RUST_NEW_ERROR_FORMAT");
    let flags = format!("--sysroot {} -Dwarnings", sysroot);
    for_all_targets(sysroot, |target| {
        let mut config = compiletest::default_config();
        config.host_rustcflags = Some(flags.clone());
        config.mode = mode.parse().expect("Invalid mode");
        config.run_lib_path = Path::new(sysroot).join("lib").join("rustlib").join(&target).join("lib");
        config.rustc_path = "target/debug/miri".into();
        config.src_base = PathBuf::from(format!("tests/{}", dir));
        config.target = target.to_owned();
        config.target_rustcflags = Some(flags.clone());
        compiletest::run_tests(&config);
    });
}

fn for_all_targets<F: FnMut(String)>(sysroot: &str, mut f: F) {
    for target in std::fs::read_dir(format!("{}/lib/rustlib/", sysroot)).unwrap() {
        let target = target.unwrap();
        if !target.metadata().unwrap().is_dir() {
            continue;
        }
        let target = target.file_name().into_string().unwrap();
        if target == "etc" {
            continue;
        }
        let stderr = std::io::stderr();
        writeln!(stderr.lock(), "running tests for target {}", target).unwrap();
        f(target);
    }
}

#[test]
fn compile_test() {
    let mut failed = false;
    // Taken from https://github.com/Manishearth/rust-clippy/pull/911.
    let home = option_env!("RUSTUP_HOME").or(option_env!("MULTIRUST_HOME"));
    let toolchain = option_env!("RUSTUP_TOOLCHAIN").or(option_env!("MULTIRUST_TOOLCHAIN"));
    let sysroot = match (home, toolchain) {
        (Some(home), Some(toolchain)) => format!("{}/toolchains/{}", home, toolchain),
        _ => option_env!("RUST_SYSROOT")
            .expect("need to specify RUST_SYSROOT env var or use rustup or multirust")
            .to_owned(),
    };
    run_mode("compile-fail", "compile-fail", &sysroot);
    for_all_targets(&sysroot, |target| {
        for file in std::fs::read_dir("tests/run-pass").unwrap() {
            let file = file.unwrap();
            if !file.metadata().unwrap().is_file() {
                continue;
            }
            let file = file.path();
            let stderr = std::io::stderr();
            write!(stderr.lock(), "test [miri-pass] {} ", file.to_str().unwrap()).unwrap();
            let mut cmd = std::process::Command::new("target/debug/miri");
            cmd.arg(file);
            cmd.arg("-Dwarnings");
            cmd.arg(format!("--target={}", target));
            let libs = Path::new(&sysroot).join("lib");
            let sysroot = libs.join("rustlib").join(&target).join("lib");
            let paths = std::env::join_paths(&[libs, sysroot]).unwrap();
            cmd.env(compiletest::procsrv::dylib_env_var(), paths);
            match cmd.output() {
                Ok(ref output) if output.status.success() => writeln!(stderr.lock(), "ok").unwrap(),
                Ok(output) => {
                    failed = true;
                    writeln!(stderr.lock(), "FAILED with exit code {}", output.status.code().unwrap_or(0)).unwrap();
                    writeln!(stderr.lock(), "stdout: \n {}", std::str::from_utf8(&output.stdout).unwrap()).unwrap();
                    writeln!(stderr.lock(), "stderr: \n {}", std::str::from_utf8(&output.stderr).unwrap()).unwrap();
                }
                Err(e) => {
                    failed = true;
                    writeln!(stderr.lock(), "FAILED: {}", e).unwrap();
                },
            }
        }
        let stderr = std::io::stderr();
        writeln!(stderr.lock(), "").unwrap();
    });
    if failed {
        panic!("some tests failed");
    }
}