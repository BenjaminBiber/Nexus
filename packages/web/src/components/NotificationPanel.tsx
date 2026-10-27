import { useEffect, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { NotificationMode, UserSettingsDto } from '../api/types.js';
import { useAsync } from '../hooks/useAsync.js';
import { useLiveConnection } from '../live/useLive.js';
import { Notice } from './settings/Notice.js';
import { SettingsPanel, useSaveStatus } from './settings/SettingsPanel.js';
import { SettingRow, SettingsSection } from './settings/SettingRow.js';

// Auswahl der Art: Kennung, Beschriftung, Erklärung und Symbol (Pfad auf 24er-Raster)
const MODES: Array<{ mode: NotificationMode; label: string; hint: string; icon: string }> = [
  { mode: 'off', label: 'Aus', hint: 'Keine Meldung.', icon: 'M5 5l14 14M9.5 4.6A6 6 0 0 1 18 10v3l1.5 3H8M6 10v3l-1.5 3H15M10 19a2 2 0 0 0 4 0' },
  { mode: 'browser', label: 'Browser', hint: 'Einfache Meldung, nur bei geöffnetem Board.', icon: 'M3 5h18v14H3zM3 9h18M6 7h.01M9 7h.01' },
  { mode: 'windows', label: 'Windows', hint: 'Auch ohne geöffnetes Board; bei offenem Board zeigt es der Tab, ein Klick bleibt darin.', icon: 'M4 5.5l7-1v7H4zM13 4.3l7-1.3v8.5h-7zM4 13h7v7l-7-1zM13 13h7v8l-7-1.3z' },
];

// Art der Meldung bei bereiter oder wartender Session (Aus, Browser, Windows) samt Optionen der Windows-Meldung
// und zusätzlichem Push über ntfy.
export function NotificationPanel() {
  const api = useApi();
  const { reconnect } = useLiveConnection();
  const loaded = useAsync(() => api.getUserSettings(), [api]);
  const { state, run, setState } = useSaveStatus();
  const [test, setTest] = useState<{ ok: boolean; text: string } | null>(null);
  const [ntfyTest, setNtfyTest] = useState<{ ok: boolean; text: string } | null>(null);
  const settings = loaded.state.status === 'ready' ? loaded.state.data : null;
  const supported = typeof Notification !== 'undefined';

  // Speichert eine Änderung; für „Browser“ wird zuerst die Erlaubnis des Browsers eingeholt.
  // Für „Windows“ wird sie nur erfragt: mit Erlaubnis zeigt ein offener Tab die Meldung (Klick bleibt im Tab), sonst Windows.
  const save = async (patch: Partial<UserSettingsDto>) => {
    setTest(null);
    if (patch.notifications === 'browser' && supported && Notification.permission !== 'granted') {
      const answer = await Notification.requestPermission();
      if (answer !== 'granted') return setState({ kind: 'error', message: 'Der Browser hat Benachrichtigungen nicht erlaubt.' });
    }
    if (patch.notifications === 'windows' && supported && Notification.permission === 'default') {
      if ((await Notification.requestPermission()) === 'granted') reconnect();
    }
    const saved = await run(() => api.saveUserSettings(patch));
    if (saved) loaded.set(saved);
    return saved;
  };

  // Schickt die Beispielmeldung an das ntfy-Thema
  const sendNtfyTest = async () => {
    setNtfyTest(null);
    try {
      const result = await api.sendTestNtfy();
      setNtfyTest(result.ok ? { ok: true, text: 'Testmeldung an ntfy gesendet.' } : { ok: false, text: result.error ?? 'Die Testmeldung konnte nicht gesendet werden.' });
    } catch (e) {
      setNtfyTest({ ok: false, text: (e as Error).message });
    }
  };

  // Zeigt eine Beispielmeldung, damit man Darstellung und Schaltflächen prüfen kann
  const sendTest = async () => {
    setTest(null);
    try {
      const result = await api.sendTestNotification();
      setTest(result.ok ? { ok: true, text: 'Testmeldung gesendet. Bei offenem Board zeigt sie der Browser, sonst Windows – jeweils unten rechts.' } : { ok: false, text: result.error ?? 'Die Testmeldung konnte nicht gezeigt werden.' });
    } catch (e) {
      setTest({ ok: false, text: (e as Error).message });
    }
  };

  return (
    <SettingsPanel label="Benachrichtigungen" status={state}>
      <SettingsSection title="Art der Meldung" bare>
        <fieldset className="choice-group" disabled={!settings}>
          <legend className="sr-only">Art der Benachrichtigung</legend>
          {MODES.map(({ mode, label, hint, icon }) => {
            const unavailable = mode === 'browser' && !supported;
            return (
              <label key={mode} className={`choice${settings?.notifications === mode ? ' selected' : ''}${unavailable ? ' unavailable' : ''}`}>
                <input
                  type="radio"
                  name="notification-mode"
                  className="sr-only"
                  checked={settings?.notifications === mode}
                  disabled={unavailable}
                  onChange={() => void save({ notifications: mode })}
                />
                <svg className="choice-icon" viewBox="0 0 24 24" aria-hidden="true">
                  <path d={icon} />
                </svg>
                <span className="choice-label">{label}</span>
                <span className="choice-hint">{unavailable ? 'Dieser Browser unterstützt das nicht.' : hint}</span>
              </label>
            );
          })}
        </fieldset>
      </SettingsSection>
      {settings?.notifications === 'windows' && (
        <SettingsSection title="Windows-Meldung">
          <SettingRow label="Großes Bild bei „braucht deine Antwort“" hint="Macht wartende Sessions in der Meldung auffälliger.">
            <input type="checkbox" checked={settings.heroImages} onChange={(e) => void save({ heroImages: e.target.checked })} />
          </SettingRow>
          <SettingRow label="Testmeldung" hint="Zeigt eine Beispielmeldung, um Aussehen und Schaltflächen zu prüfen." group>
            <button type="button" className="btn small" onClick={() => void sendTest()}>
              Testmeldung senden
            </button>
          </SettingRow>
          {test && <Notice tone={test.ok ? 'ok' : 'error'}>{test.text}</Notice>}
        </SettingsSection>
      )}
      {settings && (
        <SettingsSection
          title="Push über ntfy"
          description="Schickt dieselben Meldungen zusätzlich aufs Handy, unabhängig von der Art oben."
          info="ntfy ist ein freier Push-Dienst. Installiere die ntfy-App, abonniere dort ein Thema und trage es hier ein. Wer den Namen eines Themas auf ntfy.sh kennt, kann mitlesen – nimm einen schwer zu erratenden Namen oder einen eigenen Server mit Zugriffstoken."
        >
          <SettingRow label="ntfy einschalten" hint="Bereite und wartende Sessions, neue Fragen und Dispatcher-Ereignisse.">
            <input type="checkbox" checked={settings.ntfy} onChange={(e) => void save({ ntfy: e.target.checked })} />
          </SettingRow>
          <SettingRow label="Server" hint="https://ntfy.sh oder die Adresse deines eigenen Servers.">
            <DraftInput value={settings.ntfyServer} placeholder="https://ntfy.sh" onCommit={(ntfyServer) => save({ ntfyServer })} />
          </SettingRow>
          <SettingRow label="Thema" hint="Buchstaben, Ziffern, - und _; dasselbe Thema abonnierst du in der App.">
            <DraftInput value={settings.ntfyTopic} placeholder="z. B. harness-7f3k2q" onCommit={(ntfyTopic) => save({ ntfyTopic })} />
          </SettingRow>
          <SettingRow label="Zugriffstoken" hint="Nur für geschützte Themen; leer lassen für ntfy.sh ohne Konto.">
            <DraftInput value={settings.ntfyToken} type="password" placeholder="tk_…" onCommit={(ntfyToken) => save({ ntfyToken })} />
          </SettingRow>
          <SettingRow label="Testmeldung" hint="Schickt eine Beispielmeldung an das Thema." group>
            <button type="button" className="btn small" disabled={!settings.ntfyTopic} onClick={() => void sendNtfyTest()}>
              An ntfy senden
            </button>
          </SettingRow>
          {ntfyTest && <Notice tone={ntfyTest.ok ? 'ok' : 'error'}>{ntfyTest.text}</Notice>}
        </SettingsSection>
      )}
    </SettingsPanel>
  );
}

// Textfeld, das erst beim Verlassen (oder Enter) speichert und bei Fehlern den gespeicherten Wert zurückholt
function DraftInput({ value, type = 'text', placeholder, onCommit }: { value: string; type?: string; placeholder?: string; onCommit: (value: string) => Promise<unknown> }) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const commit = () => {
    if (draft.trim() !== value) void onCommit(draft.trim()).then((saved) => saved || setDraft(value));
  };
  return (
    <input
      type={type}
      value={draft}
      placeholder={placeholder}
      autoComplete="off"
      spellCheck={false}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === 'Enter' && e.currentTarget.blur()}
    />
  );
}
