import { useEffect, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { SpeechConfig, SpeechStatusDto, SpeechTestDto, TelegramConfig, TelegramConfigInput } from '../../api/telegramTypes.js';
import { useAsync } from '../../hooks/useAsync.js';
import { CardSection, PowerToggle, SaveBar, sinceText, StatusBadge, type Tone } from './CardSettings.js';
import { Notice } from './Notice.js';
import { SettingsPanel } from './SettingsPanel.js';

// Vorlagen für die bekannten Dienste mit der Schnittstelle von OpenAI. Die Adresse steht ohne
// `/v1` da: den Pfad `/v1/audio/transcriptions` hängt der Server selbst an.
interface Template {
  id: string;
  label: string;
  baseUrl: string;
  model: string;
  /** Kürzel auf der Ersatzkachel, solange kein Logo unter /speech-logos/<id>.svg liegt */
  short: string;
  /** Was man am Dienst sonst noch wissen muss */
  hint?: string;
  /** true = ohne Schlüssel antwortet der Dienst nicht */
  needsKey?: boolean;
}

const TEMPLATES: Template[] = [
  { id: 'openai', label: 'OpenAI', short: 'AI', baseUrl: 'https://api.openai.com', model: 'gpt-4o-mini-transcribe', needsKey: true },
  { id: 'speaches', label: 'Speaches', short: 'Sp', baseUrl: 'http://localhost:8000', model: 'Systran/faster-whisper-small' },
  {
    id: 'whisper-cpp',
    label: 'whisper.cpp',
    short: 'wc',
    baseUrl: 'http://localhost:8080',
    model: 'whisper-1',
    hint: 'Den Server mit --inference-path /v1/audio/transcriptions --convert starten; das Modell wählt er selbst.',
  },
  { id: 'localai', label: 'LocalAI', short: 'LA', baseUrl: 'http://localhost:8080', model: 'whisper-1' },
];

// Wie oft der Stand (letzte Erkennung) neu geholt wird; „vor 5 min“ zählt dabei mit
const STATUS_POLL_MS = 30_000;

// Adresse ohne Schrägstrich am Ende: so vergleicht auch der Server
const trimUrl = (url: string) => url.trim().replace(/\/+$/, '');

// Die Vorlage, zu der Adresse und Modell passen; "custom" = Eigene
function templateOf(speech: SpeechConfig): string {
  const found = TEMPLATES.find((t) => t.baseUrl === trimUrl(speech.baseUrl) && t.model === speech.model);
  return found?.id ?? 'custom';
}

// true = die Aufnahmen gehen zu OpenAI und damit vom eigenen Rechner weg
const goesToOpenAi = (baseUrl: string) => /(^|\/\/|\.)api\.openai\.com(\/|$|:)/.test(trimUrl(baseUrl));

// Zahl der Felder, in denen das Formular vom gespeicherten Stand abweicht (der Schlüssel zählt mit)
function countChanges(form: SpeechConfig, saved: SpeechConfig, apiKey: string | null): number {
  let n = 0;
  if (form.backend !== saved.backend) n++;
  if (form.baseUrl !== saved.baseUrl) n++;
  if (form.model !== saved.model) n++;
  if ((form.language ?? '') !== (saved.language ?? '')) n++;
  if (apiKey) n++;
  return n;
}

// Stand für das Etikett im Kopf: aus, Fehler (neuer als der letzte Erfolg), funktioniert oder noch nie benutzt
function badgeOf(backend: SpeechConfig['backend'], status: SpeechStatusDto | null): { tone: Tone; text: string } {
  if (backend === 'off') return { tone: 'info', text: 'Aus' };
  const ok = status?.lastSuccessAt ? new Date(status.lastSuccessAt).getTime() : 0;
  const failed = status?.lastErrorAt ? new Date(status.lastErrorAt).getTime() : 0;
  if (failed > ok) return { tone: 'error', text: 'Fehler' };
  if (ok > 0) return { tone: 'ok', text: 'Funktioniert' };
  return { tone: 'info', text: 'Noch nicht genutzt' };
}

// Bereich Spracherkennung: Stand mit Dienst und letzter Erkennung oben, darunter Dienst und Zugang.
export function SpeechPanel() {
  const api = useApi();
  const loaded = useAsync(() => api.getTelegramConfig(), [api]);
  const statusLoad = useAsync(() => api.getSpeechStatus(), [api]);
  const data = loaded.state.status === 'ready' ? loaded.state.data : null;
  const status = statusLoad.state.status === 'ready' ? statusLoad.state.data : null;
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
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    if (data && !form) setForm(data);
  }, [data, form]);

  // Stand regelmäßig nachladen, damit eine Sprachnachricht im Bot hier auftaucht
  useEffect(() => {
    const timer = window.setInterval(() => {
      setNow(Date.now());
      void statusLoad.refresh();
    }, STATUS_POLL_MS);
    return () => window.clearInterval(timer);
    // statusLoad ist je Panel ein festes Objekt
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const change = (patch: Partial<SpeechConfig>) => {
    setInfo(null);
    setForm((current) => (current ? { ...current, speech: { ...current.speech, ...patch } } : current));
  };
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
    const { chats: _chats, profile: _profile, botToken: _masked, speech: current, ...rest } = form;
    const { apiKey: _key, ...withoutKey } = current;
    const input: TelegramConfigInput = {
      ...rest,
      speech: apiKey ? { ...withoutKey, apiKey } : withoutKey,
    };
    try {
      const saved = await api.saveTelegramConfig(input);
      loaded.set(saved);
      setForm(saved);
      setApiKey(null);
      setPicked(null);
      setInfo('Gespeichert.');
    } catch (e) {
      setError((e as Error).message);
    }
  };

  // Verwirft alle ungespeicherten Änderungen und kehrt zum gespeicherten Stand zurück
  const discard = () => {
    if (!data) return;
    start();
    setForm(data);
    setApiKey(null);
    setPicked(null);
  };

  // Erkennt das mitgelieferte Beispiel mit der gespeicherten Einstellung; der Stand zählt den Versuch mit
  const runTest = async () => {
    start();
    setTesting(true);
    try {
      setTest(await api.testSpeech());
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setTesting(false);
      setNow(Date.now());
      void statusLoad.refresh();
    }
  };

  if (!form || !data) return <SettingsPanel label="Spracherkennung" className="telegram-panel">{null}</SettingsPanel>;

  const speech = form.speech;
  const template = picked ?? templateOf(speech);
  const chosen = TEMPLATES.find((t) => t.id === template);
  const openAi = goesToOpenAi(speech.baseUrl);
  const on = speech.backend !== 'off';
  const changes = countChanges(speech, data.speech, apiKey);
  // Der Kopf zeigt den laufenden, also gespeicherten Dienst – nicht das, was gerade im Formular steht
  const live = data.speech;
  const liveTemplate = TEMPLATES.find((t) => t.id === templateOf(live));
  const liveOn = live.backend !== 'off';
  const badge = badgeOf(live.backend, status);
  const failedLast = badge.tone === 'error' && status?.lastError;

  const last = status?.lastSuccessAt
    ? `Zuletzt erkannt ${sinceText(status.lastSuccessAt, now)} (${status.lastSuccessVia === 'voice' ? 'Sprachnachricht' : 'Test'}${status.lastMillis !== null ? `, ${(status.lastMillis / 1000).toLocaleString('de-DE', { maximumFractionDigits: 1 })} s` : ''})`
    : 'Seit dem Start noch nichts erkannt';
  const summary = liveOn
    ? [last, goesToOpenAi(live.baseUrl) ? 'Aufnahmen gehen zu OpenAI' : 'Aufnahmen bleiben im eigenen Netz', live.language ? `Sprache ${live.language}` : 'Sprache automatisch'].join(' · ')
    : 'Sprachnachrichten an den Bot werden nicht verarbeitet.';

  return (
    <SettingsPanel label="Spracherkennung" className="telegram-panel">
      <section className="tg-hero" aria-label="Stand der Spracherkennung" role="note">
        {liveOn ? <ServiceLogo template={liveTemplate} size={56} /> : <span className="speech-logo off" aria-hidden="true"><span className="material-icons">mic_off</span></span>}
        <div className="tg-hero-text">
          <div className="tg-hero-title">
            <strong>{liveOn ? liveTemplate?.label ?? 'Eigener Dienst' : 'Spracherkennung'}</strong>
            {liveOn && live.model && <span className="mono">{live.model}</span>}
            <StatusBadge tone={badge.tone}>{badge.text}</StatusBadge>
          </div>
          <span className="tg-hero-summary">{summary}</span>
        </div>
        <div className="tg-hero-actions">
          <button
            type="button"
            className="btn"
            disabled={!liveOn || testing}
            title={!liveOn ? 'Erst eine Erkennung einschalten und speichern.' : 'Probiert die zuletzt gespeicherte Einstellung aus.'}
            onClick={() => void runTest()}
          >
            {testing ? 'Läuft …' : 'Testen'}
          </button>
          <PowerToggle label="Erkennung aktiv" on={on} onChange={(next) => change({ backend: next ? 'openai-compatible' : 'off' })} />
        </div>
      </section>

      {failedLast && status?.lastErrorAt && (
        <Notice tone="error" role="note" label="Letzter Fehler">
          Letzter Versuch {sinceText(status.lastErrorAt, now)} gescheitert: {status.lastError}
        </Notice>
      )}
      {test && (
        <Notice tone="ok" role="note" label="Ergebnis des Tests">
          <p>„{test.text}“</p>
          <p className="muted">
            {test.language ? `Sprache ${test.language}, ` : ''}{test.millis} ms
          </p>
        </Notice>
      )}

      <CardSection title="Dienst" description="Sprachnachrichten werden über einen Dienst mit der Schnittstelle von OpenAI in Text verwandelt.">
        <fieldset className="speech-services" disabled={!on} aria-label="Vorlage">
            <div className="speech-service-grid">
              {[...TEMPLATES, null].map((t) => {
                const id = t?.id ?? 'custom';
                return (
                  <label key={id} className={`speech-service${template === id ? ' on' : ''}`}>
                    <input type="radio" name="speech-template" value={id} checked={template === id} onChange={() => (t ? applyTemplate(id) : setPicked('custom'))} />
                    <ServiceLogo template={t ?? undefined} size={32} />
                    <span>{t?.label ?? 'Eigene'}</span>
                  </label>
                );
              })}
            </div>
        </fieldset>
        {chosen?.hint && on && <div className="speech-note"><span className="material-icons" aria-hidden="true">info</span>{chosen.hint}</div>}
        {/* Nur der Fall, dass die Aufnahmen den Rechner verlassen, braucht einen Hinweis */}
        {openAi && (
          <div className="speech-note warn" role="note" aria-label="Datenschutz">
            <span className="material-icons" aria-hidden="true">cloud_upload</span>
            Die Aufnahmen verlassen den Rechner und gehen zu OpenAI.
          </div>
        )}
      </CardSection>

      <CardSection title="Zugang" description="Adresse, Schlüssel und Modell des Dienstes.">
        <div className="tg-card">
          <div className="tg-field-row">
            <span className="tg-row-text">
              <label className="tg-row-label" htmlFor="speech-url">Adresse</label>
              <span className="tg-row-hint">Ohne /v1 und ohne Schrägstrich am Ende; den Pfad hängt Nexus an.</span>
            </span>
            <input id="speech-url" className="mono speech-wide" value={speech.baseUrl} onChange={(e) => edit({ baseUrl: e.target.value })} />
          </div>
          <div className="tg-field-row">
            <span className="tg-row-text">
              <span className="tg-row-label">Schlüssel</span>
              <span className="tg-row-hint">
                {apiKey !== null
                  ? 'Der neue Schlüssel wird beim Speichern übernommen.'
                  : speech.apiKey
                    ? 'Gespeichert, nur maskiert sichtbar'
                    : chosen?.needsKey ? 'Dieser Dienst antwortet nur mit Schlüssel.' : 'Leer: lokale Dienste brauchen keinen.'}
              </span>
            </span>
            {apiKey !== null ? (
              <>
                <input className="mono tg-token-input" type="password" autoComplete="off" aria-label="Neuer Schlüssel" value={apiKey} placeholder="sk-…" onChange={(e) => setApiKey(e.target.value)} />
                <button type="button" className="btn" onClick={() => setApiKey(null)}>Abbrechen</button>
              </>
            ) : (
              <>
                <span className="mono tg-token" aria-label="Gespeicherter Schlüssel">{speech.apiKey || '(keiner)'}</span>
                <button type="button" className="btn" onClick={() => setApiKey('')}>{speech.apiKey ? 'Ändern' : 'Eintragen'}</button>
              </>
            )}
          </div>
          <div className="tg-field-row">
            <span className="tg-row-text">
              <label className="tg-row-label" htmlFor="speech-model">Modell</label>
              <span className="tg-row-hint">Name des Modells beim Dienst</span>
            </span>
            <input id="speech-model" className="mono speech-wide" value={speech.model} onChange={(e) => edit({ model: e.target.value })} />
          </div>
          <div className="tg-field-row">
            <span className="tg-row-text">
              <label className="tg-row-label" htmlFor="speech-language">Sprache</label>
              <span className="tg-row-hint">Kürzel der Aufnahme, z. B. de; leer = der Dienst erkennt sie selbst</span>
            </span>
            <input id="speech-language" className="mono short" value={speech.language ?? ''} placeholder="auto" onChange={(e) => change({ language: e.target.value.trim() === '' ? null : e.target.value })} />
          </div>
        </div>
      </CardSection>

      <SaveBar changes={changes} error={error} info={info} onDiscard={discard} onSave={() => void save()} />
    </SettingsPanel>
  );
}

// Logo des Dienstes aus /speech-logos/<id>.svg; fehlt die Datei, eine Kachel mit Kürzel (Eigene: Wellen-Symbol)
function ServiceLogo({ template, size }: { template?: Template; size: number }) {
  const [missing, setMissing] = useState(false);
  useEffect(() => setMissing(false), [template?.id]);
  const style = { width: size, height: size };
  if (!template) {
    return <span className={`speech-logo custom${size < 40 ? ' small' : ''}`} style={style} aria-hidden="true"><span className="material-icons">graphic_eq</span></span>;
  }
  if (missing) {
    return <span className={`speech-logo tile-${template.id}${size < 40 ? ' small' : ''}`} style={style} aria-hidden="true">{template.short}</span>;
  }
  return (
    <img
      className={`speech-logo img${size < 40 ? ' small' : ''}`}
      src={`/speech-logos/${template.id}.svg`}
      alt=""
      style={style}
      width={size}
      height={size}
      onError={() => setMissing(true)}
    />
  );
}
