#requires -Version 5.1
<#
.SYNOPSIS
  Builds the Audiora Virtual Audio driver (test-signed) and collects what the test VM needs.
.DESCRIPTION
  Needs Visual Studio 2022 (C++ workload) and the Windows Driver Kit (run check-driver-env.ps1).
  Builds drivers\virtual-audio\AudioraVirtualAudio.sln for Release|x64. The WDK signs the package
  with its test certificate ("WDKTestCert <user>"), which this script exports next to the driver,
  so the VM can trust it. Nothing is installed on this machine.

  Safety gates, in order; the package is only produced when all of them pass:
    1. scripts\driver-test.ps1: the mic cable's tests (user mode, real compiler).
    2. The build itself with the WDK's code analysis (driver rules: IRQL, paging, locks).
    3. With -Sdv: Static Driver Verifier.

  Output: dist\driver\  (AudioraVA.inf, AudioraVA.sys, audiorava.cat, AudioraVA-test.cer,
  driver-vm-install.ps1). Copy that folder into the VM and run driver-vm-install.ps1 there.
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\driver-build.ps1
#>
param(
    [ValidateSet("Release", "Debug")][string]$Configuration = "Release",
    # also run Static Driver Verifier (slow: checks IRQL, locking and PnP rules on every code path)
    [switch]$Sdv
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$sln = Join-Path $root "drivers\virtual-audio\AudioraVirtualAudio.sln"

# the INF is generated from one table so the six endpoints stay consistent
python (Join-Path $root "drivers\virtual-audio\tools\make_inx.py")
if ($LASTEXITCODE -ne 0) { throw "INF generation failed" }

& (Join-Path $PSScriptRoot "driver-test.ps1")

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$msbuild = & $vswhere -latest -products * -requires Microsoft.Component.MSBuild -find "MSBuild\**\Bin\MSBuild.exe" | Select-Object -First 1
if (-not $msbuild) { throw "MSBuild not found: install Visual Studio 2022 with the C++ workload" }

Write-Host "Building Audiora Virtual Audio ($Configuration|x64)" -ForegroundColor Cyan
# code analysis with the driver rule set; its warnings fail the build
& $msbuild $sln /m /nologo /v:minimal "/p:Configuration=$Configuration" /p:Platform=x64 /p:RunCodeAnalysis=true /p:CodeAnalysisTreatWarningsAsErrors=true
if ($LASTEXITCODE -ne 0) { throw "driver build or code analysis failed (is the WDK installed? run scripts\check-driver-env.ps1)" }

if ($Sdv) {
    $main = Join-Path $root "drivers\virtual-audio\Source\Main\Main.vcxproj"
    & $msbuild $main /nologo /t:sdv "/p:Inputs=/check:default.sdv" "/p:Configuration=$Configuration" /p:Platform=x64
    if ($LASTEXITCODE -ne 0) { throw "Static Driver Verifier failed: see the SDV report in drivers\virtual-audio\Source\Main" }
}

$pkg = Get-ChildItem (Join-Path $root "drivers\virtual-audio") -Recurse -Directory -Filter package |
    Where-Object { $_.FullName -match "\\x64\\$Configuration\\" -and (Test-Path (Join-Path $_.FullName "AudioraVA.inf")) } |
    Select-Object -First 1
if (-not $pkg) { throw "built driver package not found" }

$out = Join-Path $root "dist\driver"
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item (Join-Path $pkg.FullName "*") $out -Force
Copy-Item (Join-Path $PSScriptRoot "driver-vm-install.ps1") $out -Force

$cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object { $_.Subject -like "CN=WDKTestCert*" } |
    Sort-Object NotAfter -Descending | Select-Object -First 1
if ($cert) {
    Export-Certificate -Cert $cert -FilePath (Join-Path $out "AudioraVA-test.cer") | Out-Null
} else {
    Write-Host "WDK test certificate not found in CurrentUser\My; the VM will not trust the package." -ForegroundColor Yellow
}

Write-Host ""
Write-Host "Driver package ready in $out" -ForegroundColor Green
Get-ChildItem $out | ForEach-Object { Write-Host ("  " + $_.Name) }
Write-Host "Copy the folder into the TEST VM and run driver-vm-install.ps1 there (as administrator)."
