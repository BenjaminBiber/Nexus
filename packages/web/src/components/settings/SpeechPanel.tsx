import { useEffect, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { SpeechBackend, SpeechConfig, SpeechTestDto, TelegramConfig, TelegramConfigInput } from '../../api/telegramTypes.js';
import { useAsync } from '../../hooks/useAsync.js';
import { Notice } from './Notice.js';
import { SettingsFooter, SettingsPanel } from './SettingsPanel.js';
import { SettingRow, SettingsSection } from './SettingRow.js';

// Vorlagen für die bekannten Dienste mit der Schnittstelle von OpenAI. Die Adresse steht ohne
// `/v1` da: den Pfad `/v1/audio/transcriptions` hängt der Server selbst an.
interface Template {
  id: string;
  label: string;
  baseUrl: string;
  model: string;
  /** Was man am Dienst sonst noch wissen muss */
  hint?: string;
  /** true = ohne Schlüssel antwortet der Dienst nicht */
  needsKey?: boolean;
}

const TEMPLATES: Template[] = [
  { id: 'openai', label: 'OpenAI', baseUrl: 'https://api.openai.com', model: 'gpt-4o-mini-transcribe', needsKey: true },
  { id: 'speaches', label: 'Speaches', baseUrl: 'http://localhost:8000', model: 'Systran/faster-whisper-small' },
  {
    id: 'whisper-cpp',
    label: 'whisper.cpp-Server',
    baseUrl: 'http://localhost:8080',
    model: 'whisper-1',
    hint: 'Den Server mit --inference-path /v1/audio/transcriptions --convert starten; das Modell wählt er selbst.',
  },
  { id: 'localai', label: 'LocalAI', baseUrl: 'http://localhost:8080', model: 'whisper-1' },
];

// Adresse ohne Schrägstrich am Ende: so vergleicht auch der Server
const trimUrl = (url: string) => url.trim().replace(/\/+$/, '');

// Die Vorlage, zu der Adresse und Modell passen; "custom" = Eigene
function templateOf(speech: SpeechConfig): string {
  const found = TEMPLATES.find((t) => t.baseUrl === trimUrl(speech.baseUrl) && t.model === speech.model);
  return found?.id ?? 'custom';
}

// true = die Aufnahmen gehen zu OpenAI und damit vom eigenen Rechner weg
const goesToOpenAi = (baseUrl: string) => /(^|\/\/|\.)api\.openai\.com(\/|$|:)/.test(trimUrl(baseUrl));

// Bereich Spracherkennung: Backend, Vorlagen, Adresse, Schlüssel, Modell, Sprache und Testen.
export function SpeechPanel() {
  const api = useApi();
  const loaded = useAsync(() => api.getTelegramConfig(), [api]);
  const data = loaded.state.status === 'ready' ? loaded.state.data : null;
  const [form, setForm] = useState<TelegramConfig | null>(null);
  // Neuer Schlüssel im Klartext; null = der gespeicherte bleibt
  const [apiKey, setApiKey] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [info, setInfo] = useState<string | null>(null);
  const [test, setTest] = useState<SpeechTestDto | null>(null);
  const [testing, setTesting] = useState(false);
  // Zuletzt gewählte Vorlage; null = die Vorlage ergibt sich aus Adresse und Modell. Nötig,
  // weil zwei Vorlagen dieselbe Adresse und dasselbe Modell haben können.
  const [picked, setPicked] = useState<string | null>(null);

  useEffect(() => {
    if (data && !form) setForm(data);
  }, [data, form]);

  const speech = form?.speech ?? null;
  const change = (patch: Partial<SpeechConfig>) =>
    setForm((current) => (current ? { ...current, speech: { ...current.speech, ...patch } } : current));
  // Jede Aktion fängt mit leeren Meldungen an, damit nicht zwei Rückmeldungen nebeneinander stehen
  const start = () => {
    setError(null);
    setInfo(null);
    setTest(null);
  };

  // Füllt Adresse und Modell aus der Vorlage; der Schlüssel bleibt, wie er ist
  const applyTemplate = (id: string) => {
    const template = TEMPLATES.find((t) => t.id === id);
    setPicked(id);
    if (template) change({ baseUrl: template.baseUrl, model: template.model });
  };

  // Adresse oder Modell von Hand geändert: die Vorlage ergibt sich wieder aus den Feldern
  const edit = (patch: Partial<SpeechConfig>) => {
    setPicked(null);
    change(patch);
  };

  // Speichert die Einstellung. Der Schlüssel geht nur mit, wenn ein neuer eingetippt wurde –
  // sonst bliebe die Maske im Formular und der Server müsste sie erst wieder zurückrechnen.
  const save = async () => {
    if (!form) return;
    start();
    const { chats: _chats, profile, botToken: _masked, speech: current, ...rest } = form;
    const { apiKey: _key, ...withoutKey } = current;
    const input: TelegramConfigInput = {
      ...rest,
      profile: { brand: profile.brand },
      speech: apiKey !== null ? { ...withoutKey, apiKey } : withoutKey,
    };
    try {
      const saved = await api.saveTelegramConfig(input);
      loaded.set(saved);
      setForm(saved);
      setApiKey(null);
      setInfo('Gespeichert.');
    } catch (e) {
      setError((e as Error).message);
    }
  };

  // Erkennt das mitgelieferte Beispiel mit der gespeicherten Einstellung
  const runTest = async () => {
    start();
    setTesting(true);
    try {
      setTest(await api.testSpeech());
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setTesting(false);
    }
  };

  const template = picked ?? (speech ? templateOf(speech) : 'custom');
  const chosen = TEMPLATES.find((t) => t.id === template);
  const openAi = speech ? goesToOpenAi(speech.baseUrl) : false;
  const off = speech?.backend === 'off';

  return (
    <SettingsPanel label="Spracherkennung">
      {speech && (
        <SettingsSection title="Dienst" description="Sprachnachrichten werden über einen Dienst mit der Schnittstelle von OpenAI in Text verwandelt.">
          <SettingRow label="Erkennung" hint="Aus heißt: Sprachnachrichten werden nicht verarbeitet.">
            <select value={speech.backend} onChange={(e) => change({ backend: e.target.value as SpeechBackend })}>
              <option value="openai-compatible">OpenAI-kompatibler Dienst</option>
              <option value="off">Aus</option>
            </select>
          </SettingRow>
          <SettingRow
            label="Vorlage"
            hint="Füllt Adresse und Modell für einen bekannten Dienst aus."
            info="Die Vorlage setzt nur Adresse und Modell. Der Schlüssel bleibt stehen, und jedes Feld lässt sich danach noch ändern – dann steht hier „Eigene“."
          >
            <select value={template} onChange={(e) => applyTemplate(e.target.value)}>
              {TEMPLATES.map((t) => (
                <option key={t.id} value={t.id}>{t.label}</option>
              ))}
              <option value="custom">Eigene</option>
            </select>
          </SettingRow>
          {chosen?.hint && <Notice tone="info" role="note">{chosen.hint}</Notice>}
          <Notice tone={openAi ? 'warn' : 'ok'} role="note" label="Datenschutz">
            {openAi
              ? 'Die Aufnahmen verlassen den Rechner und gehen zu OpenAI.'
              : 'Die Aufnahmen bleiben im eigenen Netz; sie gehen nur an die eingetragene Adresse.'}
          </Notice>
        </SettingsSection>
      )}
      {speech && (
        <SettingsSection title="Zugang" description="Adresse, Schlüssel und Modell des Dienstes.">
          <SettingRow label="Adresse" hint="Ohne /v1 und ohne Schrägstrich am Ende; den Pfad hängt Nexus an." wide>
            <input className="mono" value={speech.baseUrl} onChange={(e) => edit({ baseUrl: e.target.value })} />
          </SettingRow>
          <SettingRow
            label="Schlüssel"
            hint={apiKey !== null
              ? 'Der neue Schlüssel wird beim Speichern übernommen.'
              : speech.apiKey
                ? 'Gespeichert; hier steht nur die Maske.'
                : chosen?.needsKey ? 'Dieser Dienst antwortet nur mit Schlüssel.' : 'Leer: lokale Dienste brauchen keinen.'}
            info="Der Schlüssel wird nur auf diesem Rechner gespeichert und in der Oberfläche nie im Klartext gezeigt."
            wide
            group
          >
            {apiKey !== null ? (
              <div className="actions">
                <input className="mono" type="password" autoComplete="off" aria-label="Neuer Schlüssel" value={apiKey} placeholder="sk-…" onChange={(e) => setApiKey(e.target.value)} />
                <button type="button" className="btn small" onClick={() => setApiKey(null)}>Abbrechen</button>
              </div>
            ) : (
              <div className="actions">
                <input className="mono" readOnly aria-label="Gespeicherter Schlüssel" value={speech.apiKey} placeholder="(keiner)" />
                <button type="button" className="btn small" onClick={() => setApiKey('')}>Ändern</button>
              </div>
            )}
          </SettingRow>
          <SettingRow label="Modell" wide>
            <input className="mono" value={speech.model} onChange={(e) => edit({ model: e.target.value })} />
          </SettingRow>
          <SettingRow label="Sprache" hint="Kürzel der Aufnahme, z. B. de; leer = der Dienst erkennt sie selbst.">
            <input className="short mono" value={speech.language ?? ''} placeholder="automatisch" onChange={(e) => change({ language: e.target.value.trim() === '' ? null : e.target.value })} />
          </SettingRow>
        </SettingsSection>
      )}
      {speech && (
        <SettingsSection title="Test" description="Nexus schickt ein mitgeliefertes Beispiel an den Dienst und zeigt, was zurückkommt.">
          <SettingRow
            label="Testen"
            hint={off ? 'Erst eine Erkennung wählen und speichern.' : 'Probiert die zuletzt gespeicherte Einstellung aus.'}
            group
          >
            <button type="button" className="btn" disabled={off || testing} onClick={() => void runTest()}>
              {testing ? 'Läuft …' : 'Testen'}
            </button>
          </SettingRow>
          {test && (
            <Notice tone="ok" role="note" label="Ergebnis des Tests">
              <p>„{test.text}“</p>
              <p className="muted">
                {test.backend === 'openai-compatible' ? 'OpenAI-kompatibler Dienst' : test.backend}
                {test.language ? `, Sprache ${test.language}` : ''}, {test.millis} ms
              </p>
            </Notice>
          )}
        </SettingsSection>
      )}
      <SettingsFooter state={info ? <Notice tone="ok">{info}</Notice> : error ? <Notice tone="error">{error}</Notice> : 'Änderungen gelten erst nach dem Speichern.'}>
        <button type="button" className="btn primary" disabled={!form} onClick={() => void save()}>Speichern</button>
      </SettingsFooter>
    </SettingsPanel>
  );
}
