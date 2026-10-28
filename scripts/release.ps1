#requires -Version 5.1
<#
.SYNOPSIS
  Builds a signed Audiora release: the Windows installer (engine included) and the update manifest.
.DESCRIPTION
  1. Bump "version" in app\src-tauri\tauri.conf.json first.
  2. Run this script. It builds the engine and the app, signs the installer with the update key
     (%USERPROFILE%\.audiora\updater.key, never committed) and writes latest.json.
  3. Publish: with -Publish the script creates the GitHub release v<version> itself (GitHub CLI
     `gh`, signed in once with `gh auth login`); otherwise create it on github.com and upload the
     two files the script prints. Audiora checks for updates when it opens (and every six hours)
     and offers the new version in a pop-up.
  The repository must be public for running copies of Audiora to reach the update.
  -Notes: one line per change (`n between lines); they are listed in the update pop-up.
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\release.ps1 -Notes "Music channel`nNew Setup page" -Publish
#>
param(
    [string]$Notes = "Improvements and fixes.",
    [string]$Repo = "Roujanm/Audiora",
    [switch]$Publish
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$conf = Get-Content app\src-tauri\tauri.conf.json -Raw | ConvertFrom-Json
$version = $conf.version
$key = Join-Path $env:USERPROFILE ".audiora\updater.key"
if (-not (Test-Path $key)) { throw "Update signing key not found at $key (create it with: npx tauri signer generate --ci -w $key)" }

Write-Host "Building Audiora $version" -ForegroundColor Cyan
cargo build --release -p audiora-svc -p audiora-cli
if ($LASTEXITCODE -ne 0) { throw "engine build failed" }

# the installer carries the engine next to the app (Tauri "external binary")
New-Item -ItemType Directory -Force app\src-tauri\binaries | Out-Null
Copy-Item target\release\audiora-svc.exe app\src-tauri\binaries\audiora-svc-x86_64-pc-windows-msvc.exe -Force

$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content $key -Raw
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
Push-Location app
try {
    npx tauri build
    if ($LASTEXITCODE -ne 0) { throw "app build failed" }
} finally {
    Pop-Location
    Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}

$nsis = "app\src-tauri\target\release\bundle\nsis"
$setup = Get-ChildItem $nsis -Filter "*_$version*-setup.exe" | Select-Object -First 1
if (-not $setup) { throw "installer for $version not found in $nsis" }
$sig = Get-Content "$($setup.FullName).sig" -Raw

$out = Join-Path $root "dist\release-v$version"
New-Item -ItemType Directory -Force $out | Out-Null
Copy-Item $setup.FullName $out -Force
$manifest = [ordered]@{
    version   = $version
    notes     = $Notes
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            signature = $sig.Trim()
            url       = "https://github.com/$Repo/releases/download/v$version/$($setup.Name)"
        }
    }
}
# UTF-8 without a byte-order mark: the updater's JSON parser rejects one
[IO.File]::WriteAllText((Join-Path $out "latest.json"), ($manifest | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding($false)))

Write-Host ""
Write-Host "Release ready in $out" -ForegroundColor Green
Write-Host "  $($setup.Name)   (installer, also the update package)"
Write-Host "  latest.json      (update manifest)"
if ($Publish) {
    $gh = Get-Command gh -ErrorAction SilentlyContinue
    if (-not $gh) { throw "GitHub CLI not found: install it (winget install GitHub.cli), run 'gh auth login' once, then run with -Publish again" }
    & gh release create "v$version" (Join-Path $out $setup.Name) (Join-Path $out "latest.json") --repo $Repo --title "Audiora $version" --notes $Notes
    if ($LASTEXITCODE -ne 0) { throw "publishing the release failed" }
    Write-Host "Published: https://github.com/$Repo/releases/tag/v$version" -ForegroundColor Green
} else {
    Write-Host "Publish: create release v$version on https://github.com/$Repo/releases and upload both files (or run again with -Publish)."
}
