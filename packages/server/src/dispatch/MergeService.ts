import { existsSync } from 'node:fs';
import { basename } from 'node:path';
import {
  BranchMerger, DISPATCH_MARKERS, HarnessError, HUMAN_INTERACTION_PREFIX, isHumanInteraction, MAX_BLOCKED_REASON_LENGTH, MERGE_REASON_PREFIX, MERGE_SESSION_TOOLS, MainCheckout,
  PathKey, TargetBranches, TimeWindowCheck,
  type Board, type BoardActor, type EnsuredBranch, type BoardTicketDetail, type DispatchState, type MergeEntry, type RunEnd, type RunRecord, type RunSettings,
} from '@harness/core';
import { BoardRuntimeHolder } from '../dbboard/BoardRuntimeHolder.js';
import type { DispatchContext, RepoWork } from './DispatchContext.js';
import type { DispatchReport } from './DispatchService.js';

// Offene Aufträge eines Branches mit ihren noch erledigten Tickets
interface BranchJob {
  branch: string;
  entries: MergeEntry[];
  /** Tickets der Aufträge, die noch auf Erledigt mit diesem Branch stehen (erstes = Ticket der Merge-Session) */
  tickets: BoardTicketDetail[];
}

// Ort des Merges: Hauptordner, ein anderer Worktree des Zielbranches oder der eigene Merge-Worktree
interface MergePlace {
  /** Ordner, in dem gemergt wird */
  path: string;
  /** Branch, in den gemergt wird */
  into: string;
  /** Zielbranch der Tickets; null im Fallback (ausgecheckter Branch des Hauptordners) */
  target: string | null;
  /** Selbst geöffneter Merge-Worktree: nach dem Merge wieder schließen */
  opened: boolean;
  /** Bezeichnung des Ordners für Meldungen („Hauptordner“, „Worktree <Pfad>“, „Merge-Worktree <Pfad>“) */
  label: string;
}

// Warum Git den Merge im Ordner gerade abweisen würde; `kind` ist die Art des Grundes (für „schon kommentiert?“)
interface HoldReason {
  kind: 'busy' | 'staged' | 'overlap' | 'leftover';
  text: string;
}

// Bezeichnung des Hauptordners in Meldungen
const MAIN_LABEL = 'Hauptordner';

// Mergt erledigte Tickets in ihren Zielbranch (ohne Zielbranch in den ausgecheckten Branch des Hauptordners), startet bei Konflikten eine Merge-Session
// im Worktree und räumt nach dem Merge auf. Lässt der Ordner des Merges ihn gerade nicht zu (laufende Git-Aktion, vorgemerkte
// oder mit dem Branch überschneidende lokale Änderungen), wartet der Auftrag; andere Probleme setzen das Ticket auf Blockiert
// mit Grund „Merge: …“.
// Aufträge werden je Branch verarbeitet: Tickets eines gemeinsamen Worktrees teilen Phase, Merge-Session und Ergebnis.
export class MergeService {
  private readonly checkout = new MainCheckout();
  private readonly merger = new BranchMerger();
  private readonly window = new TimeWindowCheck();
  private readonly targets = new TargetBranches();
  private readonly keys = new PathKey();

  constructor(private readonly ctx: DispatchContext) {}

  // Alle offenen Aufträge, je Branch höchstens einmal (laufende Merge-Sessions warten auf ihr Ergebnis).
  // Verbindungsfehler brechen den Tick ab, fachliche Fehler landen je Branch im Bericht.
  async processAll(report: DispatchReport): Promise<void> {
    const { merges } = await this.ctx.stores.state.read();
    const branches = [...new Set(Object.values(merges).map((e) => e.branch))];
    for (const branch of branches) {
      try {
        report.merged.push(...(await this.processBranch(branch)));
      } catch (error) {
        if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
        const ids = Object.values(merges).filter((e) => e.branch === branch).map((e) => e.ticketId).join(', ');
        report.errors.push(`${ids}: ${(error as Error).message}`);
      }
    }
  }

  // Nach der Merge-Session: Ziel enthalten und Worktree sauber → erneut mergen (im selben Tick), sonst Blockiert.
  // Fachliche Fehler beim Auswerten blockieren ebenfalls (sonst bliebe der Auftrag hängen); Verbindungsfehler gehen weiter.
  async afterSession(record: RunRecord, end: RunEnd): Promise<void> {
    try {
      await this.evaluateSession(record, end);
    } catch (error) {
      if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      const job = await this.job(record.branch);
      if (job.tickets.length) await this.block(job, null, `Die Merge-Session ließ sich nicht auswerten: ${(error as Error).message}`);
      else await this.dropAll(job);
    }
  }

  // „Erneut mergen“: nur Erledigt mit Branch oder Blockiert mit Grund „Merge: …“ auf einem eigenen Board, nicht während eines Merge-Laufs;
  // setzt alle Aufträge des Branches zurück (auch einen hängenden in Phase „Session“), erlaubt wieder eine Merge-Session; liefert die Ticket-ID
  async retry(id: string, actor: BoardActor): Promise<string> {
    const detail = await this.ctx.services.tickets.get(id);
    const mergeBlocked = detail.status === 'blocked' && (detail.blockedReason ?? '').startsWith(MERGE_REASON_PREFIX);
    if (!detail.branch || (!mergeBlocked && detail.status !== 'done')) {
      throw new HarnessError('merge-not-possible', `${detail.id} wartet auf keinen Merge (nur Erledigt mit Branch oder Blockiert mit Grund „Merge: …“).`);
    }
    const board = await this.ctx.board(detail.boardId);
    if (board && board.ownerId !== this.ctx.me.userId) {
      throw new HarnessError('merge-not-possible', `${detail.id} liegt auf dem Board eines anderen Benutzers.`);
    }
    const branch = detail.branch;
    const entries = this.entriesOf(await this.ctx.stores.state.read(), branch);
    if (await this.sessionRunning(branch, entries)) {
      throw new HarnessError('merge-not-possible', `${detail.id}: Die Merge-Session läuft noch.`);
    }
    if (mergeBlocked) await this.ctx.services.tickets.move(detail.id, 'done', actor, { force: true });
    const since = this.ctx.now().toISOString();
    await this.ctx.stores.state.update((s) => {
      for (const e of Object.values(s.merges)) {
        if (e.branch !== branch) continue;
        e.phase = 'pending';
        e.sessionUsed = false;
        e.target = null;
        e.conflictSeen = false;
        e.heldBy = null;
      }
      s.merges[detail.id] = { ticketId: detail.id, branch, phase: 'pending', sessionUsed: false, waitNoted: false, target: null, conflictSeen: false, since };
    });
    await this.ctx.comment(detail.id, `${DISPATCH_MARKERS.MERGE}: erneuter Merge angestoßen von ${actor.name}.`);
    return detail.id;
  }

  // „Abnehmen“ (Mensch) eines Tickets, das mit Grund „Human Interaction: …“ blockiert ist: Erledigt (Abnahme-Text Pflicht),
  // Grund leeren, Verlauf; liegt es mit Branch auf einem eigenen Board mit Repo, hakt der Harness die Aufgabe ab und stößt den
  // Merge an wie „Erneut mergen“ (merge: true). Sonst wird es nur erledigt; `text` sagt dann, warum nichts gemergt wird
  async accept(id: string, actor: BoardActor): Promise<{ ticketId: string; merge: boolean; text: string }> {
    const detail = await this.ctx.services.tickets.get(id);
    if (detail.status !== 'blocked' || !isHumanInteraction(detail.blockedReason)) {
      throw new HarnessError('accept-not-possible', `${detail.id} wartet auf keine Abnahme (nur Blockiert mit Grund „${HUMAN_INTERACTION_PREFIX} …“).`);
    }
    const board = await this.ctx.board(detail.boardId);
    const own = board !== null && board.ownerId === this.ctx.me.userId && Boolean(board.repoPath);
    if (own && detail.branch) {
      const work = await this.ctx.repo(board);
      await work.manager.accept(detail.id, { repoBoard: work.board }, work.root, actor);
      await this.retry(detail.id, actor);
      return { ticketId: detail.id, merge: true, text: '' };
    }
    await this.ctx.services.tickets.accept(detail.id, actor);
    const why = !detail.branch
      ? 'kein Branch eingetragen, es gibt nichts zu mergen'
      : 'das Board gehört einem anderen Benutzer oder hat keinen Repo-Pfad – gemergt wird dort („Mergen“ bzw. harness board merge)';
    return { ticketId: detail.id, merge: false, text: `${detail.id} abgenommen und erledigt; ${why}.` };
  }

  // Auswertung einer Merge-Session; steht das Ticket nicht mehr auf Erledigt mit dem Branch des Laufs, wird nur sein Auftrag entfernt
  private async evaluateSession(record: RunRecord, end: RunEnd): Promise<void> {
    if (!this.entriesOf(await this.ctx.stores.state.read(), record.branch).length) return;
    const detail = await this.load(record.ticketId);
    if (!detail || detail.status !== 'done' || detail.branch !== record.branch) {
      await this.drop(record.ticketId);
      // Die übrigen Aufträge des Branches warten nicht mehr auf diese Session
      await this.setPhase(record.branch, 'pending');
      return;
    }
    const job = await this.job(record.branch);
    if (!job.tickets.length) return this.dropAll(job);
    const board = await this.ctx.board(detail.boardId);
    const work = board?.repoPath ? await this.ctx.repo(board) : null;
    const target = record.target ?? job.entries.find((e) => e.target)?.target ?? null;
    if (!(work && target && this.resolved(work.root, target, record))) {
      const exit = end.timedOut ? 'Zeitlimit' : `Exit-Code ${end.code}`;
      await this.block(job, work, `Die Merge-Session konnte den Konflikt nicht lösen (${exit}, Protokoll ${basename(record.logFile)}).`);
      return;
    }
    await this.setPhase(record.branch, 'pending');
  }

  // Hat die Merge-Session das Ziel in den Branch geholt und einen sauberen Worktree ohne laufende Git-Aktion hinterlassen?
  // Beide als volle Branch-Referenz, damit ein gleichnamiges Tag nicht gewinnt
  private resolved(root: string, target: string, record: RunRecord): boolean {
    if (!this.merger.contains(root, `refs/heads/${target}`, `refs/heads/${record.branch}`)) return false;
    try {
      const worktree = this.checkout.inspect(record.cwd);
      return !worktree.busy && worktree.blockingFiles.length === 0;
    } catch {
      return false; // Worktree fehlt oder ist kaputt
    }
  }

  // Alle Aufträge eines Branches; liefert die gemergten Tickets
  private async processBranch(branch: string): Promise<string[]> {
    const state = await this.ctx.stores.state.read();
    const entries = this.entriesOf(state, branch);
    if (!entries.length) return [];
    if (await this.sessionRunning(branch, entries)) return [];
    const job = await this.job(branch);
    if (!job.tickets.length) {
      await this.dropAll(job);
      return [];
    }
    // Phase „Session“ ohne Merge-Lauf: die Auswertung fehlt (z. B. abgebrochen) → gilt als ungelöst
    if (job.entries.some((e) => e.phase === 'session')) return this.block(job, null, 'Die Merge-Session endete ohne Auswertung.');
    const primary = job.tickets[0];
    const board = await this.ctx.board(primary.boardId);
    if (!board?.repoPath) return this.block(job, null, 'Das Board hat keinen Repo-Pfad.');
    const work = await this.repoOf(board);
    if (typeof work === 'string') return this.block(job, null, work);
    if (!(await work.manager.isReady(branch))) return this.wait(job, work);
    // Die Bereitschaft ist eine Momentaufnahme aus „finish“: ein wieder geöffnetes Ticket des Worktrees hält den Merge an
    const open = await this.openTickets(job, work);
    if (open.length) return this.wait(job, work, open);
    // Konflikt schon gesehen und die Merge-Session darf noch nicht starten: nicht jede Minute mergen und abbrechen –
    // mit Zielbranch schon vor dem Ort, sonst öffnete jeder Tick einen Merge-Worktree, nur um zu warten
    const notNow = this.sessionNotAllowed(state);
    const conflictWaits = Boolean(notNow) && job.entries.some((e) => e.conflictSeen);
    if (conflictWaits && primary.targetBranch) return [];
    const place = await this.placeOf(job, work);
    if (typeof place === 'string') return this.block(job, work, place);
    // Merge schon im Ziel, Branch gelöscht (Abbruch zwischen Merge und Aufräumen): abschließen statt erneut mergen
    const earlier = this.merger.mergedEarlier(work.root, branch, `refs/heads/${place.into}`);
    if (earlier) return this.finish(job, work, place, earlier, this.close(work, place));
    // Lokale Arbeit im Ordner hält den Merge nur an, wenn Git ihn sonst abweisen würde; dann wird gewartet, nicht blockiert
    const held = this.holdReason(place, branch);
    if (held || conflictWaits) {
      // Ein Merge-Worktree mit liegengebliebenen Änderungen lässt sich nicht entfernen; er bleibt, bis jemand aufräumt
      if (held?.kind !== 'leftover') this.close(work, place);
      return held ? this.hold(job, place, held) : [];
    }
    let outcome: ReturnType<BranchMerger['merge']>;
    try {
      outcome = this.merger.merge(place.path, branch, await this.mergeMessage(job, work), this.ctx.cfg.mergeStrategy);
    } catch (error) {
      // Der eigene Merge ließ sich nicht zurücksetzen: der Ordner braucht einen Menschen
      if (error instanceof HarnessError && error.code === 'merge-abort-failed') return this.block(job, work, this.abortFailure(error, place));
      throw error;
    }
    // Die Dateiliste einer Abweisung braucht den Ordner noch; danach wird ein selbst geöffneter Merge-Worktree geschlossen
    const refusedOverlap = outcome.kind === 'refused' ? this.overlapping(place.path, branch) : [];
    const closeWarning = this.close(work, place);
    if (outcome.kind === 'merged') return this.finish(job, work, place, outcome.commit, closeWarning);
    if (outcome.kind === 'refused') {
      if (refusedOverlap.length) return this.hold(job, place, { kind: 'overlap', text: this.overlapText(refusedOverlap) });
      return this.block(job, work, this.refusal(outcome.message, refusedOverlap));
    }
    if (job.entries.some((e) => e.sessionUsed)) return this.block(job, work, `Merge-Konflikt in ${this.list(outcome.files)} – die Merge-Session hat ihn nicht gelöst.`);
    // Ausgeschaltet, Pause oder außerhalb des Zeitfensters: keine Merge-Session; der Auftrag bleibt und merkt sich den Konflikt
    if (notNow) {
      await this.ctx.stores.state.update((s) => {
        for (const e of Object.values(s.merges)) if (e.branch === branch) e.conflictSeen = true;
      });
      this.ctx.log(`Merge-Konflikt ${primary.id} – ${notNow}, Merge-Session wartet`);
      return [];
    }
    try {
      await this.startSession(job, work, place.into, outcome.files);
    } catch (error) {
      // Ohne Session käme der Auftrag jede Minute wieder; fachliche Fehler blockieren daher das Ticket
      if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      return this.block(job, work, `Merge-Konflikt in ${this.list(outcome.files)}; die Merge-Session startete nicht: ${(error as Error).message}`);
    }
    return [];
  }

  // Ort des Merges: ohne Zielbranch der Hauptordner mit seinem ausgecheckten Branch (bisheriger Ablauf), sonst der des Zielbranches;
  // liefert den Grund, warum dort nicht gemergt werden kann (ob Git den Merge gerade zulässt, prüft holdReason)
  private async placeOf(job: BranchJob, work: RepoWork): Promise<MergePlace | string> {
    const target = job.tickets[0].targetBranch;
    if (target) return this.targetPlace(job, work, target);
    const branch = this.checkout.inspect(work.root).branch;
    if (!branch) return 'Der Hauptordner steht auf keinem Branch (Detached HEAD).';
    const solved = job.entries.find((e) => e.target)?.target ?? null;
    if (solved && solved !== branch) return `Zielbranch gewechselt (Konflikt gelöst gegen ${solved}, ausgecheckt ist ${branch}) – bitte Erneut mergen.`;
    return { path: work.root, into: branch, target: null, opened: false, label: MAIN_LABEL };
  }

  // Zielbranch anlegen, falls er fehlt (Kommentar an alle Tickets), dann im Hauptordner, in einem anderen Worktree des
  // Zielbranches oder im Merge-Worktree mergen
  private async targetPlace(job: BranchJob, work: RepoWork, target: string): Promise<MergePlace | string> {
    let ensured: EnsuredBranch;
    let found: string | null;
    try {
      ensured = this.targets.ensure(work.root, target);
      found = this.targets.checkedOutAt(work.root, target);
    } catch (error) {
      // Git-Fehler (z. B. Ref-Konflikt beim Anlegen) blockieren, sonst käme der Auftrag jede Minute wieder
      if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      return `Zielbranch ${target} nicht nutzbar: ${(error as Error).message}`;
    }
    if (ensured.created) {
      for (const t of job.tickets) await this.ctx.comment(t.id, `${DISPATCH_MARKERS.MERGE}: Zielbranch ${target} neu angelegt von ${ensured.from}.`);
    }
    // Ein eingetragener, aber verschwundener Worktree gilt als nicht ausgecheckt (openMergeWorktree räumt ihn per prune weg)
    const at = found && existsSync(found) ? found : null;
    let place: MergePlace = { path: work.root, into: target, target, opened: false, label: MAIN_LABEL };
    if (at && !this.keys.same(at, work.root) && !this.keys.same(at, this.targets.mergeWorktreePath(work.root, target))) {
      place = { ...place, path: at, label: `Worktree ${at}` };
    } else if (!at || !this.keys.same(at, work.root)) {
      try {
        const path = this.targets.openMergeWorktree(work.root, target);
        place = { ...place, path, opened: true, label: `Merge-Worktree ${path}` };
      } catch (error) {
        if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
        return (error as Error).message;
      }
    }
    return place;
  }

  // Warum Git den Merge im Ordner gerade abweisen würde (laufende Git-Aktion, vorgemerkte oder mit dem Branch überschneidende
  // lokale Änderungen) oder ein selbst geöffneter Merge-Worktree nach dem Merge nicht entfernt werden könnte
  // (liegengebliebene Änderungen eines früheren Versuchs), sonst null
  private holdReason(place: MergePlace, branch: string): HoldReason | null {
    const state = this.checkout.inspect(place.path);
    if (state.busy) {
      const text = place.label === MAIN_LABEL ? state.busy : `Im ${place.label} läuft gerade ${this.checkout.running(place.path) ?? 'eine Git-Aktion'}.`;
      return { kind: 'busy', text };
    }
    if (place.opened && state.blockingFiles.length) return { kind: 'leftover', text: `liegengebliebene Änderungen in ${this.list(state.blockingFiles)}` };
    if (state.stagedFiles.length) return { kind: 'staged', text: `vorgemerkte Änderungen (git add) in ${this.list(state.stagedFiles)}` };
    const overlap = this.overlapping(place.path, branch);
    return overlap.length ? { kind: 'overlap', text: this.overlapText(overlap) } : null;
  }

  // Grund, wenn sich der eigene Merge nicht zurücksetzen ließ: nennt den Ordner, in dem von Hand aufzuräumen ist (im Hauptordner die Meldung des BranchMergers)
  private abortFailure(error: HarnessError, place: MergePlace): string {
    if (place.label === MAIN_LABEL) return error.message;
    const git = typeof error.details?.git === 'string' ? error.details.git : null;
    const command = typeof error.details?.command === 'string' ? error.details.command : null;
    if (git === null || command === null) return `Im ${place.label}: ${error.message}`;
    return `Der ${place.label} steht noch im Merge-Zustand: ${git}. Bitte dort von Hand aufräumen (${command}).`;
  }

  // Schließt einen selbst geöffneten Merge-Worktree; liefert die Warnung, wenn er stehen bleibt, sonst null
  private close(work: RepoWork, place: MergePlace): string | null {
    if (!place.opened) return null;
    const failure = this.targets.closeMergeWorktree(work.root, place.path);
    if (failure === null) return null;
    const warning = `Merge-Worktree ${place.path} nicht entfernt: ${failure.replace(/\s+/g, ' ').trim()}`;
    this.ctx.log(warning);
    return warning;
  }

  // Merge-Nachricht: ein Ticket „Merge <ID> „<Titel>“ (<branch>)“, mehrere Tickets des Worktrees „Merge <ID>, <ID> (<branch>)“;
  // bei Squash „<ID> „<Titel>“ (squash von <branch>)“ bzw. „<ID>, <ID> (squash von <branch>)“
  private async mergeMessage(job: BranchJob, work: RepoWork): Promise<string> {
    const ids = [...new Set([...(await work.manager.ticketsOf(job.branch)), ...job.tickets.map((t) => t.id)])];
    const squash = this.ctx.cfg.mergeStrategy === 'squash';
    const head = ids.length === 1 ? `${ids[0]} „${job.tickets[0].title}“` : ids.join(', ');
    return squash ? `${head} (squash von ${job.branch})` : `Merge ${head} (${job.branch})`;
  }

  // Hauptordner des Boards oder die Meldung, warum der Repo-Pfad nicht nutzbar ist (Verbindungsfehler gehen weiter)
  private async repoOf(board: Board): Promise<RepoWork | string> {
    try {
      return await this.ctx.repo(board);
    } catch (error) {
      if (BoardRuntimeHolder.isConnectionFailure(error)) throw error;
      return `Repo-Pfad ${board.repoPath} nicht nutzbar: ${(error as Error).message}`;
    }
  }

  // Warum gerade keine Merge-Session starten darf (ausgeschaltet, Pause, außerhalb des Zeitfensters), sonst null
  private sessionNotAllowed(state: DispatchState): string | null {
    const now = this.ctx.now();
    if (!this.ctx.cfg.enabled) return 'Dispatcher ausgeschaltet';
    if (state.pausedUntil && new Date(state.pausedUntil) > now) return 'Pause aktiv';
    if (!this.window.contains(now, this.ctx.cfg.window)) return 'außerhalb des Zeitfensters';
    return null;
  }

  // Tickets des Worktrees, die noch diesen Branch haben und wieder offen sind (nicht Erledigt, nicht „Nicht mehr nötig“,
  // nicht wegen des Merges blockiert)
  private async openTickets(job: BranchJob, work: RepoWork): Promise<string[]> {
    const open: string[] = [];
    for (const id of await work.manager.ticketsOf(job.branch)) {
      if (job.tickets.some((t) => t.id === id)) continue;
      const detail = await this.load(id);
      if (!detail || detail.branch !== job.branch || detail.status === 'done' || detail.status === 'not_needed') continue;
      if (detail.status === 'blocked' && (detail.blockedReason ?? '').startsWith(MERGE_REASON_PREFIX)) continue;
      open.push(detail.id);
    }
    return open;
  }

  // Dateien, an denen ein abgewiesener Merge scheitert; lässt sich das nicht ermitteln, keine (dann zählt die Git-Meldung)
  private overlapping(root: string, branch: string): string[] {
    try {
      return this.merger.overlapping(root, branch);
    } catch {
      return [];
    }
  }

  // Worktree noch nicht bereit oder ein Ticket wieder offen: je Ticket einmal kommentieren, dann still warten
  private async wait(job: BranchJob, work: RepoWork, open?: string[]): Promise<string[]> {
    const inWorktree = open ?? (await work.manager.ticketsOf(job.branch));
    for (const entry of job.entries) {
      if (entry.waitNoted || !job.tickets.some((t) => t.id === entry.ticketId)) continue;
      const others = inWorktree.filter((t) => t !== entry.ticketId);
      await this.ctx.comment(entry.ticketId,
        `${DISPATCH_MARKERS.MERGE_WAIT} auf ${others.join(', ') || 'weitere Tickets derselben Spec'} – der Branch wird gemergt, sobald alle Tickets des Worktrees fertig sind.`);
    }
    await this.ctx.stores.state.update((s) => {
      for (const e of Object.values(s.merges)) {
        if (e.branch !== job.branch) continue;
        e.phase = 'waiting';
        e.waitNoted = true;
      }
    });
    return [];
  }

  // Der Ordner lässt den Merge gerade nicht zu: je Art des Grundes einmal kommentieren, dann still warten – eine andere
  // Dateiliste desselben Grundes kommentiert nicht erneut. Jeder Durchlauf prüft erneut und mergt, sobald der Grund weg ist
  // (committet, verworfen, Git-Aktion beendet)
  private async hold(job: BranchJob, place: MergePlace, reason: HoldReason): Promise<string[]> {
    const fresh = job.entries.filter((e) => e.heldBy !== reason.kind);
    if (!fresh.length) return [];
    for (const entry of fresh) {
      await this.ctx.comment(entry.ticketId, `${DISPATCH_MARKERS.MERGE_WAIT} auf den ${place.label}: ${reason.text} – gemergt wird automatisch, sobald das erledigt ist.`);
    }
    await this.ctx.stores.state.update((s) => {
      for (const e of Object.values(s.merges)) {
        if (e.branch !== job.branch) continue;
        e.phase = 'waiting';
        e.heldBy = reason.kind;
      }
    });
    this.ctx.log(`Merge wartet ${job.tickets.map((t) => t.id).join(', ')}: ${reason.text}`);
    return [];
  }

  // Grund für lokale Änderungen, die der Branch ebenfalls ändert
  private overlapText(files: string[]): string {
    return `lokale Änderungen in ${this.list(files)}, die der Branch ebenfalls ändert`;
  }

  // Gemergt: aufräumen, Merge-blockierte Tickets des Worktrees wieder auf Erledigt, alle kommentieren, Aufträge entfernen, melden
  // (ein nicht entfernter Merge-Worktree zählt als unvollständiges Aufräumen)
  private async finish(job: BranchJob, work: RepoWork, place: MergePlace, commit: string, closeWarning: string | null): Promise<string[]> {
    const primary = job.tickets[0];
    const target = place.into;
    const cleaned = await work.manager.removeMerged(job.branch, this.ctx.actor, place.target);
    const warnings = closeWarning ? [...cleaned.warnings, closeWarning] : cleaned.warnings;
    const ids = [...new Set([...cleaned.tickets, ...job.tickets.map((t) => t.id)])];
    const head = `${DISPATCH_MARKERS.MERGED} in ${target} (${commit}).`;
    const lines = warnings.length
      ? [`${head} Aufräumen unvollständig:`, ...warnings]
      : [`${head} Branch ${job.branch} aufgeräumt${cleaned.removed ? `, Worktree ${cleaned.removed} entfernt` : ''}.`];
    const reopened = await this.reopenMergeBlocked(ids);
    for (const id of ids) {
      const extra = reopened.includes(id) ? ['Das Ticket war wegen des Merges blockiert und steht wieder auf Erledigt.'] : [];
      await this.ctx.comment(id, [...lines, ...extra].join('\n'));
    }
    await this.dropAll(job);
    for (const id of ids) await this.ctx.stores.reviews.remove(id);
    this.ctx.log(`gemergt ${ids.join(', ')} in ${target} (${commit})`);
    await this.ctx.notify({ kind: 'merged', ticketId: primary.id, title: primary.title, text: `in ${target} gemergt (${commit})`, repoPath: work.board.repoPath });
    return ids;
  }

  // Tickets, die mit Grund „Merge: …“ auf Blockiert stehen, wieder auf Erledigt (force); liefert deren IDs
  private async reopenMergeBlocked(ids: string[]): Promise<string[]> {
    const reopened: string[] = [];
    for (const id of ids) {
      const detail = await this.load(id);
      if (!detail || detail.status !== 'blocked' || !(detail.blockedReason ?? '').startsWith(MERGE_REASON_PREFIX)) continue;
      await this.ctx.services.tickets.move(id, 'done', this.ctx.actor, { force: true });
      reopened.push(id);
    }
    return reopened;
  }

  // Merge-Session im Worktree des ersten Tickets starten; Phase, Session und Ziel gelten für alle Aufträge des Branches
  private async startSession(job: BranchJob, work: RepoWork, target: string, files: string[]): Promise<void> {
    const detail = job.tickets[0];
    const cwd = await work.manager.ensureWorktree(detail.id);
    const run = this.atLeastDefault(this.ctx.chooser.choose(detail, this.ctx.attempts.failed(detail.comments) + 1));
    const agent = this.ctx.agentName(detail.id);
    const prompt = this.ctx.prompts.merge({ ticketId: detail.id, title: detail.title, agent, branch: job.branch, target, cfg: this.ctx.cfg });
    await this.ctx.launch({ kind: 'merge', ticketId: detail.id, boardId: work.board.id, title: detail.title, agent, cwd, branch: job.branch, target, run, prompt, extraTools: MERGE_SESSION_TOOLS });
    await this.ctx.stores.state.update((s) => {
      for (const e of Object.values(s.merges)) {
        if (e.branch !== job.branch) continue;
        e.phase = 'session';
        e.sessionUsed = true;
        e.target = target;
      }
    });
    for (const t of job.tickets) {
      await this.ctx.comment(t.id,
        `${DISPATCH_MARKERS.MERGE_CONFLICT} in ${this.list(files)} beim Merge in ${target} – Merge-Session gestartet (Profil ${run.profile}).`);
    }
    this.ctx.log(`Merge-Konflikt ${job.tickets.map((t) => t.id).join(', ')} (${files.join(', ')}) – Merge-Session gestartet`);
  }

  // Merge-Session mindestens mit dem Standardprofil
  private atLeastDefault(run: RunSettings): RunSettings {
    const { ladder, defaultProfile, profiles } = this.ctx.cfg;
    if (ladder.indexOf(run.profile) >= ladder.indexOf(defaultProfile)) return run;
    // Grund und Regelnummer bleiben die des gewählten Basisprofils – wie bei einer Eskalation
    return {
      ...profiles[defaultProfile], profile: defaultProfile, why: `${run.why}, Merge-Session mindestens ${defaultProfile}`,
      ...(run.reason ? { reason: run.reason } : {}), ...(run.ruleIndex != null ? { ruleIndex: run.ruleIndex } : {}),
    };
  }

  // Blockiert mit Grund „Merge: …“ alle erledigten Tickets des Worktrees (ohne Worktree-Verwaltung: die der Aufträge),
  // Kommentar je Ticket, Aufträge des Branches entfernen, melden
  private async block(job: BranchJob, work: RepoWork | null, problem: string): Promise<string[]> {
    const full = `${MERGE_REASON_PREFIX} ${problem}`.replace(/\s+/g, ' ').trim();
    const reason = full.length > MAX_BLOCKED_REASON_LENGTH ? `${full.slice(0, MAX_BLOCKED_REASON_LENGTH - 1)}…` : full;
    const ids = new Set(job.tickets.map((t) => t.id));
    if (work) for (const id of await work.manager.ticketsOf(job.branch)) ids.add(id);
    const blocked: BoardTicketDetail[] = [];
    for (const id of ids) {
      const detail = job.tickets.find((t) => t.id === id) ?? (await this.load(id));
      if (!detail || detail.status !== 'done' || detail.branch !== job.branch) continue;
      await this.ctx.services.tickets.update(detail.id, { status: 'blocked', blockedReason: reason }, this.ctx.actor, { force: true });
      await this.ctx.comment(detail.id, `${DISPATCH_MARKERS.MERGE} nicht möglich: ${problem}`);
      blocked.push(detail);
    }
    await this.dropAll(job);
    if (!blocked.length) return [];
    this.ctx.log(`Merge blockiert ${blocked.map((t) => t.id).join(', ')}: ${problem}`);
    const first = blocked[0];
    const board = await this.ctx.board(first.boardId);
    await this.ctx.notify({ kind: 'blocked', ticketId: first.id, title: first.title, text: reason, repoPath: board?.repoPath ?? null });
    return [];
  }

  // Grund einer Abweisung durch Git: die lokal geänderten Dateien, die der Branch ebenfalls ändert; ist diese Liste leer,
  // die Dateien aus der Git-Meldung („would be overwritten by merge:“), sonst die Meldung in einer Zeile
  private refusal(message: string, overlapping: string[]): string {
    if (overlapping.length) return `Git lehnt den Merge ab, er würde lokale Änderungen überschreiben: ${this.list(overlapping)}`;
    const lines = message.split(/\r?\n/);
    const files: string[] = [];
    for (let i = 0; i < lines.length; i += 1) {
      if (!/would be overwritten by merge:\s*$/.test(lines[i])) continue;
      for (i += 1; i < lines.length && /^\s+\S/.test(lines[i]); i += 1) files.push(lines[i].trim());
      i -= 1;
    }
    if (files.length) return `Git lehnt den Merge ab, er würde lokale Änderungen überschreiben: ${this.list(files)}`;
    return `Git lehnt den Merge ab: ${message.replace(/\s+/g, ' ').trim()}`;
  }

  // Aufträge des Branches und ihre Tickets; Aufträge, deren Ticket fehlt, nicht mehr erledigt ist oder den Branch wechselte, fallen weg
  private async job(branch: string): Promise<BranchJob> {
    const entries = this.entriesOf(await this.ctx.stores.state.read(), branch);
    const tickets: BoardTicketDetail[] = [];
    for (const entry of entries) {
      const detail = await this.load(entry.ticketId);
      if (detail && detail.status === 'done' && detail.branch === branch) tickets.push(detail);
      else await this.drop(entry.ticketId);
    }
    return { branch, entries: entries.filter((e) => tickets.some((t) => t.id === e.ticketId)), tickets };
  }

  // Aufträge eines Branches im Zustand
  private entriesOf(state: DispatchState, branch: string): MergeEntry[] {
    return Object.values(state.merges).filter((e) => e.branch === branch);
  }

  // Läuft für den Branch oder eines seiner Tickets eine Merge-Session?
  private async sessionRunning(branch: string, entries: MergeEntry[]): Promise<boolean> {
    const ids = new Set(entries.map((e) => e.ticketId));
    return (await this.ctx.stores.runs.list()).some((r) => r.kind === 'merge' && (r.branch === branch || ids.has(r.ticketId)));
  }

  // Setzt die Phase aller Aufträge eines Branches
  private async setPhase(branch: string, phase: MergeEntry['phase']): Promise<void> {
    await this.ctx.stores.state.update((s) => {
      for (const e of Object.values(s.merges)) if (e.branch === branch) e.phase = phase;
    });
  }

  // Entfernt den Auftrag eines Tickets
  private async drop(ticketId: string): Promise<void> {
    await this.ctx.stores.state.update((s) => {
      delete s.merges[ticketId];
    });
  }

  // Entfernt alle Aufträge des Branches
  private async dropAll(job: BranchJob): Promise<void> {
    await this.ctx.stores.state.update((s) => {
      for (const [id, e] of Object.entries(s.merges)) if (e.branch === job.branch) delete s.merges[id];
    });
  }

  // Ticket oder null, wenn es gelöscht wurde; andere Fehler (Verbindung) gehen weiter
  private async load(id: string): Promise<BoardTicketDetail | null> {
    try {
      return await this.ctx.services.tickets.get(id);
    } catch (error) {
      if (error instanceof HarnessError && error.code === 'ticket-not-found') return null;
      throw error;
    }
  }

  // Höchstens zehn Dateien, danach „und n weitere“; ohne Dateien „unbekannten Dateien“
  private list(files: string[]): string {
    if (!files.length) return 'unbekannten Dateien';
    return files.length > 10 ? `${files.slice(0, 10).join(', ')} und ${files.length - 10} weitere` : files.join(', ');
  }
}
