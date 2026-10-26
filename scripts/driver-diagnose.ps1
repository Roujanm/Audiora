#requires -Version 5.1
#requires -RunAsAdministrator
<#
.SYNOPSIS
  Collects why the Audiora Virtual Audio driver is (not) working. Read-only.
.DESCRIPTION
  Prints, and saves next to itself as diagnose.txt: the device and its problem code, the driver
  package, the service, the signature, the audio endpoints Windows built, and the driver
  installation log lines that mention Audiora.
.EXAMPLE
  powershell -ExecutionPolicy Bypass -File .\driver-diagnose.ps1
#>
$out = Join-Path $PSScriptRoot "diagnose.txt"
$lines = New-Object System.Collections.Generic.List[string]
function Say($title, $body) {
    $lines.Add("===== $title")
    foreach ($l in (($body | Out-String -Width 200) -split "`r?`n")) { if ($l.Trim()) { $lines.Add($l.TrimEnd()) } }
}

$dev = Get-PnpDevice -ErrorAction SilentlyContinue | Where-Object { $_.HardwareID -contains "ROOT\AudioraVirtualAudio" }
Say "device" ($dev | Format-List FriendlyName, InstanceId, Status, Problem, ProblemDescription, Class, Present)
foreach ($d in $dev) {
    $props = Get-PnpDeviceProperty -InstanceId $d.InstanceId -KeyName DEVPKEY_Device_ProblemCode, DEVPKEY_Device_ProblemStatus, DEVPKEY_Device_DriverInfPath, DEVPKEY_Device_DriverVersion, DEVPKEY_Device_Service -ErrorAction SilentlyContinue
    Say "device properties" ($props | Format-Table KeyName, Data -AutoSize)
    Say "children (KS filters)" (Get-PnpDevice -ErrorAction SilentlyContinue | Where-Object { $_.InstanceId -like "SWD\MMDEVAPI*" -and $_.FriendlyName -like "*Audiora*" } | Format-Table FriendlyName, Status, Problem -AutoSize)
}
Say "driver packages" (pnputil /enum-drivers | Select-String -Context 0,6 "audiorava")
Say "service" (sc.exe query AudioraVA)
Say "service config" (sc.exe qc AudioraVA)
$sys = Join-Path $env:WINDIR "System32\drivers\AudioraVA.sys"
$store = Get-ChildItem "$env:WINDIR\System32\DriverStore\FileRepository" -Recurse -Filter AudioraVA.sys -ErrorAction SilentlyContinue | Select-Object -First 1
Say "driver file" ("System32\drivers: " + (Test-Path $sys) + "   DriverStore: " + $(if ($store) { $store.FullName } else { "none" }))
if ($store) { Say "signature" (Get-AuthenticodeSignature $store.FullName | Format-List Status, StatusMessage, SignerCertificate) }
Say "test signing" (bcdedit /enum "{current}" | Select-String "testsigning")
Say "audio endpoints" (Get-PnpDevice -Class AudioEndpoint -ErrorAction SilentlyContinue | Format-Table FriendlyName, Status, Problem -AutoSize)
Say "audio services" (Get-Service Audiosrv, AudioEndpointBuilder | Format-Table Name, Status -AutoSize)

# the KS filters the driver registered (one interface per category per endpoint) and whether enabled
foreach ($cat in @{ AUDIO = "{6994AD04-93EF-11D0-A3CC-00A0C9223196}"; RENDER = "{65E8773E-8F56-11D0-A3B9-00A0C9223196}"; CAPTURE = "{65E8773D-8F56-11D0-A3B9-00A0C9223196}"; TOPOLOGY = "{DDA54A40-1E4C-11D1-A050-405705C10000}" }.GetEnumerator()) {
    $raw = pnputil /enum-interfaces /class $cat.Value
    $hits = for ($i = 0; $i -lt $raw.Count; $i++) { if ($raw[$i] -match "ROOT#MEDIA") { $raw[$i].Trim(); if ($i + 5 -lt $raw.Count) { ($raw[($i+1)..($i+5)] | ForEach-Object { "    " + $_.Trim() }) } } }
    Say "interfaces KSCATEGORY_$($cat.Key)" $(if ($hits) { $hits } else { "none" })
}
$mc = "HKLM:\SYSTEM\CurrentControlSet\Control\MediaCategories"
Say "endpoint names (MediaCategories)" (@("EC7886EF-060A-488F-927D-2B523F5F75D2", "0640AC36-33C0-4357-8165-FE1035592547") | ForEach-Object {
    $k = Join-Path $mc "{$_}"; "{$_}: " + $(if (Test-Path $k) { (Get-ItemProperty $k).Name } else { "missing" }) })
$mm = "HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio"
Say "MMDevices mentioning Audiora" (Get-ChildItem "$mm\Render", "$mm\Capture" -ErrorAction SilentlyContinue | ForEach-Object {
    $p = Get-ItemProperty (Join-Path $_.PSPath "Properties") -ErrorAction SilentlyContinue
    $text = ($p.PSObject.Properties | ForEach-Object { "$($_.Value)" }) -join " | "
    if ($text -match "Audiora|ROOT#MEDIA") { "$($_.PSChildName) state=$((Get-ItemProperty $_.PSPath).DeviceState)" } })
Say "audio event log (last hour)" (Get-WinEvent -LogName "Microsoft-Windows-Audio/Operational" -MaxEvents 30 -ErrorAction SilentlyContinue |
    Where-Object { $_.TimeCreated -gt (Get-Date).AddHours(-1) } | Format-List TimeCreated, Id, LevelDisplayName, Message)

$setupLog = Join-Path $env:WINDIR "INF\setupapi.dev.log"
if (Test-Path $setupLog) {
    $text = Get-Content $setupLog
    $idx = @(); for ($i = 0; $i -lt $text.Count; $i++) { if ($text[$i] -match "AudioraVirtualAudio|audiorava") { $idx += $i } }
    if ($idx) {
        $from = [Math]::Max(0, $idx[-1] - 60); $to = [Math]::Min($text.Count - 1, $idx[-1] + 40)
        Say "setupapi.dev.log (last Audiora install)" ($text[$from..$to] | Where-Object { $_ -match "!!!|!\s|fail|error|Exit status|Device install|Install Device|AudioraVirtualAudio|audiorava|Problem|code" })
    } else { Say "setupapi.dev.log" "no Audiora entries" }
}
Say "system log (driver/kernel-pnp, last hour)" (Get-WinEvent -FilterHashtable @{ LogName = "System"; StartTime = (Get-Date).AddHours(-1) } -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -match "Audiora|AudioraVA" -or $_.ProviderName -match "Kernel-PnP|Service Control" } | Select-Object -First 15 | Format-List TimeCreated, ProviderName, Id, Message)

$lines | Set-Content $out -Encoding UTF8
$lines | ForEach-Object { Write-Host $_ }
Write-Host ""
Write-Host "Saved to $out" -ForegroundColor Green
