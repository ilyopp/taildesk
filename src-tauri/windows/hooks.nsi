; Hooks d'installation NSIS pour BrainConnect (référencé par tauri.conf.json)

!macro NSIS_HOOK_POSTINSTALL
  ; Mémorise la langue choisie dans l'installeur pour le premier
  ; démarrage de l'application ("English" ou "French")
  FileOpen $0 "$INSTDIR\language.txt" w
  FileWrite $0 $LANGUAGE
  FileClose $0
!macroend
