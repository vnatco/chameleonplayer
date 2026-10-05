; Registers Chameleon Player with Windows so it shows up in "Open with"
; and in Settings > Apps > Default apps, where the user can make it the
; default player (Windows 10/11 don't let apps set that themselves).
; SHCTX is HKLM for an all-users install and HKCU for a per-user one.

!define CAPS "Software\Chameleon Player\Capabilities"

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "${CAPS}" "ApplicationName" "Chameleon Player"
  WriteRegStr SHCTX "${CAPS}" "ApplicationDescription" "A cover-first music player. The album cover is the player."
  WriteRegStr SHCTX "${CAPS}" "ApplicationIcon" "$INSTDIR\${MAINBINARYNAME}.exe,0"
  WriteRegStr SHCTX "Software\Classes\.mp3\OpenWithProgids" "ChameleonPlayer.MP3" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".mp3" "ChameleonPlayer.MP3"
  WriteRegStr SHCTX "Software\Classes\.m4a\OpenWithProgids" "ChameleonPlayer.M4A" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".m4a" "ChameleonPlayer.M4A"
  WriteRegStr SHCTX "Software\Classes\.m4b\OpenWithProgids" "ChameleonPlayer.M4A" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".m4b" "ChameleonPlayer.M4A"
  WriteRegStr SHCTX "Software\Classes\.flac\OpenWithProgids" "ChameleonPlayer.FLAC" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".flac" "ChameleonPlayer.FLAC"
  WriteRegStr SHCTX "Software\Classes\.ogg\OpenWithProgids" "ChameleonPlayer.OGG" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".ogg" "ChameleonPlayer.OGG"
  WriteRegStr SHCTX "Software\Classes\.oga\OpenWithProgids" "ChameleonPlayer.OGG" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".oga" "ChameleonPlayer.OGG"
  WriteRegStr SHCTX "Software\Classes\.wav\OpenWithProgids" "ChameleonPlayer.WAV" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".wav" "ChameleonPlayer.WAV"
  WriteRegStr SHCTX "Software\Classes\.aac\OpenWithProgids" "ChameleonPlayer.AAC" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".aac" "ChameleonPlayer.AAC"
  WriteRegStr SHCTX "Software\Classes\.aif\OpenWithProgids" "ChameleonPlayer.AIFF" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".aif" "ChameleonPlayer.AIFF"
  WriteRegStr SHCTX "Software\Classes\.aiff\OpenWithProgids" "ChameleonPlayer.AIFF" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".aiff" "ChameleonPlayer.AIFF"
  WriteRegStr SHCTX "Software\Classes\.aifc\OpenWithProgids" "ChameleonPlayer.AIFF" ""
  WriteRegStr SHCTX "${CAPS}\FileAssociations" ".aifc" "ChameleonPlayer.AIFF"
  WriteRegStr SHCTX "Software\RegisteredApplications" "Chameleon Player" "${CAPS}"
  ; Tell Explorer the associations changed.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  DeleteRegValue SHCTX "Software\RegisteredApplications" "Chameleon Player"
  DeleteRegKey SHCTX "Software\Chameleon Player"
  DeleteRegValue SHCTX "Software\Classes\.mp3\OpenWithProgids" "ChameleonPlayer.MP3"
  DeleteRegValue SHCTX "Software\Classes\.m4a\OpenWithProgids" "ChameleonPlayer.M4A"
  DeleteRegValue SHCTX "Software\Classes\.m4b\OpenWithProgids" "ChameleonPlayer.M4A"
  DeleteRegValue SHCTX "Software\Classes\.flac\OpenWithProgids" "ChameleonPlayer.FLAC"
  DeleteRegValue SHCTX "Software\Classes\.ogg\OpenWithProgids" "ChameleonPlayer.OGG"
  DeleteRegValue SHCTX "Software\Classes\.oga\OpenWithProgids" "ChameleonPlayer.OGG"
  DeleteRegValue SHCTX "Software\Classes\.wav\OpenWithProgids" "ChameleonPlayer.WAV"
  DeleteRegValue SHCTX "Software\Classes\.aac\OpenWithProgids" "ChameleonPlayer.AAC"
  DeleteRegValue SHCTX "Software\Classes\.aif\OpenWithProgids" "ChameleonPlayer.AIFF"
  DeleteRegValue SHCTX "Software\Classes\.aiff\OpenWithProgids" "ChameleonPlayer.AIFF"
  DeleteRegValue SHCTX "Software\Classes\.aifc\OpenWithProgids" "ChameleonPlayer.AIFF"
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
!macroend
