# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
#
# Stop, rebuild, start. Roamling is meant to be running all the time, and a
# running copy holds its own exe open -- `cargo build` fails with "access is
# denied" until it is closed. So the three steps belong together.
#
#   .\scripts\run.ps1            release, tray only (the daily build)
#   .\scripts\run.ps1 -Debug     keeps a console for the state log

param([switch]$Debug)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

$machine = [Environment]::GetEnvironmentVariable("Path", "Machine")
$user = [Environment]::GetEnvironmentVariable("Path", "User")
$env:Path = "$machine;$user"

$running = Get-Process -Name roamling -ErrorAction SilentlyContinue
if ($running) {
    foreach ($process in $running) {
        Write-Host "stopping the running pet (pid $($process.Id))" -ForegroundColor DarkGray
        $process.Kill()
    }
    # The file lock outlives the process by a moment.
    $process.WaitForExit(5000) | Out-Null
    Start-Sleep -Milliseconds 300
}

Set-Location (Join-Path $root "rust")
$profileName = if ($Debug) { "debug" } else { "release" }
Write-Host "building $profileName" -ForegroundColor Cyan
if ($Debug) { cargo build -p roamling-win } else { cargo build -p roamling-win --release }
if ($LASTEXITCODE -ne 0) {
    Write-Host "build failed; the pet stays down" -ForegroundColor Red
    exit $LASTEXITCODE
}

$exe = Join-Path $root "rust\target\$profileName\roamling.exe"
Start-Process -FilePath $exe -WorkingDirectory (Split-Path $exe)
Start-Sleep -Milliseconds 800
$started = Get-Process -Name roamling -ErrorAction SilentlyContinue
if ($started) {
    Write-Host "roaming again (pid $($started.Id))" -ForegroundColor Green
} else {
    Write-Host "it did not come back up" -ForegroundColor Red
    exit 1
}
