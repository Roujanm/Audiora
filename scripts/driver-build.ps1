#requires -Version 5.1
<#
.SYNOPSIS
  Builds the Audiora Virtual Audio driver. Installs nothing.
.DESCRIPTION
  Uses Microsoft's WDK and SDK NuGet packages (downloaded once into drivers\virtual-audio\packages,
  ignored by git) and the C++ compiler of Visual Studio 2022 / Build Tools. No WDK installation,
  no administrator rights, no Visual Studio driver extension: the compiler and linker are called
  with the same options the WDK's MSBuild files use for a KMDF kernel driver.

  Safety gates, in order; nothing is packaged unless all of them pass:
    1. scripts\driver-test.ps1: the mic cable's tests (user mode, real compiler).
    2. The kernel build at warning level 4 with warnings as errors.
    3. MSVC code analysis (/analyze) with the WDK's driver rules (IRQL, paging, locks).

  Output: dist\driver\  (AudioraVA.sys, AudioraVA.inf, AudioraVA.pdb)
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\driver-build.ps1
#>
param([switch]$SkipAnalysis)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$drv = Join-Path $root "drivers\virtual-audio"
$src = Join-Path $drv "Source"
$obj = Join-Path $root "target\driver\obj"
$out = Join-Path $root "dist\driver"
$wdkVersion = "10.0.26100.6584"
$kit = "10.0.26100.0"
$kmdf = "1.15"

& (Join-Path $PSScriptRoot "driver-test.ps1")

# --- the kits (NuGet packages are zip files; fetched once) ----------------------------------
$pkgDir = Join-Path $drv "packages"
New-Item -ItemType Directory -Force $pkgDir | Out-Null
foreach ($id in "microsoft.windows.wdk.x64", "microsoft.windows.sdk.cpp", "microsoft.windows.sdk.cpp.x64") {
    $dir = Join-Path $pkgDir "$id.$wdkVersion"
    if (Test-Path $dir) { continue }
    $nupkg = "$dir.nupkg"
    Write-Host "Downloading $id $wdkVersion from nuget.org"
    Invoke-WebRequest "https://api.nuget.org/v3-flatcontainer/$id/$wdkVersion/$id.$wdkVersion.nupkg" -OutFile $nupkg -UseBasicParsing
    Expand-Archive $nupkg $dir
}
$wdk = Join-Path $pkgDir "microsoft.windows.wdk.x64.$wdkVersion\c"
$sdk = Join-Path $pkgDir "microsoft.windows.sdk.cpp.$wdkVersion\c"
$sdk64 = Join-Path $pkgDir "microsoft.windows.sdk.cpp.x64.$wdkVersion\c"

$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
$vs = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw "Visual Studio 2022 (or Build Tools) with the C++ workload is required" }
$vcvars = Join-Path $vs "VC\Auxiliary\Build\vcvars64.bat"

# --- options (mirroring WindowsDriver.KernelMode*.props, KMDF, x64) -------------------------
$inc = @("$src\Inc", "$src\Filters", "$src\Utilities", "$src\Main") | ForEach-Object { "/I`"$_`"" }
# Microsoft's headers are "external": warnings and code analysis apply to Audiora's code only
$inc += @(
    "$wdk\Include\$kit\km\crt", "$wdk\Include\$kit\km", "$wdk\Include\$kit\shared", "$wdk\Include\wdf\kmdf\$kmdf",
    "$sdk\Include\$kit\shared"
) | ForEach-Object { "/external:I`"$_`"" }
$inc += "/external:W0", "/analyze:external-"

$defines = @(
    "_WIN64", "_AMD64_", "AMD64",
    "_WIN32_WINNT=0x0A00", "WINVER=0x0A00", "WINNT=1", "NTDDI_VERSION=0x0A00000B",   # Windows 11 21H2+
    "DEPRECATE_DDK_FUNCTIONS=1", "MSC_NOOPT", "POOL_NX_OPTIN=1",
    "KMDF_VERSION_MAJOR=1", "KMDF_VERSION_MINOR=15",
    "_WIN32", "UNICODE", "_UNICODE", "PC_IMPLEMENTATION", "DEBUG_LEVEL=DEBUGLVL_TERSE",
    "_USE_WAVERT_", "_USE_IPortClsRuntimePower", "_NEW_DELETE_OPERATORS_", "NDEBUG"
) | ForEach-Object { "/D$_" }

$cflags = @(
    "/nologo", "/c", "/kernel", "/W4", "/WX", "/Zi", "/O2", "/Oi", "/GS", "/Gy", "/Gz", "/GR-",
    "/Zp8", "/Zc:wchar_t-", "/Zc:forScope", "/Zc:inline", "/fp:precise", "/std:c++17",
    "/d1import_no_registry", "/d2AllowCompatibleILVersions", "/d2Zi+", "/d1nodatetime",
    "/GF", "/cbstring", "/d2epilogunwind", "/guard:cf",
    "/wd4603", "/wd4627", "/wd4986", "/wd4987", "/wd4748", "/wd4324"
)
if (-not $SkipAnalysis) {
    # the WDK's driver rules on top of the standard ones
    $cflags += "/analyze", "/analyze:plugin`"$wdk\bin\$kit\x64\DRIVERS.dll`""
}

$sources = @(
    Get-ChildItem "$src\Main\*.cpp"
    Get-ChildItem "$src\Filters\*.cpp"
    Get-ChildItem "$src\Utilities\*.cpp"
)

$kmLib = "$wdk\Lib\$kit\km\x64"
$libs = @(
    "$kmLib\BufferOverflowFastFailK.lib", "$kmLib\ntoskrnl.lib", "$kmLib\hal.lib", "$kmLib\wmilib.lib",
    "$wdk\Lib\wdf\kmdf\x64\$kmdf\WdfLdr.lib", "$wdk\Lib\wdf\kmdf\x64\$kmdf\WdfDriverEntry.lib",
    "$kmLib\portcls.lib", "$kmLib\stdunk.lib", "$kmLib\libcntpr.lib", "$kmLib\ntstrsafe.lib", "$kmLib\ksguid.lib"
) | ForEach-Object { "`"$_`"" }

$lflags = @(
    "/nologo", "/DRIVER", "/KERNEL", "/SUBSYSTEM:NATIVE,10.00", "/ENTRY:FxDriverEntry", "/NODEFAULTLIB",
    "/RELEASE", "/DEBUG", "/OPT:REF", "/OPT:ICF", "/MERGE:_TEXT=.text", "/MERGE:_PAGE=PAGE", "/SECTION:INIT,d",
    "/INTEGRITYCHECK", "/guard:cf", "/osversion:10.0", "/debugtype:pdata", "/WX",
    "/IGNORE:4198,4010,4037,4039,4065,4070,4078,4087,4089,4221,4108,4088,4218,4235"
)

Remove-Item -Recurse -Force $obj, $out -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $obj, $out | Out-Null

$rsp = Join-Path $obj "cl.rsp"
($cflags + $defines + $inc + @("/Fo`"$obj\\`"", "/Fd`"$obj\AudioraVA.c.pdb`"") + ($sources | ForEach-Object { "`"$($_.FullName)`"" })) -join "`n" |
    Set-Content $rsp -Encoding ASCII
$rcInc = "/I`"$sdk\Include\$kit\shared`" /I`"$sdk\Include\$kit\um`" /I`"$wdk\Include\$kit\km`""
$lrsp = Join-Path $obj "link.rsp"
($lflags + @("/OUT:`"$out\AudioraVA.sys`"", "/PDB:`"$out\AudioraVA.pdb`"", "`"$obj\*.obj`"", "`"$obj\AudioraVA.res`"") + $libs) -join "`n" |
    Set-Content $lrsp -Encoding ASCII

Write-Host "Compiling Audiora Virtual Audio ($($sources.Count) files)" -ForegroundColor Cyan
cmd /c "`"$vcvars`" >nul 2>nul && cl @`"$rsp`""
if ($LASTEXITCODE -ne 0) { throw "driver compile failed" }
cmd /c "`"$vcvars`" >nul 2>nul && rc /nologo $rcInc /fo `"$obj\AudioraVA.res`" `"$src\Main\AudioraVA.rc`""
if ($LASTEXITCODE -ne 0) { throw "resource compile failed" }
cmd /c "`"$vcvars`" >nul 2>nul && link @`"$lrsp`""
if ($LASTEXITCODE -ne 0) { throw "driver link failed" }

# --- INF: generated from one table, then stamped with version and KMDF version --------------
python (Join-Path $drv "tools\make_inx.py")
if ($LASTEXITCODE -ne 0) { throw "INF generation failed" }
$inf = Join-Path $out "AudioraVA.inf"
Copy-Item "$src\Main\AudioraVA.inx" $inf -Force
& "$wdk\bin\$kit\x64\stampinf.exe" -f $inf -a amd64 -k $kmdf -v "0.1.1.0" -d "*" 1>$null
if ($LASTEXITCODE -ne 0) { throw "stampinf failed" }

$infverif = "$wdk\tools\$kit\x64\infverif.exe"
& $infverif /v $inf | Out-Null
if ($LASTEXITCODE -ne 0) { throw "InfVerif rejected AudioraVA.inf" }

# --- test signing ----------------------------------------------------------------------------
# A self-signed "Audiora Test Driver Signing" certificate kept as a file next to the update key
# (%USERPROFILE%\.audiora, never committed). It is created once and removed from the certificate
# store right away: building trusts nothing and changes no settings. Only a machine that has
# test signing on and trusts AudioraVA-test.cer (driver-install.ps1 does that) loads the driver.
$keyDir = Join-Path $env:USERPROFILE ".audiora"
$pfx = Join-Path $keyDir "driver-test.pfx"
$pfxPass = ConvertTo-SecureString "audiora-test-signing" -AsPlainText -Force
if (-not (Test-Path $pfx)) {
    New-Item -ItemType Directory -Force $keyDir | Out-Null
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=Audiora Test Driver Signing" `
        -CertStoreLocation Cert:\CurrentUser\My -KeyExportPolicy Exportable -HashAlgorithm SHA256 -NotAfter (Get-Date).AddYears(3)
    Export-PfxCertificate -Cert $cert -FilePath $pfx -Password $pfxPass | Out-Null
    Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)"
}
$pub = (New-Object System.Security.Cryptography.X509Certificates.X509Certificate2($pfx, "audiora-test-signing")).Export("Cert")
[IO.File]::WriteAllBytes((Join-Path $out "AudioraVA-test.cer"), $pub)

$signtool = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\signtool.exe" | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $signtool) { throw "signtool.exe not found (Windows SDK)" }
$sign = { param($file) & $signtool.FullName sign /q /fd SHA256 /f $pfx /p "audiora-test-signing" $file; if ($LASTEXITCODE -ne 0) { throw "signing $file failed" } }

& $sign "$out\AudioraVA.sys"
# the catalog lists the package's files; PnP checks it against the signature at install time
& "$wdk\bin\$kit\x86\Inf2Cat.exe" /driver:$out /os:10_CO_X64 /uselocaltime | Out-Null
if ($LASTEXITCODE -ne 0) { throw "Inf2Cat failed" }
& $sign "$out\AudioraVA.cat"

Copy-Item (Join-Path $PSScriptRoot "driver-install.ps1") $out -Force

Write-Host ""
Write-Host "Driver built and test-signed: $out" -ForegroundColor Green
Get-ChildItem $out | ForEach-Object { Write-Host ("  {0,-26} {1,10:N0} bytes" -f $_.Name, $_.Length) }
