# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
#
# The Windows counterpart of scripts/build-rust-core.sh, which is zsh.
#
# Nothing here is optional groundwork: `Sources/RoamlingCoreRs` and
# `Sources/CRoamlingCoreFFI` are generated and git-untracked, and Package.swift
# names both as targets. Without them SwiftPM cannot even load the manifest, so
# this runs before any `swift build` on a fresh Windows checkout.
#
# Only the file names differ from the zsh version -- `roamling_core.lib` and
# `roamling_core.dll` where macOS has `libroamling_core.a` and `.dylib`. The
# crate already declares `staticlib` and `cdylib`, so cargo produces both here
# without a manifest change.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

$machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
$user = [Environment]::GetEnvironmentVariable("Path", "User")
$env:Path = "$machine;$user"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "cargo is not on PATH. Install the Rust toolchain first." -ForegroundColor Red
    exit 1
}

$manifest = Join-Path $root "rust\Cargo.toml"
$generated = Join-Path $root "Sources\RoamlingCoreRs"
$headers = Join-Path $root "Sources\CRoamlingCoreFFI"
$libDir = Join-Path $root ".build\rust"
foreach ($dir in @($generated, $headers, $libDir)) {
    New-Item -ItemType Directory -Force -Path $dir | Out-Null
}

cargo build --quiet --release --manifest-path $manifest -p roamling-core
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$archive = Join-Path $root "rust\target\release\roamling_core.lib"
$shared = Join-Path $root "rust\target\release\roamling_core.dll"
foreach ($produced in @($archive, $shared)) {
    if (-not (Test-Path $produced)) {
        Write-Host "cargo produced no $produced" -ForegroundColor Red
        exit 1
    }
}

# uniffi reads the built library rather than the source, so the bindings cannot
# drift from what actually shipped in it.
cargo run --quiet --release --manifest-path $manifest -p roamling-core --bin uniffi-bindgen -- `
    generate --library $shared --language swift --out-dir $generated --no-format
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

# SwiftPM wants the module map under the systemLibrary target and named
# `module.modulemap`; uniffi writes both files next to the Swift.
Move-Item -Force (Join-Path $generated "roamling_coreFFI.h") $headers
Move-Item -Force (Join-Path $generated "roamling_coreFFI.modulemap") (Join-Path $headers "module.modulemap")
Copy-Item -Force $archive (Join-Path $libDir "roamling_core.lib")

Write-Host "generated bindings in Sources\RoamlingCoreRs and Sources\CRoamlingCoreFFI" -ForegroundColor Green
