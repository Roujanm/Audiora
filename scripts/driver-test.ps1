#requires -Version 5.1
<#
.SYNOPSIS
  Runs the Audiora Virtual Audio driver's user-mode tests. Needs only Visual Studio (no WDK) and
  installs nothing: the driver's own source files are compiled into an ordinary test program.
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\driver-test.ps1
#>
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$drv = Join-Path $root "drivers\virtual-audio"
$out = Join-Path $root "target\driver-tests"
New-Item -ItemType Directory -Force $out | Out-Null

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw "Visual Studio with the C++ workload not found" }
$vcvars = Join-Path $vs "VC\Auxiliary\Build\vcvars64.bat"

# /W4 /WX: the test build is as strict as the driver build; /analyze runs MSVC's code analysis
$cl = "cl /nologo /EHsc /std:c++17 /W4 /WX /analyze /wd28251 /I `"$drv\tests\cable`" /I `"$drv\Source\Main`" " +
      "`"$drv\tests\cable\cable_test.cpp`" `"$drv\Source\Main\cable.cpp`" /Fo`"$out\\`" /Fe`"$out\cable_test.exe`""
cmd /c "`"$vcvars`" >nul && $cl"
if ($LASTEXITCODE -ne 0) { throw "test build failed" }
& "$out\cable_test.exe"
if ($LASTEXITCODE -ne 0) { throw "cable tests failed" }
