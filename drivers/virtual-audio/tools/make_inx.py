"""Writes drivers/virtual-audio/Source/Main/AudioraVA.inx (UTF-16 LE with BOM, CRLF), one block
per endpoint so the six endpoints cannot drift apart."""
import sys

import os
OUT = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "Source", "Main", "AudioraVA.inx")

# (KS name suffix, shown name, render?)
ENDPOINTS = [
    ("Game", "Audiora Game", True),
    ("Chat", "Audiora Chat", True),
    ("Media", "Audiora Media", True),
    ("Aux", "Audiora Aux", True),
    ("MicFeed", "Audiora Mic Feed", True),
    ("Mic", "Audiora Microphone", False),
]
# must match the GUIDs in Filters/speakertoptable.h and Filters/micarray1toptable.h
NAME_GUIDS = {
    "Game": "{EC7886EF-060A-488F-927D-2B523F5F75D2}",
    "Chat": "{82C2FA21-67ED-4AEB-AC99-48D3019B2FD7}",
    "Media": "{49BC6D4D-4F67-44B4-A4F1-D335AC33E306}",
    "Aux": "{97806044-D36E-4633-9065-12993BE90127}",
    "MicFeed": "{71E5F8B0-439E-4008-9E62-57FE0D181013}",
    "Mic": "{0640AC36-33C0-4357-8165-FE1035592547}",
}

L = []
w = L.append
w("""; Audiora Virtual Audio
; Copyright (c) Roujan Digital Agency. Derived from Microsoft's SimpleAudioSample (MS-PL).
;
; Endpoints: Audiora Game / Chat / Media / Aux (outputs the Audiora engine mixes per channel),
; Audiora Mic Feed (the engine plays the processed microphone here) and Audiora Microphone
; (what programs record: the mic feed, through the kernel mic cable).

[Version]
Signature   = "$Windows NT$"
Class       = MEDIA
Provider    = %ProviderName%
ClassGUID   = {4d36e96c-e325-11ce-bfc1-08002be10318}
DriverVer   = 10/26/2026, 0.1.0.0
CatalogFile = AudioraVA.cat
PnpLockDown = 1

[SourceDisksNames]
222 = "Audiora Virtual Audio Disk","",222

[SourceDisksFiles]
AudioraVA.sys = 222

[SignatureAttributes]
AudioraVA.sys = SignatureAttributes.DRM

[SignatureAttributes.DRM]
DRMLevel = 1300

[SignatureAttributes.PETrust]
PETrust = true

[Manufacturer]
%MfgName% = AUDIORA,NT$ARCH$.10.0...22000

[AUDIORA.NT$ARCH$.10.0...22000]
%AUDIORA_SA.DeviceDesc% = AUDIORA_SA, ROOT\\AudioraVirtualAudio

[DestinationDirs]
AUDIORA_SA.CopyList = 13   ; 13 = package's DriverStore directory

;======================================================
; AUDIORA_SA
;======================================================
[AUDIORA_SA.CopyList]
AudioraVA.sys

[AUDIORA_SA.AddReg]
HKR,,AssociatedFilters,,"wdmaud,swmidi,redbook"
HKR,,Driver,,AudioraVA.sys

; A virtual device has no power to save, and the engine expects the endpoints to answer at once.
HKR,PowerSettings,SingleComponentMultiFxStates,3,1,0,0,0
HKR,PowerSettings,ConservationIdleTime,3,0,0,0,0
HKR,PowerSettings,PerformanceIdleTime,3,0,0,0,0
HKR,PowerSettings,IdlePowerState,3,0,0,0,0

HKR,Drivers,SubClasses,,"wave,midi,mixer"

HKR,Drivers\\wave\\wdmaud.drv,Driver,,wdmaud.drv
HKR,Drivers\\midi\\wdmaud.drv,Driver,,wdmaud.drv
HKR,Drivers\\mixer\\wdmaud.drv,Driver,,wdmaud.drv

HKR,Drivers\\wave\\wdmaud.drv,Description,,%AUDIORA_SA.DeviceDesc%
HKR,Drivers\\mixer\\wdmaud.drv,Description,,%AUDIORA_SA.DeviceDesc%

; Endpoint names: the topology bridge pin of each endpoint carries one of these name GUIDs.
[AUDIORA_SA.MediaCategories]
""")
for key, shown, _ in ENDPOINTS:
    w(f"HKLM,%MediaCategories%\\%NameGuid.{key}%,Name,,%Name.{key}%")
    w(f"HKLM,%MediaCategories%\\%NameGuid.{key}%,Display,1,00,00,00,00")
w("")

for key, shown, render in ENDPOINTS:
    kind = "render" if render else "capture"
    w(";======================================================")
    w(f"; {kind} interfaces: {shown}")
    w(";======================================================")
    w(f"[AUDIORA.I.Wave{key}]")
    w(f"AddReg = AUDIORA.I.Wave{key}.AddReg")
    w(f"[AUDIORA.I.Wave{key}.AddReg]")
    w("HKR,,CLSID,,%Proxy.CLSID%")
    w(f"HKR,,FriendlyName,,%AUDIORA.Wave{key}.szPname%")
    w("")
    w(f"[AUDIORA.I.Topology{key}]")
    w(f"AddReg = AUDIORA.I.Topology{key}.AddReg")
    w(f"[AUDIORA.I.Topology{key}.AddReg]")
    w("HKR,,CLSID,,%Proxy.CLSID%")
    w(f"HKR,,FriendlyName,,%AUDIORA.Topology{key}.szPname%")
    w("HKR,EP\\0,%PKEY_AudioEndpoint_Association%,,%KSNODETYPE_ANY%")
    w("HKR,EP\\0,%PKEY_AudioEndpoint_Supports_EventDriven_Mode%,0x00010001,0x1")
    w("")

w(""";======================================================
; AUDIORA_SA
;======================================================
[AUDIORA_SA.NT]
Include = ks.inf, wdmaudio.inf
Needs = KS.Registration, WDMAUDIO.Registration
CopyFiles = AUDIORA_SA.CopyList
AddReg = AUDIORA_SA.AddReg, AUDIORA_SA.MediaCategories

[AUDIORA_SA.NT.Interfaces]""")
for key, shown, render in ENDPOINTS:
    flow = "KSCATEGORY_RENDER" if render else "KSCATEGORY_CAPTURE"
    w(f"; {shown}")
    w(f"AddInterface = %KSCATEGORY_AUDIO%,    %KSNAME_Wave{key}%,     AUDIORA.I.Wave{key}")
    w(f"AddInterface = %{flow}%,   %KSNAME_Wave{key}%,     AUDIORA.I.Wave{key}")
    w(f"AddInterface = %KSCATEGORY_REALTIME%, %KSNAME_Wave{key}%,     AUDIORA.I.Wave{key}")
    w(f"AddInterface = %KSCATEGORY_AUDIO%,    %KSNAME_Topology{key}%, AUDIORA.I.Topology{key}")
    w(f"AddInterface = %KSCATEGORY_TOPOLOGY%, %KSNAME_Topology{key}%, AUDIORA.I.Topology{key}")
w("""
[AUDIORA_SA.NT.Services]
AddService = AudioraVA, 0x00000002, AudioraVA_Service_Inst

[AudioraVA_Service_Inst]
DisplayName   = %AudioraVA.SvcDesc%
ServiceType   = 1
StartType     = 3
ErrorControl  = 1
ServiceBinary = %13%\\AudioraVA.sys

[AUDIORA_SA.NT.HW]
AddReg = AUDIOHW.AddReg

[AUDIOHW.AddReg]
; FILE_DEVICE_SOUND
HKR,,DeviceType,0x10001,0x0000001D
; SDDL_DEVOBJ_SYS_ALL_ADM_RWX_WORLD_RWX_RES_RWX
HKR,,Security,,"D:P(A;;GA;;;SY)(A;;GRGWGX;;;BA)(A;;GRGWGX;;;WD)(A;;GRGWGX;;;RC)"

;======================================================
; WDF
;======================================================
[AUDIORA_SA.NT.Wdf]
KmdfService = AudioraVA, AUDIORA_SA_WdfSect
[AUDIORA_SA_WdfSect]
KmdfLibraryVersion = $KMDFVERSION$

;======================================================
; COMMON
;======================================================
[Strings]

;Non-localizable
""")
for key, _, _ in ENDPOINTS:
    w(f'KSNAME_Wave{key} = "Wave{key}"')
    w(f'KSNAME_Topology{key} = "Topology{key}"')
for key, _, _ in ENDPOINTS:
    w(f'NameGuid.{key} = "{NAME_GUIDS[key]}"')
w('''
MediaCategories = "SYSTEM\\CurrentControlSet\\Control\\MediaCategories"

Proxy.CLSID         = "{17CCA71B-ECD7-11D0-B908-00A0C9223196}"
KSCATEGORY_AUDIO    = "{6994AD04-93EF-11D0-A3CC-00A0C9223196}"
KSCATEGORY_RENDER   = "{65E8773E-8F56-11D0-A3B9-00A0C9223196}"
KSCATEGORY_CAPTURE  = "{65E8773D-8F56-11D0-A3B9-00A0C9223196}"
KSCATEGORY_REALTIME = "{EB115FFC-10C8-4964-831D-6DCB02E6F23F}"
KSCATEGORY_TOPOLOGY = "{DDA54A40-1E4C-11D1-A050-405705C10000}"

KSNODETYPE_ANY = "{00000000-0000-0000-0000-000000000000}"

PKEY_AudioEndpoint_Association               = "{1DA5D803-D492-4EDD-8C23-E0C0FFEE7F0E},2"
PKEY_AudioEndpoint_Supports_EventDriven_Mode = "{1DA5D803-D492-4EDD-8C23-E0C0FFEE7F0E},7"

;Localizable

ProviderName = "Roujan Digital Agency"
MfgName      = "Roujan Digital Agency"

AUDIORA_SA.DeviceDesc = "Audiora Virtual Audio"
AudioraVA.SvcDesc     = "Audiora Virtual Audio Driver"
''')
for key, shown, _ in ENDPOINTS:
    w(f'Name.{key} = "{shown}"')
    w(f'AUDIORA.Wave{key}.szPname = "{shown}"')
    w(f'AUDIORA.Topology{key}.szPname = "{shown}"')

text = "\r\n".join(L) + "\r\n"
with open(OUT, "wb") as f:
    f.write(b"\xff\xfe" + text.encode("utf-16-le"))
print("wrote", OUT, len(L), "lines")
