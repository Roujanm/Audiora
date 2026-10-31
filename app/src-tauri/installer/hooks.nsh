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

; Page texts. MUI reads these when a page is inserted, and this file is included before the pages.
!define NL "$\r$\n"
!define MUI_WELCOMEPAGE_TITLE "Welcome to Audiora"
!define MUI_WELCOMEPAGE_TEXT "Audiora is a professional audio mixer and voice processor for Windows.${NL}${NL}This installs:${NL}${NL}  -  Audiora${NL}      channel mixer, voice processor and audio engine${NL}${NL}  -  Audiora Virtual Audio${NL}      Audiora Microphone and the channel outputs, when this PC allows it (Windows asks once)${NL}${NL}Your settings and presets are kept."
!define MUI_LICENSEPAGE_TEXT_TOP "Please read the Audiora licence before you install."
!define MUI_FINISHPAGE_TITLE "Audiora is ready"
!define MUI_FINISHPAGE_TEXT "Audiora has been installed.${NL}${NL}To give HiNekst, Discord and games your processed voice, pick Audiora Microphone (or CABLE Output) as their microphone, or drag them onto the Mic channel in Audiora."
