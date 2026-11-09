// Sound-Auflösung: Versions-JSON → Asset-Index → resources.download.minecraft.net, alles über den Asset-Proxy (fetch gemockt)
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { themeAssetUrl } from '@nexus/theme-sdk';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { MINECRAFT_SOUNDS, RESOURCES_URL, resolveSoundUrls } from '../src/sounds.js';
import { MINECRAFT_SETTINGS } from '../src/settings.js';
import theme from '../src/main.js';

const INDEX = 'https://piston-meta.mojang.com/v1/packages/idx/19.json';
const CLICK = '0123456789abcdef0123456789abcdef01234567';
const POP = 'abcdef0123456789abcdef0123456789abcdef01';

const versionJson = { assetIndex: { url: INDEX } };

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe('resolveSoundUrls', () => {
  it('liefert je vorhandenem Ton die Proxy-URL des Objekts, fehlende Einträge fehlen', async () => {
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      expect(String(input)).toBe(themeAssetUrl('minecraft', INDEX));
      return new Response(
        JSON.stringify({ objects: { [MINECRAFT_SOUNDS.click]: { hash: CLICK }, [MINECRAFT_SOUNDS.hotbar]: { hash: POP } } }),
      );
    });
    vi.stubGlobal('fetch', fetchMock);
    const urls = await resolveSoundUrls('minecraft', versionJson);
    expect(urls).toEqual({
      click: themeAssetUrl('minecraft', `${RESOURCES_URL}/01/${CLICK}`),
      hotbar: themeAssetUrl('minecraft', `${RESOURCES_URL}/ab/${POP}`),
    });
    expect(fetchMock).toHaveBeenCalledTimes(1);
  });

  it('Index nicht ladbar oder fehlend → leeres Ergebnis ohne Fehler', async () => {
    vi.stubGlobal('fetch', vi.fn(async () => new Response('nein', { status: 502 })));
    expect(await resolveSoundUrls('minecraft', versionJson)).toEqual({});
    vi.stubGlobal('fetch', vi.fn(async () => Promise.reject(new Error('offline'))));
    expect(await resolveSoundUrls('minecraft', versionJson)).toEqual({});
    expect(await resolveSoundUrls('minecraft', {})).toEqual({});
  });
});

describe('Einstellung „Sounds“', () => {
  it('Manifest und ThemeDefinition deklarieren dasselbe Schema, Schalter standardmäßig an', () => {
    const manifest = JSON.parse(readFileSync(join(import.meta.dirname, '..', 'manifest.json'), 'utf8')) as {
      settingsSchema: unknown;
      permissions: { network: string[] };
    };
    expect(manifest.settingsSchema).toEqual(MINECRAFT_SETTINGS);
    expect(theme.settingsSchema).toEqual(MINECRAFT_SETTINGS);
    expect(MINECRAFT_SETTINGS.sounds).toMatchObject({ type: 'boolean', label: 'Sounds', default: true });
    expect(manifest.permissions.network).toContain('resources.download.minecraft.net');
  });
});
