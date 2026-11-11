# SQL-Skripte des Boards

Versions-Skripte (001–003) spielt niemand automatisch ein; additive Erweiterungen (004, 005, …) spielt Nexus selbst
ein, siehe unten. Reihenfolge:

1. `000_create_databases.sql` – einmalig als Administrator; `<LOGIN>` vorher ersetzen.
   Bei bereits angelegten Datenbanken die beiden `ALTER DATABASE … SET READ_COMMITTED_SNAPSHOT ON`-Anweisungen
   am Ende des Skripts **zusätzlich** ausführen (als Administrator; trennt offene Verbindungen). Ohne
   Zeilenversionierung blockieren sich gleichzeitige Schreiber und Leser bis hin zum Deadlock.
2. `001_initial.sql` – in `AiHarness` **und** in `AiHarness_Test` ausführen.
3. `002_questions.sql` – Fragen (Schema-Version 2); in `AiHarness_Test` **und** `AiHarness`, nach 001.
   Das Skript bricht ohne Änderung ab, wenn Version 1 fehlt oder Version 2 schon eingespielt ist.
4. `003_harness.sql` – Harness-Funktionen am Ticket (Schema-Version 3); in `AiHarness_Test` **und** `AiHarness`, nach 002.
   Das Skript bricht ohne Änderung ab, wenn Version 2 fehlt oder Version 3 schon eingespielt ist.
5. `004_dispatch_runs.sql` – Läufe des Dispatchers für das Dashboard (Erweiterung `dispatch-runs`); in `AiHarness_Test` **und**
   `AiHarness`, nach 003 und **vor** dem Update von Nexus. Das Skript bricht ohne Änderung ab, wenn Version 3 fehlt oder die
   Erweiterung schon eingespielt ist. Es erhöht die Schema-Version **nicht**: ältere Stände laufen mit der erweiterten DB weiter.
6. `005_note_structure.sql` – Titel und Zähler an Notizen (Erweiterung `note-structure`); in `AiHarness_Test` **und** `AiHarness`,
   nach 004 und **vor** dem Update von Nexus. Bricht ohne Änderung ab, wenn `dispatch-runs` fehlt oder die Erweiterung schon da ist.
   Die Schema-Version bleibt 3.
7. `007_target_branch.sql` – Zielbranch am Ticket (Erweiterung `target-branch`, Spalte `board.Tickets.TargetBranch`); in `AiHarness_Test` **und**
   `AiHarness`, nach 004 und **vor** dem Update von Nexus. Das Skript bricht ohne Änderung ab, wenn `board.SchemaFeatures` (004) fehlt oder die
   Erweiterung schon eingespielt ist. Es erhöht die Schema-Version **nicht**: ältere Stände lesen und schreiben Tickets ohne die Spalte weiter.
8. Jedes weitere Skript `00N_….sql` – in beiden Datenbanken, bevor das passende Nexus-Update genutzt wird.

`harness` liest `board.SchemaVersion` und bricht mit einem Hinweis auf das fehlende Skript ab,
wenn die höchste Version nicht zur erwarteten passt. Rein additive Änderungen (neue Tabellen, Indizes, Spalten, die NULL erlauben) erhöhen die Version
nicht, sondern tragen sich in `board.SchemaFeatures` ein; ein Stand, der sie braucht, verlangt den Eintrag und nennt das Skript.
Ältere Stände lesen `board.SchemaFeatures` nicht und laufen weiter. Die Version steigt nur bei Änderungen, mit denen ältere Stände
nicht mehr arbeiten können.

## Erweiterungen spielt Nexus selbst ein

Rein additive Skripte (`004_dispatch_runs.sql`, `005_note_structure.sql` und folgende – erkennbar daran, dass sie die
Schema-Version nicht erhöhen) spielt Nexus beim Verbinden selbst ein, sofern der Dienst-Login `db_ddladmin` hat.
Der zweite Aufruf findet dann nichts mehr zu tun.

Fehlt das Recht, scheitert der Verbindungsaufbau **nicht**: es gibt eine Warnung, und nur die Befehle, die die neuen
Spalten wirklich brauchen (etwa `nexus board note …`), melden die fehlende Erweiterung samt Skript. `show`, `comment`,
`step`, `done` und `search` laufen weiter.

Für bestehende Installationen reicht ein Einzeiler, als Administrator in `AiHarness` **und** in `AiHarness_Test`:

```sql
ALTER ROLE db_ddladmin ADD MEMBER [<LOGIN>];
```

Versions-Skripte (001–003) bleiben Handarbeit: sie sind nicht abwärtskompatibel, ältere Stände könnten danach nicht
mehr arbeiten.
