; Tauri includes this file right after `SetCompressor /SOLID lzma`, before any
; data is compressed. A 128 MB window lets LZMA match cap-cli.exe against
; Cap.exe, which share most of their code; the default 8 MB window cannot.
SetCompressorDictSize 128

SetFont /LANG=2052 "Microsoft YaHei UI" 9

!macro EndFrameEverMcpServers
  Push $0
  System::Call 'kernel32::SetEnvironmentVariable(t "FRAMEEVER_INSTALL_DIR", t "$INSTDIR")i'
  nsExec::Exec `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$cli = Join-Path $$env:FRAMEEVER_INSTALL_DIR 'frameever-cli.exe'; Get-CimInstance Win32_Process | Where-Object { $$_.Name -eq 'frameever-cli.exe' -and $$_.ExecutablePath -eq $$cli -and $$_.CommandLine -match '\smcp\s+serve(\s|$$)' } | ForEach-Object { Stop-Process -Id $$_.ProcessId -Force -ErrorAction SilentlyContinue; Wait-Process -Id $$_.ProcessId -Timeout 5 -ErrorAction SilentlyContinue }"`
  Pop $0
  Pop $0
!macroend

LangString FrameEverToolsRunning 1033 "FrameEver's command line tool is still running, for example a recording started with frameever record or an export. Stop it, then click Retry."
LangString FrameEverToolsRunning 2052 "帧刻的命令行工具仍在运行，例如用 frameever record 开始的录制或导出。请先停止它，然后点击“重试”。"
LangString FrameEverNotUpdated 1033 "FrameEver was not updated because its command line tool is still running."
LangString FrameEverNotUpdated 2052 "命令行工具仍在运行，帧刻未更新。"
LangString FrameEverNotUninstalled 1033 "FrameEver was not uninstalled because its command line tool is still running."
LangString FrameEverNotUninstalled 2052 "命令行工具仍在运行，帧刻未卸载。"

!macro WaitForFrameEverCommandLineTools ABORT_TEXT
  !define FrameEverToolsCheck frameever_tools_check_${__LINE__}
  Push $0
  Push $1
  System::Call 'kernel32::SetEnvironmentVariable(t "FRAMEEVER_INSTALL_DIR", t "$INSTDIR")i'
  ${FrameEverToolsCheck}:
  nsis_tauri_utils::FindProcessCurrentUser "frameever-cli.exe"
  Pop $0
  ${If} $0 != 0
    nsis_tauri_utils::FindProcessCurrentUser "frameever-muxer.exe"
    Pop $0
  ${EndIf}
  ${If} $0 = 0
    nsExec::ExecToStack `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "try { $$dir = [IO.Path]::GetFullPath($$env:FRAMEEVER_INSTALL_DIR).TrimEnd('\'); $$deadline = (Get-Date).AddSeconds(10); do { $$running = @(Get-CimInstance Win32_Process -ErrorAction Stop | Where-Object { ($$_.Name -eq 'frameever-cli.exe' -or $$_.Name -eq 'frameever-muxer.exe') -and $$_.ExecutablePath -and ([IO.Path]::GetDirectoryName($$_.ExecutablePath) -eq $$dir) }); if ($$running.Count -eq 0) { exit 0 }; Start-Sleep -Milliseconds 500 } while ((Get-Date) -lt $$deadline); exit 2 } catch { exit 0 }"`
    Pop $0
    Pop $1
    ${If} $0 == 2
      MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(FrameEverToolsRunning)" /SD IDCANCEL IDRETRY ${FrameEverToolsCheck}
      Abort "${ABORT_TEXT}"
    ${EndIf}
  ${EndIf}
  Pop $1
  Pop $0
  !undef FrameEverToolsCheck
!macroend

; Cap 0.6.0 ran its GPUI app as cap-gpui.exe next to Cap.exe. The template
; only closes Cap.exe before copying files, so a running cap-gpui.exe would keep
; the shared FFmpeg DLLs locked and stop the update with a file-in-use error.
!macro NSIS_HOOK_PREINSTALL
  !insertmacro CheckIfAppIsRunning "cap-gpui.exe" "${PRODUCTNAME}"
  !insertmacro EndFrameEverMcpServers
  !insertmacro WaitForFrameEverCommandLineTools "$(FrameEverNotUpdated)"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro EndFrameEverMcpServers
  !insertmacro WaitForFrameEverCommandLineTools "$(FrameEverNotUninstalled)"
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Delete "$INSTDIR\cap-gpui.exe"
  Delete "$INSTDIR\cap-exporter.exe"
  Delete "$INSTDIR\cap-cli.exe"
  Delete "$INSTDIR\cap-muxer.exe"
  RMDir /r "$INSTDIR\assets\rive"
  ${If} ${FileExists} "$INSTDIR\frameever-setup.exe"
    WriteRegStr SHCTX "${UNINSTKEY}" "UninstallString" "$\"$INSTDIR\frameever-setup.exe$\" --uninstall"
    WriteRegStr SHCTX "${UNINSTKEY}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
  ${EndIf}
!macroend
