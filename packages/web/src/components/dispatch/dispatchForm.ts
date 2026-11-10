import type { DispatchProfileDto, DispatchPromptKindDto, DispatchRuleDto, RawDispatchConfigDto } from '../../api/dispatchTypes.js';

// Laufarten der Prompt-Vorlagen in der Reihenfolge der Oberfläche
// (wie PROMPT_KINDS in crates/nexus-core/src/dispatch/config.rs)
export const PROMPT_KINDS = ['work', 'plan', 'research', 'merge'] as const;
export type PromptKind = DispatchPromptKindDto;
// Kurze Beschriftungen der Editoren; RUN_KIND_LABELS (dispatchTypes.ts) beschreibt dagegen Läufe und sagt „Merge-Session“
export const PROMPT_KIND_LABELS: Record<PromptKind, string> = {
  work: 'Umsetzung', plan: 'Planung', research: 'Recherche', merge: 'Merge',
};

export interface ProfileRow {
  name: string;
  model: string;
  effort: string;
  maxTurns: string;
  timeoutMinutes: string;
  /** Account-IDs, durch Komma getrennt; leer = jeder aktive Account */
  accounts: string;
}

export interface SetupRow {
  cwd: string;
  run: string;
}

export interface RuleRow {
  labels: string;
  priority: string;
  filesMin: string;
  planMinChars: string;
  profile: string;
  /** Account-IDs, durch Komma getrennt; leer = wie das Profil */
  accounts: string;
}

// Formularzustand der Einstellungen; Zahlen und Listen als Text, damit Eingaben unterwegs ungültig sein dürfen
export interface DispatchForm {
  enabled: boolean;
  boards: number[];
  wip: string;
  from: string;
  to: string;
  optIn: string;
  optOut: string;
  maxAttempts: string;
  pause: string;
  /** Merge-Commit (alle Commits bleiben) oder Squash (ein Commit je Ticket) */
  mergeStrategy: 'merge' | 'squash';
  /** Vor dem Merge auf Freigabe warten */
  reviewBeforeMerge: boolean;
  /** Ein Worktree je Ticket (none) oder Tickets einer Spec in einem gemeinsamen Worktree (sameSpec, alt) */
  joinWorktrees: 'none' | 'sameSpec';
  /** Einrichtung neu angelegter Worktrees */
  worktreeSetup: SetupRow[];
  profiles: ProfileRow[];
  defaultProfile: string;
  rules: RuleRow[];
  escalate: boolean;
  allowFable: boolean;
  /** Wahl unter mehreren Accounts */
  accountStrategy: 'most-free' | 'order';
  /** Ab dieser Auslastung (Prozent) gilt ein Account als voll */
  accountLimit: string;
  /** Reserve (Prozentpunkte) eines Laufs im 5-Stunden-Fenster */
  accountReserveFiveHour: string;
  /** Reserve (Prozentpunkte) eines Laufs in der Woche */
  accountReserveWeek: string;
  /** Minuten bis zur Rücksetzung des 5-Stunden-Fensters, ab denen die Reserve entfällt */
  accountResetGrace: string;
  command: string;
  permissionMode: string;
  gitBashPath: string;
  allowedTools: string;
  promptExtra: string;
  /** Eigene Vorlage je Laufart; leerer Text = Standardvorlage */
  prompts: Record<PromptKind, string>;
  /** Neue offene Fragen automatisch recherchieren lassen */
  researchAuto: boolean;
  /** Gleichzeitige Recherchen (1–5) */
  researchWip: string;
  /** Automatische Recherchen je Tag */
  researchMaxPerDay: string;
  /** Profil der Recherche; leer = Profil „research“, falls vorhanden, sonst Standardprofil */
  researchProfile: string;
  /** Tickets mit Planungsbedarf automatisch planen lassen */
  planningAuto: boolean;
  /** Gleichzeitige Planungen (1–5) */
  planningWip: string;
  /** Die Planung füllt auch die Abnahme des Tickets */
  planningAcceptance: boolean;
  /** Profil der Planung („Planen“ im Ticket); leer = Profil „planning“, falls vorhanden, sonst Standardprofil */
  planningProfile: string;
  /** Autopilot eingeschaltet */
  autopilot: boolean;
  /** Tickets mit diesem Label laufen ohne Autopilot; leer = keine Ausnahme */
  autopilotExceptLabel: string;
  /** Merge-Sessions je Konflikt (1–5) */
  autopilotMaxMergeSessions: string;
}

const text = (v: unknown): string => (v === undefined || v === null ? '' : String(v));
const list = (v: string, sep: RegExp): string[] => v.split(sep).map((s) => s.trim()).filter(Boolean);
const num = (v: string): number | undefined => (v.trim() === '' ? undefined : Number(v));

// Einstellung → Formular
export function toForm(raw: RawDispatchConfigDto): DispatchForm {
  return {
    enabled: raw.enabled === true,
    boards: (raw.boards ?? []).map(Number),
    wip: text(raw.wip ?? 1),
    from: raw.window?.from ?? '',
    to: raw.window?.to ?? '',
    optIn: raw.optInLabel === undefined ? 'auto' : text(raw.optInLabel),
    optOut: raw.optOutLabel === undefined ? 'manuell' : text(raw.optOutLabel),
    maxAttempts: text(raw.maxAttempts ?? 2),
    pause: text(raw.pauseMinutesOnLimit ?? 60),
    mergeStrategy: String(raw.mergeStrategy ?? '').trim() === 'squash' ? 'squash' : 'merge',
    reviewBeforeMerge: raw.reviewBeforeMerge !== false,
    joinWorktrees: String(raw.joinWorktrees ?? '').trim() === 'sameSpec' ? 'sameSpec' : 'none',
    worktreeSetup: (raw.worktreeSetup ?? []).map((s) => ({ cwd: text(s?.cwd), run: text(s?.run) })),
    profiles: Object.entries(raw.profiles ?? {}).map(([name, p]) => ({
      name, model: text(p.model), effort: text(p.effort), maxTurns: text(p.maxTurns), timeoutMinutes: text(p.timeoutMinutes), accounts: (p.accounts ?? []).join(', '),
    })),
    defaultProfile: raw.defaultProfile ?? '',
    rules: (raw.rules ?? []).map((r) => ({
      labels: (r.when?.labels ?? []).join(', '), priority: (r.when?.priority ?? []).join(', '),
      filesMin: text(r.when?.filesMin), planMinChars: text(r.when?.planMinChars), profile: r.profile ?? '', accounts: (r.accounts ?? []).join(', '),
    })),
    escalate: raw.escalate !== false,
    allowFable: raw.allowFable === true,
    accountStrategy: raw.accountStrategy === 'order' ? 'order' : 'most-free',
    accountLimit: text(raw.accountLimitPercent ?? 90),
    accountReserveFiveHour: text(raw.accountReserveFiveHour ?? 10),
    accountReserveWeek: text(raw.accountReserveWeek ?? 2),
    accountResetGrace: text(raw.accountResetGraceMinutes ?? 30),
    command: raw.claude?.command ?? 'claude',
    permissionMode: text(raw.claude?.permissionMode),
    gitBashPath: text(raw.claude?.gitBashPath),
    allowedTools: (raw.claude?.allowedTools ?? []).join('\n'),
    promptExtra: raw.promptExtra ?? '',
    prompts: { work: text(raw.prompts?.work), plan: text(raw.prompts?.plan), research: text(raw.prompts?.research), merge: text(raw.prompts?.merge) },
    researchAuto: raw.research?.auto === true,
    researchWip: text(raw.research?.wip ?? 1),
    researchMaxPerDay: text(raw.research?.maxPerDay ?? 10),
    researchProfile: text(raw.research?.profile),
    planningAuto: raw.planning?.auto === true,
    planningWip: text(raw.planning?.wip ?? 1),
    planningAcceptance: raw.planning?.acceptance !== false,
    planningProfile: text(raw.planning?.profile),
    autopilot: raw.autopilot?.enabled === true,
    // Vorgabe des Servers „interaktiv“; ein ausdrückliches null heißt „keine Ausnahme“
    autopilotExceptLabel: raw.autopilot?.exceptLabel === undefined ? 'interaktiv' : text(raw.autopilot.exceptLabel),
    autopilotMaxMergeSessions: text(raw.autopilot?.maxMergeSessions ?? 3),
  };
}

// Prüft eine Zahl im Textfeld: leer ist erlaubt, sonst ganze Zahl ab `min`; liefert die Fehlermeldung oder null
function numberError(label: string, value: string, min: number, max?: number): string | null {
  const t = value.trim();
  if (t === '') return null;
  if (max !== undefined) return /^[0-9]+$/.test(t) && Number(t) >= min && Number(t) <= max ? null : `${label} muss eine ganze Zahl von ${min} bis ${max} sein.`;
  return /^[0-9]+$/.test(t) && Number(t) >= min ? null : `${label} muss eine ganze Zahl sein.`;
}

// Prüft das Formular vor dem Speichern (Zahlenfelder, doppelte Profilnamen); liefert die erste Fehlermeldung oder null
export function checkForm(form: DispatchForm): string | null {
  const errors: Array<string | null> = [
    numberError('Gleichzeitig (WIP)', form.wip, 0),
    numberError('Versuche', form.maxAttempts, 1),
    numberError('Pause nach Limit (min)', form.pause, 1),
    numberError('Account voll ab (%)', form.accountLimit, 50, 100),
    numberError('Reserve 5 Stunden (%)', form.accountReserveFiveHour, 0, 50),
    numberError('Reserve Woche (%)', form.accountReserveWeek, 0, 50),
    numberError('Ohne Reserve, wenn 5 h zurücksetzt in (min)', form.accountResetGrace, 0, 300),
    numberError('Gleichzeitige Recherchen', form.researchWip, 1, 5),
    numberError('Automatische Recherchen je Tag', form.researchMaxPerDay, 1),
    numberError('Gleichzeitige Planungen', form.planningWip, 1, 5),
    numberError('Merge-Sessions je Konflikt', form.autopilotMaxMergeSessions, 1, 5),
  ];
  const seen = new Set<string>();
  form.profiles.forEach((p, i) => {
    const name = p.name.trim();
    const label = name || `Profil ${i + 1}`;
    // Eine ganz leere Zeile ist nur angelegt; mit Werten, aber ohne Namen ginge sie beim Speichern still verloren
    if (!name && (p.model.trim() || p.effort || p.maxTurns.trim() || p.timeoutMinutes.trim() || p.accounts.trim())) errors.push(`${label} braucht einen Namen.`);
    errors.push(numberError(`${label}: Schritte`, p.maxTurns, 1), numberError(`${label}: Minuten`, p.timeoutMinutes, 1));
    if (name && seen.has(name)) errors.push(`Profilname ${name} doppelt`);
    seen.add(name);
  });
  form.rules.forEach((r, i) => {
    errors.push(numberError(`Regel ${i + 1}: ab Dateien`, r.filesMin, 1), numberError(`Regel ${i + 1}: ab Zeichen Plan`, r.planMinChars, 1));
  });
  return errors.find((e) => e !== null) ?? null;
}

// Übernimmt geänderte Profilzeilen und zieht Umbenennungen in Standardprofil und Regeln nach; beim Löschen werden Verweise geleert
export function applyProfiles(form: DispatchForm, profiles: ProfileRow[]): DispatchForm {
  const old = form.profiles;
  const renames = new Map<string, string>();
  if (profiles.length === old.length) {
    profiles.forEach((p, i) => {
      const from = old[i].name.trim();
      if (from && from !== p.name.trim()) renames.set(from, p.name.trim());
    });
  } else if (profiles.length < old.length) {
    let at = profiles.findIndex((p, i) => p !== old[i]);
    if (at < 0) at = profiles.length;
    const gone = old[at].name.trim();
    if (gone && !profiles.some((p) => p.name.trim() === gone)) renames.set(gone, '');
  }
  if (renames.size === 0) return { ...form, profiles };
  const map = (ref: string) => renames.get(ref.trim()) ?? ref;
  return { ...form, profiles, defaultProfile: map(form.defaultProfile), researchProfile: map(form.researchProfile), planningProfile: map(form.planningProfile), rules: form.rules.map((r) => ({ ...r, profile: map(r.profile) })) };
}

// Verschiebt ein Profil an eine neue Stelle (Eskalationsreihenfolge). Anders als applyProfiles ist das keine Umbenennung:
// Namen und damit Verweise von Standardprofil, Recherche und Regeln bleiben unverändert.
export function moveProfile(form: DispatchForm, from: number, to: number): DispatchForm {
  const count = form.profiles.length;
  if (from === to || from < 0 || to < 0 || from >= count || to >= count) return form;
  const profiles = [...form.profiles];
  const [moved] = profiles.splice(from, 1);
  profiles.splice(to, 0, moved);
  return { ...form, profiles };
}

// Abschnitt research der Einstellung; fehlt er in `base` und weicht das Formular nicht von den Vorgaben ab, bleibt er weg.
// maxPerDay kommt nur dazu, wenn es schon dastand oder von der Vorgabe 10 abweicht
function research(form: DispatchForm, base: RawDispatchConfigDto): Pick<RawDispatchConfigDto, 'research'> {
  const perDayChanged = form.researchMaxPerDay.trim() !== '10';
  const changed = form.researchAuto || form.researchWip.trim() !== '1' || perDayChanged || form.researchProfile.trim() !== '';
  if (!base.research && !changed) return {};
  const perDay = base.research?.maxPerDay !== undefined || perDayChanged ? { maxPerDay: num(form.researchMaxPerDay) } : {};
  return { research: { ...base.research, auto: form.researchAuto, wip: num(form.researchWip), ...perDay, profile: form.researchProfile.trim() || null } };
}

// Abschnitt planning der Einstellung; fehlt er in `base` und weicht das Formular nicht von den Vorgaben ab, bleibt er weg.
// acceptance kommt nur dazu, wenn es schon dastand oder von der Vorgabe (an) abweicht; ein altes maxPerDay fällt weg
function planning(form: DispatchForm, base: RawDispatchConfigDto): Pick<RawDispatchConfigDto, 'planning'> {
  const profile = form.planningProfile.trim() || null;
  const changed = form.planningAuto || form.planningWip.trim() !== '1' || !form.planningAcceptance || profile !== null;
  if (!base.planning && !changed) return {};
  const { maxPerDay: _old, ...rest } = (base.planning ?? {}) as NonNullable<RawDispatchConfigDto['planning']> & { maxPerDay?: number };
  const acceptance = rest.acceptance !== undefined || !form.planningAcceptance ? { acceptance: form.planningAcceptance } : {};
  return { planning: { ...rest, auto: form.planningAuto, wip: num(form.planningWip), ...acceptance, profile } };
}

// Abschnitt autopilot der Einstellung; fehlt er in `base` und weicht das Formular nicht von den Vorgaben ab, bleibt er weg.
function autopilot(form: DispatchForm, base: RawDispatchConfigDto): Pick<RawDispatchConfigDto, 'autopilot'> {
  const exceptLabel = form.autopilotExceptLabel.trim() || null;
  const changed = form.autopilot || exceptLabel !== 'interaktiv' || form.autopilotMaxMergeSessions.trim() !== '3';
  if (!base.autopilot && !changed) return {};
  return { autopilot: { ...base.autopilot, enabled: form.autopilot, exceptLabel, maxMergeSessions: num(form.autopilotMaxMergeSessions) } };
}

// Abschnitt prompts der Einstellung; ohne eigene Vorlage und ohne Abschnitt in der Datei bleibt er weg.
// Nur auf „leer“ geprüft: Einrückungen und Leerzeilen der Vorlage müssen zeichengleich erhalten bleiben.
function prompts(form: DispatchForm, base: RawDispatchConfigDto): Pick<RawDispatchConfigDto, 'prompts'> {
  const own = PROMPT_KINDS.some((k) => form.prompts[k].trim() !== '');
  if (!base.prompts && !own) return {};
  const out: NonNullable<RawDispatchConfigDto['prompts']> = {};
  for (const k of PROMPT_KINDS) out[k] = form.prompts[k].trim() === '' ? null : form.prompts[k];
  return { prompts: out };
}

// Einrichtungsbefehle; Zeilen ohne Ordner und Befehl fallen weg, ein leerer Ordner heißt „.“ (Worktree selbst)
function setup(form: DispatchForm, base: RawDispatchConfigDto): Pick<RawDispatchConfigDto, 'worktreeSetup'> {
  const rows = form.worktreeSetup.filter((r) => r.cwd.trim() || r.run.trim()).map((r) => ({ cwd: r.cwd.trim() || '.', run: r.run.trim() }));
  return rows.length || base.worktreeSetup !== undefined ? { worktreeSetup: rows } : {};
}

// Formular → Einstellung; Felder, die das Formular nicht kennt, bleiben aus `base` erhalten
export function toRaw(form: DispatchForm, base: RawDispatchConfigDto): RawDispatchConfigDto {
  const profiles: Record<string, DispatchProfileDto> = {};
  for (const row of form.profiles) {
    if (!row.name.trim()) continue;
    const { accounts: _old, ...rest } = base.profiles?.[row.name.trim()] ?? {};
    const accounts = list(row.accounts, /,/);
    profiles[row.name.trim()] = { ...rest, model: row.model.trim() || null, effort: row.effort || null, maxTurns: num(row.maxTurns), timeoutMinutes: num(row.timeoutMinutes), ...(accounts.length ? { accounts } : {}) };
  }
  const rules: DispatchRuleDto[] = form.rules.map((row, i) => {
    const when: NonNullable<DispatchRuleDto['when']> = {};
    if (row.labels.trim()) when.labels = list(row.labels, /,/);
    if (row.priority.trim()) when.priority = list(row.priority, /,/);
    if (row.filesMin.trim()) when.filesMin = num(row.filesMin);
    if (row.planMinChars.trim()) when.planMinChars = num(row.planMinChars);
    const { accounts: _old, ...rest } = base.rules?.[i] ?? {};
    const accounts = list(row.accounts, /,/);
    return { ...rest, when, profile: row.profile, ...(accounts.length ? { accounts } : {}) };
  });
  return {
    ...base,
    enabled: form.enabled,
    boards: form.boards.length ? form.boards : null,
    wip: num(form.wip),
    window: form.from.trim() || form.to.trim() ? { from: form.from.trim(), to: form.to.trim() } : null,
    optInLabel: form.optIn.trim() || null,
    optOutLabel: form.optOut.trim() || null,
    maxAttempts: num(form.maxAttempts),
    pauseMinutesOnLimit: num(form.pause),
    // Ohne Eintrag in der Datei und ohne Wechsel kommt kein Feld dazu (Vorgabe merge)
    ...(form.mergeStrategy !== 'merge' || base.mergeStrategy !== undefined ? { mergeStrategy: form.mergeStrategy } : {}),
    // Ebenso für die Freigabe vor dem Merge (Vorgabe true)
    ...(!form.reviewBeforeMerge || base.reviewBeforeMerge !== undefined ? { reviewBeforeMerge: form.reviewBeforeMerge } : {}),
    // Ebenso für die Worktrees (Vorgabe none)
    ...(form.joinWorktrees !== 'none' || base.joinWorktrees !== undefined ? { joinWorktrees: form.joinWorktrees } : {}),
    // Einrichtung: leere Zeilen fallen weg; ohne Befehle und ohne Eintrag in der Datei kommt kein Feld dazu
    ...setup(form, base),
    profiles,
    defaultProfile: form.defaultProfile,
    rules,
    escalate: form.escalate,
    allowFable: form.allowFable,
    // Ohne Eintrag in der Datei und ohne Abweichung von den Vorgaben kommen keine Felder dazu
    ...(form.accountStrategy !== 'most-free' || base.accountStrategy !== undefined ? { accountStrategy: form.accountStrategy } : {}),
    ...(form.accountLimit.trim() !== '90' || base.accountLimitPercent !== undefined ? { accountLimitPercent: num(form.accountLimit) } : {}),
    ...(form.accountReserveFiveHour.trim() !== '10' || base.accountReserveFiveHour !== undefined ? { accountReserveFiveHour: num(form.accountReserveFiveHour) } : {}),
    ...(form.accountReserveWeek.trim() !== '2' || base.accountReserveWeek !== undefined ? { accountReserveWeek: num(form.accountReserveWeek) } : {}),
    ...(form.accountResetGrace.trim() !== '30' || base.accountResetGraceMinutes !== undefined ? { accountResetGraceMinutes: num(form.accountResetGrace) } : {}),
    promptExtra: form.promptExtra,
    ...prompts(form, base),
    ...research(form, base),
    ...planning(form, base),
    ...autopilot(form, base),
    claude: { ...base.claude, command: form.command.trim() || 'claude', permissionMode: form.permissionMode.trim() || null, allowedTools: list(form.allowedTools, /\n/),
      ...(form.gitBashPath.trim() ? { gitBashPath: form.gitBashPath.trim() } : 'gitBashPath' in (base.claude ?? {}) ? { gitBashPath: null } : {}),
    },
  };
}
