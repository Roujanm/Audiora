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

# Windows makes a newly added output the default one, which would send every program's sound into
# Audiora Mic Feed (you would hear nothing and others would hear your PC). The default output is
# read before the install and put back afterwards.
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;

public static class AudioraDefaults
{
    [ComImport, Guid("BCDE0395-E52F-467C-8E3D-C4579291692E")] class MMDeviceEnumerator {}

    [ComImport, Guid("A95664D2-9614-4F35-A746-DE8DB63617E6"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IMMDeviceEnumerator
    {
        int EnumAudioEndpoints(int flow, int mask, out IntPtr devices);
        int GetDefaultAudioEndpoint(int flow, int role, out IMMDevice device);
    }

    [ComImport, Guid("D666063F-1587-4E43-81F1-B948E807363F"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IMMDevice
    {
        int Activate(ref Guid iid, int ctx, IntPtr p, out IntPtr iface);
        int OpenPropertyStore(int access, out IntPtr store);
        int GetId([MarshalAs(UnmanagedType.LPWStr)] out string id);
    }

    [ComImport, Guid("870AF99C-171D-4F9E-AF0D-E63DF40C2BC9")] class PolicyConfigClient {}

    // IPolicyConfig (Windows 7 and later): only SetDefaultEndpoint is used; the methods before it keep the order
    [ComImport, Guid("F8679F50-850A-41CF-9C72-430F290290C8"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    interface IPolicyConfig
    {
        int GetMixFormat(IntPtr a, IntPtr b);
        int GetDeviceFormat(IntPtr a, int b, IntPtr c);
        int ResetDeviceFormat(IntPtr a);
        int SetDeviceFormat(IntPtr a, IntPtr b, IntPtr c);
        int GetProcessingPeriod(IntPtr a, int b, IntPtr c, IntPtr d);
        int SetProcessingPeriod(IntPtr a, IntPtr b);
        int GetShareMode(IntPtr a, IntPtr b);
        int SetShareMode(IntPtr a, IntPtr b);
        int GetPropertyValue(IntPtr a, int b, IntPtr c, IntPtr d);
        int SetPropertyValue(IntPtr a, int b, IntPtr c, IntPtr d);
        int SetDefaultEndpoint([MarshalAs(UnmanagedType.LPWStr)] string id, int role);
    }

    /// The default output for a role (0 console, 1 multimedia, 2 communications), or null.
    public static string GetOutput(int role)
    {
        IMMDevice d;
        var e = (IMMDeviceEnumerator)new MMDeviceEnumerator();
        if (e.GetDefaultAudioEndpoint(0, role, out d) != 0 || d == null) return null;
        string id;
        return d.GetId(out id) == 0 ? id : null;
    }

    public static void SetOutput(string id, int role)
    {
        ((IPolicyConfig)new PolicyConfigClient()).SetDefaultEndpoint(id, role);
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

# remember where the user listens (never an Audiora device: that is what this protects against)
$defaults = @{}
foreach ($role in 0, 1, 2) {
    $id = [AudioraDefaults]::GetOutput($role)
    $dev = Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue | Where-Object { $_.InstanceId -like "*$id*" } | Select-Object -First 1
    if ($id -and -not ($dev.FriendlyName -like "*Audiora*")) { $defaults[$role] = $id }
}

# a reinstall replaces the old device instead of adding a second one
[AudioraDevice]::Remove($hwid) | Out-Null
pnputil /add-driver $inf | Out-Null
$reboot = [AudioraDevice]::Install($inf, $hwid)
# Windows switches the default output to a new endpoint a moment after it appears: put it back
Start-Sleep -Seconds 3
foreach ($role in $defaults.Keys) {
    try { [AudioraDefaults]::SetOutput($defaults[$role], $role) } catch { Write-Host "could not restore the default output: $_" }
}
Write-Host "Audiora Virtual Audio installed." -ForegroundColor Green
if ($reboot) { Write-Host "Windows asks for a restart to finish." -ForegroundColor Yellow }
Write-Host "Check Settings > System > Sound: Audiora Game, Chat, Media, Aux, Mic Feed and Audiora Microphone."
