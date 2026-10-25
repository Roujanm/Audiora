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
  .\install-user.ps1 -Shortcuts
  .\install-user.ps1 -Autostart
  .\install-user.ps1 -Uninstall
#>
param(
    [switch]$Autostart,
    [switch]$Shortcuts,
    [switch]$Uninstall,
    # Folder holding audiora-svc.exe, audiora.exe and audiora-app.exe (default: next to this script).
    [string]$Source = $PSScriptRoot
)
$ErrorActionPreference = "Stop"
$target = Join-Path $env:LOCALAPPDATA "Roujan\Audiora"
$runKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run"
$valueName = "Audiora Engine"
$links = @(
    (Join-Path ([Environment]::GetFolderPath("Desktop")) "Audiora.lnk"),
    (Join-Path ([Environment]::GetFolderPath("Programs")) "Audiora.lnk")
)

if ($Uninstall) {
    Get-Process audiora-svc -ErrorAction SilentlyContinue | Stop-Process
    Get-Process audiora-app -ErrorAction SilentlyContinue | Stop-Process
    Remove-ItemProperty -Path $runKey -Name $valueName -ErrorAction SilentlyContinue
    $links | Where-Object { Test-Path $_ } | Remove-Item -Force
    if (Test-Path $target) { Remove-Item $target -Recurse -Force }
    Write-Host "Audiora removed. Presets in $env:APPDATA\Roujan\Audiora and recordings were kept."
    return
}

New-Item -ItemType Directory -Force -Path $target | Out-Null
# A running copy would keep the files locked; the app asks its engine to stop when it closes.
Get-Process audiora-app -ErrorAction SilentlyContinue | Stop-Process
Start-Sleep -Milliseconds 500
Get-Process audiora-svc -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$target*" } | Stop-Process
Copy-Item (Join-Path $Source "audiora-svc.exe") $target -Force
Copy-Item (Join-Path $Source "audiora.exe") $target -Force
$app = Join-Path $target "audiora-app.exe"
if (Test-Path (Join-Path $Source "audiora-app.exe")) {
    Copy-Item (Join-Path $Source "audiora-app.exe") $target -Force
}

if ($Shortcuts -and (Test-Path $app)) {
    $shell = New-Object -ComObject WScript.Shell
    foreach ($path in $links) {
        $link = $shell.CreateShortcut($path)
        $link.TargetPath = $app
        $link.WorkingDirectory = $target
        $link.IconLocation = "$app,0"
        $link.Description = "Audiora by Roujan Digital Agency"
        $link.Save()
    }
    Write-Host "Shortcuts created on the desktop and in the Start menu."
}

if ($Autostart) {
    $cmd = '"' + (Join-Path $target "audiora-svc.exe") + '"'
    Set-ItemProperty -Path $runKey -Name $valueName -Value $cmd
    Write-Host "Engine will start at sign-in."
}
Write-Host "Installed to $target"
