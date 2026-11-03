import { useEffect, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type {
  BotStatus, IdeaTarget, TelegramConfig, TelegramConfigInput, TelegramPairingDto, TelegramTestDto,
} from '../api/telegramTypes.js';
import { useAsync } from '../hooks/useAsync.js';
import { Notice } from './settings/Notice.js';
import { SettingsFooter, SettingsPanel } from './settings/SettingsPanel.js';
import { SettingRow, SettingsSection } from './settings/SettingRow.js';

// Name und Kurzbeschreibung, die Nexus am Bot einrichtet (crates/nexus-server/src/telegram/texts.rs)
const PROFILE_NAME = 'Nexus';
const PROFILE_SHORT = 'Nexus-Board unterwegs: Ideen per Sprache, Freigaben, Status';

// Vorgabe der Nachtruhe, wenn sie gerade eingeschaltet wird
const QUIET_DEFAULT = { from: '22:00', to: '07:00' };

// Anlässe in der Reihenfolge, in der sie in der Oberfläche stehen
const NOTIFY_ROWS: Array<{ key: keyof TelegramConfig['notify']; label: string; hint: string }> = [
  { key: 'review', label: 'Review', hint: 'Ein Lauf ist fertig und wartet auf Review.' },
  { key: 'blocked', label: 'Blockiert', hint: 'Ein Ticket ist blockiert.' },
  { key: 'question', label: 'Frage', hint: 'Eine Frage wartet auf eine Entscheidung.' },
  { key: 'merge', label: 'Merge gescheitert', hint: 'Ein Ticket ist bereit zum Mergen, der Merge ging aber nicht durch.' },
  { key: 'merged', label: 'Gemergt', hint: 'Ein Merge ist durch.' },
  { key: 'paused', label: 'Pause', hint: 'Der Dispatcher pausiert.' },
];

// Der Stand des Pollers in Worten; der Ton bestimmt, wie die Zeile aussieht
function statusText(bot: BotStatus): { tone: 'ok' | 'info' | 'warn' | 'error'; text: string } {
  switch (bot.state) {
    case 'off':
      return { tone: 'info', text: 'Aus – der Bot pollt nicht und schickt keine Meldungen.' };
    case 'starting':
      return { tone: 'info', text: 'Startet …' };
    case 'running':
      return { tone: 'ok', text: bot.botName ? `Läuft als @${bot.botName}` : 'Läuft' };
    case 'conflict':
      return { tone: 'warn', text: 'Läuft auf einem anderen Rechner – dort holt schon jemand die Updates ab.' };
    case 'tokenInvalid':
      return { tone: 'error', text: 'Token ungültig – trag das Token von @BotFather neu ein.' };
    case 'error':
      return { tone: 'error', text: bot.lastError ?? 'Der Bot läuft nicht.' };
  }
}

// Bereich Telegram: Token, Kopplung, gekoppelte Chats, Anlässe, Nachtruhe, Ideen, Bot-Profil und Testnachricht.
export function TelegramPanel() {
  const api = useApi();
  const loaded = useAsync(() => api.getTelegramConfig(), [api]);
  const status = useAsync(() => api.getTelegramStatus(), [api]);
  const data = loaded.state.status === 'ready' ? loaded.state.data : null;
  const bot = status.state.status === 'ready' ? status.state.data.bot : null;
  const [form, setForm] = useState<TelegramConfig | null>(null);
  // Neues Token im Klartext; null = das gespeicherte bleibt
  const [token, setToken] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [pairing, setPairing] = useState<TelegramPairingDto | null>(null);
  const [test, setTest] = useState<TelegramTestDto | null>(null);

  useEffect(() => {
    if (data && !form) setForm(data);
  }, [data, form]);

  const change = (patch: Partial<TelegramConfig>) => setForm((current) => (current ? { ...current, ...patch } : current));
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
    const { chats: _chats, profile, botToken: _masked, ...rest } = form;
    const input: TelegramConfigInput = { ...rest, profile: { brand: profile.brand } };
    if (token !== null) input.botToken = token;
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

  // Holt einen Einmal-Code; der Link öffnet den Chat mit dem Bot und schickt ihn gleich mit
  const pair = async () => {
    start();
    setPairing(null);
    try {
      setPairing(await api.pairTelegram());
    } catch (e) {
      setError((e as Error).message);
    }
  };

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

  // Richtet das Bot-Profil neu ein, auch wenn es schon gesetzt war
  const applyProfile = async () => {
    start();
    try {
      const result = await api.applyTelegramProfile();
      if (result.error) setError(result.error);
      else setInfo(result.applied ? 'Profil des Bots neu gesetzt.' : 'Am Profil war nichts zu tun.');
      await status.refresh();
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const line = bot ? statusText(bot) : null;
  const chats = form?.chats ?? [];

  return (
    <SettingsPanel label="Telegram">
      {line && (
        <Notice tone={line.tone} role="note" label="Stand des Bots">{line.text}</Notice>
      )}
      {bot?.profileError && (
        <Notice tone="warn" role="note">Das Profil des Bots ließ sich nicht einrichten: {bot.profileError}. Der Bot läuft trotzdem.</Notice>
      )}
      {form && (
        <SettingsSection title="Bot" description="Der Bot kommt von @BotFather in Telegram; von dort stammt auch das Token.">
          <SettingRow label="Telegram-Bot" hint="Holt Updates ab und schickt Meldungen in die gekoppelten Chats.">
            <input type="checkbox" checked={form.enabled} onChange={(e) => change({ enabled: e.target.checked })} />
          </SettingRow>
          <SettingRow
            label="Bot-Token"
            hint={token !== null ? 'Das neue Token wird beim Speichern übernommen.' : form.botToken ? 'Gespeichert; hier steht nur die Maske.' : 'Noch kein Token eingetragen.'}
            info="Das Token von @BotFather. Es wird nur auf diesem Rechner gespeichert und in der Oberfläche nie im Klartext gezeigt."
            wide
            group
          >
            {token !== null ? (
              <div className="actions">
                <input className="mono" type="password" autoComplete="off" aria-label="Neues Bot-Token" value={token} placeholder="123456:ABC…" onChange={(e) => setToken(e.target.value)} />
                <button type="button" className="btn small" onClick={() => setToken(null)}>Abbrechen</button>
              </div>
            ) : (
              <div className="actions">
                <input className="mono" readOnly aria-label="Gespeichertes Bot-Token" value={form.botToken} placeholder="(keins)" />
                <button type="button" className="btn small" onClick={() => setToken('')}>Ändern</button>
              </div>
            )}
          </SettingRow>
          <SettingRow label="Adresse der Bot-API" info="Nur für einen selbst betriebenen telegram-bot-api-Server zu ändern." wide>
            <input className="mono" value={form.apiBase} onChange={(e) => change({ apiBase: e.target.value })} />
          </SettingRow>
        </SettingsSection>
      )}
      {form && (
        <SettingsSection title="Kopplung" description="Ein Chat bekommt nur Meldungen, wenn er gekoppelt ist.">
          <SettingRow label="Chat koppeln" hint="„Koppeln“ gibt einen Einmal-Code aus; der Link öffnet den Chat mit dem Bot und schickt ihn mit." group>
            <button type="button" className="btn" onClick={() => void pair()}>Koppeln</button>
          </SettingRow>
          {pairing && (
            <Notice tone="info" role="note" label="Kopplungscode">
              <p>Code <strong className="mono">{pairing.code}</strong>, gültig bis {new Date(pairing.expiresAt).toLocaleTimeString()}.</p>
              <p><a href={pairing.link} target="_blank" rel="noreferrer">Chat mit dem Bot öffnen</a></p>
            </Notice>
          )}
          <SettingRow label="Gekoppelte Chats" group>
            {chats.length === 0 ? (
              <p className="muted">Noch kein Chat gekoppelt.</p>
            ) : (
              <ul className="telegram-chats">
                {chats.map((chatId) => (
                  <li key={chatId}>
                    <span className="mono">{chatId}</span>
                    <button type="button" className="btn small" onClick={() => void unpair(chatId)}>Entkoppeln</button>
                  </li>
                ))}
              </ul>
            )}
          </SettingRow>
          <SettingRow label="Testnachricht" hint="Schickt eine kurze Nachricht an alle gekoppelten Chats." group>
            <button type="button" className="btn" onClick={() => void sendTest()}>Testnachricht</button>
          </SettingRow>
          {test && (
            <Notice tone={test.errors.length > 0 ? 'warn' : 'ok'} role="note" label="Ergebnis der Testnachricht">
              <p>{test.sent === 1 ? 'An 1 Chat verschickt.' : `An ${test.sent} Chats verschickt.`}</p>
              {test.errors.map((e) => (
                <p key={e.chatId}>Chat {e.chatId}: {e.error}</p>
              ))}
            </Notice>
          )}
        </SettingsSection>
      )}
      {form && (
        <SettingsSection title="Meldungen" description="Anlässe, zu denen der Bot von sich aus schreibt.">
          {NOTIFY_ROWS.map((row) => (
            <SettingRow key={row.key} label={row.label} hint={row.hint}>
              <input
                type="checkbox"
                checked={form.notify[row.key]}
                onChange={(e) => change({ notify: { ...form.notify, [row.key]: e.target.checked } })}
              />
            </SettingRow>
          ))}
          <SettingRow label="Nachtruhe" hint="In diesem Fenster schickt der Bot keine Meldungen.">
            <input
              type="checkbox"
              checked={form.quietHours !== null}
              onChange={(e) => change({ quietHours: e.target.checked ? { ...QUIET_DEFAULT } : null })}
            />
          </SettingRow>
          {form.quietHours && (
            <>
              <SettingRow label="Von">
                <input type="time" value={form.quietHours.from} onChange={(e) => change({ quietHours: { ...form.quietHours!, from: e.target.value } })} />
              </SettingRow>
              <SettingRow label="Bis">
                <input type="time" value={form.quietHours.to} onChange={(e) => change({ quietHours: { ...form.quietHours!, to: e.target.value } })} />
              </SettingRow>
            </>
          )}
        </SettingsSection>
      )}
      {form && (
        <SettingsSection title="Ideen" description="Was mit einer Idee passiert, die du dem Bot schickst.">
          <SettingRow label="Vorgabe" hint="Wohin eine Idee standardmäßig geht; im Chat lässt sich das noch umstellen.">
            <select value={form.ideas.defaultTarget} onChange={(e) => change({ ideas: { ...form.ideas, defaultTarget: e.target.value as IdeaTarget } })}>
              <option value="note">Notiz</option>
              <option value="ticket">Ticket</option>
            </select>
          </SettingRow>
          <SettingRow label="Höchstens Runden" hint="So oft fragt der Bot zu einer Idee nach (1–10).">
            <input className="short" type="number" min={1} max={10} value={form.ideas.maxRounds} onChange={(e) => change({ ideas: { ...form.ideas, maxRounds: Number(e.target.value) } })} />
          </SettingRow>
        </SettingsSection>
      )}
      {form && (
        <SettingsSection title="Profil des Bots" description="Profilbild, Name und Beschreibung des Bots in Telegram.">
          <SettingRow
            label="Profil des Bots von Nexus einrichten"
            hint="Nexus setzt Profilbild, Name, Beschreibungen und Befehlsliste beim Start."
            info="Ohne diesen Schalter bleibt am Bot, was in @BotFather eingetragen ist. Nexus setzt es nur einmal und merkt sich, was es gesetzt hat."
          >
            <input type="checkbox" checked={form.profile.brand} onChange={(e) => change({ profile: { ...form.profile, brand: e.target.checked } })} />
          </SettingRow>
          <SettingRow label="Vorschau" group>
            <div className="bot-profile-preview">
              <img src="/favicon.svg" alt="Profilbild des Bots: Nexus-Logo" width={48} height={48} />
              <div>
                <strong>{PROFILE_NAME}</strong>
                <p className="muted">{PROFILE_SHORT}</p>
              </div>
            </div>
          </SettingRow>
          <SettingRow
            label="Profil neu setzen"
            hint={form.profile.applied ? 'Setzt Bild, Name, Beschreibungen und Befehlsliste noch einmal.' : 'Noch nichts eingerichtet.'}
            group
          >
            <button type="button" className="btn" onClick={() => void applyProfile()}>Profil neu setzen</button>
          </SettingRow>
        </SettingsSection>
      )}
      <SettingsFooter state={info ? <Notice tone="ok">{info}</Notice> : error ? <Notice tone="error">{error}</Notice> : 'Änderungen gelten erst nach dem Speichern.'}>
        <button type="button" className="btn primary" disabled={!form} onClick={() => void save()}>Speichern</button>
      </SettingsFooter>
    </SettingsPanel>
  );
}
