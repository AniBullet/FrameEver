SetCompressorDictSize 64

SetFont /LANG=2052 "Microsoft YaHei UI" 9

; Cap 0.6.0 ran its GPUI app as cap-gpui.exe next to Cap.exe. The template
; only closes Cap.exe before copying files, so a running cap-gpui.exe would keep
; the shared FFmpeg DLLs locked and stop the update with a file-in-use error.
!macro NSIS_HOOK_PREINSTALL
  !insertmacro CheckIfAppIsRunning "cap-gpui.exe" "${PRODUCTNAME}"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Delete "$INSTDIR\cap-gpui.exe"
  Delete "$INSTDIR\cap-exporter.exe"
  Delete "$INSTDIR\cap-cli.exe"
  Delete "$INSTDIR\cap-muxer.exe"
  RMDir /r "$INSTDIR\assets\rive"
!macroend
