!macro NSIS_HOOK_PREINSTALL
  nsExec::ExecToLog 'taskkill /F /IM "Cap-aniBullet.exe"'
  nsExec::ExecToLog 'taskkill /F /IM "Cap-aniBullet-Development.exe"'
  Sleep 1000
!macroend

!macro NSIS_HOOK_POSTINSTALL
!macroend
