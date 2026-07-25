!macro NSIS_HOOK_PREINSTALL
  StrCpy $0 "$INSTDIR\resources\engine\Everything.exe"
  IfFileExists "$0" bundled_engine_upgrade_found 0
  StrCpy $0 "$INSTDIR\engine\Everything.exe"
  IfFileExists "$0" bundled_engine_upgrade_found bundled_engine_upgrade_done

bundled_engine_upgrade_found:
  DetailPrint "Stopping the existing Everything Modern engine..."
  nsExec::ExecToLog '"$0" -instance "EverythingModern" -quit'
  Pop $1
  Sleep 500

  ; The service runs from the bundled executable, so remove it before replacing
  ; that file. POSTINSTALL recreates it from the new runtime.
  nsExec::ExecToLog '"$0" -instance "EverythingModern" -uninstall-service'
  Pop $1
  Sleep 500

bundled_engine_upgrade_done:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  StrCpy $0 "$INSTDIR\resources\engine\Everything.exe"
  IfFileExists "$0" bundled_engine_install_found 0
  StrCpy $0 "$INSTDIR\engine\Everything.exe"
  IfFileExists "$0" bundled_engine_install_found 0

  MessageBox MB_ICONSTOP|MB_OK "The bundled Everything 1.5 engine is missing from the installer." /SD IDOK
  Abort

bundled_engine_install_found:
  DetailPrint "Installing the private Everything Modern indexing service..."
  nsExec::ExecToLog '"$0" -instance "EverythingModern" -install-service -install-service-pipe-name "\\.\PIPE\Everything Service (EverythingModern)"'
  Pop $1
  StrCmp $1 "0" bundled_service_install_done 0

  MessageBox MB_ICONSTOP|MB_OK "The Everything Modern search service could not be installed (exit code $1). Installation will stop." /SD IDOK
  Abort

bundled_service_install_done:
  ; Give shortcuts an explicit icon file. This avoids Explorer retaining the
  ; icon embedded in an older version of the executable at the same path.
  !if "${STARTMENUFOLDER}" != ""
    StrCpy $2 "$SMPROGRAMS\$AppStartMenuFolder\${PRODUCTNAME}.lnk"
  !else
    StrCpy $2 "$SMPROGRAMS\${PRODUCTNAME}.lnk"
  !endif
  IfFileExists "$2" 0 refresh_desktop_shortcut
  Delete "$2"
  CreateShortcut "$2" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\icons\icon.ico" 0
  !insertmacro SetLnkAppUserModelId "$2"

refresh_desktop_shortcut:
  IfFileExists "$DESKTOP\${PRODUCTNAME}.lnk" 0 shortcuts_refreshed
  Delete "$DESKTOP\${PRODUCTNAME}.lnk"
  CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe" "" "$INSTDIR\icons\icon.ico" 0
  !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"

shortcuts_refreshed:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  StrCpy $0 "$INSTDIR\resources\engine\Everything.exe"
  IfFileExists "$0" bundled_engine_uninstall_found 0
  StrCpy $0 "$INSTDIR\engine\Everything.exe"
  IfFileExists "$0" bundled_engine_uninstall_found 0

  MessageBox MB_ICONSTOP|MB_OK "The bundled Everything 1.5 engine is missing, so the private service cannot be removed safely. Restore the installation and retry." /SD IDOK
  Abort

bundled_engine_uninstall_found:
  DetailPrint "Stopping the private Everything Modern engine..."
  nsExec::ExecToLog '"$0" -instance "EverythingModern" -quit'
  Pop $1
  Sleep 500

  DetailPrint "Removing the private Everything Modern indexing service..."
  nsExec::ExecToLog '"$0" -instance "EverythingModern" -uninstall-service'
  Pop $1
  StrCmp $1 "0" bundled_service_uninstall_done 0

  MessageBox MB_ICONSTOP|MB_OK "The Everything Modern search service could not be removed (exit code $1). Uninstallation will stop to avoid leaving an orphaned service." /SD IDOK
  Abort

bundled_service_uninstall_done:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Everything Modern"
!macroend
