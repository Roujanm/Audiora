#requires -Version 5.1
<#
.SYNOPSIS
  Builds Audiora in release mode and assembles a portable distribution folder + zip.
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\package.ps1
#>
param(
    [string]$Out = "dist"
)
$ErrorActionPreference = "Stop"
Set-Location (Split-Path -Parent $PSScriptRoot)

cargo build --release --workspace
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$version = (Select-String -Path Cargo.toml -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
$name = "Audiora-$version-win-x64"
$stage = Join-Path $Out $name
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage | Out-Null

Copy-Item target\release\audiora-svc.exe $stage
Copy-Item target\release\audiora.exe $stage
Copy-Item README.md $stage
Copy-Item docs $stage -Recurse
Copy-Item scripts\install-user.ps1 $stage
if (Test-Path app\src-tauri\target\release\audiora-app.exe) {
    Copy-Item app\src-tauri\target\release\audiora-app.exe $stage
}

$zip = Join-Path $Out "$name.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path $stage\* -DestinationPath $zip

$hash = (Get-FileHash $zip -Algorithm SHA256).Hash
"$hash  $name.zip" | Set-Content (Join-Path $Out "$name.zip.sha256") -Encoding ascii
Write-Host "Created $zip"
Write-Host "SHA256  $hash"
