import { useEffect, useState } from 'react';
import { USAGE_REASONS } from './ClaudeUsageMeter.js';
import { useApi } from '../api/ApiContext.js';
import type { AccountDto, AccountListDto, UsageWindowDto } from '../api/usageTypes.js';
import { useAsync } from '../hooks/useAsync.js';
import { Badge } from './Badge.js';
import { ClaudeUsageMeter } from './ClaudeUsageMeter.js';
import { CopyCommand } from './CopyCommand.js';
import { ErrorState } from './ErrorState.js';
import { LoadingState } from './LoadingState.js';
import { Notice } from './settings/Notice.js';
import { SettingsPanel, useSaveStatus } from './settings/SettingsPanel.js';
import { SettingRow, SettingsSection } from './settings/SettingRow.js';

const WINDOWS = typeof navigator !== 'undefined' && /win/i.test(navigator.platform);
/** So oft fragt die Seite nach, solange eine Anmeldung läuft */
const LOGIN_POLL_MS = 2000;

type Change = (fn: () => Promise<AccountListDto>) => Promise<boolean>;

// Claude-Accounts des Dispatchers: mit Claude anmelden (der Ordner entsteht von selbst), Plugin einrichten, abschalten,
// entfernen; je Account Anmeldung, Plugin, Limits und eine laufende Pause. Einen vorhandenen Ordner zu übernehmen ist eine
// erweiterte Option. Welche Profile welche Accounts nutzen, steht in den Dispatcher-Einstellungen.
export function AccountsPanel() {
  const api = useApi();
  const loaded = useAsync(() => api.getAccounts(), [api]);
  const { state, run } = useSaveStatus();
  const data = loaded.state.status === 'ready' ? loaded.state.data : null;
  const loggingIn = data?.accounts.some((a) => a.login?.state === 'running') ?? false;

  // Solange eine Anmeldung läuft, regelmäßig nachladen, damit Erfolg oder Fehler ohne Klick erscheinen
  useEffect(() => {
    if (!loggingIn) return;
    const timer = setInterval(() => void loaded.reload(), LOGIN_POLL_MS);
    return () => clearInterval(timer);
  }, [loggingIn, loaded.reload]);

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
    body = (
      <>
        <SettingsSection info={INFO}>
          <Notice tone="warn" role="note" label="Hinweis">
            Mehrere Abo-Accounts, um die Limits eines einzelnen Abos zu umgehen, können gegen die Nutzungsbedingungen von Anthropic verstoßen. Nutze nur Accounts, die dafür vorgesehen sind (z. B. mehrere Team-Plätze).
          </Notice>
          <p className="setting-hint accounts-mode">
            {list.multi
              ? 'Mehrere Accounts aktiv: Der Dispatcher verteilt die Läufe und liest dafür die Limits jedes aktiven Accounts aus.'
              : 'Ein Account aktiv: Der Dispatcher arbeitet wie bisher mit einem Account und fragt keine Limits ab.'}
          </p>
        </SettingsSection>
        {list.accounts.map((account) => (
          <AccountCard key={account.id} account={account} pluginCommands={list.pluginCommands} accountsDir={list.accountsDir} change={change} />
        ))}
        <AddAccount data={list} change={change} />
      </>
    );
  }
  // Oben die Limits aller aktiven Accounts als Ringe mit Werten (wie in der Seitenleiste, dort nur die Ringe)
  return (
    <SettingsPanel label="Accounts" status={state}>
      <ClaudeUsageMeter variant="full" />
      {body}
    </SettingsPanel>
  );
}

function AccountCard({ account, pluginCommands, accountsDir, change }: { account: AccountDto; pluginCommands: string[]; accountsDir: string; change: Change }) {
  const api = useApi();
  const [name, setName] = useState(account.name);
  const [removing, setRemoving] = useState(false);
  const [deleteDir, setDeleteDir] = useState(false);
  const [busy, setBusy] = useState(false);
  const standard = account.configDir === null;
  const running = account.login?.state === 'running';
  const paused = account.pausedUntil && new Date(account.pausedUntil) > new Date() ? account.pausedUntil : null;
  const prefix = standard ? '' : WINDOWS ? `$env:CLAUDE_CONFIG_DIR="${account.effectiveDir}"; ` : `CLAUDE_CONFIG_DIR="${account.effectiveDir}" `;
  // Braucht der Account Aufmerksamkeit (Anmeldung läuft oder fehlt), ist die Kachel offen; sonst gilt die gemerkte Wahl
  const attention = running || account.login?.state === 'failed' || !account.loggedIn;
  const [open, setOpen] = useState(() => attention || readOpen(account.id));
  const toggle = () => {
    setOpen(!open);
    writeOpen(account.id, !open);
  };

  // Name folgt dem Server (z. B. nach der Anmeldung die E-Mail), solange niemand tippt
  useEffect(() => setName(account.name), [account.name]);
  // Eine neue Anmeldung oder ein Fehler klappt die Kachel auf
  useEffect(() => {
    if (attention) setOpen(true);
  }, [attention]);

  // Knopf, der während der Anfrage gesperrt bleibt
  const act = async (fn: () => Promise<AccountListDto>) => {
    setBusy(true);
    try {
      await change(fn);
    } finally {
      setBusy(false);
    }
  };

  const rename = () => {
    const next = name.trim();
    if (next && next !== account.name) void act(() => api.updateAccount(account.id, { name: next }));
    else setName(account.name);
  };

  return (
    <SettingsSection>
      <button type="button" className="account-head" aria-expanded={open} aria-controls={`account-${account.id}`} onClick={toggle}>
        <span className="account-chevron" aria-hidden="true">▸</span>
        <span className="account-head-text">
          <strong>{account.name}</strong>
          <span className="mono">{account.id}{account.email && account.email !== account.name ? ` · ${account.email}` : ''}</span>
        </span>
        <span className="account-head-side">
          {!account.enabled && <Badge>abgeschaltet</Badge>}
          {account.enabled && account.fallback && <Badge>Fallback</Badge>}
          {running
            ? <Badge kind="waiting">Anmeldung läuft</Badge>
            : account.loggedIn ? <Badge kind="running">angemeldet</Badge> : <Badge kind="blocked">nicht angemeldet</Badge>}
          {!account.plugin && <Badge kind="waiting">Plugin fehlt</Badge>}
          {paused && <Badge kind="waiting">pausiert bis {hhmm(paused)}</Badge>}
          {account.usage.available && <span className="account-head-usage">{usageSummary(account)}</span>}
        </span>
      </button>
      {open && <div id={`account-${account.id}`} className="account-body">
      <div className="account-status">
        {account.loggedIn && account.email && <Badge kind="running">angemeldet als {account.email}</Badge>}
        {account.usage.plan && <Badge>Abo {account.usage.plan}</Badge>}
        {account.plugin && <Badge kind="running">Plugin eingerichtet</Badge>}
      </div>
      <UsageRows account={account} />
      {account.login && <LoginBox account={account} busy={busy} act={act} />}
      {!account.loggedIn && !running && account.login?.state !== 'failed' && (
        <SettingRow label="Anmelden" hint="Öffnet die Anmeldeseite von Claude im Browser. Melde dich dort mit diesem Account an." group>
          <button type="button" className="btn small primary" disabled={busy} onClick={() => void act(() => api.loginAccount(account.id))}>Mit Claude anmelden</button>
        </SettingRow>
      )}
      <SettingRow label="Aktiv" hint="Abgeschaltete Accounts wählt der Dispatcher nie.">
        <input type="checkbox" checked={account.enabled} disabled={busy} onChange={(e) => void act(() => api.updateAccount(account.id, { enabled: e.target.checked }))} />
      </SettingRow>
      <SettingRow label="Nur als Fallback" hint="Der Dispatcher nimmt diesen Account erst, wenn kein anderer erlaubter Account frei ist.">
        <input type="checkbox" checked={account.fallback} disabled={busy || !account.enabled} onChange={(e) => void act(() => api.updateAccount(account.id, { fallback: e.target.checked }))} />
      </SettingRow>
      <SettingRow label="Obergrenzen" hint="Über dieser Auslastung nimmt der Dispatcher den Account nicht mehr. Leer: keine eigene Grenze (Vorgabe des Dispatchers)." wide group>
        <div className="account-limits">
          <LimitInput label="5 Stunden" value={account.limitFiveHour} disabled={busy} onSave={(limitFiveHour) => void act(() => api.updateAccount(account.id, { limitFiveHour }))} />
          <LimitInput label="Woche" value={account.limitWeek} disabled={busy} onSave={(limitWeek) => void act(() => api.updateAccount(account.id, { limitWeek }))} />
        </div>
      </SettingRow>
      <SettingRow label="Name">
        <input value={name} maxLength={60} disabled={busy} onChange={(e) => setName(e.target.value)} onBlur={rename} onKeyDown={(e) => e.key === 'Enter' && rename()} />
      </SettingRow>
      {account.loggedIn && !account.plugin && !running && (
        <SettingRow label="Nexus-Plugin" hint="Ohne Plugin arbeitet ein Lauf ohne Board-Anbindung; der Dispatcher wählt den Account dann nicht." group>
          <button type="button" className="btn small" disabled={busy || !pluginCommands.length} onClick={() => void act(() => api.installAccountPlugin(account.id))}>
            Plugin einrichten
          </button>
        </SettingRow>
      )}
      <details className="account-more">
        <summary>Ordner und Terminal</summary>
        <div className="account-more-body">
          <p className="setting-hint">Ordner: <span className="mono">{account.effectiveDir}</span></p>
          <p className="setting-hint">Anmelden im Terminal (danach <code>/login</code>):</p>
          <CopyCommand command={`${prefix}claude`} compact />
          {pluginCommands.length > 0 && <p className="setting-hint">Plugin im Terminal einrichten:</p>}
          {pluginCommands.map((c) => <CopyCommand key={c} command={`${prefix}${c}`} compact />)}
        </div>
      </details>
      {(paused || !standard || (account.loggedIn && !running)) && !removing && (
        <div className="account-actions">
          {account.loggedIn && !running && (
            <button type="button" className="btn small" disabled={busy} onClick={() => void act(() => api.loginAccount(account.id))}>Neu anmelden</button>
          )}
          {paused && (
            <button type="button" className="btn small" disabled={busy} onClick={() => void act(() => api.resumeAccount(account.id))}>Pause aufheben</button>
          )}
          {!standard && (
            <button type="button" className="btn small danger" disabled={busy} onClick={() => setRemoving(true)}>Entfernen …</button>
          )}
        </div>
      )}
      {removing && (
        <div className="account-remove" role="group" aria-label={`${account.name} entfernen`}>
          <p>
            {account.references.length
              ? <>Der Account wird auch aus der Dispatcher-Einstellung entfernt ({account.references.join(', ')}).</>
              : <>Der Account wird aus der Liste entfernt.</>}
          </p>
          <label className="option-row">
            <input type="checkbox" checked={deleteDir} onChange={(e) => setDeleteDir(e.target.checked)} />
            <span>Auch den Ordner samt Anmeldung löschen (nur unter <span className="mono">{accountsDir}</span>)</span>
          </label>
          <div className="actions">
            <button type="button" className="btn small" onClick={() => { setRemoving(false); setDeleteDir(false); }}>Abbrechen</button>
            <button type="button" className="btn small danger" disabled={busy} onClick={() => void act(() => api.deleteAccount(account.id, deleteDir))}>Entfernen</button>
          </div>
        </div>
      )}
      </div>}
    </SettingsSection>
  );
}

// Kurzfassung der Limits für den Kopf der Kachel, z. B. „5 h 22 % · Woche 35 %“
function usageSummary(account: AccountDto): string {
  const { fiveHour, sevenDay } = account.usage;
  return [fiveHour ? `5 h ${fiveHour.percent} %` : null, sevenDay ? `Woche ${sevenDay.percent} %` : null].filter(Boolean).join(' · ');
}

// Auf- oder zugeklappt je Account im Browser merken; ohne Speicher (privates Fenster) bleibt es beim Standard
function readOpen(id: string): boolean {
  try {
    return localStorage.getItem(`harness.account-open.${id}`) === '1';
  } catch {
    return false;
  }
}

function writeOpen(id: string, open: boolean): void {
  try {
    localStorage.setItem(`harness.account-open.${id}`, open ? '1' : '0');
  } catch {
    /* nur Komfort */
  }
}

// Stand der Anmeldung: läuft (Link zur Anmeldeseite, Code-Feld, Abbrechen), gescheitert (Grund, erneut) oder fertig (Plugin)
function LoginBox({ account, busy, act }: { account: AccountDto; busy: boolean; act: (fn: () => Promise<AccountListDto>) => Promise<void> }) {
  const api = useApi();
  const [code, setCode] = useState('');
  const login = account.login!;
  if (login.state === 'cancelled') return null;
  if (login.state === 'done') {
    return login.plugin ? <p className="setting-hint account-login-note">Angemeldet. {login.plugin}.</p> : null;
  }
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

// Zwei Balken: 5 Stunden und Woche; ohne Werte der Grund
function UsageRows({ account }: { account: AccountDto }) {
  const { usage } = account;
  if (!usage.available) {
    const reason = usage.reason && usage.reason !== 'no-login' ? USAGE_REASONS[usage.reason] : null;
    return reason ? <p className="setting-hint account-usage-none">{reason}</p> : null;
  }
  return (
    <div className="account-usage">
      <UsageBar label="5 Stunden" window={usage.fiveHour} limit={account.limitFiveHour} />
      <UsageBar label="Woche" window={usage.sevenDay} limit={account.limitWeek} />
    </div>
  );
}

// Balken mit eigener Obergrenze als Strich; ab der Grenze (sonst ab 90 %) rot
function UsageBar({ label, window, limit }: { label: string; window: UsageWindowDto | null; limit: number | null }) {
  if (!window) return null;
  const percent = Math.min(100, window.percent);
  const full = percent >= (limit ?? 90);
  return (
    <div className="account-usage-row">
      <span className="account-usage-label">{label}</span>
      <div className="progress-bar account-usage-bar" role="img" aria-label={`${label}: ${window.percent} % genutzt${limit != null ? `, Obergrenze ${limit} %` : ''}`}>
        <span style={{ width: `${percent}%` }} className={full ? 'full' : undefined} />
        {limit != null && <i className="account-usage-limit" style={{ left: `${limit}%` }} aria-hidden="true" />}
      </div>
      <span className="account-usage-value">{window.percent} %{limit != null ? ` von max. ${limit} %` : ''}{window.resetsAt ? ` · zurück ${when(window.resetsAt)}` : ''}</span>
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

// Neuer Account: Anmelden ist der Normalfall (Ordner entsteht von selbst); einen vorhandenen Ordner übernehmen nur unter „Erweitert“
function AddAccount({ data, change }: { data: AccountListDto; change: Change }) {
  const api = useApi();
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [dirName, setDirName] = useState('');
  const [dir, setDir] = useState('');
  const login = async () => {
    if (await change(() => api.loginNewAccount(name.trim(), email.trim()))) {
      setName('');
      setEmail('');
    }
  };
  const adopt = async () => {
    if (await change(() => api.createAccount(dirName.trim(), dir.trim()))) {
      setDirName('');
      setDir('');
    }
  };
  return (
    <SettingsSection title="Account hinzufügen" description="Melde dich mit einem weiteren Claude-Account an; Ordner und Plugin richtet Nexus selbst ein.">
      <SettingRow label="Name (optional)" hint="Leer: der Account heißt nach der Anmeldung wie seine E-Mail.">
        <input value={name} maxLength={60} placeholder="Max privat" onChange={(e) => setName(e.target.value)} />
      </SettingRow>
      <SettingRow label="E-Mail (optional)" hint="Wird auf der Anmeldeseite vorausgefüllt.">
        <input type="email" value={email} placeholder="ich@example.com" onChange={(e) => setEmail(e.target.value)} />
      </SettingRow>
      <div className="account-actions">
        <button type="button" className="btn small primary" onClick={() => void login()}>Mit Claude anmelden</button>
      </div>
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
            <button type="button" className="btn small" disabled={!dirName.trim() || !dir.trim()} onClick={() => void adopt()}>Übernehmen</button>
          </div>
        </div>
      </details>
    </SettingsSection>
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

// Uhrzeit, bei mehr als einem Tag Abstand mit Datum
function when(iso: string): string {
  const at = new Date(iso);
  const sameDay = at.getTime() - Date.now() < 24 * 3_600_000;
  return sameDay ? hhmm(iso) : at.toLocaleString('de-DE', { weekday: 'short', day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit' });
}

const INFO = (
  <ul>
    <li>„Mit Claude anmelden“ legt einen eigenen Ordner für Claude Code an (unter <code>~/.nexus/accounts</code>), öffnet die Anmeldeseite im Browser und richtet danach das Nexus-Plugin ein. „Standard“ ist die Anmeldung des Servers.</li>
    <li>Der Dispatcher startet jeden Lauf mit <code>CLAUDE_CONFIG_DIR</code> des gewählten Accounts. Welche Accounts ein Profil nutzen darf, legst du unter Dispatcher → Profile fest; ein Profil ohne Accounts darf jeden aktiven verwenden.</li>
    <li>Ein Account „Nur als Fallback“ kommt erst zum Zug, wenn alle anderen erlaubten Accounts voll, pausiert oder abgemeldet sind – etwa ein privater Account, der nur einspringen soll.</li>
    <li>Erreicht ein Account sein Nutzungslimit, pausiert nur er – bis sein Limit zurückgesetzt wird. Das Ticket startet auf einem anderen freien Account neu.</li>
    <li>Die Limits liest Nexus mit dem Anmelde-Token des Accounts (macOS: Schlüsselbund, sonst <code>.credentials.json</code>) über einen nicht dokumentierten Anthropic-Endpunkt, höchstens einmal pro Minute je Account. Das Token wird nie gespeichert oder erneuert.</li>
  </ul>
);
