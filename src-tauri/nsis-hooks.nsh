; Startup is opt-in. Uninstall removes only OpenToken's own scheduled task.
!macro NSIS_HOOK_PREUNINSTALL
  ExecWait '"$INSTDIR\opentoken.exe" --remove-startup'
  ExecWait '"$INSTDIR\opentoken.exe" quit'
  Sleep 1200
!macroend
