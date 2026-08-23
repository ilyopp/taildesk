
!macro NSIS_HOOK_POSTINSTALL
  FileOpen $0 "$INSTDIR\language.txt" w
  StrCmp $LANGUAGE "1036" write_fr write_en
write_fr:
  FileWrite $0 "fr"
  Goto done
write_en:
  FileWrite $0 "en"
done:
  FileClose $0
!macroend
