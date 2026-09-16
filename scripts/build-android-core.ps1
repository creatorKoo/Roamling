# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
param([ValidateRange(1, 64)][int]$Jobs = 2)

$ErrorActionPreference = 'Stop'
$repositoryDir = Split-Path -Parent $PSScriptRoot
$generated = Join-Path $repositoryDir 'android\core\src\main\kotlin'
$native = Join-Path $repositoryDir 'android\core\src\main\jniLibs'

# A terminal opened before setup can still use the user's installed tools.
foreach ($name in @('ANDROID_HOME', 'ANDROID_NDK_HOME')) {
    if (-not [Environment]::GetEnvironmentVariable($name, 'Process')) {
        $value = [Environment]::GetEnvironmentVariable($name, 'User')
        if ($value) { [Environment]::SetEnvironmentVariable($name, $value, 'Process') }
    }
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    $env:Path += ';' + (Join-Path $env:USERPROFILE '.cargo\bin')
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Install Rust first.' }
& cargo ndk --version
if ($LASTEXITCODE -ne 0) { throw 'Install cargo-ndk: cargo install cargo-ndk --version 4.1.2 --locked' }
if (-not $env:ANDROID_NDK_HOME -or -not (Test-Path (Join-Path $env:ANDROID_NDK_HOME 'source.properties'))) {
    throw 'Set ANDROID_NDK_HOME to an installed Android NDK directory.'
}

New-Item -ItemType Directory -Force -Path $generated, $native | Out-Null
Push-Location (Join-Path $repositoryDir 'rust')
try {
    & cargo build --locked --release -p roamling-android -p roamling-core --jobs $Jobs
    if ($LASTEXITCODE -ne 0) { throw 'Host native build failed.' }
    $metadataJson = & cargo metadata --locked --format-version 1 --no-deps
    if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed.' }
    $targetDir = ($metadataJson | ConvertFrom-Json).target_directory
    $hostLibrary = Join-Path $targetDir 'release\roamling_android.dll'
    $bindgen = Join-Path $targetDir 'release\uniffi-bindgen.exe'
    & $bindgen generate --library $hostLibrary --language kotlin --out-dir $generated --no-format
    if ($LASTEXITCODE -ne 0) { throw 'Kotlin binding generation failed.' }

    # Run from rust/ so Cargo reads its target-scoped 16 KB linker flags.
    & cargo ndk -t arm64-v8a -t x86_64 --platform 30 build --locked --release -p roamling-android --lib --jobs $Jobs
    if ($LASTEXITCODE -ne 0) { throw 'Android native build failed.' }
    # cargo-ndk -o also copies dependency cdylibs. Both UniFFI components are
    # already in our one library; do not package a second copy of the core.
    foreach ($target in @(
        @{ Abi = 'arm64-v8a'; Triple = 'aarch64-linux-android' },
        @{ Abi = 'x86_64'; Triple = 'x86_64-linux-android' }
    )) {
        $destination = Join-Path $native $target.Abi
        New-Item -ItemType Directory -Force -Path $destination | Out-Null
        Copy-Item -LiteralPath (Join-Path $targetDir "$($target.Triple)\release\libroamling_android.so") -Destination $destination -Force
    }
    foreach ($abi in @('arm64-v8a', 'x86_64')) {
        if (-not (Test-Path (Join-Path $native "$abi\libroamling_android.so"))) { throw "Missing $abi library." }
    }
    foreach ($component in @('roamling_core', 'roamling_android')) {
        if (-not (Test-Path (Join-Path $generated "uniffi\$component\$component.kt"))) {
            throw "Missing $component Kotlin bindings."
        }
    }
    Write-Host 'Android libraries and both Kotlin binding components are ready.'
} finally {
    Pop-Location
}
