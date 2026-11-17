# Prozesse, die aus dem Installationsordner laufen: die App, ihre Dispatch-Worker
# (`nexus-desktop.exe dispatch-worker <lauf>` bzw. `nexus.exe dispatch-worker <lauf>`), ein
# abgelöster `nexus serve` und kurze `nexus`-Aufrufe aus dem PATH. Solange einer davon läuft, ist
# seine Programmdatei gesperrt, und der Installer bricht mit „Fehler beim Öffnen der Datei zum
# Schreiben“ ab.
#
# Aufruf aus installer-hooks.nsh; der Ordner kommt über NEXUS_BIN_DIR, damit kein Quoting nötig ist.
#   -Mode update     Update: beendet nur die App und `serve`. Dispatch-Worker und kurze Aufrufe laufen
#                    weiter; ihre gesperrten Dateien werden umbenannt (das erlaubt Windows auch bei
#                    laufenden Programmen), damit der Installer die neuen unter dem alten Namen anlegt.
#   -Mode install    wie update; leert danach die Ressourcen-Ordner web, toast und plugin. Sie werden
#                    ganz neu kopiert, und ohne Deinstallation blieben sonst die Dateien alter Builds
#                    liegen (die Namen der JS- und CSS-Dateien ändern sich mit jedem Build).
#   -Mode workers    Exit-Code = Zahl der laufenden Dispatch-Worker
#   -Mode servers    Exit-Code = Zahl der Prozesse, die ein Update beendet (App, `serve`)
#   -Mode uninstall  beendet alles aus dem Ordner, auch die Worker
param([Parameter(Mandatory = $true)][ValidateSet('install', 'update', 'workers', 'servers', 'uninstall')][string]$Mode)

$ErrorActionPreference = 'SilentlyContinue'
$dir = $env:NEXUS_BIN_DIR
if (-not $dir) { exit 0 }
$dir = $dir.TrimEnd([char]92)
$prefix = $dir + [char]92
# Namensteil umbenannter Dateien; was frei geworden ist, löscht das nächste Update
$aside = '.nexus-old'

function Get-InstallProcesses {
    Get-CimInstance Win32_Process | Where-Object {
        $_.ProcessId -ne $PID -and $_.ExecutablePath -and
        $_.ExecutablePath.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase) -and
        # Ruft der Installer den alten Deinstaller auf, läuft der aus dem Ordner selbst
        -not $_.ExecutablePath.EndsWith('uninstall.exe', [StringComparison]::OrdinalIgnoreCase)
    }
}

function Test-Worker($process) {
    $process.CommandLine -match '\sdispatch-worker\s'
}

# Die App selbst und ein `nexus serve`: Server, die die neue Version ersetzt
function Test-Server($process) {
    if (Test-Worker $process) { return $false }
    $process.Name -ieq 'nexus-desktop.exe' -or $process.CommandLine -match '\sserve(\s|$)'
}

function Stop-Matching([scriptblock]$filter) {
    # Mehrere Runden: ein Claude-Hook oder ein Terminal kann `nexus` gerade wieder starten
    for ($round = 0; $round -lt 10; $round++) {
        $running = @(Get-InstallProcesses | Where-Object $filter)
        if ($running.Count -eq 0) { return }
        $running | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
        Start-Sleep -Milliseconds 300
    }
}

function Move-LockedFiles {
    Get-ChildItem -LiteralPath $dir -Recurse -Force -File -Include '*.exe', '*.dll' |
        Where-Object { $_.Name -notlike "*$aside*" -and $_.Name -ine 'uninstall.exe' } |
        ForEach-Object {
            $file = $_
            try {
                [IO.File]::Open($file.FullName, 'Open', 'ReadWrite', 'None').Close()
            } catch {
                $newName = $file.Name + $aside + '-' + [DateTime]::Now.Ticks
                Rename-Item -LiteralPath $file.FullName -NewName $newName -Force
            }
        }
}

function Remove-AsideFiles {
    Get-ChildItem -LiteralPath $dir -Recurse -Force -File -Filter "*$aside*" |
        ForEach-Object { Remove-Item -LiteralPath $_.FullName -Force }
}

switch ($Mode) {
    'workers' {
        exit @(Get-InstallProcesses | Where-Object { Test-Worker $_ }).Count
    }
    'servers' {
        exit @(Get-InstallProcesses | Where-Object { Test-Server $_ }).Count
    }
    'update' {
        Stop-Matching { Test-Server $_ }
        Remove-AsideFiles
        Move-LockedFiles
        exit 0
    }
    'install' {
        Stop-Matching { Test-Server $_ }
        Remove-AsideFiles
        Move-LockedFiles
        foreach ($resources in 'web', 'toast', 'plugin') {
            Remove-Item -LiteralPath (Join-Path $dir $resources) -Recurse -Force
        }
        exit 0
    }
    'uninstall' {
        Stop-Matching { $true }
        Move-LockedFiles
        Remove-AsideFiles
        exit 0
    }
}
