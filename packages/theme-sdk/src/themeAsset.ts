import { useEffect, useState } from 'react';
import { useOptionalHost } from './hooks.js';

/** Optionen von `useThemeAsset` und `themeAssetUrl` */
export interface ThemeAssetOptions {
  /** Theme, dessen Domain-Freigabe und Cache gelten; Standard ist das aktive Theme des Hosts */
  themeId?: string;
  /** Erwartete sha256-Prüfsumme (hex) der Ressource; weicht sie ab, antwortet der Proxy mit 502 */
  sha256?: string;
  /** Pfad eines Eintrags, wenn die Ressource ein ZIP/JAR ist */
  entry?: string;
}

/** Fehler des Asset-Proxys: `code` aus dessen JSON-Antwort, `network` bei Netzfehler, sonst `http_<status>` */
export interface ThemeAssetError {
  /** HTTP-Status der Antwort; null bei Netzfehler */
  status: number | null;
  code: string;
  message: string;
}

/** Zustand einer Ressource: `url` ist nur bei `ready` gesetzt, `error` nur bei `error` */
export interface ThemeAssetState {
  status: 'idle' | 'loading' | 'ready' | 'error';
  url: string | null;
  error: ThemeAssetError | null;
}

const IDLE: ThemeAssetState = { status: 'idle', url: null, error: null };
const LOADING: ThemeAssetState = { status: 'loading', url: null, error: null };

// Adresse und Start-Token des Servers, die die Desktop-App ins Fenster legt (`window.__NEXUS__`).
// Ihre Oberfläche kommt aus den Dateien der App, hat also eine andere Herkunft als der Server.
function appServer(): { base: string; token: string } | undefined {
  const values = typeof window === 'undefined' ? undefined : (window as { __NEXUS__?: { base?: string; token?: string } }).__NEXUS__;
  return values?.base && values.token ? { base: values.base, token: values.token } : undefined;
}

/**
 * Lokale Proxy-URL einer fremden Ressource, z. B. für `url(...)` in CSS; prüft nichts.
 * In der Desktop-App zeigt sie auf den lokalen Server und trägt dessen Start-Token, damit auch
 * ein eigenes `fetch` des Themes durchkommt.
 */
export function themeAssetUrl(themeId: string, url: string, opts: Omit<ThemeAssetOptions, 'themeId'> = {}): string {
  const query = new URLSearchParams({ url });
  if (opts.sha256) query.set('sha256', opts.sha256);
  if (opts.entry) query.set('entry', opts.entry);
  const app = appServer();
  if (app) query.set('token', app.token);
  return `${app?.base ?? ''}/api/themes/${encodeURIComponent(themeId)}/fetch?${query}`;
}

// Fehler aus der Antwort des Proxys lesen (`{ code, message }`), sonst `http_<status>`
async function readError(res: Response): Promise<ThemeAssetError> {
  try {
    const body = (await res.json()) as { code?: unknown; message?: unknown };
    if (typeof body.code === 'string') return { status: res.status, code: body.code, message: typeof body.message === 'string' ? body.message : res.statusText };
  } catch {
    // kein JSON: Rückfall unten
  }
  return { status: res.status, code: `http_${res.status}`, message: res.statusText || `HTTP ${res.status}` };
}

/**
 * Lädt eine fremde Ressource über den Server-Proxy des aktiven Themes (Domain-Freigabe in
 * `permissions.network`) und liefert die lokale URL, sobald sie erreichbar ist
 */
export function useThemeAsset(url: string | null | undefined, opts: ThemeAssetOptions = {}): ThemeAssetState {
  const hostThemeId = useOptionalHost()?.colorScheme.themeId;
  const themeId = opts.themeId ?? hostThemeId;
  if (!themeId) throw new Error('useThemeAsset braucht den ThemeHostProvider des Nexus-Hosts oder opts.themeId – die Komponente läuft außerhalb der Nexus-Oberfläche.');

  const proxyUrl = url ? themeAssetUrl(themeId, url, opts) : null;
  // Ergebnis gehört zu genau einer Proxy-URL; nach einem Wechsel gilt wieder `loading`
  const [settled, setSettled] = useState<{ key: string; state: ThemeAssetState } | null>(null);

  useEffect(() => {
    if (!proxyUrl) return;
    const controller = new AbortController();
    const settle = (state: ThemeAssetState) => {
      if (!controller.signal.aborted) setSettled({ key: proxyUrl, state });
    };
    fetch(proxyUrl, { signal: controller.signal }).then(
      async (res) => {
        if (res.ok) {
          // Der Browser-Cache hält die Bytes; der Rumpf wird hier nicht gebraucht
          void res.body?.cancel().catch(() => {});
          settle({ status: 'ready', url: proxyUrl, error: null });
        } else {
          settle({ status: 'error', url: null, error: await readError(res) });
        }
      },
      (e: unknown) => settle({ status: 'error', url: null, error: { status: null, code: 'network', message: e instanceof Error ? e.message : 'Netzwerkfehler' } }),
    );
    return () => controller.abort();
  }, [proxyUrl]);

  if (!proxyUrl) return IDLE;
  return settled?.key === proxyUrl ? settled.state : LOADING;
}
