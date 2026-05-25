!macro NSIS_HOOK_PREINSTALL
  nsExec::ExecToLog 'taskkill /F /IM "Cap-aniBullet.exe"'
  nsExec::ExecToLog 'taskkill /F /IM "Cap-aniBullet-Development.exe"'
  Sleep 1000

  SetRegView 64
  ReadRegStr $R0 HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\Cap-aniBullet" "UninstallString"
  StrCmp $R0 "" skip_uninstall
    nsExec::ExecToLog '"$R0" /S'
    Sleep 2000
  skip_uninstall:
!macroend

!macro NSIS_HOOK_POSTINSTALL
!macroend
