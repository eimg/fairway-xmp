!include "LogicLib.nsh"
!include "WinVer.nsh"

!macro NSIS_HOOK_PREINSTALL
  ${IfNot} ${AtLeastWin10}
    MessageBox MB_ICONSTOP "Fairway XMP needs Windows 10 22H2 (64-bit) or Windows 11."
    Abort
  ${EndIf}
  SetRegView 64
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\Windows NT\CurrentVersion" "CurrentBuildNumber"
  ${If} $0 < 19045
    MessageBox MB_ICONSTOP "Please update Windows to Windows 10 22H2 or later before installing Fairway XMP."
    Abort
  ${EndIf}
  ${If} ${FileExists} "$INSTDIR\fairway.exe"
    ExecWait '"$INSTDIR\fairway.exe" --prepare-uninstall' $0
    ${If} $0 != 0
      MessageBox MB_ICONSTOP "Open Fairway XMP, stop all services, and quit the app before upgrading. Your projects and databases will be preserved."
      Abort
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  SetRegView 64
  ReadRegDWORD $0 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Installed"
  ${If} $0 != 1
    ExecWait '"$INSTDIR\runtime\vc_redist.x64.exe" /install /quiet /norestart' $0
    ${If} $0 == 3010
      SetRebootFlag true
    ${ElseIf} $0 != 0
      MessageBox MB_ICONSTOP "The Microsoft Visual C++ runtime could not be installed. Install it and run Fairway XMP Setup again."
      Abort
    ${EndIf}
  ${EndIf}
  ExecWait '"$INSTDIR\fairway.exe" --install-cli' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Fairway XMP could not enable PHP and Composer in your user PATH. Open Settings in Fairway XMP to retry."
    Abort
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\fairway.exe" --prepare-uninstall' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Open Fairway XMP, stop all services, and quit the app before uninstalling."
    Abort
  ${EndIf}
  ExecWait '"$INSTDIR\fairway.exe" --remove-domains' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Fairway XMP could not remove its local project domains. Approve the Windows prompt, then retry uninstalling."
    Abort
  ${EndIf}
  ExecWait '"$INSTDIR\fairway.exe" --remove-https-trust' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Fairway XMP could not remove its local HTTPS trust. Retry uninstalling after closing Fairway XMP."
    Abort
  ${EndIf}
  ExecWait '"$INSTDIR\fairway.exe" --remove-cli' $0
  ${If} $0 != 0
    MessageBox MB_ICONSTOP "Fairway XMP could not remove its PATH integration. Open Fairway XMP Settings and remove command-line integration, then retry uninstalling."
    Abort
  ${EndIf}
  MessageBox MB_YESNO|MB_DEFBUTTON2 "Also delete Fairway XMP's databases, backups, settings, and caches? Your coursework will remain unless you choose to remove it next." /SD IDNO IDNO keep_data
    ExecWait '"$INSTDIR\fairway.exe" --remove-data' $0
    ${If} $0 != 0
      MessageBox MB_ICONSTOP "Data cleanup failed. Your remaining files have been preserved."
      Abort
    ${EndIf}
  keep_data:
  MessageBox MB_YESNO|MB_DEFBUTTON2 "Also permanently delete all coursework in your fairway\www folder? Choose No to keep your projects." /SD IDNO IDNO keep_coursework
    ExecWait '"$INSTDIR\fairway.exe" --remove-coursework' $0
    ${If} $0 != 0
      MessageBox MB_ICONSTOP "Coursework cleanup failed. Your remaining files have been preserved."
      Abort
    ${EndIf}
  keep_coursework:
!macroend
