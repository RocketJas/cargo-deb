use std::env;
use std::env::consts::DLL_PREFIX;
use std::env::consts::DLL_SUFFIX;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// file extension of the compression format cargo-deb uses unless explicitly specified.
const DEFAULT_COMPRESSION_EXT: &str = "xz";

#[test]
fn build_workspaces() {
    let (cdir, ddir) = extract_built_package_from_manifest("tests/test-workspace/test-ws1/Cargo.toml", DEFAULT_COMPRESSION_EXT, &["--no-strip", "--fast"]);
    assert!(ddir.path().join("usr/local/bin/renamed2").exists());
    assert!(ddir.path().join("usr/local/bin/decoy").exists());

    let control = fs::read_to_string(cdir.path().join("control")).unwrap();
    assert!(control.contains("Version: 1.0.0-ws-1\n"));
    assert!(control.contains("Package: test1-crate-name\n"));
    assert!(control.contains("Maintainer: ws\n"));

    let (_, ddir) = extract_built_package_from_manifest("tests/test-workspace/test-ws2/Cargo.toml", DEFAULT_COMPRESSION_EXT, &["--no-strip"]);
    assert!(ddir.path().join("usr/bin/renamed2").exists());
    assert!(ddir.path().join(format!("usr/lib/{DLL_PREFIX}test2lib{DLL_SUFFIX}")).exists());
    assert!(ddir.path().join("usr/share/doc/test2/a-read-me").exists());
}

#[test]
fn build_with_explicit_compress_type() {
    // ws1 with gzip
    let (_, ddir) = extract_built_package_from_manifest("tests/test-workspace/test-ws1/Cargo.toml", "gz", &["--no-strip", "--compress-type", "gzip"]);
    assert!(ddir.path().join("usr/local/bin/decoy").exists());

    // ws2 with xz
    let (_, ddir) = extract_built_package_from_manifest("tests/test-workspace/test-ws2/Cargo.toml", "xz", &["--no-strip", "--compress-type", "xz"]);
    assert!(ddir.path().join("usr/bin/renamed2").exists());
}

#[test]
fn build_with_command_line_compress() {
    // ws1 with system xz
    let (_, ddir) = extract_built_package_from_manifest("tests/test-workspace/test-ws1/Cargo.toml", "xz", &["--no-strip", "--compress-system", "--compress-type", "xz"]);
    assert!(ddir.path().join("usr/local/bin/decoy").exists());

    // ws2 with system gzip
    let (_, ddir) = extract_built_package_from_manifest("tests/test-workspace/test-ws2/Cargo.toml", "gz", &["--no-strip", "--compress-system", "--compress-type", "gz"]);
    assert!(ddir.path().join("usr/bin/renamed2").exists());
}

#[track_caller]
fn extract_built_package_from_manifest(manifest_path: &str, ext: &str, args: &[&str]) -> (TempDir, TempDir) {
    let (_bdir, deb_path) = cargo_deb(manifest_path, args);

    let ardir = tempfile::tempdir().expect("testdir");
    assert!(ardir.path().exists());
    assert!(Command::new("ar")
        .current_dir(ardir.path())
        .arg("-x")
        .arg(deb_path)
        .status().unwrap().success());

    assert_eq!("2.0\n", fs::read_to_string(ardir.path().join("debian-binary")).unwrap());

    assert!(ardir.path().join(format!("data.tar.{ext}")).exists());
    assert!(ardir.path().join(format!("control.tar.{ext}")).exists());

    let cdir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xf")
        .current_dir(cdir.path())
        .arg(ardir.path().join(format!("control.tar.{ext}")))
        .status().unwrap().success());

    let ddir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xf")
        .current_dir(ddir.path())
        .arg(ardir.path().join(format!("data.tar.{ext}")))
        .status().unwrap().success());

    (cdir, ddir)
}

/// Run `cargo-deb` for the manifest with extra args, returns the `TempDir` holding the built package
/// and the path to the built package.
///
/// The `--manifest-path` and `--output` args are automatically set.
#[track_caller]
fn cargo_deb(manifest_path: &str, args: &[&str]) -> (TempDir, PathBuf) {
    let cargo_dir = tempfile::tempdir().unwrap();
    let deb_path = cargo_dir.path().join("test.deb");

    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let cmd_path = root.join(env!("CARGO_BIN_EXE_cargo-deb"));
    assert!(cmd_path.exists());
    let output = Command::new(cmd_path)
        .env("CARGO_TARGET_DIR", cargo_dir.path()) // use isolated 'target' directories
        .arg(format!("--manifest-path={}", root.join(manifest_path).display()))
        .arg(format!("--output={}", deb_path.display()))
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), 
            "Cmd failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

    // prints deb path on the last line
    let last_line = output.stdout[..output.stdout.len() - 1].split(|&c| c == b'\n').last().unwrap();
    let printed_deb_path = Path::new(::std::str::from_utf8(last_line).unwrap());
    assert_eq!(printed_deb_path, deb_path);
    assert!(deb_path.exists());

    (cargo_dir, deb_path)
}

#[test]
#[cfg(all(feature = "lzma", target_family = "unix", not(target_os = "macos")))]
fn run_cargo_deb_command_on_example_dir() {
    let (cdir, ddir) = extract_built_package_from_manifest("example/Cargo.toml", DEFAULT_COMPRESSION_EXT, &[]);

    let control = fs::read_to_string(cdir.path().join("control")).unwrap();
    assert!(control.contains("Package: example\n"));
    assert!(control.contains("Version: 0.1.0-1\n"));
    assert!(control.contains("Section: utils\n"));
    assert!(control.contains("Architecture: "));
    assert!(control.contains("Maintainer: cargo-deb developers <cargo-deb@example.invalid>\n"));

    let sha256sums = fs::read_to_string(cdir.path().join("sha256sums")).unwrap();
    assert!(sha256sums.contains(" usr/bin/example\n"));
    assert!(sha256sums.contains(" usr/share/doc/example/changelog.Debian.gz\n"));
    assert!(sha256sums.contains("5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03  var/lib/example/1.txt\n"));
    assert!(sha256sums.contains("e258d248fda94c63753607f7c4494ee0fcbe92f1a76bfdac795c9d84101eb317  var/lib/example/2.txt\n"));
    assert!(sha256sums.contains("6d1ad4737d3113321867260562fd6a6df6dddd7b93c8476beec829d7d8473690  var/lib/example/3.txt\n"));
    assert!(sha256sums.contains("7b0cc11d7acd89db0288d56c2ddee2b67de185357d5305a3726d1a5dd7685e86  usr/share/doc/example/copyright\n"), "{sha256sums:?}");

    assert!(ddir.path().join("var/lib/example/1.txt").exists());
    assert!(ddir.path().join("var/lib/example/2.txt").exists());
    assert!(ddir.path().join("var/lib/example/3.txt").exists());
    assert!(ddir.path().join("usr/share/doc/example/copyright").exists());
    assert!(ddir.path().join("usr/share/doc/example/changelog.Debian.gz").exists());
    assert!(ddir.path().join("usr/bin/example").exists());
    // changelog.Debian.gz starts with the gzip magic
    assert_eq!(
        &[0x1F, 0x8B],
        &fs::read(ddir.path().join("usr/share/doc/example/changelog.Debian.gz")).unwrap()[..2]
    );
}

#[test]
#[cfg(all(target_family = "unix", not(target_os = "macos")))]
fn run_cargo_deb_command_on_example_dir_with_separate_debug_symbols() {
    let (_cdir, ddir) = extract_built_package_from_manifest("example/Cargo.toml", DEFAULT_COMPRESSION_EXT, &["--separate-debug-symbols"]);

    let stripped = ddir.path().join("usr/bin/example");
    let debug = ddir.path().join("usr/lib/debug/usr/bin/example.debug");

    assert!(stripped.exists());
    assert!(
        debug.exists(),
        "unable to find executable with debug symbols {} for stripped executable {}",
        debug.display(),
        stripped.display()
    );

    assert!(
        stripped.metadata().unwrap().len() < debug.metadata().unwrap().len(),
        "stripped executable should be smaller than debug executable"
    );
}

#[test]
#[cfg(feature = "lzma")]
fn run_cargo_deb_command_on_example_dir_with_variant() {
    let args = ["--variant=debug", "--no-strip"];
    let (_bdir, deb_path) = cargo_deb("example/Cargo.toml", &args);

    let ardir = tempfile::tempdir().unwrap();
    assert!(ardir.path().exists());
    assert!(Command::new("ar")
        .current_dir(ardir.path())
        .arg("-x")
        .arg(deb_path)
        .status().unwrap().success());

    assert_eq!("2.0\n", fs::read_to_string(ardir.path().join("debian-binary")).unwrap());
    let ext = DEFAULT_COMPRESSION_EXT;
    assert!(ardir.path().join(format!("data.tar.{ext}")).exists());
    assert!(ardir.path().join(format!("control.tar.{ext}")).exists());

    let cdir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xJf")
        .current_dir(cdir.path())
        .arg(ardir.path().join(format!("control.tar.{ext}")))
        .status().unwrap().success());

    let control = fs::read_to_string(cdir.path().join("control")).unwrap();
    assert!(control.contains("Package: example-debug\n"), "Control is: {control:?}");
    assert!(control.contains("Version: 0.1.0-1\n"));
    assert!(control.contains("Section: utils\n"));
    assert!(control.contains("Architecture: "));
    assert!(control.contains("Maintainer: cargo-deb developers <cargo-deb@example.invalid>\n"));

    let sha256sums = fs::read_to_string(cdir.path().join("sha256sums")).unwrap();
    assert!(sha256sums.contains(" usr/bin/example\n"));
    assert!(sha256sums.contains(" usr/share/doc/example-debug/changelog.Debian.gz\n"));
    assert!(sha256sums.contains("5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03  var/lib/example/1.txt\n"));
    assert!(sha256sums.contains("e258d248fda94c63753607f7c4494ee0fcbe92f1a76bfdac795c9d84101eb317  var/lib/example/2.txt\n"));
    assert!(sha256sums.contains("cf8970827eac78e5c539d0eaed4356d6d9822c7fa61afd2421c8abbd4a50d8c0  var/lib/example/4.txt\n"));
    assert!(sha256sums.contains("ff3c1510993f227a21b14683e0e65c2f0165f034c4e5f6fe3d77661b30af2b35  usr/share/doc/example-debug/copyright\n"), "{sha256sums:?}");

    let ddir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xJf")
        .current_dir(ddir.path())
        .arg(ardir.path().join(format!("data.tar.{ext}")))
        .status().unwrap().success());

    assert!(ddir.path().join("var/lib/example/1.txt").exists());
    assert!(ddir.path().join("var/lib/example/2.txt").exists());
    assert!(ddir.path().join("var/lib/example/4.txt").exists());
    assert!(ddir.path().join("usr/share/doc/example-debug/copyright").exists());
    assert!(ddir.path().join("usr/share/doc/example-debug/changelog.Debian.gz").exists());
    assert!(ddir.path().join("usr/bin/example").exists());
}

#[test]
#[cfg(all(feature = "lzma", target_family = "unix", not(target_os = "macos")))]
fn run_cargo_deb_command_on_example_dir_with_version() {
    let (_bdir, deb_path) = cargo_deb("example/Cargo.toml", &["--deb-version=my-custom-version"]);

    let ardir = tempfile::tempdir().unwrap();
    assert!(ardir.path().exists());
    assert!(Command::new("ar")
        .current_dir(ardir.path())
        .arg("-x")
        .arg(deb_path)
        .status().unwrap().success());

    let ext = DEFAULT_COMPRESSION_EXT;
    assert_eq!("2.0\n", fs::read_to_string(ardir.path().join("debian-binary")).unwrap());
    assert!(ardir.path().join(format!("data.tar.{ext}")).exists());
    assert!(ardir.path().join(format!("control.tar.{ext}")).exists());

    let cdir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xf")
        .current_dir(cdir.path())
        .arg(ardir.path().join(format!("control.tar.{ext}")))
        .status().unwrap().success());

    let control = fs::read_to_string(cdir.path().join("control")).unwrap();
    assert!(control.contains("Package: example\n"));
    assert!(control.contains("Version: my-custom-version\n"));
    assert!(control.contains("Section: utils\n"));
    assert!(control.contains("Architecture: "));
    assert!(control.contains("Maintainer: cargo-deb developers <cargo-deb@example.invalid>\n"));

    let sha256sums = fs::read_to_string(cdir.path().join("sha256sums")).unwrap();
    assert!(sha256sums.contains(" usr/bin/example\n"));
    assert!(sha256sums.contains(" usr/share/doc/example/changelog.Debian.gz\n"));
    assert!(sha256sums.contains("5891b5b522d5df086d0ff0b110fbd9d21bb4fc7163af34d08286a2e846f6be03  var/lib/example/1.txt\n"));
    assert!(sha256sums.contains("e258d248fda94c63753607f7c4494ee0fcbe92f1a76bfdac795c9d84101eb317  var/lib/example/2.txt\n"));
    assert!(sha256sums.contains("6d1ad4737d3113321867260562fd6a6df6dddd7b93c8476beec829d7d8473690  var/lib/example/3.txt\n"));
    assert!(sha256sums.contains("7b0cc11d7acd89db0288d56c2ddee2b67de185357d5305a3726d1a5dd7685e86  usr/share/doc/example/copyright\n"), "has:\n{sha256sums}");

    let ddir = tempfile::tempdir().unwrap();
    assert!(Command::new("tar")
        .arg("xJf")
        .current_dir(ddir.path())
        .arg(ardir.path().join(format!("data.tar.{ext}")))
        .status().unwrap().success());

    assert!(ddir.path().join("var/lib/example/1.txt").exists());
    assert!(ddir.path().join("var/lib/example/2.txt").exists());
    assert!(ddir.path().join("var/lib/example/3.txt").exists());
    assert!(ddir.path().join("usr/share/doc/example/copyright").exists());
    assert!(ddir.path().join("usr/share/doc/example/changelog.Debian.gz").exists());
    assert!(ddir.path().join("usr/bin/example").exists());
    // changelog.Debian.gz starts with the gzip magic
    assert_eq!(
        &[0x1F, 0x8B],
        &fs::read(ddir.path().join("usr/share/doc/example/changelog.Debian.gz")).unwrap()[..2]
    );
}
