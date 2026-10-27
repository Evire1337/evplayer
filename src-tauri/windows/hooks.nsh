; Moving over from EvPlayer 0.3 and older (the Electron version).
;
; The old version updates itself by downloading this installer and running it with
; "--updated /S". So that people end up with exactly one, working EvPlayer:
;  - before installing, the old program is removed with its own uninstaller (silently;
;    it keeps the data folder %APPDATA%\EvPlayer, which the new version reads on first
;    start to bring the library, playlists and settings over)
;  - after installing, the new version is started, the way the old updater would have
;    restarted the old one

; where the Electron installer recorded itself (electron-builder: Software\<guid of appId>)
!define OLD_EVPLAYER_KEY "Software\ddbcecd8-2c4f-51c8-9de1-7dfce20f7a76"

!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R5 HKCU "${OLD_EVPLAYER_KEY}" InstallLocation
  ${If} $R5 != ""
  ${AndIf} $R5 != $INSTDIR
  ${AndIf} ${FileExists} "$R5\Uninstall EvPlayer.exe"
    DetailPrint "Removing the previous EvPlayer (Electron) from $R5"
    ; _?= runs the uninstaller in place, so this waits until it has finished
    ExecWait '"$R5\Uninstall EvPlayer.exe" /S /currentuser _?=$R5'
    ; an uninstaller run in place cannot delete itself
    Delete "$R5\Uninstall EvPlayer.exe"
    RMDir "$R5"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ClearErrors
  ${GetOptions} $CMDLINE "--updated" $R6
  ${IfNot} ${Errors}
    nsis_tauri_utils::RunAsUser "$INSTDIR\${MAINBINARYNAME}.exe" ""
  ${EndIf}
!macroend
