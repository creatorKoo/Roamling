# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
# Builds/install nothing implicitly except the named APK on the named emulator.
param(
    [Parameter(Mandatory = $true)][string]$Serial,
    [string]$Apk = '',
    [ValidateRange(5, 120)][int]$TimeoutSeconds = 30
)
$ErrorActionPreference = 'Stop'
$repositoryDir = Split-Path -Parent $PSScriptRoot
if (-not $Apk) { $Apk = Join-Path $repositoryDir 'android\app\build\outputs\apk\debug\app-debug.apk' }
if (-not (Test-Path -LiteralPath $Apk)) { throw 'Build the debug APK first.' }
$sdkRoot = $env:ANDROID_HOME
if (-not $sdkRoot) { $sdkRoot = [Environment]::GetEnvironmentVariable('ANDROID_HOME', 'User') }
if (-not $sdkRoot) { throw 'Set ANDROID_HOME first.' }
$adb = Join-Path $sdkRoot 'platform-tools\adb.exe'
if (-not (Test-Path $adb)) { throw 'Android platform-tools are missing.' }
if ($Serial -notmatch '^emulator-\d+$') { throw 'This smoke runner only targets an explicitly named emulator.' }
$state = & $adb -s $Serial get-state
if ($LASTEXITCODE -ne 0 -or $state -ne 'device') { throw 'The named emulator is not ready.' }
$boot = & $adb -s $Serial shell getprop sys.boot_completed
if ($LASTEXITCODE -ne 0 -or $boot.Trim() -ne '1') { throw 'Wait for the emulator to finish booting.' }

& $adb -s $Serial install -r $Apk
if ($LASTEXITCODE -ne 0) { throw 'APK installation failed.' }
$token = [guid]::NewGuid().ToString('N')
& $adb -s $Serial shell am start -W -n io.github.creatorkoo.roamling/.CoreSmokeActivity --es smoke_token $token
if ($LASTEXITCODE -ne 0) { throw 'A0 activity launch failed.' }
$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
do {
    $lines = & $adb -s $Serial logcat -d -s 'RoamlingA0:I' '*:S'
    if ($LASTEXITCODE -ne 0) { throw 'Could not read the emulator log.' }
    if ($lines -match "A0 FAIL token=$token") {
        $lines | Write-Output
        throw 'Android -> UniFFI -> Rust smoke failed.'
    }
    $pass = $lines | Select-String -Pattern "A0 PASS token=$token x=\S+ y=\S+ state=\d+"
    if ($pass) { $pass.Line | Write-Output; exit 0 }
    Start-Sleep -Milliseconds 500
} while ([DateTime]::UtcNow -lt $deadline)
throw 'The current launch did not report a native tick before the timeout.'
