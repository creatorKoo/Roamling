// SPDX-FileCopyrightText: 2026 GooBeom Jeoung
// SPDX-License-Identifier: GPL-3.0-only

//! Embeds the application manifest, the icon and the version block.
//!
//! There is a crate for each of these and neither is worth a dependency: the
//! MSVC linker takes a manifest directly, and the Windows SDK ships `rc.exe`
//! next to the linker that is already required to build at all.
//!
//! The icon is the same file the macOS app uses. See `roamling.rc`.

use std::path::{Path, PathBuf};

fn main() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let manifest = manifest_dir.join("roamling.manifest");
    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
        manifest.display()
    );

    let script = manifest_dir.join("roamling.rc");
    let icon = manifest_dir.join("../../assets/Roamling.ico");
    println!("cargo:rerun-if-changed={}", script.display());
    println!("cargo:rerun-if-changed={}", icon.display());

    // Without an icon the executable gets the shell's generic one, which is a
    // cosmetic failure rather than a broken build -- so a missing `rc.exe` is a
    // warning and the build goes on. It is present in every Windows SDK, and
    // the SDK is already there because `link.exe` came from it.
    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe not found; building without an icon or version block");
        return;
    };

    // The version has to reach the resource script as two shapes: 1,2,0,0 for
    // the numeric fields and "1.2.0" for the strings. Deriving both from
    // CARGO_PKG_VERSION keeps them from drifting apart, and from the crate.
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let mut parts: Vec<&str> = version.split('.').collect();
    parts.resize(4, "0");
    let commas = parts.join(",");

    // The version is substituted into a copy rather than passed with `/d`.
    // `rc.exe` takes a string define only with embedded quotes, and getting
    // those through Windows' argument splitting intact is a fight with nothing
    // at the end of it. The icon path becomes absolute in the same pass, since
    // the copy no longer sits beside it.
    let Ok(template) = std::fs::read_to_string(&script) else {
        println!("cargo:warning=could not read {}; building without an icon", script.display());
        return;
    };
    let filled = template
        .replace("ROAMLING_VERSION_COMMAS", &commas)
        .replace("ROAMLING_VERSION_TEXT", &format!("{version:?}"))
        .replace(
            "\"../../assets/Roamling.ico\"",
            &format!("{:?}", icon.display().to_string()),
        );

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    let generated = out.join("roamling.rc");
    if std::fs::write(&generated, filled).is_err() {
        println!("cargo:warning=could not write {}; building without an icon", generated.display());
        return;
    }
    let compiled = out.join("roamling.res");
    let status = std::process::Command::new(&rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&compiled)
        .arg(&generated)
        .status();

    match status {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg-bins={}", compiled.display());
        }
        Ok(status) => {
            println!("cargo:warning=rc.exe failed ({status}); building without an icon");
        }
        Err(error) => {
            println!("cargo:warning=could not run rc.exe ({error}); building without an icon");
        }
    }
}

/// The newest `rc.exe` under the installed Windows SDKs.
///
/// Cargo does not put the SDK on the path the way a Visual Studio command
/// prompt would, so it is looked up the same way the linker's own toolchain
/// detection does: by version directory, newest first.
fn find_rc() -> Option<PathBuf> {
    if let Ok(found) = which("rc.exe") {
        return Some(found);
    }
    let roots = [
        std::env::var("ProgramFiles(x86)").unwrap_or_default(),
        std::env::var("ProgramFiles").unwrap_or_default(),
    ];
    let architecture = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };

    let mut candidates: Vec<PathBuf> = Vec::new();
    for root in roots.iter().filter(|root| !root.is_empty()) {
        let bin = Path::new(root).join("Windows Kits").join("10").join("bin");
        let Ok(entries) = std::fs::read_dir(&bin) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path().join(architecture).join("rc.exe");
            if candidate.is_file() {
                candidates.push(candidate);
            }
        }
    }
    // Directory names are SDK versions, so the last one sorted is the newest.
    candidates.sort();
    candidates.pop()
}

fn which(name: &str) -> Result<PathBuf, ()> {
    let path = std::env::var_os("PATH").ok_or(())?;
    std::env::split_paths(&path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or(())
}
