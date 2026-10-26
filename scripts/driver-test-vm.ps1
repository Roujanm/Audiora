#requires -Version 5.1
#requires -RunAsAdministrator
<#
.SYNOPSIS
  Creates a Hyper-V test VM with the Audiora Virtual Audio driver installed, without any setup
  screens. Your PC's own settings are not changed by this script.
.DESCRIPTION
  Needs: Hyper-V turned on (Windows Features, then restart: your step), a Windows 11 ISO (found in
  Downloads by default), and a built driver (scripts\driver-build.ps1 -> dist\driver).

  What it does:
    1. Writes Windows from the ISO straight onto a new virtual disk (no setup wizard, no TPM check).
    2. Inside that virtual disk only: turns test signing on, adds an unattend file that creates a
       local "Audiora" administrator, signs in automatically and installs the driver at first
       sign-in (log: C:\AudioraTest\install.log in the VM).
    3. Creates a Generation 2 VM (4 CPUs, 6 GB, Default Switch, Secure Boot off in the VM's own
       firmware), takes a checkpoint "Before first boot" and starts it.
  The VM account's password is generated and saved to %USERPROFILE%\.audiora\test-vm.txt.

  -Remove deletes the VM and its virtual disk.
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\driver-test-vm.ps1
  powershell -ExecutionPolicy Bypass -File scripts\driver-test-vm.ps1 -Iso D:\Win11_24H2_x64.iso
  powershell -ExecutionPolicy Bypass -File scripts\driver-test-vm.ps1 -Remove
#>
param(
    [string]$Iso,
    [string]$VmName = "Audiora Test",
    [int]$MemoryGB = 6,
    [int]$DiskGB = 64,
    [switch]$Remove
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

if (-not (Get-Command New-VM -ErrorAction SilentlyContinue)) {
    throw "Hyper-V is not turned on. Windows Features > Hyper-V (all of it), OK, restart, then run this again."
}

$vhdDir = (Get-VMHost).VirtualHardDiskPath
$vhd = Join-Path $vhdDir "AudioraTest.vhdx"

if ($Remove) {
    if (Get-VM -Name $VmName -ErrorAction SilentlyContinue) {
        Stop-VM -Name $VmName -TurnOff -Force -ErrorAction SilentlyContinue
        Get-VMSnapshot -VMName $VmName | Remove-VMSnapshot
        Remove-VM -Name $VmName -Force
    }
    Remove-Item $vhd -Force -ErrorAction SilentlyContinue
    Write-Host "Removed $VmName and its disk."
    return
}

if (Get-VM -Name $VmName -ErrorAction SilentlyContinue) { throw "A VM named '$VmName' already exists (use -Remove first)." }

$driver = Join-Path $root "dist\driver"
if (-not (Test-Path (Join-Path $driver "AudioraVA.sys"))) { throw "Build the driver first: scripts\driver-build.ps1" }

if (-not $Iso) {
    $Iso = Get-ChildItem (Join-Path $env:USERPROFILE "Downloads") -Filter "*.iso" -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match "win.*11|11.*win" } | Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
}
if (-not $Iso -or -not (Test-Path $Iso)) { throw "Windows 11 ISO not found in Downloads; pass -Iso <path>." }
Write-Host "ISO: $Iso"

# --- the Windows image ------------------------------------------------------------------------
$isoImage = Mount-DiskImage -ImagePath $Iso -PassThru
$disk = $null
try {
    $isoLetter = ($isoImage | Get-Volume).DriveLetter
    $wim = @("${isoLetter}:\sources\install.wim", "${isoLetter}:\sources\install.esd") | Where-Object { Test-Path $_ } | Select-Object -First 1
    if (-not $wim) { throw "No install.wim/esd in the ISO" }
    $images = Get-WindowsImage -ImagePath $wim
    $edition = $images | Where-Object { $_.ImageName -match "Pro$" } | Select-Object -First 1
    if (-not $edition) { $edition = $images | Select-Object -First 1 }
    $lang = (Get-WindowsImage -ImagePath $wim -Index $edition.ImageIndex).Languages | Select-Object -First 1
    Write-Host "Edition: $($edition.ImageName) ($lang)"

    # --- the virtual disk: EFI + MSR + Windows -------------------------------------------------
    New-Item -ItemType Directory -Force $vhdDir | Out-Null
    if (Test-Path $vhd) { throw "$vhd already exists (use -Remove first)." }
    New-VHD -Path $vhd -SizeBytes ($DiskGB * 1GB) -Dynamic | Out-Null
    $disk = Mount-VHD -Path $vhd -Passthru | Get-Disk
    Initialize-Disk -Number $disk.Number -PartitionStyle GPT
    $efi = New-Partition -DiskNumber $disk.Number -Size 260MB -GptType "{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}" -AssignDriveLetter
    Format-Volume -Partition $efi -FileSystem FAT32 -NewFileSystemLabel "System" -Confirm:$false | Out-Null
    New-Partition -DiskNumber $disk.Number -Size 16MB -GptType "{e3c9e316-0b5c-4db8-817d-f92df00215ae}" | Out-Null
    $os = New-Partition -DiskNumber $disk.Number -UseMaximumSize -AssignDriveLetter
    Format-Volume -Partition $os -FileSystem NTFS -NewFileSystemLabel "Windows" -Confirm:$false | Out-Null
    $s = (Get-Partition -DiskNumber $disk.Number -PartitionNumber $efi.PartitionNumber).DriveLetter
    $w = (Get-Partition -DiskNumber $disk.Number -PartitionNumber $os.PartitionNumber).DriveLetter

    Write-Host "Writing Windows to the virtual disk (a few minutes)..."
    Expand-WindowsImage -ImagePath $wim -Index $edition.ImageIndex -ApplyPath "${w}:\" | Out-Null
    & bcdboot "${w}:\Windows" /s "${s}:" /f UEFI | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "bcdboot failed" }
    # test signing inside the VM's own boot configuration (not this PC's)
    & bcdedit /store "${s}:\EFI\Microsoft\Boot\BCD" /set "{default}" testsigning on | Out-Null
    if ($LASTEXITCODE -ne 0) { throw "could not enable test signing in the VM's boot store" }

    # --- what the VM needs on first boot ----------------------------------------------------------
    $testDir = "${w}:\AudioraTest"
    New-Item -ItemType Directory -Force "$testDir\driver" | Out-Null
    Copy-Item "$driver\*" "$testDir\driver" -Recurse
    $setup = Get-ChildItem (Join-Path $root "dist") -Recurse -Filter "Audiora_*_x64-setup.exe" -ErrorAction SilentlyContinue |
        Sort-Object LastWriteTime -Descending | Select-Object -First 1
    if ($setup) { Copy-Item $setup.FullName $testDir }

    $chars = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789".ToCharArray()
    $password = -join (1..16 | ForEach-Object { $chars | Get-Random })
    $keyDir = Join-Path $env:USERPROFILE ".audiora"
    New-Item -ItemType Directory -Force $keyDir | Out-Null
    Set-Content (Join-Path $keyDir "test-vm.txt") "VM: $VmName`r`nUser: Audiora`r`nPassword: $password" -Encoding ASCII

    $unattend = (Get-Content (Join-Path $PSScriptRoot "driver-test-vm.unattend.xml") -Raw).
        Replace("@PASSWORD@", [Security.SecurityElement]::Escape($password)).
        Replace("@UILANGUAGE@", $lang)
    New-Item -ItemType Directory -Force "${w}:\Windows\Panther" | Out-Null
    [IO.File]::WriteAllText("${w}:\Windows\Panther\unattend.xml", $unattend, (New-Object Text.UTF8Encoding($false)))
}
finally {
    if ($disk) { Dismount-VHD -Path $vhd -ErrorAction SilentlyContinue }
    Dismount-DiskImage -ImagePath $Iso -ErrorAction SilentlyContinue | Out-Null
}

# --- the VM ------------------------------------------------------------------------------------
$vmArgs = @{ Name = $VmName; Generation = 2; MemoryStartupBytes = ($MemoryGB * 1GB); VHDPath = $vhd }
if (Get-VMSwitch -Name "Default Switch" -ErrorAction SilentlyContinue) { $vmArgs.SwitchName = "Default Switch" }
$vm = New-VM @vmArgs
Set-VMProcessor -VM $vm -Count 4
Set-VMMemory -VM $vm -DynamicMemoryEnabled $false
Set-VMFirmware -VM $vm -EnableSecureBoot Off
Set-VM -VM $vm -AutomaticCheckpointsEnabled $false -EnhancedSessionTransportType HvSocket
Enable-VMIntegrationService -VM $vm -Name "Guest Service Interface"
Checkpoint-VM -VM $vm -SnapshotName "Before first boot"
Start-VM -VM $vm

Write-Host ""
Write-Host "'$VmName' is starting. Windows prepares itself (a few minutes, one or two restarts)," -ForegroundColor Green
Write-Host "signs in as 'Audiora' and installs the driver. Open it from Hyper-V Manager (Connect)."
Write-Host "Password: $keyDir\test-vm.txt"
