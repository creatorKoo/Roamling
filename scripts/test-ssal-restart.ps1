# SPDX-FileCopyrightText: 2026 GooBeom Jeoung
# SPDX-License-Identifier: GPL-3.0-only
# Run against a debug shell after building it; never reads the user's settings.
param([Parameter(Mandatory=$true)][string]$Exe)
$ErrorActionPreference='Stop'
$Exe=(Resolve-Path -LiteralPath $Exe).Path
$root=Split-Path -Parent $PSScriptRoot
$testRoot=Join-Path $root ('output/ssal-restart-'+[Guid]::NewGuid().ToString('N'))
$oldAppData=$env:APPDATA
$oldSmoke=$env:ROAMLING_SMOKE_TEST
$oldExpected=$env:ROAMLING_EXPECT_SSAL_PRESET
$oldSelected=$env:ROAMLING_SELECT_SSAL_PRESET
$oldPaletteSmoke=$env:ROAMLING_SSAL_PALETTE_SMOKE
function Run-Pet([string]$name,[string]$marker) {
    $log=Join-Path $testRoot ($name+'.log')
    $err=Join-Path $testRoot ($name+'.err')
    $p=Start-Process -FilePath $Exe -WindowStyle Hidden -PassThru -RedirectStandardOutput $log -RedirectStandardError $err
    $null=$p.Handle
    if(!$p.WaitForExit(45000)){Stop-Process -Id $p.Id;throw "$name timed out"}
    if($p.ExitCode -ne 0){Get-Content -LiteralPath $err -Tail 12;throw "$name failed: $($p.ExitCode)"}
    if($marker -and ![IO.File]::ReadAllText($log).Contains($marker)){throw "$name missing $marker"}
}
try {
    $env:APPDATA=Join-Path $testRoot 'appdata'
    $null=New-Item -ItemType Directory -Force (Join-Path $env:APPDATA 'Roamling')
    $settings=Join-Path $env:APPDATA 'Roamling/settings.txt'
    # The failing combination: default Ssal has NO palette key, Bori has a remembered colour.
    $seed="roamling.builtInPet=ssal`nroamling.autoUpdate=false`nroamling.palette=48,48,30,90,200;38,38,86,95,33;100,100,0,82,180`n"
    [IO.File]::WriteAllText($settings,$seed,[Text.UTF8Encoding]::new($false))
    $env:ROAMLING_SMOKE_TEST='1'
    $env:ROAMLING_EXPECT_SSAL_PRESET='0'
    $env:ROAMLING_SELECT_SSAL_PRESET=$null
    $env:ROAMLING_SSAL_PALETTE_SMOKE=$null
    Run-Pet 'initial-default' 'smoke.ssal_restart=PASS preset=0'
    $env:ROAMLING_EXPECT_SSAL_PRESET=$null
    $env:ROAMLING_SSAL_PALETTE_SMOKE='1'
    Run-Pet 'menu-switches' 'smoke.ssal_palette=PASS'
    $env:ROAMLING_SSAL_PALETTE_SMOKE=$null
    $env:ROAMLING_EXPECT_SSAL_PRESET='0'
    Run-Pet 'default-after-bori' 'smoke.ssal_restart=PASS preset=0'
    $bori=@(Get-Content $settings | Where-Object {$_ -match '^roamling\.palette='})
    foreach($index in 0..8) {
        $env:ROAMLING_EXPECT_SSAL_PRESET=$null
        $env:ROAMLING_SELECT_SSAL_PRESET=[string]$index
        Run-Pet "select-$index" ''
        $saved=[IO.File]::ReadAllText($settings)
        if($saved -notmatch '(?m)^roamling.builtInPet=ssal\r?$'){throw 'Lost Ssal selection'}
        if(($index -eq 0) -ne ($saved -notmatch '(?m)^roamling.ssalPalette=')){throw 'Incorrect default palette persistence'}
        if(($bori -join '') -ne (@(Get-Content $settings | Where-Object {$_ -match '^roamling\.palette='}) -join '')){throw 'Bori palette changed'}
        $env:ROAMLING_SELECT_SSAL_PRESET=$null
        $env:ROAMLING_EXPECT_SSAL_PRESET=[string]$index
        Run-Pet "restart-$index" "smoke.ssal_restart=PASS preset=$index"
        Write-Output "PASS Ssal $index selected, saved, restarted: pixels, tracks, name, menu; Bori preserved"
    }
    Write-Output "PASS all Ssal restart checks. Logs: $testRoot"
} finally {
    $env:APPDATA=$oldAppData
    $env:ROAMLING_SMOKE_TEST=$oldSmoke
    $env:ROAMLING_EXPECT_SSAL_PRESET=$oldExpected
    $env:ROAMLING_SELECT_SSAL_PRESET=$oldSelected
    $env:ROAMLING_SSAL_PALETTE_SMOKE=$oldPaletteSmoke
}
