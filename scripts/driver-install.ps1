#requires -Version 5.1
#requires -RunAsAdministrator
<#
.SYNOPSIS
  Installs (or removes) the test-signed Audiora Virtual Audio driver.
.DESCRIPTION
  Run from the folder driver-build.ps1 produced (dist\driver), in an elevated PowerShell. It
    1. refuses to run unless test signing is on (that switch is yours to flip, see below),
    2. trusts the Audiora test certificate (LocalMachine Root + TrustedPublisher),
    3. adds the driver package to the driver store and creates the ROOT\AudioraVirtualAudio device.
  -Uninstall removes the device, the driver package and the test certificate again.

  Before the first install: create a System Restore point. Test signing (then restart; Secure Boot
  must be off in the firmware settings, which also stops some anti-cheat games until it is back on):
      bcdedit /set testsigning on
  To go back afterwards: run -Uninstall, then  bcdedit /set testsigning off  and restart.
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\driver-install.ps1
  powershell -ExecutionPolicy Bypass -File .\driver-install.ps1 -Uninstall
#>
param([switch]$Uninstall)
$ErrorActionPreference = "Stop"
$here = $PSScriptRoot
$hwid = "ROOT\AudioraVirtualAudio"

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class AudioraDevice
{
    const int DIF_REGISTERDEVICE = 0x19;
    const int DIF_REMOVE = 0x05;
    const int DICD_GENERATE_ID = 0x1;
    const int SPDRP_HARDWAREID = 0x1;
    const int DIGCF_ALLCLASSES = 0x4;
    const int INSTALLFLAG_FORCE = 0x1;

    [StructLayout(LayoutKind.Sequential)]
    struct SP_DEVINFO_DATA { public int cbSize; public Guid ClassGuid; public int DevInst; public IntPtr Reserved; }

    [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern IntPtr SetupDiCreateDeviceInfoList(ref Guid classGuid, IntPtr hwnd);
    [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern IntPtr SetupDiGetClassDevs(IntPtr classGuid, IntPtr enumerator, IntPtr hwnd, int flags);
    [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool SetupDiCreateDeviceInfo(IntPtr set, string name, ref Guid classGuid, string desc, IntPtr hwnd, int flags, ref SP_DEVINFO_DATA data);
    [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool SetupDiSetDeviceRegistryProperty(IntPtr set, ref SP_DEVINFO_DATA data, int prop, byte[] buf, int size);
    [DllImport("setupapi.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool SetupDiGetDeviceRegistryProperty(IntPtr set, ref SP_DEVINFO_DATA data, int prop, out int type, byte[] buf, int size, out int required);
    [DllImport("setupapi.dll", SetLastError = true)]
    static extern bool SetupDiCallClassInstaller(int fn, IntPtr set, ref SP_DEVINFO_DATA data);
    [DllImport("setupapi.dll", SetLastError = true)]
    static extern bool SetupDiEnumDeviceInfo(IntPtr set, int index, ref SP_DEVINFO_DATA data);
    [DllImport("setupapi.dll", SetLastError = true)]
    static extern bool SetupDiDestroyDeviceInfoList(IntPtr set);
    [DllImport("newdev.dll", SetLastError = true, CharSet = CharSet.Unicode)]
    static extern bool UpdateDriverForPlugAndPlayDevices(IntPtr hwnd, string hwid, string inf, int flags, out bool reboot);

    static readonly Guid MediaClass = new Guid("4d36e96c-e325-11ce-bfc1-08002be10318");

    /// Creates the root-enumerated device (what "devcon install" does) and binds the INF to it.
    public static bool Install(string inf, string hwid)
    {
        Guid cls = MediaClass;
        IntPtr set = SetupDiCreateDeviceInfoList(ref cls, IntPtr.Zero);
        if (set == new IntPtr(-1)) throw new System.ComponentModel.Win32Exception();
        try
        {
            var data = new SP_DEVINFO_DATA { cbSize = Marshal.SizeOf(typeof(SP_DEVINFO_DATA)) };
            if (!SetupDiCreateDeviceInfo(set, "MEDIA", ref cls, null, IntPtr.Zero, DICD_GENERATE_ID, ref data))
                throw new System.ComponentModel.Win32Exception();
            byte[] ids = System.Text.Encoding.Unicode.GetBytes(hwid + "\0\0");
            if (!SetupDiSetDeviceRegistryProperty(set, ref data, SPDRP_HARDWAREID, ids, ids.Length))
                throw new System.ComponentModel.Win32Exception();
            if (!SetupDiCallClassInstaller(DIF_REGISTERDEVICE, set, ref data))
                throw new System.ComponentModel.Win32Exception();
        }
        finally { SetupDiDestroyDeviceInfoList(set); }
        bool reboot;
        if (!UpdateDriverForPlugAndPlayDevices(IntPtr.Zero, hwid, inf, INSTALLFLAG_FORCE, out reboot))
            throw new System.ComponentModel.Win32Exception();
        return reboot;
    }

    /// Removes every device with this hardware id. Returns how many were removed.
    public static int Remove(string hwid)
    {
        IntPtr set = SetupDiGetClassDevs(IntPtr.Zero, IntPtr.Zero, IntPtr.Zero, DIGCF_ALLCLASSES);
        if (set == new IntPtr(-1)) throw new System.ComponentModel.Win32Exception();
        int removed = 0;
        try
        {
            var data = new SP_DEVINFO_DATA { cbSize = Marshal.SizeOf(typeof(SP_DEVINFO_DATA)) };
            byte[] buf = new byte[4096];
            for (int i = 0; SetupDiEnumDeviceInfo(set, i, ref data); i++)
            {
                int type, req;
                if (!SetupDiGetDeviceRegistryProperty(set, ref data, SPDRP_HARDWAREID, out type, buf, buf.Length, out req)) continue;
                string[] list = System.Text.Encoding.Unicode.GetString(buf, 0, req).Split('\0');
                if (Array.Exists(list, s => string.Equals(s, hwid, StringComparison.OrdinalIgnoreCase))
                    && SetupDiCallClassInstaller(DIF_REMOVE, set, ref data))
                    removed++;
            }
        }
        finally { SetupDiDestroyDeviceInfoList(set); }
        return removed;
    }
}
"@

function Get-AudioraPackages {
    # "Published Name : oemNN.inf" blocks whose original name is audiorava.inf
    $out = pnputil /enum-drivers
    $names = @()
    $current = $null
    foreach ($line in $out) {
        if ($line -match "^\s*Published Name\s*:\s*(\S+)") { $current = $Matches[1] }
        elseif ($line -match "^\s*Original Name\s*:\s*audiorava\.inf" -and $current) { $names += $current }
    }
    $names
}

if ($Uninstall) {
    $n = [AudioraDevice]::Remove($hwid)
    Write-Host "Removed $n Audiora Virtual Audio device(s)."
    foreach ($p in Get-AudioraPackages) {
        pnputil /delete-driver $p /uninstall /force | Out-Null
        Write-Host "Removed driver package $p"
    }
    foreach ($store in "Root", "TrustedPublisher") {
        Get-ChildItem "Cert:\LocalMachine\$store" | Where-Object { $_.Subject -eq "CN=Audiora Test Driver Signing" } | Remove-Item
    }
    Write-Host "Removed the Audiora test certificate. To turn test signing off: bcdedit /set testsigning off, then restart."
    return
}

$ts = bcdedit /enum "{current}" | Select-String "testsigning\s+Yes"
if (-not $ts) {
    Write-Host "Test signing is off. Create a System Restore point, then run as administrator:" -ForegroundColor Yellow
    Write-Host "    bcdedit /set testsigning on" -ForegroundColor Yellow
    Write-Host "restart (Secure Boot must be off in the firmware settings) and run this script again." -ForegroundColor Yellow
    exit 1
}

$inf = Join-Path $here "AudioraVA.inf"
if (-not (Test-Path $inf)) { throw "AudioraVA.inf not found next to this script" }

$cer = Join-Path $here "AudioraVA-test.cer"
if (Test-Path $cer) {
    # certutil, not Import-Certificate: the PowerShell cmdlet is refused writing to the machine Root
    # store on some Windows 11 builds even when elevated
    foreach ($store in "Root", "TrustedPublisher") {
        certutil -f -addstore $store $cer | Out-Null
        if ($LASTEXITCODE -ne 0) { throw "could not add the test certificate to LocalMachine\$store (certutil exit $LASTEXITCODE)" }
    }
    Write-Host "Trusted the Audiora test certificate."
}

# a reinstall replaces the old device instead of adding a second one
[AudioraDevice]::Remove($hwid) | Out-Null
pnputil /add-driver $inf | Out-Null
$reboot = [AudioraDevice]::Install($inf, $hwid)
Write-Host "Audiora Virtual Audio installed." -ForegroundColor Green
if ($reboot) { Write-Host "Windows asks for a restart to finish." -ForegroundColor Yellow }
Write-Host "Check Settings > System > Sound: Audiora Game, Chat, Media, Aux, Mic Feed and Audiora Microphone."
