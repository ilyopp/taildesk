
!macro NSIS_HOOK_PREINSTALL
  nsExec::Exec 'powershell.exe -NoProfile -Command "Get-Process tailscaled, tailscale-ipn -ErrorAction SilentlyContinue | Stop-Process -Force"'
  Pop $0
  ; Ancienne tâche planifiée éventuelle
  nsExec::Exec 'schtasks.exe /end /tn Taildesk'
  Pop $0
  ; Ancien service SCM (versions <= 0.3.0)
  nsExec::Exec 'sc.exe stop Taildesk'
  Pop $0
  nsExec::Exec 'sc.exe delete Taildesk'
  Pop $0
  nsExec::Exec 'sc.exe query Tailscale'
  Pop $0
  IntCmp $0 0 pi_off_found pi_off_done pi_off_done
pi_off_found:
  nsExec::Exec 'sc.exe config Tailscale start= disabled'
  Pop $0
pi_off_done:
  nsExec::Exec 'powershell.exe -NoProfile -EncodedCommand RwBlAHQALQBDAGgAaQBsAGQASQB0AGUAbQAgACcASABLAEwATQA6AFwAUwBZAFMAVABFAE0AXABDAHUAcgByAGUAbgB0AEMAbwBuAHQAcgBvAGwAUwBlAHQAXABFAG4AdQBtAFwAUwBXAEQAXABXAGkAbgB0AHUAbgAnACAALQBFAHIAcgBvAHIAQQBjAHQAaQBvAG4AIABTAGkAbABlAG4AdABsAHkAQwBvAG4AdABpAG4AdQBlACAAfAAgAEYAbwByAEUAYQBjAGgALQBPAGIAagBlAGMAdAAgAHsAIAAkAHAAIAA9ACAARwBlAHQALQBJAHQAZQBtAFAAcgBvAHAAZQByAHQAeQAgAC0ATABpAHQAZQByAGEAbABQAGEAdABoACAAJABfAC4AUABTAFAAYQB0AGgAIAAtAEUAcgByAG8AcgBBAGMAdABpAG8AbgAgAFMAaQBsAGUAbgB0AGwAeQBDAG8AbgB0AGkAbgB1AGUAOwAgAGkAZgAgACgAJABwACAALQBhAG4AZAAgACQAcAAuAEYAcgBpAGUAbgBkAGwAeQBOAGEAbQBlACAALQBlAHEAIAAnAFQAYQBpAGwAZABlAHMAawAnACkAIAB7ACAAcABuAHAAdQB0AGkAbAAgAC8AcgBlAG0AbwB2AGUALQBkAGUAdgBpAGMAZQAgACIAUwBXAEQAXABXAGkAbgB0AHUAbgBcACQAKAAkAF8ALgBQAFMAQwBoAGkAbABkAE4AYQBtAGUAKQAiACAAMgA+ACQAbgB1AGwAbAA7ACAAUgBlAG0AbwB2AGUALQBJAHQAZQBtACAALQBMAGkAdABlAHIAYQBsAFAAYQB0AGgAIAAkAF8ALgBQAFMAUABhAHQAaAAgAC0AUgBlAGMAdQByAHMAZQAgAC0ARgBvAHIAYwBlACAALQBFAHIAcgBvAHIAQQBjAHQAaQBvAG4AIABTAGkAbABlAG4AdABsAHkAQwBvAG4AdABpAG4AdQBlACAAfQAgAH0ACgA='
  Pop $0
  ; Laisser le temps au nettoyage PnP (adaptateur wintun) de se terminer :
  ; un démarrage trop tôt fige le moteur du nouveau daemon (NoState permanent).
  Sleep 6000
!macroend

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
  nsExec::Exec 'netsh advfirewall firewall add rule name="Taildesk" dir=in action=allow program="$INSTDIR\Taildesk.exe" protocol=tcp'
  Pop $0
  ; Lanceur VBS : tailscaled est une application console, on le démarre
  ; masqué (Run ..., 0) pour qu'aucune fenêtre n'apparaisse. Le True final
  ; fait que wscript attend la fin de tailscaled : la tâche reste "Running",
  ; et `schtasks /end /tn Taildesk` tue bien tout l'arbre du daemon.
  FileWrite $1 'CreateObject("Wscript.Shell").Run """$INSTDIR\tailscale-bundle\tailscaled.exe"" --statedir C:\ProgramData\Taildesk --tun Taildesk --no-logs-no-support", 0, True'
  FileClose $1
  ; Tâche planifiée au logon (privilèges élevés) plutôt que service SCM :
  ; tailscaled en contexte service (parent + enfant /subproc) peut rester
  ; figé en "NoState" sans jamais répondre, alors qu'un lancement interactif
  ; élevé se connecte seul de façon fiable. L'app peut aussi relancer la
  ; tâche elle-même sans élévation si le daemon meurt.
  ; La création passe par un script PowerShell généré : les guillemets
  ; imbriqués schtasks-via-cmd cassent le chemin "C:\Program Files\...".
  FileOpen $1 "$INSTDIR\task-setup.ps1" w
  FileWrite $1 "$$vbs = Join-Path $$PSScriptRoot 'daemon-launcher.vbs'$\r$\n"
  FileWrite $1 "$$action = New-ScheduledTaskAction -Execute 'wscript.exe' -Argument ('$\"' + $$vbs + '$\"')$\r$\n"
  FileWrite $1 "$$trigger = New-ScheduledTaskTrigger -AtLogOn$\r$\n"
  FileWrite $1 "$$principal = New-ScheduledTaskPrincipal -UserId $$env:USERNAME -LogonType Interactive -RunLevel Highest$\r$\n"
  FileWrite $1 "Register-ScheduledTask -TaskName 'Taildesk' -Action $$action -Trigger $$trigger -Principal $$principal -Force | Out-Null$\r$\n"
  FileWrite $1 "Start-ScheduledTask -TaskName 'Taildesk'$\r$\n"
  FileClose $1
  nsExec::Exec 'powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$INSTDIR\task-setup.ps1"'
  Pop $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Arrêter la tâche planifiée et les daemons résiduels
  nsExec::Exec 'schtasks.exe /end /tn Taildesk'
  Pop $0
  nsExec::Exec 'schtasks.exe /delete /tn Taildesk /f'
  Pop $0
  nsExec::Exec 'powershell.exe -NoProfile -Command "Get-Process tailscaled -ErrorAction SilentlyContinue | Stop-Process -Force"'
  Pop $0
  ; Nettoyer un éventuel ancien service SCM (versions <= 0.3.0)
  nsExec::Exec 'sc.exe stop Taildesk'
  Pop $0
  nsExec::Exec 'sc.exe delete Taildesk'
  Pop $0
  ; Supprimer TOUT l'état : compte Tailscale mémorisé, données de l'app,
  ; adaptateur réseau résiduel. La réinstallation repart de zéro et redemande
  ; une connexion de compte.
  Sleep 1500
  RMDir /r "$PROGRAMDATA\Taildesk"
  RMDir /r "$LOCALAPPDATA\com.taildesk.app"
  RMDir /r "$APPDATA\com.taildesk.app"
  ; Deuxième passe par sécurité (fichiers déverrouillés en retard)
  RMDir /r "$PROGRAMDATA\Taildesk"
  ; Trace : permet de vérifier quel désinstalleur a réellement tourné
  FileOpen $1 "$TEMP\taildesk-uninstall-ran.txt" w
  FileWrite $1 "nouveau desinstalleur (avec effacement complet), $INSTDIR"
  FileClose $1
  nsExec::Exec 'powershell.exe -NoProfile -EncodedCommand RwBlAHQALQBDAGgAaQBsAGQASQB0AGUAbQAgACcASABLAEwATQA6AFwAUwBZAFMAVABFAE0AXABDAHUAcgByAGUAbgB0AEMAbwBuAHQAcgBvAGwAUwBlAHQAXABFAG4AdQBtAFwAUwBXAEQAXABXAGkAbgB0AHUAbgAnACAALQBFAHIAcgBvAHIAQQBjAHQAaQBvAG4AIABTAGkAbABlAG4AdABsAHkAQwBvAG4AdABpAG4AdQBlACAAfAAgAEYAbwByAEUAYQBjAGgALQBPAGIAagBlAGMAdAAgAHsAIAAkAHAAIAA9ACAARwBlAHQALQBJAHQAZQBtAFAAcgBvAHAAZQByAHQAeQAgAC0ATABpAHQAZQByAGEAbABQAGEAdABoACAAJABfAC4AUABTAFAAYQB0AGgAIAAtAEUAcgByAG8AcgBBAGMAdABpAG8AbgAgAFMAaQBsAGUAbgB0AGwAeQBDAG8AbgB0AGkAbgB1AGUAOwAgAGkAZgAgACgAJABwACAALQBhAG4AZAAgACQAcAAuAEYAcgBpAGUAbgBkAGwAeQBOAGEAbQBlACAALQBlAHEAIAAnAFQAYQBpAGwAZABlAHMAawAnACkAIAB7ACAAcABuAHAAdQB0AGkAbAAgAC8AcgBlAG0AbwB2AGUALQBkAGUAdgBpAGMAZQAgACIAUwBXAEQAXABXAGkAbgB0AHUAbgBcACQAKAAkAF8ALgBQAFMAQwBoAGkAbABkAE4AYQBtAGUAKQAiACAAMgA+ACQAbgB1AGwAbAA7ACAAUgBlAG0AbwB2AGUALQBJAHQAZQBtACAALQBMAGkAdABlAHIAYQBsAFAAYQB0AGgAIAAkAF8ALgBQAFMAUABhAHQAaAAgAC0AUgBlAGMAdQByAHMAZQAgAC0ARgBvAHIAYwBlACAALQBFAHIAcgBvAHIAQQBjAHQAaQBvAG4AIABTAGkAbABlAG4AdABsAHkAQwBvAG4AdABpAG4AdQBlACAAfQAgAH0ACgA='
  Pop $0
  nsExec::Exec 'sc.exe query Tailscale'
  Pop $0
  IntCmp $0 0 un_off_found un_off_done un_off_done
un_off_found:
  nsExec::Exec 'sc.exe config Tailscale start= auto'
  Pop $0
  nsExec::Exec 'sc.exe start Tailscale'
  Pop $0
un_off_done:
  nsExec::Exec 'netsh advfirewall firewall delete rule name="Taildesk"'
  Pop $0
!macroend
