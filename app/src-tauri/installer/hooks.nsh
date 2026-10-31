; Audiora installer additions (Tauri "installerHooks").
; The file properties of Audiora_x.y.z_x64-setup.exe: Tauri's template sets product, description
; and copyright; the company is added here so Windows shows who made the installer.
VIAddVersionKey "CompanyName" "Roujan Digital Agency"

; After the files are in place: install or update Audiora's virtual audio driver (Audiora
; Microphone and the channel outputs). driver-setup.ps1 decides without administrator rights and
; does nothing when the driver is current or test signing is off, so an app update stays silent;
; otherwise Windows asks once for permission. The app is installed either way.
!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Setting up Audiora Virtual Audio..."
  nsExec::Exec 'powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "$INSTDIR\driver\driver-setup.ps1"'
  Pop $0
!macroend
