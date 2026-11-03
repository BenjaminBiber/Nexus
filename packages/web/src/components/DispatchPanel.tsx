import { useEffect, useRef, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import { ApiError } from '../api/ApiError.js';
import type { DbBoardDto } from '../api/dbTypes.js';
import type { DispatchConfigViewDto, RawDispatchConfigDto } from '../api/dispatchTypes.js';
import { useAsync } from '../hooks/useAsync.js';
import { Notice } from './settings/Notice.js';
import { SettingsFooter, SettingsPanel } from './settings/SettingsPanel.js';
import { SettingRow, SettingsSection } from './settings/SettingRow.js';
import { DispatchProfilesEditor } from './dispatch/DispatchProfilesEditor.js';
import { ProfileOptions } from './dispatch/ProfileOptions.js';
import { DispatchRulesEditor } from './dispatch/DispatchRulesEditor.js';
import { DispatchSetupEditor } from './dispatch/DispatchSetupEditor.js';
import { OptimizationSection } from './dispatch/OptimizationSection.js';
import { DispatchPromptEditor, type PromptProblem } from './dispatch/DispatchPromptEditor.js';
import { PromptPreview } from './dispatch/PromptPreview.js';
import { applyProfiles, checkForm, moveProfile, toForm, toRaw, PROMPT_KINDS, PROMPT_KIND_LABELS, type DispatchForm, type PromptKind } from './dispatch/dispatchForm.js';

// Einstellungen des Dispatchers (~/.harness/dispatch.yaml): Schalter, Boards, Grenzen, Merge, Profile, Regeln, Claude-Aufruf.
// Unterbereiche des Dispatchers; die Einstellungs-Navigation zeigt sie als Untermenü (Hash #dispatch/<id>).
// Wartezeit nach der letzten Eingabe bis zum automatischen Speichern.
const AUTOSAVE_DELAY_MS = 1200;

export const DISPATCH_SECTIONS = [
  { id: 'general', label: 'Allgemein', icon: 'power_settings_new' },
  { id: 'limits', label: 'Zeitplan & Grenzen', icon: 'schedule' },
  { id: 'merge', label: 'Merge & Worktrees', icon: 'merge' },
  { id: 'profiles', label: 'Profile & Regeln', icon: 'account_tree' },
  { id: 'optimize', label: 'Optimierungen', icon: 'insights' },
  { id: 'research', label: 'Recherchen', icon: 'travel_explore' },
  { id: 'planning', label: 'Planungen', icon: 'event_note' },
  { id: 'prompts', label: 'Prompts', icon: 'description' },
  { id: 'claude', label: 'Claude-Aufruf', icon: 'terminal' },
] as const;
export type DispatchSection = (typeof DISPATCH_SECTIONS)[number]['id'];

// Liest die Angaben einer abgelehnten Vorlage aus `details` des Servers (dispatch-prompt-invalid).
// Der Server nennt `problems` je Laufart und zusätzlich die erste flach (kind/missing/unknown); unbekannte Formen ergeben null.
function promptProblems(error: unknown): Partial<Record<PromptKind, PromptProblem>> | null {
  if (!(error instanceof ApiError) || !error.details) return null;
  const kinds = PROMPT_KINDS as readonly string[];
  const rows = Array.isArray(error.details.problems)
    ? (error.details.problems as Array<Record<string, unknown>>)
    : [error.details];
  const out: Partial<Record<PromptKind, PromptProblem>> = {};
  for (const row of rows) {
    const kind = row?.kind;
    if (typeof kind !== 'string' || !kinds.includes(kind)) continue;
    const list = (v: unknown) => (Array.isArray(v) ? v.map(String) : []);
    out[kind as PromptKind] = { missing: list(row.missing ?? row.missingParts), unknown: list(row.unknown ?? row.unknownPlaceholders) };
  }
  return Object.keys(out).length ? out : null;
}

// Ohne sub zeigt das Panel alle Unterbereiche; das Formular bleibt beim Wechsel erhalten, gespeichert wird gemeinsam.
export function DispatchPanel({ sub }: { sub?: DispatchSection } = {}) {
  const show = (id: DispatchSection) => !sub || sub === id;
  const api = useApi();
  const loaded = useAsync(async () => {
    const [view, boards, active] = await Promise.all([
      api.getDispatchConfig(),
      api.getDbBoards().catch(() => [] as DbBoardDto[]),
      api.getDbActive().catch(() => null),
    ]);
    return { view, boards, active };
  }, [api]);
  const data = loaded.state.status === 'ready' ? loaded.state.data : null;
  const [base, setBase] = useState<RawDispatchConfigDto | null>(null);
  const [form, setForm] = useState<DispatchForm | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Abgelehnte Vorlagen je Laufart; sie gehören an den Editor, nicht ins Banner oben (das Auto-Speichern schickt jede halbfertige Vorlage mit)
  const [promptErrors, setPromptErrors] = useState<Partial<Record<PromptKind, PromptProblem>>>({});

  useEffect(() => {
    if (data && !form) {
      setBase(data.view.raw);
      setForm(toForm(data.view.raw));
    }
  }, [data, form]);

  // Jede Eingabe zählt die Version hoch; das Auto-Speichern speichert nur ungespeicherte Versionen.
  const [version, setVersion] = useState(0);
  const savedVersion = useRef(0);
  // Fassung, die der Server beim Auto-Speichern abgelehnt hat: nicht erneut senden, erst nach der nächsten Änderung
  const rejectedVersion = useRef(-1);
  const versionRef = useRef(0);
  versionRef.current = version;
  const [pending, setPending] = useState<string | null>(null);
  const edit = (update: (current: DispatchForm) => DispatchForm) => {
    setForm((current) => (current ? update(current) : current));
    setVersion((v) => v + 1);
  };
  const change = (patch: Partial<DispatchForm>) => edit((current) => ({ ...current, ...patch }));

  // Übernommener Vorschlag: neue Einstellung ins Formular, gilt als gespeichert (kein Auto-Speichern hinterher)
  const applied = (view: DispatchConfigViewDto) => {
    setBase(view.raw);
    setForm(toForm(view.raw));
    savedVersion.current = versionRef.current;
    setPending(null);
    setInfo('Vorschlag übernommen');
  };

  // Speichert; Prüffehler erscheinen oben. auto: still bei unfertigen Eingaben und ohne laufende Eingaben zu überschreiben.
  const save = async (auto = false) => {
    if (!form || !base) return;
    const at = versionRef.current;
    if (!auto) setInfo(null);
    setError(null);
    const invalid = checkForm(form);
    if (invalid) {
      if (auto) setPending(invalid);
      else setError(invalid);
      return;
    }
    setPending(null);
    setBusy(true);
    try {
      const saved = await api.saveDispatchConfig(toRaw(form, base));
      savedVersion.current = Math.max(savedVersion.current, at);
      setPromptErrors({});
      setBase(saved.raw);
      // Nur beim Speichern per Knopf bereinigen (leere Zeilen fallen weg); das Auto-Speichern darf eine gerade angelegte
      // leere Zeile nicht unter den Händen löschen.
      if (!auto && versionRef.current === at) setForm(toForm(saved.raw));
      setInfo(auto ? `Automatisch gespeichert um ${new Date().toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' })}` : `Gespeichert in ${saved.file}`);
    } catch (e) {
      const problems = promptProblems(e);
      // Eine abgelehnte Vorlage gehört an ihren Editor; die Statuszeile meldet nur, dass nicht gespeichert wurde
      if (problems) {
        setPromptErrors(problems);
        if (auto) {
          setPending((e as Error).message);
          rejectedVersion.current = at;
        }
      }
      // Der Server lehnt halbfertige Eingaben ab (leere Regel, Befehl ohne Text, Zeit „18:“): beim Auto-Speichern kein Fehler, nur Hinweis
      else if (auto && e instanceof ApiError && e.code === 'dispatch-config-invalid') {
        setPending(e.message);
        rejectedVersion.current = at;
      }
      else setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    if (version === 0 || version === savedVersion.current || version === rejectedVersion.current || busy) return;
    const timer = setTimeout(() => void save(true), AUTOSAVE_DELAY_MS);
    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- save liest form/base des aktuellen Renders
  }, [version, busy]);

  const own = data ? data.boards.filter((b) => !data.active || b.ownerId === data.active.user.id) : [];
  const toggleBoard = (id: number, on: boolean) =>
    change({ boards: on ? [...(form?.boards ?? []), id] : (form?.boards ?? []).filter((b) => b !== id) });

  const names = form ? [...new Set(form.profiles.map((p) => p.name.trim()).filter(Boolean))] : [];

  return (
    <SettingsPanel label="Dispatcher">
      {loaded.state.status === 'loading' && <p className="muted">Wird geladen …</p>}
      {loaded.state.status === 'error' && <Notice tone="error">{loaded.state.message}</Notice>}
      {data?.view.error && <Notice tone="error">{data.view.error} Speichern ersetzt die Datei.</Notice>}
      {form && <>
        {show('general') && <>
          <SettingsSection
            title="Betrieb"
            description={`Gespeichert nur auf diesem Rechner${data ? ` (${data.view.file})` : ''}.`}
            info="Der Dispatcher übernimmt bereite Tickets aus „Geplant“ deiner Boards und lässt sie von Claude im eigenen Worktree umsetzen; erledigte Tickets werden in den ausgecheckten Branch des Repos gemergt."
          >
            <SettingRow label="Dispatcher eingeschaltet" hint="Aus: Er startet nichts Neues, wertet laufende Läufe aber weiter aus.">
              <input type="checkbox" checked={form.enabled} onChange={(e) => change({ enabled: e.target.checked })} />
            </SettingRow>
          </SettingsSection>
          <SettingsSection title="Boards" description="Ohne Auswahl: das aktive Board." info="Zur Wahl stehen deine eigenen Boards. Ein Board ohne deinen Repo-Pfad lässt sich nicht neu wählen; zuordnen mit harness board boards repo <ID> <Pfad>.">
            {own.length === 0 && <p className="settings-card-body muted">Keine eigenen Boards.</p>}
            {own.map((b) => (
              <SettingRow key={b.id} label={`${b.projectKey} · ${b.name}`} hint={b.repoPath ? undefined : 'Ohne eigenen Repo-Pfad'}>
                <input type="checkbox" disabled={!b.repoPath && !form.boards.includes(b.id)} checked={form.boards.includes(b.id)} onChange={(e) => toggleBoard(b.id, e.target.checked)} />
              </SettingRow>
            ))}
          </SettingsSection>
        </>}
        {show('limits') && <>
          <SettingsSection title="Zeitfenster" description="Neue Tickets startet er nur in diesem Fenster." info="Ortszeit. „Von“ zählt mit, „Bis“ nicht. Liegt „Bis“ vor „Von“, geht das Fenster über Mitternacht (18:00 bis 07:00 = nachts).">
            <SettingRow label="Von (HH:MM)"><input className="short" value={form.from} placeholder="18:00" onChange={(e) => change({ from: e.target.value })} /></SettingRow>
            <SettingRow label="Bis (HH:MM)"><input className="short" value={form.to} placeholder="07:00" onChange={(e) => change({ to: e.target.value })} /></SettingRow>
          </SettingsSection>
          <SettingsSection title="Grenzen">
            <SettingRow label="Gleichzeitig (WIP)" hint="So viele Tickets setzt er höchstens gleichzeitig um.">
              <input className="short" inputMode="numeric" value={form.wip} onChange={(e) => change({ wip: e.target.value })} />
            </SettingRow>
            <SettingRow label="Versuche" hint="Nach so vielen abgebrochenen Läufen geht ein Ticket nach Blockiert." info="Ein Abbruch ist ein Lauf ohne Ergebnis (weder erledigt noch blockiert). Das Ticket geht dann zurück nach Geplant; mit Eskalation läuft der nächste Versuch eine Profilstufe größer.">
              <input className="short" inputMode="numeric" value={form.maxAttempts} onChange={(e) => change({ maxAttempts: e.target.value })} />
            </SettingRow>
            <SettingRow label="Pause nach Limit (min)" hint="So lange startet er nichts Neues, wenn Claude ein Nutzungslimit meldet." info="Das Ticket geht zurück nach Geplant. Ein Limit zählt nicht als Versuch, erst das dritte desselben Accounts in Folge. Mit mehreren Accounts pausiert nur der volle Account, und zwar bis sein Limit zurückgesetzt wird (diese Minuten gelten nur, wenn die Rücksetzzeit unbekannt ist).">
              <input className="short" inputMode="numeric" value={form.pause} onChange={(e) => change({ pause: e.target.value })} />
            </SettingRow>
          </SettingsSection>
          <SettingsSection title="Labels">
            <SettingRow label="Nur mit Label" hint="Übernimmt nur Tickets mit diesem Label.">
              <input value={form.optIn} placeholder="leer = alle bereiten Tickets" onChange={(e) => change({ optIn: e.target.value })} />
            </SettingRow>
            <SettingRow label="Nie mit Label" hint="Tickets mit diesem Label lässt er immer liegen.">
              <input value={form.optOut} onChange={(e) => change({ optOut: e.target.value })} />
            </SettingRow>
          </SettingsSection>
        </>}
        {show('merge') && <>
          <SettingsSection title="Zurückmergen">
            <SettingRow label="Zurückmergen" hint="Wie ein erledigter Ticket-Branch in den Branch des Hauptordners kommt.">
              <select value={form.mergeStrategy} onChange={(e) => change({ mergeStrategy: e.target.value === 'squash' ? 'squash' : 'merge' })}>
                <option value="merge">Merge-Commit (alle Commits bleiben)</option>
                <option value="squash">Squash (ein Commit je Ticket)</option>
              </select>
            </SettingRow>
            <SettingRow label="Vor dem Merge auf Freigabe warten" hint="Erledigte Tickets warten in Blockiert, bis du sie abnimmst.">
              <input type="checkbox" checked={form.reviewBeforeMerge} onChange={(e) => change({ reviewBeforeMerge: e.target.checked })} />
            </SettingRow>
            <SettingRow label="Worktrees" info="Standard: Jedes Ticket bekommt einen eigenen Branch und Worktree und wird einzeln gemergt. „Bündeln“ ist das alte Verhalten: Tickets einer Spec teilen sich einen Worktree.">
              <select value={form.joinWorktrees} onChange={(e) => change({ joinWorktrees: e.target.value === 'sameSpec' ? 'sameSpec' : 'none' })}>
                <option value="none">Ein Worktree je Ticket</option>
                <option value="sameSpec">Tickets einer Spec bündeln (alt)</option>
              </select>
            </SettingRow>
          </SettingsSection>
          <SettingsSection
            title="Einrichtung neuer Worktrees"
            description="Befehle, die er in einem neuen Worktree vor Claude ausführt, z. B. npm ci."
            info="Die Befehle laufen nacheinander; der Ordner ist relativ zum Worktree. Scheitert einer, startet Claude nicht und das Ticket geht nach Blockiert (zählt nicht als Versuch). Ein fortgesetztes Ticket wird nicht erneut eingerichtet."
          >
            <div className="settings-card-body">
              <DispatchSetupEditor rows={form.worktreeSetup} onChange={(worktreeSetup) => change({ worktreeSetup })} />
            </div>
          </SettingsSection>
        </>}
        {show('profiles') && <>
          <SettingsSection title="Profile" description="Modell, Effort und Grenzen eines Laufs, geordnet von günstig nach teuer." info="Die Reihenfolge ist zugleich die Leiter der Eskalation: Nach einem Abbruch läuft der nächste Versuch mit dem Profil darunter. Leere Felder übernehmen die Vorgaben von Claude Code.">
            <div className="settings-card-body">
              <DispatchProfilesEditor rows={form.profiles} onChange={(profiles) => edit((current) => applyProfiles(current, profiles))} onMove={(from, to) => edit((current) => moveProfile(current, from, to))} />
            </div>
            <SettingRow label="Standardprofil" hint="Gilt, wenn keine Regel und kein Label profil: greift.">
              <select value={form.defaultProfile} onChange={(e) => change({ defaultProfile: e.target.value })}>
                {form.defaultProfile === '' && <option value="">–</option>}
                <ProfileOptions names={names} value={form.defaultProfile} />
              </select>
            </SettingRow>
            <SettingRow label="Eskalation nach Abbruch" hint="Jeder weitere Versuch läuft eine Profilstufe größer.">
              <input type="checkbox" checked={form.escalate} onChange={(e) => change({ escalate: e.target.checked })} />
            </SettingRow>
            <SettingRow label="Fable erlauben" hint="Profile dürfen das Modell Fable verwenden.">
              <input type="checkbox" checked={form.allowFable} onChange={(e) => change({ allowFable: e.target.checked })} />
            </SettingRow>
          </SettingsSection>
          <SettingsSection title="Accounts" description="Wie er unter den erlaubten Claude-Accounts eines Profils wählt." info={ACCOUNTS_INFO}>
            <SettingRow label="Wahl" hint="Gilt nur, wenn mehrere Accounts erlaubt und frei sind.">
              <select value={form.accountStrategy} onChange={(e) => change({ accountStrategy: e.target.value === 'order' ? 'order' : 'most-free' })}>
                <option value="most-free">Meiste freie Kapazität</option>
                <option value="order">Erster freier der Liste</option>
              </select>
            </SettingRow>
            <SettingRow label="Account voll ab (%)" hint="Ab dieser Auslastung (5 Stunden oder Woche) nimmt er den Account nicht mehr.">
              <input className="short" inputMode="numeric" value={form.accountLimit} onChange={(e) => change({ accountLimit: e.target.value })} />
            </SettingRow>
            <SettingRow label="Reserve 5 Stunden (%)" hint="So viel Prozentpunkte plant er für einen Lauf ein, solange es zu wenige frühere Läufe für eine Schätzung gibt.">
              <input className="short" inputMode="numeric" value={form.accountReserveFiveHour} onChange={(e) => change({ accountReserveFiveHour: e.target.value })} />
            </SettingRow>
            <SettingRow label="Reserve Woche (%)" hint="So viel Prozentpunkte plant er für einen Lauf in der Woche ein, solange es zu wenige frühere Läufe für eine Schätzung gibt.">
              <input className="short" inputMode="numeric" value={form.accountReserveWeek} onChange={(e) => change({ accountReserveWeek: e.target.value })} />
            </SettingRow>
            <SettingRow label="Ohne Reserve, wenn 5 h zurücksetzt in (min)" hint="Setzt sich das 5-Stunden-Fenster so bald zurück, startet er trotz knapper Auslastung.">
              <input className="short" inputMode="numeric" value={form.accountResetGrace} onChange={(e) => change({ accountResetGrace: e.target.value })} />
            </SettingRow>
          </SettingsSection>
          <SettingsSection title="Regeln" description="Die erste passende Regel bestimmt das Profil." info="Alle ausgefüllten Bedingungen einer Regel müssen zutreffen. „ab Dateien“ zählt die Zeilen der Tabelle unter „## Integration“ im Plan, „ab Zeichen Plan“ die Länge des Plans.">
            <div className="settings-card-body">
              <DispatchRulesEditor rows={form.rules} profiles={names} onChange={(rules) => change({ rules })} />
            </div>
          </SettingsSection>
        </>}
        {show('research') && (
          <SettingsSection
            title="Recherchen"
            description="Offene Fragen lässt eine Claude-Session lesend in einem eigenen Worktree recherchieren."
            info={<>
              <p>Von Hand: „Recherche starten“ auf der Seite einer Frage.</p>
              <p>Automatisch: nur Fragen, die nach dem Einschalten entstehen und dich betreffen (von dir oder deinen Agents gestellt oder mit Tickets deiner Boards verknüpft), je Frage einmal – auch wenn der Dispatcher für Tickets ausgeschaltet ist. Pause und Zeitfenster gelten. Fragen anderer startest du von Hand.</p>
            </>}
          >
            <SettingRow label="Neue Fragen zu meinen Boards automatisch recherchieren lassen">
              <input type="checkbox" checked={form.researchAuto} onChange={(e) => change({ researchAuto: e.target.checked })} />
            </SettingRow>
            <SettingRow label="Gleichzeitige Recherchen (1–5)" hint="Zusätzlich zum WIP der Tickets.">
              <input className="short" inputMode="numeric" value={form.researchWip} onChange={(e) => change({ researchWip: e.target.value })} />
            </SettingRow>
            <SettingRow label="Automatische Recherchen je Tag">
              <input className="short" inputMode="numeric" value={form.researchMaxPerDay} onChange={(e) => change({ researchMaxPerDay: e.target.value })} />
            </SettingRow>
            <SettingRow label="Profil der Recherche">
              <select value={form.researchProfile} onChange={(e) => change({ researchProfile: e.target.value })}>
                <option value="">{names.includes('research') ? 'Profil research' : 'Standardprofil'}</option>
                <ProfileOptions names={names} value={form.researchProfile} />
              </select>
            </SettingRow>
          </SettingsSection>
        )}
        {show('planning') && (
          <SettingsSection
            title="Planungen"
            description="Tickets lässt eine Claude-Session lesend planen; der Plan landet am Ticket."
            info={<>
              <p>Von Hand: „Planen“ im Ticket eines eigenen Boards, sofort und in einem eigenen Worktree. Fehlt eine Entscheidung, entsteht eine Frage. Planungen zählen nicht zum WIP.</p>
              <p>Automatisch: nur Tickets mit Planungsbedarf auf eigenen Boards mit Repo-Pfad, die nach dem Einschalten dazukommen, je Ticket einmal – und nur, solange im Dispatcher ein Platz für Tickets frei ist. Tickets mit offener Frage oder fremdem Planungs-Claim bleiben außen vor. Pause und Zeitfenster gelten, der Schalter für Tickets nicht.</p>
            </>}
          >
            <SettingRow label="Tickets mit Planungsbedarf automatisch planen lassen">
              <input type="checkbox" checked={form.planningAuto} onChange={(e) => change({ planningAuto: e.target.checked })} />
            </SettingRow>
            <SettingRow label="Gleichzeitige Planungen (1–5)" hint="Zusätzlich zum WIP der Tickets.">
              <input className="short" inputMode="numeric" value={form.planningWip} onChange={(e) => change({ planningWip: e.target.value })} />
            </SettingRow>
            <SettingRow label="Abnahme beim Planen mit ausfüllen" hint="Die Planung schreibt auch, woran man sieht, dass das Ticket fertig ist." info="Eine vorhandene Abnahme wird geschärft, nicht verworfen.">
              <input type="checkbox" checked={form.planningAcceptance} onChange={(e) => change({ planningAcceptance: e.target.checked })} />
            </SettingRow>
            <SettingRow label="Profil der Planung">
              <select value={form.planningProfile} onChange={(e) => change({ planningProfile: e.target.value })}>
                <option value="">{names.includes('planning') ? 'Profil planning' : 'Standardprofil'}</option>
                <ProfileOptions names={names} value={form.planningProfile} />
              </select>
            </SettingRow>
          </SettingsSection>
        )}
        {show('prompts') && (
          <SettingsSection
            title="Prompt-Vorlagen"
            description="Die Arbeitsanweisung an Claude je Laufart. Leer: die Standardvorlage gilt."
            info="Die Standardvorlage zieht mit neuen Versionen des Harness automatisch mit. Eine eigene Vorlage bleibt dagegen so, wie sie hier steht."
            bare
          >
            {PROMPT_KINDS.map((kind) => {
              const info = data?.view.promptInfo?.find((p) => p.kind === kind);
              return (
                <DispatchPromptEditor
                  key={kind}
                  label={info?.label ?? PROMPT_KIND_LABELS[kind]}
                  value={form.prompts[kind]}
                  defaultTemplate={info?.default}
                  placeholders={info?.placeholders}
                  required={info?.required}
                  problem={promptErrors[kind] ?? null}
                  onChange={(value) => {
                    // Die Meldung zu genau dieser Vorlage verschwindet, sobald sie geändert wird
                    setPromptErrors((current) => (current[kind] ? { ...current, [kind]: undefined } : current));
                    change({ prompts: { ...form.prompts, [kind]: value } });
                  }}
                >
                  <PromptPreview kind={kind} template={form.prompts[kind]} defaultTemplate={info?.default} />
                </DispatchPromptEditor>
              );
            })}
          </SettingsSection>
        )}
        {show('claude') && (
          <SettingsSection title="Claude" description="Wie der Dispatcher Claude Code aufruft.">
            <SettingRow label="Claude-Aufruf" hint="claude oder ein voller Pfad.">
              <input value={form.command} onChange={(e) => change({ command: e.target.value })} />
            </SettingRow>
            <SettingRow label="Rechte-Modus" hint="Wert für --permission-mode von Claude Code.">
              <input value={form.permissionMode} onChange={(e) => change({ permissionMode: e.target.value })} />
            </SettingRow>
            <SettingRow label="Git Bash (nur Windows, leer = automatisch)" info="Pfad der bash.exe von Git for Windows. Leer sucht der Dispatcher sie selbst.">
              <input value={form.gitBashPath} onChange={(e) => change({ gitBashPath: e.target.value })} />
            </SettingRow>
            <SettingRow label="Erlaubte Tools (eine pro Zeile)" hint="Was der Agent ohne Rückfrage darf (--allowedTools)." wide>
              <textarea className="plain-text" rows={6} value={form.allowedTools} onChange={(e) => change({ allowedTools: e.target.value })} />
            </SettingRow>
            <SettingRow label="Zusatz für alle Prompts" hint={<>Wird über {'{{promptExtra}}'} an jede Vorlage angehängt – auch an eigene.</>} wide>
              <textarea className="plain-text" rows={3} value={form.promptExtra} onChange={(e) => change({ promptExtra: e.target.value })} />
            </SettingRow>
          </SettingsSection>
        )}
        {show('optimize') && (
          <OptimizationSection locked={busy || pending !== null || version !== savedVersion.current} onApplied={applied} />
        )}
        <SettingsFooter
          state={<>
            {error && <Notice tone="error">{error}</Notice>}
            <span role="status">
              {busy ? 'Wird gespeichert …' : pending ? `Nicht automatisch gespeichert: ${pending}` : version !== savedVersion.current ? 'Ungespeicherte Änderungen' : info ?? 'Änderungen werden automatisch gespeichert'}
            </span>
          </>}
        >
          <button type="button" className="btn primary" disabled={busy} onClick={() => void save()}>Speichern</button>
        </SettingsFooter>
      </>}
    </SettingsPanel>
  );
}

// Erklärung der Account-Einstellungen hinter dem (i)
const ACCOUNTS_INFO = (
  <ul>
    <li>Accounts legst du unter Einstellungen → Accounts an. In der Spalte „Accounts“ der Profile wählst du sie im Dropdown aus, z. B. <code>max</code> beim Profil <code>schwer</code>.</li>
    <li>Ein Profil ohne Accounts darf jeden aktiven Account verwenden. Eine Regel mit Accounts überschreibt die des Profils, ein Label <code>account:&lt;id&gt;</code> am Ticket beide.</li>
    <li>Bei der Eskalation bringt das größere Profil seine eigenen Accounts mit.</li>
    <li>Accounts mit „Nur als Fallback“ (Einstellungen → Accounts) nimmt er erst, wenn kein anderer erlaubter Account frei ist.</li>
    <li>Ist kein erlaubter Account frei, wartet das Ticket in Geplant – das zählt nicht als Versuch.</li>
  </ul>
);
