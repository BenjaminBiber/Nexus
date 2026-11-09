# AI Harness

Ein Werkzeug, das die parallele Arbeit mehrerer Claude-Sessions an Tickets koordiniert und sichtbar macht.

## Language

**Ticket**:
Eine Arbeitseinheit mit Beschreibung, Status, Tags, Kommentaren und Diagrammen, gespeichert als eine Markdown-Datei.
_Avoid_: Issue, Task, Karte

**Ticketnummer**:
Die eindeutige, lesbare Kennung eines Tickets (z. B. `T-0042`), vergeben ausschließlich über das CLI.
_Avoid_: Ticket-ID, Key

**Board**:
Die Spaltenansicht der Tickets nach Status.
_Avoid_: Kanban, Tafel

**Spec**:
Eine Spec-Kit-Funktionsbeschreibung (z. B. `001-feature`) mit eigener Aufgabenliste; jede Spec hat ein eigenes Board.
_Avoid_: Feature, Epic

**Zielbranch**:
Der am Ticket hinterlegte Branch, in den der Dispatcher den Ticket-Branch mergt; gibt es ihn noch nicht, legt der Harness ihn an. Ist keiner hinterlegt, wird in den im Hauptordner ausgecheckten Branch gemergt.
_Avoid_: Merge-Branch, Basisbranch

**Kommentar**:
Eine datierte Wortmeldung an einem Ticket von einem Menschen, einer Session oder einem Subagenten; bleibt „ungelesen", bis eine Session ihn berücksichtigt hat.
_Avoid_: Notiz, Anmerkung

**Kommentar (Docs)**:
Eine Anmerkung zu einer markierten Textstelle in einer Spec-Datei, im Glossar oder in einem ADR; ein Agent arbeitet sie ein und markiert sie mit `harness spec resolve` als erledigt. Im Unterschied zum Kommentar am Ticket gehört sie zum Dokument.
_Avoid_: Notiz

**Verlauf**:
Die zeitlich geordnete Folge aller Kommentare und Änderungen an einem Ticket, jeweils mit Datum.
_Avoid_: History, Log

**Favorit**:
Eine mit Stern markierte Session, die dauerhaft aufbewahrt wird, damit man sie später fortsetzen kann.
_Avoid_: Lesezeichen

**Session**:
Ein laufender Claude-Code-Lauf, der einem Ticket zugeordnet ist.
_Avoid_: Sitzung, Agent-Lauf

**Subagent**:
Ein von einer Session gestarteter Hilfs-Agent; gehört zum Ticket seiner Session.
_Avoid_: Sub-Session, Worker

**Frage**:
Ein aufgeschobener Entscheidungspunkt auf Projektebene, den Mensch oder Agenten festhalten, damit er später geklärt wird. Hat Zustand (offen, beantwortet, verworfen), Bezüge, Abhängigkeiten, Recherchen und eine Entscheidung. Verlangt im Unterschied zum Kommentar eine Entscheidung.
_Avoid_: Rückfrage, Issue, Blocker

**Option**:
Ein Lösungsvorschlag zu einer Frage mit Titel, Beschreibung, Vorteilen und Nachteilen; je Recherche ist höchstens eine „empfohlen"; fortlaufend je Frage nummeriert (`O1`, `O2`, …).
_Avoid_: Variante, Alternative

**Recherche**:
Ein datierter Eintrag eines Agenten an einer Frage mit Kontext (Fundstellen im Code) und Optionen; die neueste gilt als aktuell, ältere bleiben sichtbar.
_Avoid_: Analyse

**Entscheidung**:
Die von einem Menschen getroffene Antwort auf eine Frage: gewählte Option oder Freitext, dazu Person, Datum und wahlweise eine Begründung (Pflicht nur beim Verwerfen). Agenten entscheiden nie.
_Avoid_: Antwort des Agenten

**Anpassung**:
Eine Änderung eines Devs am Standard des Harness, die außerhalb der ausgelieferten Dateien liegt und deshalb Updates des Harness ohne Merge übersteht.
_Avoid_: Customizing, Individualisierung, Override, Patch

**Erweiterungspunkt**:
Eine Stelle des Harness, an der Anpassungen erlaubt sind (z. B. der Workflow oder die Skills).
_Avoid_: Hook, Plugin-Punkt

**Leitplanke**:
Eine Vorgabe des Harness, die sich nicht ersetzen, höchstens erweitern lässt.
_Avoid_: Regel, Policy

**Account**:
Eine Claude-Anmeldung mit eigenem Konfigurationsordner (`CLAUDE_CONFIG_DIR`), auf die der Dispatcher Läufe verteilt; „Standard“ ist die Anmeldung des Servers.
_Avoid_: Konto, Profil (ein Profil legt Modell und Grenzen fest)

**Account-Pause**:
Die Pause eines einzelnen Accounts nach einem Nutzungslimit; die übrigen Accounts arbeiten weiter.
_Avoid_: Limit-Sperre

**Kopplung**:
Die Zuordnung eines Telegram-Chats zum eigenen Server über einen in den Einstellungen erzeugten Einmal-Code; erst ein gekoppelter Chat bekommt Meldungen und darf Befehle an den Bot schicken.
_Avoid_: Pairing, Verknüpfung, Verbindung

**Ideen-Entwurf**:
Der von Claude aus einer Sprachnachricht oder einem Text im Telegram-Bot erstellte Vorschlag (Zusammenfassung, Ziel, Titel, Details, Bezüge, mögliche Dubletten); entsteht als Notiz, Ticket oder Kommentar im Board erst, nachdem man ihn über den Knopf „Anlegen“ bestätigt hat.
_Avoid_: Entwurf, Draft, Vorschlag

**Theme**:
Ein Erscheinungsbild der Oberfläche mit ID (Wert von `data-theme`), eigenen CSS-Variablen und optional eigenen Komponenten für Slots.
_Avoid_: Skin, Design (für das Paket)

**Slot**:
Eine Stelle der Oberfläche, die ein Theme durch eine eigene Komponente ersetzen darf (z. B. `Shell`, `NavIcon`, `Progress`); ohne Ersatz gilt die Standard-Komponente.
_Avoid_: Hook, Erweiterungspunkt
