!macro NSIS_HOOK_POSTINSTALL
  StrCpy $0 "$INSTDIR\resources\engine\Everything.exe"
  IfFileExists "$0" bundled_engine_install_found 0
  StrCpy $0 "$INSTDIR\engine\Everything.exe"
  IfFileExists "$0" bundled_engine_install_found 0

  MessageBox MB_ICONSTOP|MB_OK "The bundled Everything 1.5 engine is missing from the installer."
  Abort

bundled_engine_install_found:
  DetailPrint "Installing the private Everything Modern indexing service..."
  nsExec::ExecToLog '"$0" -instance "EverythingModern" -install-service -install-service-pipe-name "\\.\PIPE\Everything Service (EverythingModern)"'
  Pop $1
  StrCmp $1 "0" bundled_service_install_done 0

  MessageBox MB_ICONSTOP|MB_OK "The Everything Modern search service could not be installed (exit code $1). Installation will stop."
  Abort

bundled_service_install_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  StrCpy $0 "$INSTDIR\resources\engine\Everything.exe"
  IfFileExists "$0" bundled_engine_uninstall_found 0
  StrCpy $0 "$INSTDIR\engine\Everything.exe"
  IfFileExists "$0" bundled_engine_uninstall_found bundled_service_uninstall_done

bundled_engine_uninstall_found:
  DetailPrint "Removing the private Everything Modern indexing service..."
  nsExec::ExecToLog '"$0" -instance "EverythingModern" -uninstall-service'
  Pop $1
  StrCmp $1 "0" bundled_service_uninstall_done 0

  MessageBox MB_ICONSTOP|MB_OK "The Everything Modern search service could not be removed (exit code $1). Uninstallation will stop to avoid leaving an orphaned service."
  Abort

bundled_service_uninstall_done:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "Everything Modern"
!macroend
