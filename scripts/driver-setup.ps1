#requires -Version 5.1
<#
.SYNOPSIS
  Run by the Audiora installer after it copied the app: installs or updates the Audiora Virtual
  Audio driver when this PC can load it, and otherwise does nothing.
.DESCRIPTION
  Runs without administrator rights and decides first, so an update of the app does not ask for
  anything when the driver is already current:
    1. Test signing off (the normal, secure state of Windows)  -> nothing to do: the test-signed
       driver could not load. Audiora works without it; other programs then hear the raw mic.
    2. The installed driver has the bundled version and works   -> nothing to do.
    3. Otherwise it starts driver-install.ps1 elevated (one Windows prompt) and waits for it.
  Exit code 0 in every case: the app installation never fails because of the driver.
#>
$here = $PSScriptRoot
$log = Join-Path $env:LOCALAPPDATA "Audiora\logs\driver-setup.log"
New-Item -ItemType Directory -Force (Split-Path $log) | Out-Null
function Note($text) { Add-Content -Path $log -Value ("{0:yyyy-MM-dd HH:mm:ss} {1}" -f (Get-Date), $text) }

try {
    # test signing shows in the boot options every program can read
    $boot = (Get-ItemProperty HKLM:\SYSTEM\CurrentControlSet\Control -ErrorAction Stop).SystemStartOptions
    if ($boot -notmatch "TESTSIGNING") {
        Note "test signing is off: the driver is not installed (Audiora runs without it)"
        exit 0
    }

    $inf = Join-Path $here "AudioraVA.inf"
    $bundled = (Select-String -Path $inf -Pattern "^\s*DriverVer\s*=\s*[^,]+,\s*(\S+)").Matches[0].Groups[1].Value
    $installed = Get-CimInstance Win32_PnPSignedDriver -Filter "DeviceName='Audiora Virtual Audio'" -ErrorAction SilentlyContinue | Select-Object -First 1
    $device = Get-PnpDevice -FriendlyName "Audiora Virtual Audio" -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($installed -and $installed.DriverVersion -eq $bundled -and $device.Status -eq "OK") {
        Note "driver $bundled already installed and working"
        exit 0
    }

    Note "installing driver $bundled (installed: $($installed.DriverVersion), device: $($device.Status))"
    $script = Join-Path $here "driver-install.ps1"
    $p = Start-Process powershell.exe -Verb RunAs -Wait -PassThru -WindowStyle Hidden `
        -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "`"$script`""
    Note "driver-install.ps1 finished with exit code $($p.ExitCode)"
} catch {
    # the Windows prompt was declined, or something else went wrong: the app is installed anyway
    Note "driver not installed: $($_.Exception.Message)"
}
exit 0
