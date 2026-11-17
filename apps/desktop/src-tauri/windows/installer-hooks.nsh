; Hooks des NSIS-Installers: tragen den Installationsordner (dort liegt nexus.exe) in den
; Benutzer-PATH ein und beim Deinstallieren wieder aus, damit `nexus` im Terminal gefunden wird.
; Gesetzt über PowerShell wie in crates/nexus-core/src/install/windows_user_path.rs: kein
; direkter Registry-Zugriff aus NSIS, dessen Zeichenketten lange PATH-Werte abschneiden würden,
; und `%VAR%`-Einträge bleiben unverändert. Der Ordner kommt über die Umgebungsvariable
; NEXUS_BIN_DIR, damit kein Quoting nötig ist. `$$` ist ein `$` für PowerShell.

!define NEXUS_PATH_READ "$$d = $$env:NEXUS_BIN_DIR.TrimEnd([char]92); \
$$k = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $$true); \
$$p = [string]$$k.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames); \
$$items = @($$p -split ';' | Where-Object { $$_ -ne '' }); \
$$rest = @($$items | Where-Object { $$_.TrimEnd([char]92) -ne $$d })"

; Löscht eine nicht vorhandene Variable; löst dabei die Änderungsmeldung an neue Terminals aus
!define NEXUS_PATH_NOTIFY "[Environment]::SetEnvironmentVariable('NEXUS_PATH_REFRESH', $$null, 'User')"

!define NEXUS_PATH_ADD "${NEXUS_PATH_READ}; \
if ($$rest.Count -ne $$items.Count) { exit 0 }; \
$$k.SetValue('Path', (($$items + $$d) -join ';'), [Microsoft.Win32.RegistryValueKind]::ExpandString); \
${NEXUS_PATH_NOTIFY}"

!define NEXUS_PATH_REMOVE "${NEXUS_PATH_READ}; \
if ($$rest.Count -eq $$items.Count) { exit 0 }; \
$$k.SetValue('Path', ($$rest -join ';'), [Microsoft.Win32.RegistryValueKind]::ExpandString); \
${NEXUS_PATH_NOTIFY}"

; Backticks begrenzen die Zeile, weil das Skript ' und der Aufruf " enthält; ein Plugin-Aufruf
; nimmt sonst stillschweigend nur das Stück bis zum ersten ' als Befehl
!macro NEXUS_RUN_PATH_SCRIPT SCRIPT
  System::Call 'Kernel32::SetEnvironmentVariable(t "NEXUS_BIN_DIR", t "$INSTDIR")i'
  ; Fehlschlag bricht die Installation nicht ab; $0 bleibt für das Hauptskript erhalten
  Push $0
  nsExec::Exec `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "${SCRIPT}"`
  Pop $0
  Pop $0
!macroend

; Programme aus dem Installationsordner. Tauri beendet vor dem Kopieren nur, was nexus-desktop.exe
; benutzt – und zwar alles, auch die Dispatch-Worker (sie laufen als `nexus-desktop.exe
; dispatch-worker`). Ein abgelöster `nexus serve` oder ein `nexus` aus dem PATH überlebt dagegen und
; sperrt nexus.exe; dann scheitern Deinstallation und Installation.
; Bei einem Update beendet nexus-processes.ps1 deshalb nur die App und `serve` und benennt Dateien,
; die Worker oder kurze Aufrufe noch sperren, um. Die Worker laufen aus der umbenannten Datei weiter,
; Tauris Prüfung findet unter dem alten Namen niemanden mehr, und die neue Datei landet daneben.
; Nur wer wirklich deinstalliert, beendet alles – nach Rückfrage, wenn Dispatch-Läufe aktiv sind.
; Das Skript ist für NSIS-Zeichenketten (höchstens 1024 Zeichen) zu lang und kommt als Datei mit.
; __FILEDIR__ muss hier oben ausgewertet werden; in einem Makro wäre es der Ordner von installer.nsi.
!define NEXUS_HOOKS_DIR "${__FILEDIR__}"

; Führt nexus-processes.ps1 im angegebenen Modus aus; der Exit-Code steht danach in $0
!macro NEXUS_RUN_PROCESS_SCRIPT MODE
  InitPluginsDir
  File "/oname=$PLUGINSDIR\nexus-processes.ps1" "${NEXUS_HOOKS_DIR}\nexus-processes.ps1"
  System::Call 'Kernel32::SetEnvironmentVariable(t "NEXUS_BIN_DIR", t "$INSTDIR")i'
  nsExec::Exec `powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\nexus-processes.ps1" -Mode ${MODE}`
  Pop $0
!macroend

; Update ohne Deinstallation. Ist Nexus schon installiert, zeigt Tauri nach der Willkommensseite eine
; Seite „Vor dem Installieren deinstallieren / Nicht deinstallieren“; einstellen lässt sie sich nicht.
; Sie überspringt sich aber selbst, wenn unter dem Uninstall-Schlüssel kein `UninstallString` steht.
; Beim Verlassen der Willkommensseite (diese Datei wird vor den Seiten eingebunden) legt der
; Installer den Wert deshalb unter NexusUninstallString beiseite. Die Installation schreibt ihn neu;
; bricht sie ab oder scheitert, kommt der alte Wert zurück. Im Passiv-Modus (Updater der App) gibt es
; keine Willkommensseite, dort überspringt Tauri die Seite ohnehin.
; Die Funktionen stehen vor den Var- und define-Zeilen der Vorlage: Sie dürfen weder $PassiveMode &
; Co. noch ${UNINSTKEY} benutzen, darum steht der Schlüssel hier ausgeschrieben (Benutzerinstallation,
; productName „Nexus“). Belegt Tauri einen der beiden MUI-Haken einmal selbst, bricht der Build ab.
!define /ifndef NEXUS_UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\Nexus"
!define MUI_PAGE_CUSTOMFUNCTION_LEAVE NexusWelcomeLeave
!define MUI_CUSTOMFUNCTION_ABORT NexusRestoreUninstallString

Function NexusWelcomeLeave
  Push $0
  Push $1
  ; Läuft die App, sagt ein kurzer Hinweis, dass sie beendet wird und die Dispatch-Läufe weiterlaufen
  !insertmacro NEXUS_RUN_PROCESS_SCRIPT workers
  StrCpy $1 $0
  !insertmacro NEXUS_RUN_PROCESS_SCRIPT servers
  ; „error“ statt einer Zahl (PowerShell fehlt) zählt wie 0
  IntCmp $0 0 nexus_hidden nexus_hidden
  StrCpy $0 "Nexus läuft gerade und wird für das Update beendet."
  IntCmp $1 1 nexus_one_run nexus_hint nexus_many_runs
  nexus_one_run:
  StrCpy $0 "$0$\r$\n$\r$\nDer laufende Dispatch-Lauf läuft weiter; sein Ergebnis wertet Nexus nach dem Neustart aus."
  Goto nexus_hint
  nexus_many_runs:
  StrCpy $0 "$0$\r$\n$\r$\nDie $1 laufenden Dispatch-Läufe laufen weiter; ihre Ergebnisse wertet Nexus nach dem Neustart aus."
  nexus_hint:
  MessageBox MB_OKCANCEL|MB_ICONINFORMATION $0 /SD IDOK IDOK nexus_hidden
  Pop $1
  Pop $0
  ; Abbrechen: auf der Willkommensseite bleiben
  Abort
  nexus_hidden:
  ReadRegStr $0 SHCTX "${NEXUS_UNINSTKEY}" "UninstallString"
  StrCmp $0 "" nexus_done
  WriteRegStr SHCTX "${NEXUS_UNINSTKEY}" "NexusUninstallString" $0
  DeleteRegValue SHCTX "${NEXUS_UNINSTKEY}" "UninstallString"
  nexus_done:
  Pop $1
  Pop $0
FunctionEnd

Function NexusRestoreUninstallString
  Push $0
  ReadRegStr $0 SHCTX "${NEXUS_UNINSTKEY}" "NexusUninstallString"
  StrCmp $0 "" +3
  WriteRegStr SHCTX "${NEXUS_UNINSTKEY}" "UninstallString" $0
  DeleteRegValue SHCTX "${NEXUS_UNINSTKEY}" "NexusUninstallString"
  Pop $0
FunctionEnd

Function .onInstFailed
  Call NexusRestoreUninstallString
FunctionEnd

; Beendet App und `serve`, räumt gesperrte Dateien beiseite und leert die Ressourcen-Ordner
!macro NSIS_HOOK_PREINSTALL
  Push $0
  !insertmacro NEXUS_RUN_PROCESS_SCRIPT install
  Pop $0
!macroend

; Der Deinstaller gehört zu einem Update, wenn er mit /UPDATE läuft oder wenn ihn der Installer
; („Vor dem Installieren deinstallieren“) mit `_?=` direkt aus dem Ordner startet; sonst kopiert
; er sich nach %TEMP%. Bei einem echten Deinstallieren fragt er nach, wenn Dispatch-Läufe aktiv
; sind; still gilt „Ja“. Die Sprungmarken stehen nur einmal im Uninstall-Abschnitt.
!macro NSIS_HOOK_PREUNINSTALL
  Push $0
  StrCmp $UpdateMode 1 nexus_update
  StrCmp $EXEDIR $INSTDIR nexus_update
  !insertmacro NEXUS_RUN_PROCESS_SCRIPT workers
  ; „error“ statt einer Zahl (PowerShell fehlt) zählt wie 0
  IntCmp $0 0 nexus_uninstall nexus_uninstall
  MessageBox MB_YESNO|MB_ICONEXCLAMATION|MB_DEFBUTTON2 \
    "Es laufen noch $0 Dispatch-Läufe von Nexus. Sie werden abgebrochen, wenn du Nexus jetzt deinstallierst.$\r$\n$\r$\nTrotzdem deinstallieren?" \
    /SD IDYES IDYES nexus_uninstall
  Pop $0
  Abort "Abgebrochen: Es laufen noch Dispatch-Läufe."
  nexus_uninstall:
  !insertmacro NEXUS_RUN_PROCESS_SCRIPT uninstall
  Goto nexus_done
  nexus_update:
  !insertmacro NEXUS_RUN_PROCESS_SCRIPT update
  nexus_done:
  Pop $0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; Die Installation hat UninstallString neu geschrieben; der beiseite gelegte Wert ist überflüssig
  DeleteRegValue SHCTX "${NEXUS_UNINSTKEY}" "NexusUninstallString"
  !insertmacro NEXUS_RUN_PATH_SCRIPT "${NEXUS_PATH_ADD}"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro NEXUS_RUN_PATH_SCRIPT "${NEXUS_PATH_REMOVE}"
!macroend
