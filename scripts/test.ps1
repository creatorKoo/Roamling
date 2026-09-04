# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
#
# The Windows counterpart of scripts/test.sh, which is zsh and drives SwiftPM.
# There is no Swift here: the Windows build is Rust all the way down, so this
# is cargo and nothing else. See docs/windows.md, W4.
#
# Exits non-zero on the first failure, same contract as test.sh.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location (Join-Path $root "rust")

# A session that started before rustup was installed will not have it on PATH.
$machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
$user = [Environment]::GetEnvironmentVariable("Path", "User")
$env:Path = "$machine;$user"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "cargo is not on PATH. Install the Rust toolchain first." -ForegroundColor Red
    exit 1
}

function Step($name, [scriptblock] $body) {
    Write-Host "`n=== $name ===" -ForegroundColor Cyan
    & $body
    if ($LASTEXITCODE -ne 0) {
        Write-Host "$name failed" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

# The core's differential fixtures and the pet's sheet contract. This is the
# workspace default set, so it is the same command the macOS test.sh runs --
# roamling-win is excluded there because the `windows` crate cannot build on
# macOS. See rust/Cargo.toml.
Step "core + pet" { cargo test --release }

# The shell only builds on Windows, so it is named explicitly.
Step "shell tests" { cargo test -p roamling-win }

# A running copy holds its own exe open, and the release build ends by writing
# over that exe -- it would fail with "access is denied" every time the pet is
# up, which is meant to be always. So the pet steps aside and comes back. If it
# was not running, nothing is started: a test run should not launch anything.
$wasRunning = @(Get-Process -Name roamling -ErrorAction SilentlyContinue)
if ($wasRunning) {
    Write-Host "`nstopping the running pet so the exe can be written" -ForegroundColor DarkGray
    $wasRunning | ForEach-Object { $_.Kill() }
    $wasRunning[0].WaitForExit(5000) | Out-Null
    Start-Sleep -Milliseconds 300
}
Step "shell release build" { cargo build -p roamling-win --release }
if ($wasRunning) {
    $exe = Join-Path $root "rust\target\release\roamling.exe"
    Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe)
    Write-Host "the pet is back up" -ForegroundColor DarkGray
}

Write-Host "`nall green" -ForegroundColor Green
Write-Host "  cargo run -p roamling-win            # debug, keeps a console for the state log"
Write-Host "  .\rust\target\release\roamling.exe   # release, tray only"
