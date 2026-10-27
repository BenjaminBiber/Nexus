import { useApi } from '../api/ApiContext.js';
import type { CustomizationEntryDto, CustomizationOverviewDto, CustomizationsDto } from '../api/types.js';
import { useAsync } from '../hooks/useAsync.js';
import { CopyCommand } from './CopyCommand.js';
import { Notice } from './settings/Notice.js';
import { SettingsPanel } from './settings/SettingsPanel.js';
import { SettingsSection } from './settings/SettingRow.js';

const COMMAND = '/ai-harness:harness-customize';
const WORKFLOW = { standard: 'Standard', custom: 'angepasst', fallback: 'Rückfall auf Standard' } as const;

// Anpassungen des Devs (Workflow, Skills, Output-Styles), nur lesend; geändert wird über den Skill und npm run setup.
export function CustomizationsPanel() {
  const api = useApi();
  const { state } = useAsync(() => api.getCustomizations(), [api]);
  return (
    <SettingsPanel label="Anpassungen">
      <SettingsSection
        title="Aktiver Stand"
        description="Nur zur Ansicht. Geändert wird mit Claude."
        info="Der Skill legt eigene Workflows, Skills oder Output-Styles neben den Standard. Sie wirken erst, wenn du danach im Harness-Ordner npm run setup ausführst."
      >
        {state.status === 'loading' && <p className="settings-card-body muted">Wird geladen …</p>}
        {state.status === 'error' && <Notice tone="error">Die Anpassungen ließen sich nicht laden: {state.message}</Notice>}
        {state.status === 'ready' && <Content data={state.data} />}
      </SettingsSection>
      <SettingsSection title="Ändern" description="In einer Claude-Session aufrufen; wirkt erst nach npm run setup im Harness-Ordner." bare>
        <CopyCommand command={COMMAND} />
      </SettingsSection>
    </SettingsPanel>
  );
}

// Inhalt je nach Stand: unbekannt, keine Anpassungen oder Übersicht
function Content({ data }: { data: CustomizationsDto }) {
  if ('state' in data) return <p className="settings-card-body">Noch keine Übersicht. Führe im Harness-Ordner einmal npm run setup aus.</p>;
  if (!data.entries.length) return <p className="settings-card-body">Keine Anpassungen, überall gilt der Standard.</p>;
  return <Overview overview={data} />;
}

// Je Erweiterungspunkt eine Zeile, danach Warnungen, Rückfälle und Zeitpunkt
function Overview({ overview }: { overview: CustomizationOverviewDto }) {
  const warnings = overview.entries.flatMap((e) => e.warnings.map((w) => `${label(e)}: ${w.message}`));
  const fallbacks = overview.entries.filter((e) => e.state === 'fallback').flatMap((e) => e.problems.map((p) => `${label(e)}: ${p.message}`));
  return (
    <>
      <dl className="terms">
        <dt>Workflow</dt>
        <dd>{WORKFLOW[overview.workflow]}</dd>
        <dt>Skills</dt>
        <dd>{names(overview, 'skill')}</dd>
        <dt>Output-Styles</dt>
        <dd>{names(overview, 'output-style')}</dd>
      </dl>
      {warnings.length > 0 && <TextList title="Warnungen" items={warnings} />}
      {fallbacks.length > 0 && <TextList title="Rückfälle auf den Standard" items={fallbacks} />}
      <p className="settings-card-body muted">Zuletzt angewendet: {new Date(overview.createdAt).toLocaleString('de-DE')}</p>
    </>
  );
}

// Meldung mit Überschrift und Liste von Texten
function TextList({ title, items }: { title: string; items: string[] }) {
  return (
    <Notice tone="warn">
      <strong>{title}</strong>
      <ul aria-label={title}>
        {items.map((item) => <li key={item}>{item}</li>)}
      </ul>
    </Notice>
  );
}

// Aktive Anpassungen eines Erweiterungspunkts mit Art, sonst „Standard“
function names(overview: CustomizationOverviewDto, point: CustomizationEntryDto['point']): string {
  const active = overview.entries.filter((e) => e.point === point && e.state === 'active');
  return active.length ? active.map((e) => `${e.name} (${e.kind === 'replace' ? 'ersetzt' : 'neu'})`).join(', ') : 'Standard';
}

// Anzeigename eines Eintrags wie in der Ausgabe von setup
function label(entry: CustomizationEntryDto): string {
  if (entry.point === 'workflow') return 'Workflow';
  return `${entry.point === 'skill' ? 'Skill' : 'Output-Style'} ${entry.name}`;
}
