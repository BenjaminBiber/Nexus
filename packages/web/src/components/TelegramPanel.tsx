import { useCallback, useEffect, useState, type ReactNode } from 'react';
import { useApi } from '../api/ApiContext.js';
import type {
  BotStatus, IdeaTarget, QuietHours, TelegramConfig, TelegramConfigInput, TelegramTestDto,
} from '../api/telegramTypes.js';
import { useAsync } from '../hooks/useAsync.js';
import { CardSection, PowerToggle, SaveBar, StatusBadge, type Tone } from './settings/CardSettings.js';
import { Notice } from './settings/Notice.js';
import { TelegramPairDialog } from './TelegramPairDialog.js';
import { SettingsPanel } from './settings/SettingsPanel.js';

// Name, den Nexus am Bot einrichtet (crates/nexus-server/src/telegram/texts.rs)
const PROFILE_NAME = 'Nexus';

// Vorgabe der Ruhezeit, solange noch keine eingestellt war
const QUIET_DEFAULT: QuietHours = { from: '22:00', to: '07:00' };

type NotifyKey = keyof TelegramConfig['notify'];
interface NotifyRow { key: NotifyKey; label: string; hint: string }

// Anlässe nach Dringlichkeit: was eine Entscheidung braucht, und was nur Bescheid gibt
const NEEDS_YOU: NotifyRow[] = [
  { key: 'review', label: 'Review', hint: 'Lauf fertig, wartet auf dich' },
  { key: 'question', label: 'Frage', hint: 'Entscheidung offen' },
  { key: 'blocked', label: 'Blockiert', hint: 'Ticket kommt nicht weiter' },
  { key: 'merge', label: 'Merge gescheitert', hint: 'Bereit, aber Merge ging nicht durch' },
];
const FOR_INFO: NotifyRow[] = [
  { key: 'merged', label: 'Gemergt', hint: 'Merge ist durch' },
  { key: 'paused', label: 'Pause', hint: 'Dispatcher pausiert' },
];
const NOTIFY_COUNT = NEEDS_YOU.length + FOR_INFO.length;
const NOTIFY_LABEL = Object.fromEntries([...NEEDS_YOU, ...FOR_INFO].map((r) => [r.key, r.label])) as Record<NotifyKey, string>;


// Der Stand des Pollers: kurzes Etikett für den Kopf, ein Satz nur dort, wo er weiterhilft
function statusOf(bot: BotStatus): { tone: Tone; badge: string; text?: string } {
  switch (bot.state) {
    case 'off':
      return { tone: 'info', badge: 'Aus', text: 'Der Bot pollt nicht und schickt keine Meldungen.' };
    case 'starting':
      return { tone: 'info', badge: 'Startet …' };
    case 'running':
      return { tone: 'ok', badge: 'Läuft' };
    case 'conflict':
      return { tone: 'warn', badge: 'Woanders aktiv', text: 'Läuft auf einem anderen Rechner – dort holt schon jemand die Updates ab.' };
    case 'tokenInvalid':
      return { tone: 'error', badge: 'Token ungültig', text: 'Token ungültig – trag das Token von @BotFather neu ein.' };
    case 'error':
      return { tone: 'error', badge: 'Fehler', text: bot.lastError ?? 'Der Bot läuft nicht.' };
  }
}

// Zahl der Felder, in denen das Formular vom gespeicherten Stand abweicht (das Token zählt mit)
function countChanges(form: TelegramConfig, saved: TelegramConfig, token: string | null): number {
  const same = (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b);
  let n = 0;
  if (form.enabled !== saved.enabled) n++;
  if (form.apiBase !== saved.apiBase) n++;
  for (const key of Object.keys(form.notify) as NotifyKey[]) if (form.notify[key] !== saved.notify[key]) n++;
  if (!same(form.quietHours, saved.quietHours)) n++;
  if (form.ideas.defaultTarget !== saved.ideas.defaultTarget) n++;
  if (form.ideas.maxRounds !== saved.ideas.maxRounds) n++;
  if (token) n++;
  return n;
}

// Bereich Telegram: Stand und Testnachricht oben, darunter Meldungen, Chats, Ideen und die selten gebrauchte Verbindung.
export function TelegramPanel() {
  const api = useApi();
  const loaded = useAsync(() => api.getTelegramConfig(), [api]);
  const status = useAsync(() => api.getTelegramStatus(), [api]);
  const data = loaded.state.status === 'ready' ? loaded.state.data : null;
  const bot = status.state.status === 'ready' ? status.state.data.bot : null;
  const [form, setForm] = useState<TelegramConfig | null>(null);
  // Neues Token im Klartext; null = das gespeicherte bleibt
  const [token, setToken] = useState<string | null>(null);
  // Zuletzt eingestellte Zeiten, damit sie beim Aus- und wieder Einschalten erhalten bleiben
  const [quietTimes, setQuietTimes] = useState<QuietHours>(QUIET_DEFAULT);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [pairOpen, setPairOpen] = useState(false);
  const [test, setTest] = useState<TelegramTestDto | null>(null);

  useEffect(() => {
    if (data && !form) {
      setForm(data);
      if (data.quietHours) setQuietTimes(data.quietHours);
    }
  }, [data, form]);

  const change = (patch: Partial<TelegramConfig>) => {
    setInfo(null);
    setForm((current) => (current ? { ...current, ...patch } : current));
  };
  // Jede Aktion fängt mit leeren Meldungen an, damit nicht zwei Rückmeldungen nebeneinander stehen
  const start = () => {
    setError(null);
    setInfo(null);
    setTest(null);
  };

  // Speichert die Einstellung. Das Token geht nur mit, wenn ein neues eingetippt wurde –
  // sonst bliebe die Maske im Formular und der Server müsste sie erst wieder zurückrechnen.
  const save = async () => {
    if (!form) return;
    start();
    const { chats: _chats, profile: _profile, botToken: _masked, ...rest } = form;
    const input: TelegramConfigInput = { ...rest };
    if (token) input.botToken = token;
    try {
      const saved = await api.saveTelegramConfig(input);
      loaded.set(saved);
      setForm(saved);
      setToken(null);
      setInfo('Gespeichert.');
      await status.refresh();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  // Verwirft alle ungespeicherten Änderungen und kehrt zum gespeicherten Stand zurück
  const discard = () => {
    if (!data) return;
    start();
    setForm(data);
    setToken(null);
    setQuietTimes(data.quietHours ?? QUIET_DEFAULT);
  };

  // Der Kopplungsdialog meldet einen neuen Chat: Liste übernehmen, ohne andere ungespeicherte Änderungen anzufassen
  const paired = useCallback((fresh: TelegramConfig, chatId: number) => {
    setPairOpen(false);
    loaded.set(fresh);
    setForm((current) => (current ? { ...current, chats: fresh.chats } : fresh));
    setError(null);
    setInfo(`Chat ${chatId} gekoppelt.`);
    void status.refresh();
    // loaded und status sind je Panel feste Objekte
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const closePair = useCallback(() => setPairOpen(false), []);

  // Nimmt einen Chat aus der Kopplung und lädt die Liste neu
  const unpair = async (chatId: number) => {
    start();
    try {
      await api.unpairTelegramChat(chatId);
      const fresh = await api.getTelegramConfig();
      loaded.set(fresh);
      setForm((current) => (current ? { ...current, chats: fresh.chats } : fresh));
      setInfo(`Chat ${chatId} entkoppelt.`);
      await status.refresh();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  // Schickt die Testnachricht an alle gekoppelten Chats
  const sendTest = async () => {
    start();
    try {
      setTest(await api.testTelegram());
    } catch (e) {
      setError((e as Error).message);
    }
  };

  if (!form) return <SettingsPanel label="Telegram" className="telegram-panel">{null}</SettingsPanel>;

  const state = bot ? statusOf(bot) : null;
  const chats = form.chats;
  const activeCount = (Object.keys(form.notify) as NotifyKey[]).filter((k) => form.notify[k]).length;
  const changes = data ? countChanges(form, data, token) : 0;
  const quiet = form.quietHours ?? quietTimes;
  const setQuiet = (patch: Partial<QuietHours>) => {
    const next = { ...quiet, ...patch };
    setQuietTimes(next);
    if (form.quietHours) change({ quietHours: next });
  };
  const toggle = (key: NotifyKey) => (
    <Switch label={NOTIFY_LABEL[key]} checked={form.notify[key]} onChange={(on) => change({ notify: { ...form.notify, [key]: on } })} />
  );

  const summary = [
    chats.length === 1 ? '1 Chat gekoppelt' : `${chats.length} Chats gekoppelt`,
    `${activeCount} von ${NOTIFY_COUNT} Meldungen aktiv`,
    form.quietHours ? `Ruhezeit ${form.quietHours.from}–${form.quietHours.to}` : 'Keine Ruhezeit',
  ].join(' · ');

  return (
    <SettingsPanel label="Telegram" className="telegram-panel">
      <section className="tg-hero" aria-label="Stand des Bots" role="note">
        <img className="tg-avatar" src="/favicon.svg" alt="Profilbild des Bots: Nexus-Logo" width={56} height={56} />
        <div className="tg-hero-text">
          <div className="tg-hero-title">
            <strong>{PROFILE_NAME}</strong>
            {bot?.botName && <span className="mono">@{bot.botName}</span>}
            {state && <StatusBadge tone={state.tone}>{state.badge}</StatusBadge>}
          </div>
          <span className="tg-hero-summary">{summary}</span>
        </div>
        <div className="tg-hero-actions">
          <button type="button" className="btn" onClick={() => void sendTest()} disabled={chats.length === 0}>Testnachricht senden</button>
          <PowerToggle label="Bot aktiv" on={form.enabled} onChange={(enabled) => change({ enabled })} />
        </div>
      </section>

      {state?.text && <Notice tone={state.tone} role="note" label="Hinweis zum Bot">{state.text}</Notice>}
      {bot?.profileError && (
        <Notice tone="warn" role="note">Das Profil des Bots ließ sich nicht einrichten: {bot.profileError}. Der Bot läuft trotzdem.</Notice>
      )}
      {test && (
        <Notice tone={test.errors.length > 0 ? 'warn' : 'ok'} role="note" label="Ergebnis der Testnachricht">
          <p>{test.sent === 1 ? 'An 1 Chat verschickt.' : `An ${test.sent} Chats verschickt.`}</p>
          {test.errors.map((e) => (
            <p key={e.chatId}>Chat {e.chatId}: {e.error}</p>
          ))}
        </Notice>
      )}

      <CardSection title="Meldungen" description="Wann der Bot von sich aus schreibt.">
        <NotifyGroup title="Braucht dich" rows={NEEDS_YOU} toggle={toggle} />
        <NotifyGroup title="Zur Info" rows={FOR_INFO} toggle={toggle} />
        <div className="tg-card tg-quiet">
          <span className="tg-row-text">
            <span className="tg-row-label">Ruhezeit</span>
            <span className="tg-row-hint">In diesem Fenster bleibt der Bot still.</span>
          </span>
          <div className="tg-quiet-times">
            <label htmlFor="tg-quiet-from">von</label>
            <input id="tg-quiet-from" type="time" value={quiet.from} disabled={!form.quietHours} onChange={(e) => setQuiet({ from: e.target.value })} />
            <label htmlFor="tg-quiet-to">bis</label>
            <input id="tg-quiet-to" type="time" value={quiet.to} disabled={!form.quietHours} onChange={(e) => setQuiet({ to: e.target.value })} />
          </div>
          <input
            type="checkbox"
            role="switch"
            className="switch"
            aria-label="Ruhezeit"
            checked={form.quietHours !== null}
            onChange={(e) => change({ quietHours: e.target.checked ? { ...quietTimes } : null })}
          />
        </div>
      </CardSection>

      <CardSection title="Chats" description="Nur gekoppelte Chats bekommen Meldungen.">
        <div className="tg-card">
          {chats.length > 0 && (
            <ul className="tg-chats" aria-label="Gekoppelte Chats">
              {chats.map((chatId) => (
                <li key={chatId} className="tg-chat">
                  <span className="tg-chat-icon" aria-hidden="true"><span className="material-icons">person</span></span>
                  <span className="tg-row-text">
                    <span className="tg-row-label">Chat</span>
                    <span className="tg-row-hint mono">{chatId}</span>
                  </span>
                  <button type="button" className="btn" aria-label={`Chat ${chatId} entkoppeln`} onClick={() => void unpair(chatId)}>Entkoppeln</button>
                </li>
              ))}
            </ul>
          )}
          <button type="button" className="tg-pair" onClick={() => { start(); setPairOpen(true); }}>
            <span className="tg-chat-icon add" aria-hidden="true"><span className="material-icons">add</span></span>
            {chats.length === 0 ? 'Chat koppeln' : 'Weiteren Chat koppeln'}
          </button>
        </div>
      </CardSection>
      {pairOpen && <TelegramPairDialog knownChats={chats} onPaired={paired} onClose={closePair} />}

      <CardSection title="Ideen" description="Was mit Ideen passiert, die du dem Bot schickst.">
        <div className="tg-card">
          <div className="tg-field-row">
            <span className="tg-row-text">
              <label className="tg-row-label" htmlFor="tg-idea-target">Ablegen als</label>
              <span className="tg-row-hint">Im Chat pro Idee änderbar</span>
            </span>
            <select id="tg-idea-target" value={form.ideas.defaultTarget} onChange={(e) => change({ ideas: { ...form.ideas, defaultTarget: e.target.value as IdeaTarget } })}>
              <option value="note">Notiz</option>
              <option value="ticket">Ticket</option>
            </select>
          </div>
          <div className="tg-field-row">
            <span className="tg-row-text">
              <label className="tg-row-label" htmlFor="tg-idea-rounds">Rückfragen pro Idee</label>
              <span className="tg-row-hint">Höchstens, 1–10</span>
            </span>
            <input id="tg-idea-rounds" className="short" type="number" min={1} max={10} value={form.ideas.maxRounds} onChange={(e) => change({ ideas: { ...form.ideas, maxRounds: Number(e.target.value) } })} />
          </div>
        </div>
      </CardSection>

      <CardSection title="Verbindung" description="Token von @BotFather. Profilbild, Name und Befehle richtet Nexus beim Start selbst ein.">
        <div className="tg-card">
          <div className="tg-field-row">
            <span className="tg-row-text">
              <span className="tg-row-label">Bot-Token</span>
              <span className="tg-row-hint">
                {token !== null ? 'Das neue Token wird beim Speichern übernommen.' : form.botToken ? 'Gespeichert, nur maskiert sichtbar' : 'Noch kein Token eingetragen'}
              </span>
            </span>
            {token !== null ? (
              <>
                <input className="mono tg-token-input" type="password" autoComplete="off" aria-label="Neues Bot-Token" value={token} placeholder="123456:ABC…" onChange={(e) => setToken(e.target.value)} />
                <button type="button" className="btn" onClick={() => setToken(null)}>Abbrechen</button>
              </>
            ) : (
              <>
                <span className="mono tg-token" aria-label="Gespeichertes Bot-Token">{form.botToken || '(keins)'}</span>
                <button type="button" className="btn" onClick={() => setToken('')}>{form.botToken ? 'Ändern' : 'Eintragen'}</button>
              </>
            )}
          </div>
          <details className="tg-advanced">
            <summary>
              <span className="material-icons" aria-hidden="true">chevron_right</span>
              Erweitert
            </summary>
            <div className="tg-advanced-body">
              <label htmlFor="tg-api-base">Adresse der Bot-API</label>
              <input id="tg-api-base" type="url" className="mono" value={form.apiBase} onChange={(e) => change({ apiBase: e.target.value })} />
              <span className="tg-row-hint">Nur ändern, wenn ein eigener Bot-API-Server läuft.</span>
            </div>
          </details>
        </div>
      </CardSection>

      <SaveBar changes={changes} error={error} info={info} onDiscard={discard} onSave={() => void save()} />
    </SettingsPanel>
  );
}

// Karte mit Kopfzeile und den Anlässen in zwei Spalten; die ganze Zeile schaltet
function NotifyGroup({ title, rows, toggle }: { title: string; rows: NotifyRow[]; toggle: (key: NotifyKey) => ReactNode }) {
  return (
    <div className="tg-card" role="group" aria-label={title}>
      <div className="tg-card-head">{title}</div>
      <div className="tg-grid2">
        {rows.map((row) => (
          <label key={row.key} className="tg-toggle-row">
            <span className="tg-row-text">
              <span className="tg-row-label">{row.label}</span>
              <span className="tg-row-hint">{row.hint}</span>
            </span>
            {toggle(row.key)}
          </label>
        ))}
      </div>
    </div>
  );
}

// Schalter als Checkbox mit Schalter-Rolle; das umgebende label macht die ganze Zeile klickbar
function Switch({ label, checked, onChange }: { label: string; checked: boolean; onChange: (on: boolean) => void }) {
  return <input type="checkbox" role="switch" className="switch" aria-label={label} checked={checked} onChange={(e) => onChange(e.target.checked)} />;
}
