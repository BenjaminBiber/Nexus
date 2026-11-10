# Dispatcher-Autopilot – Design

Stand: 2026-11-10 · Zielbranch: `feat/dispatcher-autopilot`

## Ziel

Ein Feature soll vom ersten bis zum letzten Ticket durchlaufen, ohne dass der Nutzer freigeben,
abnehmen, Fragen beantworten oder Merge-Konflikte lösen muss. Unterbrechen darf der Dispatcher nur
noch bei echten Sackgassen (fehlender Zugang, kaputte Konfiguration, wiederholt gescheiterte
Lösungsversuche) – und dann mit einer Benachrichtigung.

## Heute (warum trotzdem Unterbrechungen kommen)

- Die Arbeitsanweisung (`crates/nexus-core/src/dispatch/prompts/work.txt`) bietet den Ausgang
  „Human Interaction – fertig, ein Mensch muss es noch ansehen“. Agents nutzen ihn auch, wenn
  `reviewBeforeMerge` aus ist.
- Agents und Planungen stellen Fragen und beenden den Lauf; das Ticket wartet auf eine Entscheidung
  des Menschen. Die Recherche liefert zwar eine Empfehlung, entscheiden darf aber nur ein Mensch
  (CLI und Route setzen `AuthorKind::Human`).
- Bei einem Merge-Konflikt startet genau eine Merge-Session (`MergeEntry.session_used`). Scheitert
  sie oder endet ohne Auswertung, wird das Ticket mit `Merge: …` blockiert.
- `reviewBeforeMerge` hält erledigte Tickets mit `Human Interaction: Review vor dem Merge` an.

## Entscheidungen

| Thema | Entscheidung |
|---|---|
| Ansatz | Ein Schalter `autopilot`, den die beteiligten Stellen direkt abfragen (kein nachgelagerter Aufräumer) |
| Geltung | Global für alle Boards des Dispatchers; Ausnahme über ein Label je Ticket |
| Fragen | Werden weiter angelegt; eine Recherche läuft sofort, die empfohlene Option wird automatisch übernommen |
| Sackgasse | Weitere Versuche mit größerem Profil, danach Blockiert plus ntfy |
| Markierung automatischer Entscheidungen | Über die Vorsilbe `Autopilot:` im Begründungstext der Entscheidung – keine Datenbank-Migration |

## 1. Konfiguration und Oberfläche

`~/.nexus/dispatch.yaml`, neuer Block (Werte = Vorgaben):

```yaml
autopilot:
  enabled: false          # Hauptschalter
  exceptLabel: interaktiv # Tickets mit diesem Label laufen wie heute; null = keine Ausnahme
  maxMergeSessions: 3     # Merge-Sessions je Stand des Zielbranches (1–5)
```

- Rust: `AutopilotSettings { enabled, except_label, max_merge_sessions }` in
  `crates/nexus-core/src/dispatch/config.rs`, Feld `autopilot` in `DispatchConfig`, geprüft im
  `DispatchConfigNormalizer` (Objekt; `maxMergeSessions` ganze Zahl 1–5; `exceptLabel` leer → null),
  im Beispiel (`example()`) enthalten, TS-Typ über `ts_rs` (`dispatch.ts`).
- Eine einzige Entscheidungsstelle: `pub fn autopilot_for(cfg: &DispatchConfig, labels: &[String]) -> bool`
  (Schalter an und Ticket ohne Ausnahme-Label, Vergleich ohne Groß-/Kleinschreibung). Alle anderen
  Stellen fragen nur diese Funktion.
- Konstante `AUTOPILOT_PREFIX = "Autopilot:"` neben `REVIEW_REASON`.
- Oberfläche (`packages/web/src/components/DispatchPanel.tsx`): eigener Abschnitt „Autopilot“ oben
  mit Schalter, Ausnahme-Label und Merge-Versuchen; Info-Text erklärt, was wegfällt und was bleibt.
  „Vor dem Merge auf Freigabe warten“ ist bei eingeschaltetem Autopilot ausgegraut mit Hinweis
  „Autopilot hat Vorrang (außer Tickets mit Label …)“.
- Ticket-Panel: Abzeichen „Autopilot“ an Tickets, für die `autopilot_for` gilt. Automatisch
  entschiedene Fragen zeigen „vom Autopilot entschieden“.

## 2. Arbeitsläufe und „Human Interaction“

- Neuer Platzhalter `{{outcomes}}` in `work.txt`: Er enthält die Liste der Ausgänge. Ohne Autopilot
  ist der Text wortgleich zu heute. Im Autopilot fehlt der Ausgang „Human Interaction“, dafür steht
  dort: „Es gibt keine Abnahme durch einen Menschen. Was du prüfen kannst, prüfst du selbst (Tests,
  Build) und schließt mit *erledigt* ab.“ Fragen und „blockiert mit Grund“ bleiben.
- Eigene Vorlagen ohne `{{outcomes}}` bleiben gültig (Pflichtteile unverändert). `WorkPromptInput`
  bekommt `autopilot: bool`, gesetzt aus `autopilot_for` für das Ticket.
- Sicherheitsnetz in `ResultHandler::evaluate` (`crates/nexus-server/src/dispatch/result_handler.rs`):
  Steht ein Autopilot-Ticket nach dem Lauf auf Blockiert mit `Human Interaction: …`, nimmt der
  Dispatcher es ab wie „Abnehmen“ (`MergeService::accept`, Akteur = Dispatcher). Kommentar
  „Autopilot: automatisch abgenommen – <ursprünglicher Grund>“. Ausgang des Laufs = Done, keine
  Blockiert-Benachrichtigung.
- `reviewBeforeMerge` wird für Autopilot-Tickets übersprungen (gleiche Stelle, Bedingung
  `ctx.cfg.review_before_merge && !autopilot && branch.is_some()`).
- Planungsläufe: unverändert, ihre Fragen laufen über Abschnitt 3.

## 3. Fragen werden automatisch entschieden

Gilt für offene Fragen, die ein Agent des Dispatchers gestellt hat – `origin_kind != Human` und
`origin_name` beginnt mit `{agentPrefix}-` (Umsetzung, z. B. `junior-AIH-0001`) oder `planner-`
(Planung) – und die mindestens ein Ticket betreffen, für das `autopilot_for` gilt
(„Autopilot-Frage“). Fragen interaktiver Claude-Sessions bleiben beim Menschen.

1. **Recherche-Start**: `ResearchRunner` startet Autopilot-Fragen in jedem Durchlauf, auch wenn
   `research.auto` aus ist und ohne Zählung gegen `research.maxPerDay`. `research.wip` gilt weiter;
   Autopilot-Fragen haben Vorrang vor normalen automatischen Recherchen. Pause und Zeitfenster
   gelten wie für Recherchen.
2. **Auto-Antwort**: Nach einer gespeicherten Recherche mit genau einer Option `recommended: true`
   beantwortet der Dispatcher die Frage über `QuestionService::answer` mit dieser Option.
   Akteur: `BoardQuestionActor { actor: ctx.actor() (Dispatcher), kind: AuthorKind::Session, session: Lauf-ID }`,
   Begründung `Autopilot: Empfehlung der Recherche übernommen`. Hilfsfunktion
   `is_autopilot_decision(&BoardQuestionDecision) -> bool` prüft die Vorsilbe. „Wieder öffnen“
   funktioniert unverändert.
3. **Ohne Empfehlung** (keine oder mehrere `recommended`, Lauf ohne Recherche): zweite Recherche mit
   der nächsten Profilstufe der Leiter. Ohne Empfehlung danach: die betroffenen Autopilot-Tickets
   gehen auf Blockiert mit `Autopilot: keine Empfehlung zu <Frage-ID>`, Benachrichtigung (Abschnitt 5).
   Die Zahl der Versuche je Frage steht im Dispatcher-State (`autopilot_research: {frage: versuche}`).
4. Die Antwort fließt wie heute über `{{answers}}` in den nächsten Arbeitslauf; das Ticket wird
   wieder bereit, sobald keine Frage mehr offen ist.

Fragen, die ein Mensch selbst gestellt hat, beantwortet der Autopilot nie.

## 4. Merge-Konflikte: mehrere Sessions entlang der Leiter

- `MergeEntry` (`crates/nexus-core/src/dispatch/state/state_store.rs`) bekommt
  `#[serde(default)] sessions: u8` = Merge-Sessions für den aktuellen Stand des Zielbranches.
  Laden alter Zustände: `session_used: true` und `sessions == 0` gilt als 1. `session_used` bleibt
  für die Kompatibilität und wird weiter gepflegt.
- Scheitert eine Merge-Session (nicht gelöst, Zeitlimit, „endete ohne Auswertung“) bei einem
  Autopilot-Ticket und gilt `sessions < autopilot.maxMergeSessions`, wird nicht blockiert: Der
  Auftrag geht zurück auf `Pending`, `session_used = false`, die nächste Session startet beim
  nächsten Durchlauf. Ohne Autopilot bleibt es bei einer Session.
- Profil: Versuch 1 wie heute (`at_least_default`), jeder weitere eine Stufe höher auf der Leiter
  (oberste Stufe wiederholt sich). Start-Kommentar nennt „Merge-Session n/max“ und die Eskalation.
- Neuer Platzhalter `{{previousAttempts}}` in `merge.txt`: ab Versuch 2 der letzte Kommentar der
  vorigen Merge-Session am Ticket und der Name ihres Protokolls; sonst leer.
- Läuft der Zielbranch während der Session weiter, setzt der Merge-Service den Zähler wie heute zurück
  (`sessions = 0`). „Erneut mergen“ setzt ihn ebenfalls zurück.
- Wartegründe (lokale Änderungen, laufende Git-Aktion, Dispatcher aus/pausiert) warten wie heute.

## 5. Echte Sackgassen und Benachrichtigung

Blockiert wird im Autopilot nur noch bei:

| Fall | Grund am Ticket |
|---|---|
| Alle Merge-Sessions gescheitert | `Merge: Autopilot – <n> Merge-Sessions ohne Lösung (Protokoll …)` |
| Zwei Recherchen ohne Empfehlung | `Autopilot: keine Empfehlung zu <Frage-ID>` |
| Agent blockiert mit externem Grund | unverändert |
| Abbrüche bis `maxAttempts` | unverändert |
| Board ohne Repo-Pfad, Einrichtung gescheitert | unverändert |

- Neue `DispatchNoticeKind::Stuck` („stuck“): „Autopilot steckt fest: <ID> – <Grund>“, gesendet bei
  jeder Blockade eines Autopilot-Tickets (statt der normalen Blockiert-Meldung).

## 6. Tests

- nexus-core: Normalizer (Vorgaben, Grenzen, Fehlermeldungen), `autopilot_for`, Rendering von
  `{{outcomes}}` (ohne Autopilot wortgleich) und `{{previousAttempts}}`, Laden alter `MergeEntry`.
- nexus-server (`FakeClaude`): Auto-Abnahme bei Human Interaction; `reviewBeforeMerge`
  übersprungen; Auto-Antwort mit Empfehlung; zweiter Versuch ohne Empfehlung, danach Blockiert;
  Fragen von Menschen bleiben offen; Merge mit zwei gescheiterten Sessions und Erfolg in der dritten
  inkl. Profil-Eskalation; Blockiert nach `maxMergeSessions`; Ausnahme-Label verhält sich wie heute;
  Stuck-Benachrichtigung.
- Web (vitest): Abschnitt „Autopilot“ im DispatchPanel, Ausgrauen des Review-Schalters, Abzeichen.
- Doku: Board-Skill und -Regel (`packages/plugin/manifest/skills/board/SKILL.md`,
  `packages/plugin/manifest/rules/board.md`) beschreiben den Autopilot.

## Nicht im Umfang

- Autopilot je Board oder je Spec.
- Automatisches Auflösen externer Blocker (Zugang, Umgebung).
- Änderungen am Planungs-Prompt.
