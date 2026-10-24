#requires -Version 5.1
<#
.SYNOPSIS
  Per-user install of the Audiora engine: copies binaries and (optionally) starts it at sign-in.
.DESCRIPTION
  The engine runs in the user's session, not as a Windows service: services live in session 0
  and do not see the user's default audio devices or microphone permission. Installing needs
  no administrator rights. Nothing is changed outside %LOCALAPPDATA%\Roujan\Audiora and the
  current user's Run key.
.EXAMPLE
  .\install-user.ps1 -Autostart
  .\install-user.ps1 -Uninstall
#>
param(
    [switch]$Autostart,
    [switch]$Uninstall
)
$ErrorActionPreference = "Stop"
$target = Join-Path $env:LOCALAPPDATA "Roujan\Audiora"
$runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$valueName = "Audiora Engine"

if ($Uninstall) {
    Get-Process audiora-svc -ErrorAction SilentlyContinue | Stop-Process
    Remove-ItemProperty -Path $runKey -Name $valueName -ErrorAction SilentlyContinue
    if (Test-Path $target) { Remove-Item $target -Recurse -Force }
    Write-Host "Audiora removed. Presets in $env:APPDATA\Roujan\Audiora and recordings were kept."
    return
}

New-Item -ItemType Directory -Force -Path $target | Out-Null
Copy-Item (Join-Path $PSScriptRoot "audiora-svc.exe") $target -Force
Copy-Item (Join-Path $PSScriptRoot "audiora.exe") $target -Force
if (Test-Path (Join-Path $PSScriptRoot "audiora-app.exe")) {
    Copy-Item (Join-Path $PSScriptRoot "audiora-app.exe") $target -Force
}

if ($Autostart) {
    $cmd = '"' + (Join-Path $target "audiora-svc.exe") + '"'
    Set-ItemProperty -Path $runKey -Name $valueName -Value $cmd
    Write-Host "Engine will start at sign-in."
}
Write-Host "Installed to $target"
