import { SDK_VERSION, type ThemeDefinition } from '@nexus/theme-sdk';
import { BUILTIN_THEME_IDS } from '../api/types.js';
import { registerTheme } from './registry.js';
import { isSafeTheme } from './safeTheme.js';
import { satisfiesSdkRange } from './sdkRange.js';
import { THEME_CACHE_KEY } from './theme.js';
import { setThemeNotice } from './themeNoticeStore.js';

/** Ergebnis des Ladens der main.js des aktiven Themes */
export type ThemeLoadResult = {
  status: 'loaded' | 'skipped' | 'incompatible' | 'timeout' | 'failed' | 'invalid';
  themeId?: string;
  message?: string;
};

/** Auszug aus dem Manifest, den das Laden braucht */
export type ThemeModuleManifest = { id: string; sdkVersion: string; slots: string[] };

/** Austauschbare Abhängigkeiten (Tests geben eigene mit) */
export type ThemeModuleDeps = {
  importModule?: (url: string) => Promise<unknown>;
  fetchManifest?: (id: string) => Promise<ThemeModuleManifest>;
  timeoutMs?: number;
  sdkVersion?: string;
  search?: string;
  base?: string;
};

/** Erlaubte Form einer Theme-ID (wie im Manifest) */
const THEME_ID_PATTERN = /^[a-z0-9-]{2,40}$/;

/** Zeitlimit für Manifest und main.js zusammen */
const TIMEOUT_MS = 3000;

class TimeoutError extends Error {}

// Aktiver Modus aus dem localStorage oder undefined
function storedMode(): string | undefined {
  try {
    return localStorage.getItem(THEME_CACHE_KEY) ?? undefined;
  } catch {
    return undefined;
  }
}

// Hinweis setzen, warnen und Ergebnis liefern
function fail(status: ThemeLoadResult['status'], themeId: string, message: string): ThemeLoadResult {
  console.warn(message);
  setThemeNotice(message);
  return { status, themeId, message };
}

// Ist der Export eine gültige ThemeDefinition für dieses Theme?
function isDefinition(value: unknown, id: string): value is ThemeDefinition {
  if (typeof value !== 'object' || value === null) return false;
  const def = value as { id?: unknown; slots?: unknown };
  return def.id === id && typeof def.slots === 'object' && def.slots !== null;
}

// Lädt vor dem ersten Rendern die main.js des aktiven installierten Themes und meldet sie an
export async function loadActiveThemeModule(deps: ThemeModuleDeps = {}): Promise<ThemeLoadResult> {
  const base = deps.base ?? '';
  const search = deps.search ?? window.location.search;
  const sdkVersion = deps.sdkVersion ?? SDK_VERSION;
  const importModule = deps.importModule ?? ((url: string) => import(/* @vite-ignore */ url));
  const fetchManifest =
    deps.fetchManifest ??
    (async (id: string) => {
      const res = await fetch(`${base}/themes/${id}/manifest.json`);
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      // Liefert die Adresse etwas anderes als JSON (etwa die index.html, weil /themes nicht beim Server ankommt), klar benennen
      try {
        return (await res.json()) as ThemeModuleManifest;
      } catch {
        throw new Error(`keine JSON-Antwort (${res.headers.get('content-type') ?? 'ohne Content-Type'})`);
      }
    });

  const id = storedMode();
  if (!id || (BUILTIN_THEME_IDS as readonly string[]).includes(id) || !THEME_ID_PATTERN.test(id)) {
    return { status: 'skipped' };
  }
  if (isSafeTheme(search)) {
    setThemeNotice({ kind: 'info', message: `Abgesicherter Modus: Code und CSS des Themes „${id}“ sind ausgeschaltet.` });
    return { status: 'skipped', themeId: id };
  }

  // Nach dem Zeitlimit kommt nichts mehr an; ein spätes Ergebnis wird verworfen
  let timedOut = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const timeout = new Promise<never>((_, reject) => {
    timer = setTimeout(() => {
      timedOut = true;
      reject(new TimeoutError());
    }, deps.timeoutMs ?? TIMEOUT_MS);
  });

  try {
    let manifest: ThemeModuleManifest;
    try {
      manifest = await Promise.race([fetchManifest(id), timeout]);
    } catch (e) {
      if (e instanceof TimeoutError) throw e;
      const reason = e instanceof Error ? e.message : String(e);
      return fail('failed', id, `Theme „${id}“ konnte nicht geladen werden (Manifest nicht lesbar: ${reason}) – Standard wird gezeigt.`);
    }
    if (!Array.isArray(manifest.slots) || manifest.slots.length === 0) return { status: 'skipped', themeId: id };
    if (!satisfiesSdkRange(sdkVersion, manifest.sdkVersion ?? '')) {
      return fail(
        'incompatible',
        id,
        `Theme „${id}“ passt nicht zu dieser Nexus-Version (SDK ${sdkVersion}, verlangt ${manifest.sdkVersion}) – Standard wird gezeigt.`,
      );
    }
    let mod: unknown;
    try {
      mod = await Promise.race([importModule(`${base}/themes/${id}/main.js`), timeout]);
    } catch (e) {
      if (e instanceof TimeoutError) throw e;
      return fail('failed', id, `Theme „${id}“ konnte nicht geladen werden (${String(e)}) – Standard wird gezeigt.`);
    }
    if (timedOut) throw new TimeoutError();
    const def = (mod as { default?: unknown } | null)?.default;
    if (!isDefinition(def, manifest.id ?? id) || def.id !== id) {
      return fail('invalid', id, `Theme „${id}“ liefert keine gültige Theme-Definition – Standard wird gezeigt.`);
    }
    registerTheme(def);
    return { status: 'loaded', themeId: id };
  } catch (e) {
    if (e instanceof TimeoutError) {
      return fail('timeout', id, `Theme „${id}“ wurde nicht rechtzeitig geladen – Standard wird gezeigt.`);
    }
    return fail('failed', id, `Theme „${id}“ konnte nicht geladen werden (${String(e)}) – Standard wird gezeigt.`);
  } finally {
    clearTimeout(timer);
  }
}
