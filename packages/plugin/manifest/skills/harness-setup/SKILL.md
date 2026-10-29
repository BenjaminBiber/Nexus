---
name: harness-setup
description: Richtet den Befehl nexus im normalen Terminal ein, damit er überall ohne Claude Code funktioniert, und führt bei Bedarf das Einrichten des Projekts und das Öffnen des Boards durch. Verwenden, wenn nexus im Terminal nicht gefunden wird, nach der ersten Installation des Plugins oder nach einem Plugin-Update.
---

# nexus im Terminal einrichten

Der Benutzer soll dafür nichts von Hand tun. Du führst die Schritte selbst aus und fragst nur, wo unten ausdrücklich steht, dass du fragst.

## 1. Installieren

Führe aus:

```
nexus install
```

Der Befehl kopiert das Binary nach `~/.nexus/bin`, legt die Startdateien dort an und ergänzt den PATH des Benutzers (nur Benutzerebene, nie systemweit). Er richtet außerdem das Plugin in Claude Code ein. Er ist wiederholbar und aktualisiert die Kopie nach einem Plugin-Update.

- Meldet er, dass er nur aus dem gebauten Plugin läuft, kommt der Aufruf nicht aus dem installierten Plugin. Sage dem Benutzer, dass er im Repository einmal `npm run setup` ausführen soll, und beende.
- Meldet er einen Fehler beim PATH, führe `nexus install --no-path` aus und nenne dem Benutzer den Ordner `~/.nexus/bin`, den er selbst in den PATH aufnehmen muss.

## 2. Ergebnis prüfen

Prüfe die Kopie, ohne den PATH zu brauchen:

```
nexus --help
```

Nenne dem Benutzer in einem Satz: Neue Terminals kennen `nexus` jetzt in jedem Ordner. Bereits offene Terminals müssen neu geöffnet werden.

## 3. Projekt einrichten (nur auf Wunsch)

Ist das aktuelle Verzeichnis ein Git-Projekt ohne Nexus, frage einmal: „Soll ich dieses Projekt jetzt einrichten und das Board öffnen?“ Bei „Ja“:

```
nexus init
nexus board
```

`nexus init` legt außerdem die lokalen Rules für Doku-Regeln und Board-Regel an (Board-Regel: Tickets, Board, Fragen und Notizen meinen immer das Nexus-Board); in schon eingerichteten Projekten ergänzt ein erneutes `nexus init` fehlende Rules. `nexus init` nennt, welches Board dem Repo zugeordnet ist. Ist keines zugeordnet, zeige dem Benutzer die Boards mit `nexus board boards list` und frage, welches zu diesem Repo gehört; ordne es dann mit `nexus board boards repo <id> <pfad>` zu (Pfad = Hauptordner des Repos). Fehlt die Board-Konfiguration, verweise auf Einstellungen → Board-Datenbank in der Oberfläche.

Bei „Nein“ erwähne die Befehle nur und höre auf. Richte nie ungefragt ein Projekt ein; Nexus startet nur dort, wo der Benutzer es will.

## Regeln

- Tickets nie von Hand ändern; dieser Skill legt keine Tickets an.
- Ändere keine Dateien außerhalb von `~/.nexus` und dem PATH des Benutzers.
- Schreibe Texte für Menschen in natürlichem Deutsch ohne wörtlich übersetzte Fachwörter.
