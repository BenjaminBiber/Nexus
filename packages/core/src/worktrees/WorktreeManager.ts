import { existsSync, readdirSync, rmdirSync, rmSync } from 'node:fs';
import { readFile, writeFile } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';
import { GitRunner } from '../git/GitRunner.js';
import { PathKey } from '../git/PathKey.js';
import { LONG_PATHS_ARGS, LONG_PATHS_NOTICE, LongPaths } from '../git/LongPaths.js';
import { BranchMerger } from '../dispatch/git/BranchMerger.js';
import { TargetBranches } from '../dispatch/git/TargetBranches.js';
import { HarnessError } from '../errors/HarnessError.js';
import { SlugBuilder } from '../text/SlugBuilder.js';
import type { BoardServices } from '../board/BoardServices.js';
import type { Board, BoardActor, Ticket, TicketStatus } from '../board/BoardTypes.js';
import type { HarnessLayout } from '../project/HarnessLayout.js';
import { MERGE_REASON_PREFIX, type JoinWorktrees } from '../dispatch/DispatchConfig.js';
import { ProjectPluginSwitch } from '../install/ProjectPluginSwitch.js';
import { PROJECT_RULE_PATHS, ProjectRules } from '../install/ProjectRules.js';
import { ClaudeImportApproval } from '../install/ClaudeImportApproval.js';
import { TouchesChecker } from './TouchesChecker.js';
import { WorktreeCleaner } from './WorktreeCleaner.js';
import { WorktreeLinks } from './WorktreeLinks.js';
import { WorktreeRegistry } from './WorktreeRegistry.js';

export interface StartResult {
  id: string;
  branch: string;
  path: string;
  /** true, wenn das Ticket einem bestehenden Worktree beigetreten ist */
  joined: boolean;
  warning?: string;
  /** Pfade fertiger Worktrees, die dabei aufgeräumt wurden */
  removed?: string[];
  /** Hinweis, wenn der Zielbranch des Tickets dabei neu angelegt wurde (nennt den Startpunkt) */
  targetCreated?: string;
}

export interface FinishResult {
  id: string;
  branch: string | null;
  readyToMerge: boolean;
  warnings: string[];
  /** Pfade fertiger Worktrees, die dabei aufgeräumt wurden */
  removed: string[];
}

/** Fortsetzung eines gestarteten Tickets: sein Worktree – oder der Hinweis, dass Branch und Worktree nicht mehr zu ihm gehören
 *  und am Ticket geleert wurden (dann startet es neu) */
export type ResumeResult = { path: string } | { reset: string };

export interface WorktreeManagerOptions {
  /** none = jedes Ticket im eigenen Worktree („bereit“, sobald seine Tickets fertig sind); sameSpec = Worktree bleibt offen,
   *  solange ein ungestartetes Ticket derselben Spec wartet (Dispatcher-Einstellung joinWorktrees) */
  joinWorktrees?: JoinWorktrees;
}

/** Board, das dem Repo zugeordnet ist; Tickets anderer Boards werden abgelehnt */
export type RepoBoard = Pick<Board, 'id' | 'name'>;

// Lokale Plugin-Einstellung des Harness im Worktree; zählt nie als Änderung
const HARNESS_FILE = '.claude/settings.local.json';
// Alle lokalen Dateien des Harness im Worktree (Plugin-Einstellung und Rules); zählen nie als Änderung
const HARNESS_FILES: ReadonlySet<string> = new Set([HARNESS_FILE, ...PROJECT_RULE_PATHS]);

// Fertige Tickets halten keinen Worktree mehr offen
const FINISHED: ReadonlySet<TicketStatus> = new Set(['done', 'not_needed']);

// Verwaltet Branch und Worktree je Ticket des DB-Boards: ein Branch pro Worktree, Tickets ohne Überschneidung dürfen teilen.
// Gemergt wird nur vom Dispatcher; removeMerged räumt danach auf. Das Verzeichnis der Worktrees bleibt eine lokale Datei im Git-Verzeichnis.
export class WorktreeManager {
  readonly registry: WorktreeRegistry;
  private readonly cleaner: WorktreeCleaner;
  private readonly git = new GitRunner();
  private readonly longPaths = new LongPaths();
  private readonly merger = new BranchMerger();
  private readonly targets = new TargetBranches();
  private readonly slugs = new SlugBuilder();
  private readonly touches = new TouchesChecker();
  private readonly links = new WorktreeLinks();
  private readonly keys = new PathKey();
  private readonly pluginSwitch = new ProjectPluginSwitch();
  private readonly rules = new ProjectRules();
  private readonly importApproval = new ClaudeImportApproval();
  private readonly joinWorktrees: JoinWorktrees;

  constructor(
    layout: HarnessLayout,
    private readonly services: Pick<BoardServices, 'tickets' | 'db'>,
    private readonly root: string,
    options: WorktreeManagerOptions = {},
  ) {
    this.joinWorktrees = options.joinWorktrees ?? 'none';
    this.registry = new WorktreeRegistry(layout);
    this.cleaner = new WorktreeCleaner(this.registry, this.git);
  }

  // Startet ein Ticket: Sperren prüfen, fertige Worktrees aufräumen, dann Beitritt zu `inTicket`, mit `join` sonst zu einem
  // passenden offenen Worktree derselben Spec, sonst neuer Worktree. Verknüpft auf Wunsch die Session.
  async start(
    id: string, options: { inTicket?: string; join?: boolean; force?: boolean; sessionId?: string; repoBoard?: RepoBoard; joinOnlySameSpec?: boolean },
    cwd: string, actor: BoardActor,
  ): Promise<StartResult> {
    const ticket = await this.services.tickets.get(id);
    await this.assertRepoBoard(ticket, options.repoBoard);
    if (ticket.branch) throw new HarnessError('already-started', `${ticket.id} läuft bereits auf Branch ${ticket.branch}.`);
    await this.services.tickets.checkStart(ticket.id, { force: options.force });
    const removed = await this.cleaner.cleanup(cwd);
    let result: StartResult | undefined;
    let warning: string | undefined;
    if (options.inTicket) {
      const joined = await this.tryJoin(ticket, options.inTicket, actor, options.force);
      if ('result' in joined) result = joined.result;
      else warning = joined.warning;
    } else if (options.join) {
      result = await this.autoJoin(ticket, actor, options.force, options.joinOnlySameSpec);
    }
    if (!result) {
      const started = await this.startNew(ticket, cwd, actor, options.force);
      result = { ...started, warning: [warning, started.warning].filter(Boolean).join(' ') || undefined };
    }
    if (options.sessionId) await this.services.tickets.linkSession(ticket.id, options.sessionId);
    return { ...result, removed };
  }

  // Schließt ein Ticket ab: Erledigt (Abnahme-Pflicht), Worktree „bereit“ sobald alle Tickets fertig sind.
  // Ein fertiger, sauberer Worktree wird aufgeräumt (der Branch bleibt), sofern `cwd` nicht darin liegt.
  async finish(id: string, options: { acceptance?: string; force?: boolean; repoBoard?: RepoBoard }, cwd: string, actor: BoardActor): Promise<FinishResult> {
    const ticket = await this.services.tickets.get(id);
    await this.assertRepoBoard(ticket, options.repoBoard);
    await this.services.tickets.move(ticket.id, 'done', actor, { acceptance: options.acceptance, force: options.force });
    const readyToMerge = ticket.branch ? await this.updateReady(ticket.branch) : false;
    const removed = await this.cleaner.cleanup(cwd);
    return { id: ticket.id, branch: ticket.branch, readyToMerge, warnings: [], removed };
  }

  // Worktree eines gestarteten Tickets; fehlt der Ordner (aufgeräumt), wird er aus dem Branch neu angelegt.
  // Ein vorhandener Ordner muss als Worktree genau dieses Branches eingetragen sein.
  async ensureWorktree(id: string): Promise<string> {
    const ticket = await this.services.tickets.get(id);
    if (!ticket.branch || !ticket.worktree) throw new HarnessError('ticket-not-started', `${ticket.id} läuft in keinem Worktree.`);
    if (existsSync(ticket.worktree)) {
      if (!this.isWorktreeOf(ticket.worktree, ticket.branch)) {
        throw new HarnessError(
          'worktree-mismatch',
          `Der Ordner ${ticket.worktree} ist kein Worktree von Branch ${ticket.branch}. Bitte den Ordner prüfen und von Hand aufräumen.`,
        );
      }
      return ticket.worktree;
    }
    this.git.run(this.root, ['worktree', 'prune']);
    this.longPaths.ensure(this.root);
    this.git.run(this.root, [...LONG_PATHS_ARGS, 'worktree', 'add', ticket.worktree, ticket.branch]);
    await this.registry.markRestored(ticket.branch);
    await this.enablePlugin(ticket.worktree);
    return ticket.worktree;
  }

  // Ist der Worktree des Branches „bereit zum Merge“? Ein Worktree je Ticket: jetzt geprüft (alle seine Tickets fertig, auch
  // wenn eines von Hand auf Erledigt kam); gebündelt: die Markierung aus „finish“ (alle fertig, keines derselben Spec wartet)
  async isReady(branch: string): Promise<boolean> {
    if (this.joinWorktrees === 'none') return this.updateReady(branch);
    return (await this.registry.read())[branch]?.readyToMerge === true;
  }

  // Abnahme durch einen Menschen (Blockiert mit Grund „Human Interaction: …“): Erledigt, Grund leeren, Verlauf, danach wie
  // `finish` den Worktree als bereit markieren
  async accept(id: string, options: { repoBoard?: RepoBoard }, cwd: string, actor: BoardActor): Promise<FinishResult> {
    const ticket = await this.services.tickets.get(id);
    await this.assertRepoBoard(ticket, options.repoBoard);
    await this.services.tickets.accept(ticket.id, actor);
    const readyToMerge = ticket.branch ? await this.updateReady(ticket.branch) : false;
    const removed = await this.cleaner.cleanup(cwd);
    return { id: ticket.id, branch: ticket.branch, readyToMerge, warnings: [], removed };
  }

  // Fortsetzung eines gestarteten Tickets (Dispatcher): Gehört der eingetragene Worktree laut Verzeichnis einem anderen Ticket
  // oder wurde er aufgeräumt, wird er nicht wiederhergestellt – Branch und Worktree am Ticket werden geleert (der alte Branch
  // bleibt im Repo) und das Ticket startet neu. Sonst der Worktree wie bei `ensureWorktree` (fehlender Ordner wird neu angelegt).
  async resume(id: string, actor: BoardActor): Promise<ResumeResult> {
    const ticket = await this.services.tickets.get(id);
    if (!ticket.branch || !ticket.worktree) throw new HarnessError('ticket-not-started', `${ticket.id} läuft in keinem Worktree.`);
    const why = await this.staleReason(ticket);
    if (!why) return { path: await this.ensureWorktree(ticket.id) };
    await this.services.tickets.update(ticket.id, { branch: null, worktree: null }, actor, { force: true });
    return {
      reset: `Der eingetragene Worktree ${ticket.worktree} (Branch ${ticket.branch}) ${why} und wird nicht wiederhergestellt; ` +
        `Branch und Worktree am Ticket geleert, es startet auf einem eigenen neuen Branch (der alte Branch bleibt im Repo).`,
    };
  }

  // Warum der eingetragene Worktree des Tickets nicht (mehr) zu ihm gehört – laut Verzeichnis einem anderen Ticket oder
  // aufgeräumt –, sonst null (auch ohne Branch oder ohne Eintrag im Verzeichnis); ändert nichts
  async staleReason(ticket: Pick<Ticket, 'id' | 'branch'>): Promise<string | null> {
    if (!ticket.branch) return null;
    const entry = (await this.registry.read())[ticket.branch];
    if (!entry) return null;
    if (!entry.tickets.includes(ticket.id)) return `gehört ${entry.tickets.join(', ') || 'einem anderen Ticket'}`;
    return entry.removed ? 'wurde aufgeräumt' : null;
  }

  // Steht die Einrichtung (worktreeSetup) des Worktrees noch aus? Nur für Worktrees, die der Dispatcher mit Einrichtung anlegte
  async setupPending(branch: string): Promise<boolean> {
    return (await this.registry.read())[branch]?.setup === 'pending';
  }

  // Merkt, dass der Worktree eingerichtet werden muss (pending) bzw. eingerichtet ist (done)
  async markSetup(branch: string, state: 'pending' | 'done'): Promise<void> {
    await this.registry.setSetup(branch, state);
  }

  // Tickets, die im Worktree dieses Branches arbeiten (leer, wenn es keinen Eintrag gibt)
  async ticketsOf(branch: string): Promise<string[]> {
    return (await this.registry.read())[branch]?.tickets ?? [];
  }

  // Nach dem Merge: nur wenn der Branch im Zielbranch steckt (ohne Zielbranch: im ausgecheckten HEAD des Hauptordners) oder
  // mit seiner aktuellen Spitze per Squash übernommen ist (siehe BranchMerger.integrated), den sauberen Worktree entfernen
  // (ohne Gewalt), Branch löschen und erst dann Branch/Worktree an allen Tickets des Eintrags leeren. Sonst Warnungen.
  // Mit Zielbranch wird immer auf der geprüften Spitze gelöscht, weil `git branch -d` gegen HEAD statt gegen das Ziel prüft.
  async removeMerged(branch: string, actor: BoardActor, target?: string | null): Promise<{ removed: string | null; tickets: string[]; warnings: string[] }> {
    const entry = (await this.registry.read())[branch];
    const tickets = entry?.tickets ?? [];
    const warnings: string[] = [];
    const ref = target ? `refs/heads/${target}` : 'HEAD';
    // Schon gemergt und gelöscht (Abbruch beim letzten Aufräumen): nur noch Verzeichnis und Tickets nachziehen
    if (this.merger.mergedEarlier(this.root, branch, ref)) {
      if (entry && !entry.removed && !existsSync(entry.path)) await this.registry.markRemoved(branch);
      this.git.tryRun(this.root, ['worktree', 'prune']);
      for (const t of await this.load(tickets)) {
        if (t.branch === branch) await this.services.tickets.update(t.id, { branch: null, worktree: null }, actor, { force: true });
      }
      return { removed: null, tickets, warnings };
    }
    if (!this.merger.integrated(this.root, branch, ref)) {
      return { removed: null, tickets, warnings: [`Branch ${branch} ist nicht gemergt; Worktree und Branch bleiben.`] };
    }
    // Per Squash übernommen oder in einen anderen Zielbranch als HEAD gemergt: Git hält den Branch für ungemergt,
    // gelöscht wird er nur auf der geprüften Spitze
    const squashedTip = !target && this.merger.contains(this.root, `refs/heads/${branch}`, 'HEAD') ? undefined : this.merger.head(this.root, `refs/heads/${branch}`);
    let removed: string | null = null;
    if (entry && !entry.removed && existsSync(entry.path) && this.isEmptyDir(entry.path)) {
      // Rest eines halb entfernten Worktrees (Windows ließ den gesperrten leeren Ordner stehen): ohne .git kein Worktree;
      // rmdirSync löscht nur leere Ordner
      rmdirSync(entry.path);
      await this.registry.markRemoved(branch);
      removed = entry.path;
    } else if (entry && !entry.removed && existsSync(entry.path)) {
      if (this.keys.same(entry.path, this.root)) {
        warnings.push(`Der Worktree-Eintrag von Branch ${branch} zeigt auf den Hauptordner ${entry.path}; er wird nicht entfernt.`);
        return { removed: null, tickets, warnings };
      }
      if (!this.isWorktreeOf(entry.path, branch)) {
        warnings.push(`Der Ordner ${entry.path} ist kein Worktree von Branch ${branch}; er wird nicht angefasst. Bitte von Hand prüfen.`);
        return { removed: null, tickets, warnings };
      }
      if (!this.isClean(entry.path)) {
        warnings.push(`Worktree ${entry.path} hat ungesicherte Änderungen und bleibt stehen.`);
        return { removed: null, tickets, warnings };
      }
      // Symlinks/Junctions (z. B. node_modules auf den Hauptordner) zuerst entfernen – nur die Links, nie ihr Ziel;
      // sonst könnte Git beim Entfernen durch den Link hindurch löschen
      const links = this.links.removeAll(entry.path);
      if (links.failed.length) {
        warnings.push(`Worktree ${entry.path} nicht entfernt: Links ließen sich nicht entfernen (${links.failed.join(', ')}). Bitte von Hand prüfen.`);
        return { removed: null, tickets, warnings };
      }
      // Plugin-Einstellung und Rules liegen nicht im Git und würden `worktree remove` ohne --force verhindern
      for (const file of HARNESS_FILES) rmSync(join(entry.path, ...file.split('/')), { force: true });
      const attempt = this.git.tryRun(this.root, ['worktree', 'remove', entry.path]);
      if (!attempt.ok) {
        const plugin = await this.enablePlugin(entry.path);
        warnings.push(`Worktree ${entry.path} nicht entfernt: ${attempt.err}`, ...(plugin ? [plugin] : []));
        return { removed: null, tickets, warnings };
      }
      await this.registry.markRemoved(branch);
      removed = entry.path;
    }
    this.git.tryRun(this.root, ['worktree', 'prune']);
    const failed = this.merger.deleteBranch(this.root, branch, squashedTip);
    if (failed) {
      warnings.push(`${failed} Branch und Worktree bleiben an den Tickets eingetragen.`);
      return { removed, tickets, warnings };
    }
    for (const t of await this.load(tickets)) {
      if (t.branch === branch) await this.services.tickets.update(t.id, { branch: null, worktree: null }, actor, { force: true });
    }
    return { removed, tickets, warnings };
  }

  // Sauber: keine geänderten oder neuen Dateien außer den lokalen Dateien des Harness (Umbenennungen zählen mit beiden Pfaden)
  private isClean(path: string): boolean {
    try {
      const entries = this.git.raw(path, ['-c', 'core.quotePath=false', 'status', '--porcelain=v1', '-z', '--untracked-files=all']).split('\0');
      for (let i = 0; i < entries.length; i += 1) {
        const entry = entries[i];
        if (entry.length < 4) continue;
        if (!HARNESS_FILES.has(entry.slice(3))) return false;
        // Umbenennung/Kopie: der nächste Eintrag ist der alte Pfad
        if ('RC'.includes(entry[0]) || 'RC'.includes(entry[1])) {
          i += 1;
          if (entries[i] && !HARNESS_FILES.has(entries[i])) return false;
        }
      }
      return true;
    } catch {
      return false;
    }
  }

  // Ist der Ordner in `git worktree list` als Worktree des Branches eingetragen?
  private isWorktreeOf(path: string, branch: string): boolean {
    const wanted = this.keys.of(path);
    let current: string | null = null;
    for (const line of this.git.run(this.root, ['worktree', 'list', '--porcelain']).split('\n')) {
      const text = line.trim();
      if (text.startsWith('worktree ')) current = this.keys.of(text.slice('worktree '.length));
      else if (text === `branch refs/heads/${branch}` && current === wanted) return true;
      else if (text === '') current = null;
    }
    return false;
  }

  // Ordner ohne jeden Eintrag (auch keine versteckten Dateien)
  private isEmptyDir(path: string): boolean {
    try {
      return readdirSync(path).length === 0;
    } catch {
      return false;
    }
  }

  // Lehnt Tickets ab, die nicht auf dem Board des Repos liegen (Worktrees entstehen immer neben diesem Repo)
  private async assertRepoBoard(ticket: Ticket, repoBoard: RepoBoard | undefined): Promise<void> {
    if (!repoBoard || ticket.boardId === repoBoard.id) return;
    const own = await this.services.db.transaction((u) => u.boards.byId(ticket.boardId));
    const ticketBoard = `Board ${ticket.boardId}${own ? ` „${own.name}“` : ''}`;
    throw new HarnessError(
      'board-repo-mismatch',
      `${ticket.id} liegt auf ${ticketBoard}, dieses Repo gehört aber zu Board ${repoBoard.id} „${repoBoard.name}“. ` +
        `Das Ticket im Repo seines Boards bearbeiten oder die Zuordnung mit „harness board boards repo <id> <pfad>“ ändern.`,
      { ticketBoard: ticket.boardId, repoBoard: repoBoard.id },
    );
  }

  // Versucht den Beitritt; bei anderem Zielbranch oder Überschneidung kommt eine Warnung statt eines Ergebnisses zurück
  private async tryJoin(ticket: Ticket, hostId: string, actor: BoardActor, force?: boolean): Promise<{ result: StartResult } | { warning: string }> {
    const host = await this.services.tickets.get(hostId);
    if (!host.branch || !host.worktree) throw new HarnessError('host-not-started', `${host.id} läuft in keinem Worktree.`);
    if (!this.sameTarget(ticket, host)) return { warning: `${ticket.id} hat einen anderen Zielbranch als ${host.id}; es wird ein eigener Worktree angelegt.` };
    const entry = (await this.registry.read())[host.branch];
    const other = await this.overlapping(ticket, entry?.tickets ?? []);
    if (other) return { warning: `Die Bereiche von ${ticket.id} überschneiden sich mit ${other.id}; es wird ein eigener Worktree angelegt.` };
    return { result: await this.join(ticket, host.branch, host.worktree, actor, force) };
  }

  // Branch, dem `start` das ungestartete Ticket automatisch beitreten würde (ohne Beitritt: undefined); ändert nichts.
  // Der Dispatcher prüft damit vor der Übernahme, ob in diesem Worktree gerade ein anderer Worker läuft.
  async joinCandidate(id: string, options: { joinOnlySameSpec?: boolean } = {}): Promise<string | undefined> {
    const ticket = await this.services.tickets.get(id);
    if (ticket.branch) return undefined;
    return (await this.findJoin(ticket, options.joinOnlySameSpec))?.branch;
  }

  // Tritt dem ersten offenen Worktree derselben Spec bei, dessen laufende Tickets sich nicht überschneiden.
  // sameSpecOnly: Tickets ohne Spec treten nie bei (Dispatcher: jedes Ticket ohne Spec bekommt seinen eigenen Branch).
  private async autoJoin(ticket: Ticket, actor: BoardActor, force?: boolean, sameSpecOnly?: boolean): Promise<StartResult | undefined> {
    const found = await this.findJoin(ticket, sameSpecOnly);
    return found ? this.join(ticket, found.branch, found.path, actor, force) : undefined;
  }

  // Erster offener Worktree derselben Spec und desselben Zielbranches ohne Überschneidung, dem das Ticket beitreten kann
  private async findJoin(ticket: Ticket, sameSpecOnly?: boolean): Promise<{ branch: string; path: string } | undefined> {
    if (sameSpecOnly && !ticket.spec) return undefined;
    for (const [branch, entry] of Object.entries(await this.registry.read())) {
      if (entry.removed || entry.readyToMerge || !existsSync(entry.path)) continue;
      const others = await this.load(entry.tickets);
      if (!others.every((o) => (o.spec ?? null) === (ticket.spec ?? null))) continue;
      if (!others.every((o) => this.sameTarget(ticket, o))) continue;
      if (await this.overlapping(ticket, entry.tickets)) continue;
      return { branch, path: entry.path };
    }
    return undefined;
  }

  // Haben beide Tickets denselben Zielbranch (kein Zielbranch zählt als gleich)?
  private sameTarget(a: Pick<Ticket, 'targetBranch'>, b: Pick<Ticket, 'targetBranch'>): boolean {
    return (a.targetBranch ?? null) === (b.targetBranch ?? null);
  }

  // Erstes noch nicht fertige Ticket der Liste, dessen Bereiche sich mit dem Ticket überschneiden
  private async overlapping(ticket: Ticket, ids: string[]): Promise<Ticket | undefined> {
    return (await this.load(ids)).find((o) => o.id !== ticket.id && !FINISHED.has(o.status) && this.touches.overlaps(ticket.touches, o.touches));
  }

  // Nimmt das Ticket in den bestehenden Worktree auf und setzt es auf In Arbeit
  private async join(ticket: Ticket, branch: string, path: string, actor: BoardActor, force?: boolean): Promise<StartResult> {
    await this.registry.addTicket(branch, ticket.id);
    await this.services.tickets.update(ticket.id, { branch, worktree: path, status: 'in_progress' }, actor, { force });
    return { id: ticket.id, branch, path, joined: true };
  }

  // Legt Branch und Worktree neu an, nachdem alle Voraussetzungen geprüft sind; hat das Ticket einen Zielbranch, wird er bei
  // Bedarf lokal angelegt (immer relativ zum Hauptordner, die Refs sind gemeinsam) und der Ticket-Branch zweigt von ihm ab
  private async startNew(ticket: Ticket, cwd: string, actor: BoardActor, force?: boolean): Promise<StartResult> {
    const branch = `ticket/${ticket.id}-${this.slugs.build(ticket.title)}`;
    const path = join(dirname(this.root), `${basename(this.root)}-worktrees`, ticket.id).replace(/\\/g, '/');
    this.assertReady(branch, path, cwd);
    const target = ticket.targetBranch;
    const ensured = target ? this.targets.ensure(this.root, target) : undefined;
    const longPaths = this.longPaths.ensure(cwd) ? LONG_PATHS_NOTICE : undefined;
    this.git.run(cwd, [...LONG_PATHS_ARGS, 'worktree', 'add', '-b', branch, path, ...(target ? [`refs/heads/${target}`] : [])]);
    await this.registry.add(branch, path, ticket.id);
    await this.services.tickets.update(ticket.id, { branch, worktree: path, status: 'in_progress' }, actor, { force });
    const warnings = [longPaths, await this.enablePlugin(path), await this.addRules(path)].filter(Boolean);
    const targetCreated = ensured?.created ? `Zielbranch ${target} neu angelegt von ${ensured.from}.` : undefined;
    return { id: ticket.id, branch, path, joined: false, warning: warnings.join(' ') || undefined, ...(targetCreated ? { targetCreated } : {}) };
  }

  // Schaltet das Plugin im neuen Worktree ein (die lokale Einstellung liegt nicht im Git); Fehler sind nur eine Warnung
  private async enablePlugin(path: string): Promise<string | undefined> {
    try {
      await this.pluginSwitch.enable(path);
      return undefined;
    } catch (error) {
      return `Das Plugin konnte im Worktree nicht eingeschaltet werden: ${(error as Error).message}`;
    }
  }

  // Legt die Rules für Doku- und Board-Regeln im neuen Worktree an (liegen wie die Einstellung nicht im Git) und gibt
  // ihre Importe für den Worktree frei, der für Claude Code ein eigenes Projekt ist; Fehler sind nur eine Warnung
  private async addRules(path: string): Promise<string | undefined> {
    try {
      await this.rules.ensure(path);
      await this.importApproval.approve(path);
      return undefined;
    } catch (error) {
      return `Die Rules für Doku- und Board-Regeln konnten im Worktree nicht angelegt werden:${(error as Error).message}`;
    }
  }

  // Prüft erster Commit, freien Branch-Namen und freien Ordner, bevor etwas angelegt wird
  private assertReady(branch: string, path: string, cwd: string): void {
    try {
      this.git.run(cwd, ['rev-parse', '--verify', 'HEAD']);
    } catch {
      throw new HarnessError('no-commit', 'Bitte zuerst einen Commit anlegen, sonst kann kein Worktree entstehen.');
    }
    if (this.branchExists(branch, cwd)) throw new HarnessError('branch-exists', `Branch ${branch} existiert bereits.`);
    if (existsSync(path)) throw new HarnessError('worktree-exists', `Der Ordner ${path} existiert bereits.`);
  }

  // Gibt es den Branch schon?
  private branchExists(branch: string, cwd: string): boolean {
    try {
      this.git.run(cwd, ['rev-parse', '--verify', '--quiet', `refs/heads/${branch}`]);
      return true;
    } catch {
      return false;
    }
  }

  // „Bereit zum Merge“, wenn alle Tickets des Worktrees fertig sind; gebündelt (sameSpec) zusätzlich erst, wenn kein Ticket
  // derselben Spec mehr auf den Start wartet. Schreibt das Verzeichnis nur bei einer Änderung
  private async updateReady(branch: string): Promise<boolean> {
    const entry = (await this.registry.read())[branch];
    if (!entry) return false;
    // Nur Tickets, die noch auf diesem Branch arbeiten (nach --clear-branch oder Wechsel zählen sie nicht mehr)
    const all = (await this.load(entry.tickets)).filter((t) => t.branch === branch);
    // Ein vom Merge blockiertes Ticket („Merge: …“) ist fertig – sonst wartet „Erneut mergen“ auf sich selbst
    const ready = all.length > 0 && all.every((t) => FINISHED.has(t.status) || (t.status === 'blocked' && (t.blockedReason ?? '').startsWith(MERGE_REASON_PREFIX))) && !(this.joinWorktrees === 'sameSpec' && (await this.hasWaitingTickets(all)));
    if (entry.readyToMerge !== ready) await this.registry.setReady(branch, ready);
    return ready;
  }

  // Wartet auf den Boards dieser Tickets in einer ihrer Specs noch ein ungestartetes, offenes Ticket?
  private async hasWaitingTickets(inWorktree: Ticket[]): Promise<boolean> {
    const specs = new Set(inWorktree.flatMap((t) => (t.spec ? [t.spec] : [])));
    if (specs.size === 0) return false;
    const boardIds = [...new Set(inWorktree.map((t) => t.boardId))];
    const tickets = await this.services.db.transaction((u) => u.tickets.list({ boardIds }));
    return tickets.some((t) => t.spec !== null && specs.has(t.spec) && !t.branch && !FINISHED.has(t.status));
  }

  // Lädt Tickets zu IDs in deren Reihenfolge; gelöschte fehlen
  private async load(ids: string[]): Promise<Ticket[]> {
    if (!ids.length) return [];
    const found = new Map((await this.services.db.transaction((u) => u.tickets.list({ ids }))).map((t) => [t.id, t]));
    return ids.flatMap((id) => (found.has(id) ? [found.get(id)!] : []));
  }
}
