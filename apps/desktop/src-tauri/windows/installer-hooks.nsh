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

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro NEXUS_RUN_PATH_SCRIPT "${NEXUS_PATH_ADD}"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro NEXUS_RUN_PATH_SCRIPT "${NEXUS_PATH_REMOVE}"
!macroend
