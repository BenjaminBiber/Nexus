import type { HarnessHome } from '@harness/core';
import type { WorkerLauncher } from '../dispatch/WorkerLauncher.js';
import type { BoardBackendFactory } from '../dbboard/BoardBackend.js';
import type { FolderPicker } from '../dbboard/FolderPicker.js';
import type { FileOpener } from '../docs/FileOpener.js';
import type { ClaudeUsageDto } from '../usage/ClaudeUsageService.js';
import type { ClaudeRunner } from '../dispatch/SuggestionAnalyzer.js';

// Einstellungen des Servers.
export interface ServerConfig {
  port: number;
  host: string;
  home: HarnessHome;
  /** Ordner der gebauten Oberfläche; null, wenn es keinen gibt */
  webDir: string | null;
  version: string;
  /** Ordner mit den Bildern der Windows-Benachrichtigung */
  toastDir?: string;
  /**
   * Läuft der Server als Sidecar hinter dem Rust-Server? Dann gehört `server.json` dem Rust-Prozess,
   * der gewählte Port geht auf stdout und ein Ende der stdin beendet den Server.
   */
  sidecar?: boolean;
  /** Öffentlicher Port des Rust-Servers; Links in Meldungen zeigen dorthin, nicht auf den internen Port */
  publicPort?: number;
  /** Abstand der Herzschlag-Meldungen in Millisekunden (Standard 15000) */
  heartbeatMs?: number;
  /** Board-Laufzeit für /api/db (Standard: BoardRuntime aus ~/.harness/board.yaml); Tests setzen eine In-Memory-Laufzeit */
  boardBackend?: BoardBackendFactory;
  /** Abstand der Abfrage des Änderungskennwerts in Millisekunden (Standard 2500) */
  dbPollMs?: number;
  /** Takt des Dokument-Watchers in Millisekunden (Standard 10000) */
  docsPollMs?: number;
  /** Abstand der Abfrage neuer Fragen auf der Board-Datenbank für Windows-Meldungen (Standard 10000, 0 = aus) */
  dbQuestionPollMs?: number;
  /** Takt des Dispatchers in Millisekunden (Standard 60000, 0 = aus) */
  dispatchTickMs?: number;
  /** Starter der Worker (Standard: losgelöster Prozess); Tests setzen eine Attrappe */
  dispatchLauncher?: WorkerLauncher;
  /** Ordnerdialog für „Mein Repo-Pfad“ (Standard: Windows-Dialog); Tests setzen eine Attrappe */
  folderPicker?: FolderPicker;
  /** Öffnet Dokumente im Explorer oder in VS Code (Standard: Windows); Tests setzen eine Attrappe */
  fileOpener?: FileOpener;
  /** Quelle der Claude-Limits für GET /api/claude-usage (Standard: OAuth-Endpunkt von Anthropic); Tests setzen eine Attrappe */
  claudeUsage?: { current(): Promise<ClaudeUsageDto> };
  /** Claude-Aufruf der Optimierungs-Analyse; Standard: ClaudeProcess (Tests setzen einen Ersatz) */
  suggestionClaude?: ClaudeRunner;
  /** Claude-Aufruf der Ticket-Entwürfe; Standard: ClaudeProcess (Tests setzen einen Ersatz) */
  ticketDraftClaude?: ClaudeRunner;
  /** Claude-Aufruf des Umschreibens eines Dokument-Bereichs; Standard: ClaudeProcess (Tests setzen einen Ersatz) */
  reviseClaude?: ClaudeRunner;
  /** Protokollzeile schreiben (Standard: nichts) */
  log?: (line: string) => void;
}
