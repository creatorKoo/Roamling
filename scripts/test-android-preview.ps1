# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
# Only the explicitly named emulator is changed. Restore its overlay app-op.
param([Parameter(Mandatory = $true)][string]$Serial)
$ErrorActionPreference = 'Stop'
if ($Serial -notmatch '^emulator-\d+$') { throw 'Name an emulator explicitly.' }
$root = Split-Path -Parent $PSScriptRoot
$sdk = $env:ANDROID_HOME
if (-not $sdk) { $sdk = [Environment]::GetEnvironmentVariable('ANDROID_HOME', 'User') }
if (-not $sdk) { throw 'Set ANDROID_HOME first.' }
$adb = Join-Path $sdk 'platform-tools\adb.exe'
$apk = Join-Path $root 'android\app\build\outputs\apk\debug\app-debug.apk'
$testApk = Join-Path $root 'android\app\build\outputs\apk\androidTest\debug\app-debug-androidTest.apk'
foreach ($file in @($adb, $apk, $testApk)) {
    if (-not (Test-Path -LiteralPath $file)) { throw "Missing $file. Build assembleDebug and assembleDebugAndroidTest first." }
}
$boot = & $adb -s $Serial shell getprop sys.boot_completed
if ($LASTEXITCODE -ne 0 -or $boot.Trim() -ne '1') { throw 'The named emulator is not booted.' }
foreach ($file in @($apk, $testApk)) {
    & $adb -s $Serial install -r $file
    if ($LASTEXITCODE -ne 0) { throw 'APK installation failed.' }
}
$package = 'io.github.creatorkoo.roamling'
$before = (& $adb -s $Serial shell cmd appops get $package SYSTEM_ALERT_WINDOW) -join "`n"
if ($LASTEXITCODE -ne 0) { throw 'Cannot read the existing overlay permission.' }
$mode = if ($before -match 'SYSTEM_ALERT_WINDOW:\s*(allow|deny|ignore|default|foreground)') {
    $Matches[1]
} elseif ($before -match 'No operations') { 'default' } else { throw 'Unknown overlay permission response.' }
try {
    & $adb -s $Serial shell cmd appops set $package SYSTEM_ALERT_WINDOW allow
    if ($LASTEXITCODE -ne 0) { throw 'Could not grant the test permission.' }
    $result = & $adb -s $Serial shell am instrument -w -r -e class io.github.creatorkoo.roamling.PreviewTest "$package.test/androidx.test.runner.AndroidJUnitRunner"
    $exitCode = $LASTEXITCODE
    $result | Write-Output
    if ($exitCode -ne 0 -or ($result -join "`n") -notmatch 'OK \([1-9][0-9]* tests?\)') {
        throw 'A1 instrumentation did not pass.'
    }
    $outputDir = Join-Path $root 'output\android-setup'
    New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
    foreach ($name in @('a1-preview.png', 'a1-preview-landscape.png')) {
        & $adb -s $Serial pull "/sdcard/Android/data/$package/files/$name" (Join-Path $outputDir $name)
        if ($LASTEXITCODE -ne 0) { throw 'Preview screenshot was not produced.' }
    }
} finally {
    & $adb -s $Serial shell am force-stop $package
    if ($LASTEXITCODE -ne 0) { Write-Warning 'Could not stop the test app.' }
    & $adb -s $Serial shell cmd appops set $package SYSTEM_ALERT_WINDOW $mode
    if ($LASTEXITCODE -ne 0) { throw 'Could not restore the emulator overlay permission.' }
}
