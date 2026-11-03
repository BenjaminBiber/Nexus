import { useContext, useEffect, useId, useState } from 'react';
import { createPortal } from 'react-dom';
import { useApi } from '../api/ApiContext.js';
import type { AccountDto, AccountListDto } from '../api/usageTypes.js';
import { useAsync } from '../hooks/useAsync.js';
import { Badge } from './Badge.js';
import { AccountRing, AccountUsageLines, REFRESH_MS, RETRY_MS, transient, USAGE_REASONS } from './ClaudeUsageMeter.js';
import { CopyCommand } from './CopyCommand.js';
import { DbModal } from './db/DbModal.js';
import { ErrorState } from './ErrorState.js';
import { LoadingState } from './LoadingState.js';
import { InfoTip } from './settings/InfoTip.js';
import { Notice } from './settings/Notice.js';
import { SaveStatusSlot, SettingsPanel, useSaveStatus } from './settings/SettingsPanel.js';
import { SettingRow } from './settings/SettingRow.js';

const WINDOWS = typeof navigator !== 'undefined' && /win/i.test(navigator.platform);
/** So oft fragt die Seite nach, solange eine Anmeldung läuft */
const LOGIN_POLL_MS = 2000;
/** Grenze, ab der ein Ring ohne eigene Obergrenze orange wird, bis die Vorgabe des Dispatchers geladen ist */
const FALLBACK_LIMIT = 90;

type Change = (fn: () => Promise<AccountListDto>) => Promise<boolean>;

function Icon({ name }: { name: string }) {
  return <span className="material-icons" aria-hidden="true">{name}</span>;
}

// Claude-Accounts des Dispatchers als Kacheln: je Account Limits als Doppelring, Schalter für Aktiv und Fallback, neu
// anmelden; hinter dem Zahnrad im Dialog Obergrenzen, Name, Plugin, Ordner und Entfernen. Hinzufügen (mit Claude anmelden oder einen
// vorhandenen Ordner übernehmen) im Dialog. Welche Profile welche Accounts nutzen, steht in den Dispatcher-Einstellungen.
export function AccountsPanel() {
  const api = useApi();
  const loaded = useAsync(() => api.getAccounts(), [api]);
  // Nur für die Vorgabe der Obergrenze (Farbe der Ringe ohne eigene Grenze)
  const overview = useAsync(() => api.getUsageOverview(), [api]);
  const defaultLimit = overview.state.status === 'ready' ? overview.state.data.defaultLimit : FALLBACK_LIMIT;
  const { state, run } = useSaveStatus();
  const [adding, setAdding] = useState(false);
  const headSlot = useContext(SaveStatusSlot);
  const data = loaded.state.status === 'ready' ? loaded.state.data : null;
  const loggingIn = data?.accounts.some((a) => a.login?.state === 'running') ?? false;

  // Solange eine Anmeldung läuft, regelmäßig nachladen, damit Erfolg oder Fehler ohne Klick erscheinen
  useEffect(() => {
    if (!loggingIn) return;
    const timer = setInterval(() => void loaded.reload(), LOGIN_POLL_MS);
    return () => clearInterval(timer);
  }, [loggingIn, loaded.reload]);

  // Sonst die Limits nachladen wie der Ring in der Seitenleiste: nach einem vorübergehenden Fehlschlag (etwa gleich nach
  // einer Anmeldung, während das Token wechselt) schnell, sonst alle zwei Minuten. Scheitert das Nachladen, bleibt die Liste
  // und der nächste Versuch kommt ebenso.
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    if (!data || loggingIn) return;
    const quick = data.accounts.some((a) => a.enabled && transient(a.usage));
    const timer = setTimeout(() => {
      void loaded.refresh().then((error) => {
        if (error) setAttempt((n) => n + 1);
      });
    }, quick ? RETRY_MS : REFRESH_MS);
    return () => clearTimeout(timer);
  }, [data, loggingIn, attempt, loaded.refresh]);

  // Ändert etwas und übernimmt die neue Liste
  const change: Change = async (fn) => {
    const next = await run(fn);
    if (next) loaded.set(next);
    return Boolean(next);
  };

  let body;
  if (loaded.state.status === 'loading') body = <LoadingState variant="detail" text="Accounts werden geladen …" />;
  else if (loaded.state.status === 'error') body = <ErrorState message={loaded.state.message} onRetry={loaded.retry} />;
  else {
    const list = loaded.state.data;
    // Info und Hinzufügen stehen in der Kopfzeile neben „Accounts“ (ohne Kopfzeile, etwa in Tests, über den Kacheln)
    const toolbar = (
      <div className="accounts-toolbar">
        <InfoTip label="Accounts">{INFO}</InfoTip>
        <button type="button" className="icon-btn accounts-add" aria-label="Account hinzufügen" title="Account hinzufügen" onClick={() => setAdding(true)}>
          <Icon name="add" />
        </button>
      </div>
    );
    body = (
      <>
        {headSlot ? createPortal(toolbar, headSlot) : toolbar}
        <div className="account-tiles">
          {list.accounts.map((account) => (
            <AccountTile key={account.id} account={account} defaultLimit={defaultLimit} pluginCommands={list.pluginCommands} accountsDir={list.accountsDir} change={change} />
          ))}
        </div>
        {adding && <AddAccountDialog data={list} change={change} onClose={() => setAdding(false)} />}
      </>
    );
  }
  return (
    // Speicherstand nur bei einem Fehler; Erfolg zeigt sich an den Kacheln selbst
    <SettingsPanel label="Accounts" status={state.kind === 'error' ? state : undefined} className="accounts-panel">
      {body}
      <div className="accounts-notice">
        <Notice tone="warn" role="note" label="Hinweis">
          Mehrere Abo-Accounts, um die Limits eines einzelnen Abos zu umgehen, können gegen die Nutzungsbedingungen von Anthropic verstoßen. Nutze nur Accounts, die dafür vorgesehen sind (z. B. mehrere Team-Plätze).
        </Notice>
      </div>
    </SettingsPanel>
  );
}

// Schaltknopf mit Icon (Aktiv, Fallback): gedrückt = eingeschaltet
function IconToggle({ icon, label, pressed, disabled, onChange, className }: { icon: string; label: string; pressed: boolean; disabled: boolean; onChange: (next: boolean) => void; className?: string }) {
  return (
    <button type="button" className={`icon-btn account-toggle${className ? ` ${className}` : ''}`} aria-label={label} aria-pressed={pressed} title={`${label}: ${pressed ? 'an' : 'aus'}`} disabled={disabled} onClick={() => onChange(!pressed)}>
      <Icon name={icon} />
    </button>
  );
}

// Anmeldestatus je Farbe des Punkts vor dem Namen
const LOGIN_STATES = { running: 'angemeldet', waiting: 'Anmeldung läuft', blocked: 'nicht angemeldet' } as const;

function AccountTile({ account, defaultLimit, pluginCommands, accountsDir, change }: { account: AccountDto; defaultLimit: number; pluginCommands: string[]; accountsDir: string; change: Change }) {
  const api = useApi();
  const [busy, setBusy] = useState(false);
  const [editing, setEditing] = useState(false);
  const running = account.login?.state === 'running';
  const paused = account.pausedUntil && new Date(account.pausedUntil) > new Date() ? account.pausedUntil : null;
  const loginState: keyof typeof LOGIN_STATES = running ? 'waiting' : account.loggedIn ? 'running' : 'blocked';
  const reason = !account.usage.available && account.usage.reason && account.usage.reason !== 'no-login' ? USAGE_REASONS[account.usage.reason] : null;

  // Knopf, der während der Anfrage gesperrt bleibt
  const act = async (fn: () => Promise<AccountListDto>) => {
    setBusy(true);
    try {
      await change(fn);
    } finally {
      setBusy(false);
    }
  };

  return (
    <article className={`settings-card account-tile${account.enabled ? '' : ' off'}`} aria-label={account.name}>
      <div className="account-tile-main">
        {/* Ohne Werte ein leerer Ring; den Grund zeigt erst das Darüberfahren */}
        {/* Abgeschaltet: grauer Ring mit Ein/Aus-Zeichen statt der Initialen */}
        <div className="account-tile-ring" title={account.usage.available ? undefined : reason ?? (account.loggedIn ? 'Keine Limits' : 'Nicht angemeldet – keine Limits')}>
          <AccountRing entry={account} defaultLimit={defaultLimit} />
          {!account.enabled && <Icon name="power_settings_new" />}
        </div>
        <div className="account-tile-info">
          {/* Eine Zeile: Anmeldestatus als Punkt (nur aktiv), Name, E-Mail (falls anders als der Name), „Aus“ und Abo */}
          <div className="account-tile-title">
            {account.enabled && <span className={`state-dot ${loginState}`} role="img" aria-label={LOGIN_STATES[loginState]} title={LOGIN_STATES[loginState]} />}
            <strong>{account.name}</strong>
            {account.email && account.email !== account.name && <span className="mono">{account.email}</span>}
            <span className="account-tile-badges">
              {!account.enabled && <Badge>Aus</Badge>}
              {account.usage.plan && <Badge>Abo {account.usage.plan}</Badge>}
            </span>
          </div>
          <AccountUsageLines entry={account} defaultLimit={defaultLimit} />
          <div className="account-status">
            {account.enabled && account.fallback && <Badge>Fallback</Badge>}
            {!account.plugin && <Badge kind="waiting">Plugin fehlt</Badge>}
            {account.runToken && <Badge>Langzeit-Token</Badge>}
            {paused && <Badge kind="waiting">pausiert bis {hhmm(paused)}</Badge>}
          </div>
        </div>
      </div>
      {account.login && <LoginBox account={account} busy={busy} act={act} />}
      {!account.loggedIn && !running && account.login?.state !== 'failed' && (
        <div className="account-actions">
          <button type="button" className="btn small primary" disabled={busy} onClick={() => void act(() => api.loginAccount(account.id))}>Mit Claude anmelden</button>
        </div>
      )}
      <div className="account-tile-tools" role="toolbar" aria-label={`${account.name}: Aktionen`}>
        {/* Aktiv als gedrückter Schalter; abgeschaltet als deutlicher Knopf zum Einschalten */}
        {account.enabled ? (
          <IconToggle icon="power_settings_new" label="Aktiv" className="account-power" pressed disabled={busy}
            onChange={(enabled) => void act(() => api.updateAccount(account.id, { enabled }))} />
        ) : (
          <button type="button" className="btn primary account-power-on" disabled={busy} onClick={() => void act(() => api.updateAccount(account.id, { enabled: true }))}>
            <Icon name="power_settings_new" />
            Einschalten
          </button>
        )}
        <IconToggle icon="support" label="Nur als Fallback" pressed={account.fallback} disabled={busy || !account.enabled}
          onChange={(fallback) => void act(() => api.updateAccount(account.id, { fallback }))} />
        <button type="button" className="icon-btn" aria-label={account.loggedIn ? 'Neu anmelden' : 'Mit Claude anmelden'} title={account.loggedIn ? 'Neu anmelden' : 'Mit Claude anmelden'}
          disabled={busy || running} onClick={() => void act(() => api.loginAccount(account.id))}>
          <Icon name="login" />
        </button>
        <button type="button" className="icon-btn" aria-label="Einstellungen" title="Einstellungen" onClick={() => setEditing(true)}>
          <Icon name="settings" />
        </button>
      </div>
      {editing && (
        <AccountSettingsDialog account={account} busy={busy} act={act} paused={paused} pluginCommands={pluginCommands} accountsDir={accountsDir} onClose={() => setEditing(false)} />
      )}
    </article>
  );
}

// Einstellungen eines Accounts im Dialog: Obergrenzen, Name, Plugin, Ordner und Terminal, Pause aufheben, Entfernen
function AccountSettingsDialog({ account, busy, act, paused, pluginCommands, accountsDir, onClose }: {
  account: AccountDto;
  busy: boolean;
  act: (fn: () => Promise<AccountListDto>) => Promise<void>;
  paused: string | null;
  pluginCommands: string[];
  accountsDir: string;
  onClose: () => void;
}) {
  const api = useApi();
  const titleId = useId();
  const [name, setName] = useState(account.name);
  const [removing, setRemoving] = useState(false);
  const [deleteDir, setDeleteDir] = useState(false);
  const standard = account.configDir === null;
  const running = account.login?.state === 'running';
  const prefix = standard ? '' : WINDOWS ? `$env:CLAUDE_CONFIG_DIR="${account.effectiveDir}"; ` : `CLAUDE_CONFIG_DIR="${account.effectiveDir}" `;

  // Name folgt dem Server (z. B. nach der Anmeldung die E-Mail), solange niemand tippt
  useEffect(() => setName(account.name), [account.name]);

  const rename = () => {
    const next = name.trim();
    if (next && next !== account.name) void act(() => api.updateAccount(account.id, { name: next }));
    else setName(account.name);
  };

  return (
    <DbModal labelledBy={titleId} onEscape={() => (removing ? setRemoving(false) : onClose())} onBackdrop={onClose} className="panel db-modal wide account-add-modal">
      <header className="account-add-head">
        <h2 id={titleId}>{account.name}</h2>
        <button type="button" className="icon-btn" aria-label="Schließen" onClick={onClose}><Icon name="close" /></button>
      </header>
      {/* Links was der Dispatcher nutzt (Grenzen, Name, Plugin), rechts Ordner und Befehle fürs Terminal */}
      <div className="account-settings-cols">
        <div className="account-settings-col">
          <SettingRow label="Obergrenzen" hint="Über dieser Auslastung nimmt der Dispatcher den Account nicht mehr. Leer: Vorgabe des Dispatchers." wide group>
            <div className="account-limits">
              <LimitInput label="5 Stunden" value={account.limitFiveHour} disabled={busy} onSave={(limitFiveHour) => void act(() => api.updateAccount(account.id, { limitFiveHour }))} />
              <LimitInput label="Woche" value={account.limitWeek} disabled={busy} onSave={(limitWeek) => void act(() => api.updateAccount(account.id, { limitWeek }))} />
            </div>
          </SettingRow>
          <SettingRow label="Name" wide>
            <input value={name} maxLength={60} disabled={busy} onChange={(e) => setName(e.target.value)} onBlur={rename} onKeyDown={(e) => e.key === 'Enter' && rename()} />
          </SettingRow>
          {account.loggedIn && !account.plugin && !running && (
            <SettingRow label="Nexus-Plugin" hint="Ohne Plugin arbeitet ein Lauf ohne Board-Anbindung; der Dispatcher wählt den Account dann nicht." wide group>
              <button type="button" className="btn small" disabled={busy || !pluginCommands.length} onClick={() => void act(() => api.installAccountPlugin(account.id))}>
                Plugin einrichten
              </button>
            </SettingRow>
          )}
          <RunTokenSetting account={account} busy={busy} act={act} />
        </div>
        <section className="account-settings-col" aria-labelledby={`${titleId}-dir`}>
          <h3 id={`${titleId}-dir`} className="setting-label">Ordner und Terminal</h3>
          <p className="setting-hint">Ordner: <span className="mono">{account.effectiveDir}</span></p>
          <p className="setting-hint">Anmelden im Terminal (danach <code>/login</code>):</p>
          <CopyCommand command={`${prefix}claude`} compact />
          {pluginCommands.length > 0 && <p className="setting-hint">Plugin im Terminal einrichten:</p>}
          {pluginCommands.map((c) => <CopyCommand key={c} command={`${prefix}${c}`} compact />)}
        </section>
      </div>
      {removing ? (
        <div className="account-remove" role="group" aria-label={`${account.name} entfernen`}>
          <p>
            {account.references.length
              ? <>Der Account wird auch aus der Dispatcher-Einstellung entfernt ({account.references.join(', ')}).</>
              : <>Der Account wird aus der Liste entfernt.</>}
          </p>
          <label className="option-row ack">
            <input type="checkbox" checked={deleteDir} onChange={(e) => setDeleteDir(e.target.checked)} />
            <span>Auch den Ordner samt Anmeldung löschen (nur unter <span className="mono">{accountsDir}</span>)</span>
          </label>
          <div className="actions">
            <button type="button" className="btn small" onClick={() => { setRemoving(false); setDeleteDir(false); }}>Abbrechen</button>
            <button type="button" className="btn small danger" disabled={busy} onClick={() => void act(() => api.deleteAccount(account.id, deleteDir))}>Entfernen</button>
          </div>
        </div>
      ) : (
        <div className="account-add-actions">
          {!standard && (
            <button type="button" className="btn small danger account-remove-start" disabled={busy} onClick={() => setRemoving(true)}>Entfernen …</button>
          )}
          {paused && (
            <button type="button" className="btn small" disabled={busy} onClick={() => void act(() => api.resumeAccount(account.id))}>Pause aufheben</button>
          )}
          <button type="button" className="btn small primary" onClick={onClose}>Fertig</button>
        </div>
      )}
    </DbModal>
  );
}

// Langzeit-Token für die Läufe: einfügen oder entfernen. Das Token selbst zeigt die Oberfläche nie, nur ob eins gesetzt ist
function RunTokenSetting({ account, busy, act }: { account: AccountDto; busy: boolean; act: (fn: () => Promise<AccountListDto>) => Promise<void> }) {
  const api = useApi();
  const [token, setToken] = useState('');
  const save = () => {
    if (!token.trim()) return;
    void act(() => api.setAccountRunToken(account.id, token.trim())).then(() => setToken(''));
  };
  return (
    <SettingRow label="Langzeit-Token für Läufe"
      hint="Läufe melden sich damit an statt mit der Anmeldung im Ordner und erneuern sie nie – so machen parallele Läufe die Anmeldung nicht ungültig. Im Browser denselben Account wählen; das Token gilt ein Jahr."
      wide group>
      {account.runToken ? (
        <div className="account-run-token">
          <Badge kind="running">gesetzt</Badge>
          <button type="button" className="btn small" disabled={busy} onClick={() => void act(() => api.removeAccountRunToken(account.id))}>Entfernen</button>
        </div>
      ) : (
        <>
          <CopyCommand command="claude setup-token" compact />
          <div className="account-run-token">
            <input type="password" autoComplete="off" value={token} placeholder="sk-ant-oat01-…" aria-label="Langzeit-Token" disabled={busy}
              onChange={(e) => setToken(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && save()} />
            <button type="button" className="btn small" disabled={busy || !token.trim()} onClick={save}>Speichern</button>
          </div>
        </>
      )}
    </SettingRow>
  );
}

// Stand der Anmeldung: läuft (Link zur Anmeldeseite, Code-Feld, Abbrechen) oder gescheitert (Grund, erneut); fertig zeigt
// sich am Punkt vor dem Namen
function LoginBox({ account, busy, act }: { account: AccountDto; busy: boolean; act: (fn: () => Promise<AccountListDto>) => Promise<void> }) {
  const api = useApi();
  const [code, setCode] = useState('');
  const login = account.login!;
  if (login.state === 'cancelled' || login.state === 'done') return null;
  if (login.state === 'failed') {
    return (
      <div className="account-login">
        <Notice tone="warn" role="note" label="Anmeldung gescheitert">
          Die Anmeldung ist nicht gelungen.{login.output.trim() && <pre className="account-login-output">{tail(login.output)}</pre>}
        </Notice>
        <div className="actions">
          <button type="button" className="btn small primary" disabled={busy} onClick={() => void act(() => api.loginAccount(account.id))}>Erneut anmelden</button>
        </div>
      </div>
    );
  }
  const send = () => {
    if (!code.trim()) return;
    void act(() => api.sendLoginCode(account.id, code)).then(() => setCode(''));
  };
  return (
    <div className="account-login" role="status">
      <p>Anmeldung läuft: Melde dich im Browser mit diesem Account an. Danach zeigt Claude einen Code – kopiere ihn hierher und klicke „Code senden“.</p>
      {login.url && (
        <p className="setting-hint">
          Kein Fenster aufgegangen? <a href={login.url} target="_blank" rel="noreferrer">Anmeldeseite öffnen</a>
        </p>
      )}
      <div className="account-login-code">
        <input value={code} placeholder="Code von der Claude-Seite" aria-label="Code aus dem Browser" onChange={(e) => setCode(e.target.value)} onKeyDown={(e) => e.key === 'Enter' && send()} />
        <button type="button" className="btn small" disabled={busy || !code.trim()} onClick={send}>Code senden</button>
        <button type="button" className="btn small" disabled={busy} onClick={() => void act(() => api.cancelLogin(account.id))}>Abbrechen</button>
      </div>
    </div>
  );
}


// Eingabe einer Obergrenze in Prozent; leer = keine eigene. Gespeichert wird beim Verlassen des Felds oder mit Enter
function LimitInput({ label, value, disabled, onSave }: { label: string; value: number | null; disabled: boolean; onSave: (value: number | null) => void }) {
  const [text, setText] = useState(value == null ? '' : String(value));
  useEffect(() => setText(value == null ? '' : String(value)), [value]);
  const save = () => {
    const trimmed = text.trim();
    const next = trimmed === '' ? null : Number(trimmed);
    if (next !== null && !(Number.isInteger(next) && next >= 1 && next <= 100)) {
      setText(value == null ? '' : String(value));
      return;
    }
    if (next !== value) onSave(next);
  };
  return (
    <label className="account-limit">
      <span>{label}</span>
      <input className="short" inputMode="numeric" value={text} placeholder="–" disabled={disabled} aria-label={`Obergrenze ${label} in Prozent`}
        onChange={(e) => setText(e.target.value)} onBlur={save} onKeyDown={(e) => e.key === 'Enter' && save()} />
      <span>%</span>
    </label>
  );
}


// Neuer Account im Dialog: Anmelden ist der Normalfall (Ordner entsteht von selbst); einen vorhandenen Ordner übernehmen nur
// unter „Erweitert“. Klappt es, schließt der Dialog und die neue Kachel zeigt die Anmeldung.
function AddAccountDialog({ data, change, onClose }: { data: AccountListDto; change: Change; onClose: () => void }) {
  const api = useApi();
  const titleId = useId();
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [dirName, setDirName] = useState('');
  const [dir, setDir] = useState('');
  const [busy, setBusy] = useState(false);
  const submit = async (fn: () => Promise<AccountListDto>) => {
    setBusy(true);
    const ok = await change(fn);
    setBusy(false);
    if (ok) onClose();
  };
  return (
    <DbModal labelledBy={titleId} onEscape={onClose} onBackdrop={onClose} className="panel db-modal account-add-modal">
      <header className="account-add-head">
        <h2 id={titleId}>Account hinzufügen</h2>
        <button type="button" className="icon-btn" aria-label="Schließen" onClick={onClose}><Icon name="close" /></button>
      </header>
      <p className="setting-hint">Melde dich mit einem weiteren Claude-Account an; Ordner und Plugin richtet Nexus selbst ein.</p>
      <SettingRow label="Name (optional)" hint="Leer: der Account heißt nach der Anmeldung wie seine E-Mail.">
        <input value={name} maxLength={60} placeholder="Max privat" autoFocus onChange={(e) => setName(e.target.value)} />
      </SettingRow>
      <SettingRow label="E-Mail (optional)" hint="Wird auf der Anmeldeseite vorausgefüllt.">
        <input type="email" value={email} placeholder="ich@example.com" onChange={(e) => setEmail(e.target.value)} />
      </SettingRow>
      <details className="account-more">
        <summary>Erweitert: vorhandenen Ordner übernehmen</summary>
        <div className="account-more-body">
          <p className="setting-hint">Für einen Ordner, in dem Claude Code schon angemeldet ist, z. B. ein MCACC-Profil.</p>
          <SettingRow label="Name">
            <input value={dirName} maxLength={60} placeholder="MCACC Firma" onChange={(e) => setDirName(e.target.value)} />
          </SettingRow>
          <SettingRow label="Ordner" wide>
            <input value={dir} list="account-dir-suggestions" placeholder="~/.mcacc/profiles/firma/claude" onChange={(e) => setDir(e.target.value)} />
          </SettingRow>
          <datalist id="account-dir-suggestions">
            {data.suggestions.map((s) => <option key={s} value={s} />)}
          </datalist>
          <div className="actions">
            <button type="button" className="btn small" disabled={busy || !dirName.trim() || !dir.trim()} onClick={() => void submit(() => api.createAccount(dirName.trim(), dir.trim()))}>Übernehmen</button>
          </div>
        </div>
      </details>
      <div className="account-add-actions">
        <button type="button" className="btn small" onClick={onClose}>Abbrechen</button>
        <button type="button" className="btn small primary" disabled={busy} onClick={() => void submit(() => api.loginNewAccount(name.trim(), email.trim()))}>Mit Claude anmelden</button>
      </div>
    </DbModal>
  );
}

// Grund einer gescheiterten Anmeldung: ohne Hinweis aufs Browser-Öffnen, Anmeldelink und Eingabeaufforderung
function tail(text: string): string {
  return text
    .split(/\r?\n/)
    .map((line) => line.replace(/^.*Paste code here if prompted >\s*/, ''))
    .filter((line) => line.trim() && !/https?:\/\//.test(line) && !/^Opening browser/.test(line))
    .slice(-6)
    .join('\n');
}

function hhmm(iso: string): string {
  return new Date(iso).toLocaleTimeString('de-DE', { hour: '2-digit', minute: '2-digit' });
}

const INFO = (
  <ul>
    <li>„Mit Claude anmelden“ legt einen eigenen Ordner für Claude Code an (unter <code>~/.nexus/accounts</code>), öffnet die Anmeldeseite im Browser und richtet danach das Nexus-Plugin ein. „Standard“ ist die Anmeldung des Servers.</li>
    <li>Der Dispatcher startet jeden Lauf mit <code>CLAUDE_CONFIG_DIR</code> des gewählten Accounts. Welche Accounts ein Profil nutzen darf, legst du unter Dispatcher → Profile fest; ein Profil ohne Accounts darf jeden aktiven verwenden.</li>
    <li>Ein Account „Nur als Fallback“ kommt erst zum Zug, wenn alle anderen erlaubten Accounts voll, pausiert oder abgemeldet sind – etwa ein privater Account, der nur einspringen soll.</li>
    <li>Erreicht ein Account sein Nutzungslimit, pausiert nur er – bis sein Limit zurückgesetzt wird. Das Ticket startet auf einem anderen freien Account neu.</li>
    <li>Die Limits liest Nexus mit dem Anmelde-Token des Accounts (macOS: Schlüsselbund, sonst <code>.credentials.json</code>) über einen nicht dokumentierten Anthropic-Endpunkt, höchstens einmal pro Minute je Account. Das Token wird nie gespeichert.</li>
    <li>Mit einem Langzeit-Token (<code>claude setup-token</code>) erneuern die Läufe die Anmeldung im Ordner nicht mehr. Parallele Läufe können sich dann nicht gegenseitig abmelden. Ist das Token im Ordner danach abgelaufen, erneuert Nexus es selbst, damit die Limits lesbar bleiben.</li>
  </ul>
);
