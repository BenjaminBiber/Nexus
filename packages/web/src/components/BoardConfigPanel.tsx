import { useEffect, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { BoardConfigInput } from '../api/types.js';
import { useAsync } from '../hooks/useAsync.js';
import { Notice } from './settings/Notice.js';
import { SettingsFooter, SettingsPanel } from './settings/SettingsPanel.js';
import { SettingRow, SettingsSection } from './settings/SettingRow.js';

type Form = Omit<BoardConfigInput, 'password'>;

// Verbindung zur gemeinsamen Board-Datenbank und eigene Benutzer-ID (Datei ~/.harness/board.yaml).
export function BoardConfigPanel() {
  const api = useApi();
  const loaded = useAsync(() => api.getBoardConfig(), [api]);
  const data = loaded.state.status === 'ready' ? loaded.state.data : null;
  const [form, setForm] = useState<Form | null>(null);
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);

  useEffect(() => {
    if (data && !form) {
      const { provider, dbFile, server, port, database, user, encrypt, trustServerCertificate, userId, userName } = data;
      setForm({ provider, dbFile, server, port, database, user, encrypt, trustServerCertificate, userId, userName });
    }
  }, [data, form]);

  // Eingabe für Speichern und Test; das Passwort nur, wenn eins eingetippt wurde
  const input = (): BoardConfigInput => ({ ...form!, ...(password ? { password } : {}) });
  const change = (patch: Partial<Form>) => setForm((current) => (current ? { ...current, ...patch } : current));
  // Welche Feldgruppe gezeigt wird; ausgeblendete Werte bleiben im Formular erhalten
  const sqlite = form?.provider === 'sqlite';

  // Speichert die Konfig
  const save = async () => {
    setError(null);
    setInfo(null);
    try {
      const saved = await api.saveBoardConfig(input());
      loaded.set(saved);
      setPassword('');
      setInfo(`Gespeichert in ${saved.file}`);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  // Prüft die Verbindung mit den aktuellen Eingaben
  const test = async () => {
    setError(null);
    setInfo(null);
    try {
      const result = await api.testBoardConfig(input());
      if (result.ok) setInfo(result.message);
      else setError(result.message);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <SettingsPanel label="Board-Datenbank">
      {form && (
        <SettingsSection title="Verbindung" description={`Gespeichert nur auf diesem Rechner${data ? ` (${data.file})` : ''}.`}>
          <SettingRow label="Anbieter" hint={sqlite ? 'Lokale SQLite-Datei für die Boards dieses Rechners.' : 'Gemeinsamer SQL Server für alle Boards des Teams.'} group>
            <div className="provider-switch" role="group" aria-label="Datenbank-Anbieter">
              <button type="button" className={`btn small${!sqlite ? ' primary' : ''}`} aria-pressed={!sqlite} onClick={() => change({ provider: 'mssql' })}>SQL Server</button>
              <button type="button" className={`btn small${sqlite ? ' primary' : ''}`} aria-pressed={sqlite} onClick={() => change({ provider: 'sqlite' })}>SQLite (lokal)</button>
            </div>
          </SettingRow>
          {sqlite ? (
            <>
              <Notice tone="warn" role="note">Nur dieser Rechner – nicht für das Team, nicht auf Netzlaufwerk.</Notice>
              <SettingRow label="SQLite-Datei" wide><input value={form.dbFile} onChange={(e) => change({ dbFile: e.target.value })} /></SettingRow>
            </>
          ) : (
            <>
              <SettingRow label="Server"><input value={form.server} onChange={(e) => change({ server: e.target.value })} /></SettingRow>
              <SettingRow label="Port"><input className="short" type="number" value={form.port} onChange={(e) => change({ port: Number(e.target.value) })} /></SettingRow>
              <SettingRow label="Datenbank"><input value={form.database} onChange={(e) => change({ database: e.target.value })} /></SettingRow>
              <SettingRow label="Benutzer"><input value={form.user} onChange={(e) => change({ user: e.target.value })} /></SettingRow>
              <SettingRow label="Passwort" hint={data?.hasPassword ? 'Leer lassen, um das gespeicherte zu behalten.' : undefined}>
                <input type="password" value={password} placeholder={data?.hasPassword ? '(gespeichert)' : ''} onChange={(e) => setPassword(e.target.value)} />
              </SettingRow>
              <SettingRow label="Verschlüsselt verbinden" info="Verschlüsselt die Verbindung zum SQL Server (TLS).">
                <input type="checkbox" checked={form.encrypt} onChange={(e) => change({ encrypt: e.target.checked })} />
              </SettingRow>
              <SettingRow label="Serverzertifikat vertrauen" info="Akzeptiert auch ein Zertifikat, das keine bekannte Stelle ausgestellt hat (z. B. selbst signiert im Firmennetz). Nur für Server einschalten, denen du vertraust.">
                <input type="checkbox" checked={form.trustServerCertificate} onChange={(e) => change({ trustServerCertificate: e.target.checked })} />
              </SettingRow>
            </>
          )}
        </SettingsSection>
      )}
      {form && (
        <SettingsSection title="Du auf dem Board" description="So erscheinst du an Tickets und Kommentaren.">
          <SettingRow label="Dein Name"><input value={form.userName} onChange={(e) => change({ userName: e.target.value })} /></SettingRow>
          <SettingRow label="Deine Benutzer-ID (GUID)" info="Deine Kennung in der Board-Datenbank; deine Boards gehören dieser ID. Auf jedem Rechner dieselbe ID eintragen.">
            <input className="mono" value={form.userId} onChange={(e) => change({ userId: e.target.value })} />
          </SettingRow>
        </SettingsSection>
      )}
      <SettingsFooter state={info ? <Notice tone="ok">{info}</Notice> : error ? <Notice tone="error">{error}</Notice> : 'Mit „Verbindung testen“ prüfen, dann speichern.'}>
        <button type="button" className="btn" disabled={!form} onClick={() => void test()}>Verbindung testen</button>
        <button type="button" className="btn primary" disabled={!form} onClick={() => void save()}>Speichern</button>
      </SettingsFooter>
    </SettingsPanel>
  );
}
