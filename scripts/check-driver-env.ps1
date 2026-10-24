#requires -Version 5.1
<#
.SYNOPSIS
  Checks whether this machine can build and test the Audiora kernel driver.
  Read-only: it installs nothing and changes no settings.
#>
$ErrorActionPreference = "SilentlyContinue"
$ok = $true
function Check($name, $pass, $fix) {
    if ($pass) { Write-Host ("[ok]      " + $name) -ForegroundColor Green }
    else { Write-Host ("[missing] " + $name + "  ->  " + $fix) -ForegroundColor Yellow; $script:ok = $false }
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$vs = if (Test-Path $vswhere) { & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath } else { $null }
Check "Visual Studio 2022 with 'Desktop development with C++'" ([bool]$vs) "install VS 2022 (Community is enough) and tick the C++ workload"

$kits = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\Include"
$wdk = Get-ChildItem $kits -Directory | Where-Object { Test-Path (Join-Path $_.FullName "km\wdm.h") } | Select-Object -First 1
Check "Windows Driver Kit (kernel-mode headers)" ([bool]$wdk) "install the WDK matching your Windows SDK: https://learn.microsoft.com/windows-hardware/drivers/download-the-wdk"

$ts = (bcdedit /enum '{current}' 2>$null | Select-String "testsigning\s+Yes")
Check "Test-signing enabled (only on the dedicated test machine!)" ([bool]$ts) "on the TEST machine: bcdedit /set testsigning on, then reboot (needs Secure Boot off)"

$vm = (Get-CimInstance Win32_ComputerSystem).Model
Write-Host ("Machine model: " + $vm)
if ($vm -notmatch "Virtual|VMware|VirtualBox|KVM") {
    Write-Host "This looks like a physical machine. Load unsigned/test drivers only on a spare PC or a VM." -ForegroundColor Yellow
}
if ($ok) { Write-Host "Ready to build the driver." -ForegroundColor Green } else { exit 1 }
