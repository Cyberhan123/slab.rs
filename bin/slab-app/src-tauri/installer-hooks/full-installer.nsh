!macro NSIS_HOOK_POSTINSTALL
  ExpandEnvStrings $0 "%SLAB_INSTALLER_PAYLOAD_DIR%"
  ExpandEnvStrings $1 "%SLAB_INSTALLER_HELPER_PATH%"

  StrCmp $0 "" slab_postinstall_done 0
  StrCmp $1 "" slab_postinstall_done 0

  ExecWait '"$1" apply --source "$0" --dest "$INSTDIR\resources\libs"' $2
  IntCmp $2 0 slab_postinstall_done slab_postinstall_failed slab_postinstall_failed

slab_postinstall_failed:
  MessageBox MB_OK|MB_ICONSTOP "Slab runtime payload apply failed with exit code $2."
  Abort

slab_postinstall_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  RMDir /r "$INSTDIR\resources\libs"
!macroend
