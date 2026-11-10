# Board-Regel (Nexus)

- Tickets, Board, Kanban, Spalten, Backlog, Planung, Fragen, Notizen und IDs wie `ARC-0042` oder `ARC-F0001` meinen immer das Nexus-Board: Befehle über `nexus board …`, Details im Skill `nexus:board`, Fragen über `nexus:board-question`.
- Nie ein altes Kanban (Befehl `kanban`, IDs `TCK-…`), nie GitHub- oder GitLab-Issues. Azure-DevOps-Work-Items nur lesen und nur auf ausdrückliche Bitte.
- Ein Repo arbeitet auf dem Board seines Repo-Pfads; Zuordnung mit `nexus board boards repo <id> <pfad>` (Pfad = Hauptordner des Repos).
- Autopilot (`autopilot.enabled` in den Dispatcher-Einstellungen, Ausnahme: Label `interaktiv`): Im Autopilot gibt es für Dispatcher-Agents weder Review vor dem Merge noch den Ausgang `Human Interaction`; Fragen von Dispatcher-Agents entscheidet die Recherche (Begründung „Autopilot: …“), Merge-Konflikte bekommen mehrere Merge-Sessions. Menschen- und Session-Fragen beantwortet er nie; Sackgassen blockieren das Ticket und lösen „Autopilot steckt fest“ aus. Details im Skill `nexus:board`, Abschnitt „Autopilot“.
