// Prüft jedes Claude-Theme unter themes/<id>/claude/ (Manifest mit "claudeTheme": true): Aufbau,
// Kit-Kopien, Farben jeder Variante, Szenen und den Ablauf von register mit einem gefälschten $.
import { existsSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

import { describe, expect, it, vi } from 'vitest';

// @ts-expect-error reines JavaScript ohne Typen
import { claudeThemes, defaultsOf, expectedFiles } from '../../../scripts/themes/sync-claude-kit.mjs';
import { checkColors, makeColors } from '../make-colors.js';
import type { NexusLook, NexusSettings } from '../nexus-look';

const ROOT = resolve(import.meta.dirname, '../../..');
/** Höchstlänge einer Szene (SvgProps.source) */
const MAX_SCENE = 131072;

type Spec = { type: 'boolean'; default: boolean } | { type: 'select'; default: string; options: { value: string }[] };
type Manifest = { id: string; colorScheme: 'light' | 'dark'; settingsSchema?: Record<string, Spec> };
type LookModule = { CLAUDE_SETTINGS: readonly string[]; lookFor: (settings: NexusSettings) => NexusLook };

const themes: { dir: string; manifest: Manifest }[] = claudeThemes(ROOT);

/** Alle Kombinationen der Schlüssel, die das Claude-Theme liest; übrige Einstellungen Standard */
function combinations(manifest: Manifest, keys: readonly string[]): NexusSettings[] {
  const schema = manifest.settingsSchema ?? {};
  let combos: Record<string, string | boolean>[] = [defaultsOf(schema)];
  for (const key of keys) {
    const spec = schema[key];
    if (!spec) throw new Error(`CLAUDE_SETTINGS nennt ${key}, das settingsSchema nicht`);
    const values = spec.type === 'boolean' ? [false, true] : spec.options.map((option) => option.value);
    combos = combos.flatMap((combo) => values.map((value) => ({ ...combo, [key]: value })));
  }
  return combos;
}

function label(settings: NexusSettings): string {
  return Object.entries(settings).map(([key, value]) => `${key}=${value}`).join(' ') || 'Standard';
}

async function importHooks<T>(dir: string, file: string): Promise<T> {
  return (await import(pathToFileURL(join(dir, 'claude', 'hooks', file)).href)) as T;
}

it('mindestens ein Theme bringt ein Claude-Theme mit', () => {
  expect(themes.length).toBeGreaterThan(0);
});

describe.each(themes.map((theme) => [theme.manifest.id, theme] as const))('Claude-Theme %s', (_id, { dir, manifest }) => {
  it('hat hooks.json mit register.tsx als einzigem Modul', () => {
    const hooks = JSON.parse(readFileSync(join(dir, 'claude', 'hooks', 'hooks.json'), 'utf8'));
    expect(hooks).toEqual({ modules: ['./register.tsx'] });
    expect(existsSync(join(dir, 'claude', 'hooks', 'register.tsx'))).toBe(true);
    expect(existsSync(join(dir, 'claude', 'hooks', 'look.ts'))).toBe(true);
  });

  it('trägt das Kit und die Standardwerte unverändert (npm-Skript: node scripts/themes/sync-claude-kit.mjs)', () => {
    for (const [name, content] of expectedFiles(ROOT, manifest) as Map<string, string>) {
      expect(readFileSync(join(dir, 'claude', 'hooks', name), 'utf8'), name).toBe(content);
    }
  });

  it('jede Variante malt in Claude Desktop, nur im Farbschema des Themes', async () => {
    const { CLAUDE_SETTINGS, lookFor } = await importHooks<LookModule>(dir, 'look.ts');
    for (const settings of combinations(manifest, CLAUDE_SETTINGS)) {
      const look = lookFor(settings);
      expect(look.colors.only, label(settings)).toBe(manifest.colorScheme);
      const { theme, problems } = makeColors(look.colors);
      expect(problems.filter((line) => line.startsWith("Desktop won't paint")), label(settings)).toEqual([]);
      expect(checkColors(theme).filter((line) => line.startsWith("Desktop won't paint")), label(settings)).toEqual([]);
    }
  });

  it('jede Szene ist ein eigenständiges SVG ohne Skript und fremde Adressen', async () => {
    const { CLAUDE_SETTINGS, lookFor } = await importHooks<LookModule>(dir, 'look.ts');
    for (const settings of combinations(manifest, CLAUDE_SETTINGS)) {
      const scene = lookFor(settings).scene;
      if (!scene) continue;
      const where = label(settings);
      expect(scene.svg.length, where).toBeLessThanOrEqual(MAX_SCENE);
      expect(scene.svg.trimStart().startsWith('<svg'), where).toBe(true);
      expect(scene.svg, where).toMatch(/viewBox="/);
      expect(scene.svg, where).toMatch(/preserveAspectRatio="xMidYMid slice"/);
      expect(scene.svg, where).not.toMatch(/<script|on[a-z]+="|<foreignObject|<image/i);
      expect(scene.svg.replaceAll('http://www.w3.org/', ''), where).not.toMatch(/https?:|\/\//);
      expect(scene.alt.trim().length, where).toBeGreaterThan(0);
      if (scene.glass?.tint !== undefined) expect(scene.glass.tint, where).toMatch(/^(#[0-9a-f]{6}|rgba?\()/i);
    }
  });

  it('register setzt den Look beim Sitzungsstart und zeichnet die Szene nur auf Desktop und Mobile', async () => {
    const { lookFor } = await importHooks<LookModule>(dir, 'look.ts');
    const look = lookFor(defaultsOf(manifest.settingsSchema));
    const { register } = await importHooks<{ register: (on: unknown) => void }>(dir, 'register.tsx');

    const hooks: Record<string, (...args: unknown[]) => unknown> = {};
    register((event: string, matcherOrHook: unknown, hook?: unknown) => {
      hooks[event] = (hook ?? matcherOrHook) as (...args: unknown[]) => unknown;
    });

    const set = vi.fn(async () => ({ isDrawn: true }));
    const next = vi.fn(async () => ({}));
    await hooks['session.start']({ theme: { set } }, { cwd: '/' }, next);
    expect(set).toHaveBeenCalledTimes(1);
    const args = (set.mock.calls[0] as unknown[])[0] as { colors: unknown; backdrop?: { scene: string } };
    expect(args.colors).toEqual(makeColors(look.colors).theme);
    expect(next).toHaveBeenCalled();

    if (!look.scene) {
      expect(args.backdrop).toBeUndefined();
      expect(hooks['ui.render']).toBeUndefined();
      return;
    }
    expect(args.backdrop?.scene).toBe('nexus');
    const Svg = () => null;
    const $ = { ui: { resolve: () => ({ Svg }) } };
    const drawn = hooks['ui.render']($, { surface: 'desktop', props: { scene: 'nexus' } }, next) as {
      props: { source: string; alt: string };
    };
    expect(drawn.props.source).toBe(look.scene.svg);
    const passed = vi.fn(() => 'engine');
    expect(hooks['ui.render']($, { surface: 'terminal', props: { scene: 'nexus' } }, passed)).toBe('engine');
    expect(hooks['ui.render']($, { surface: 'desktop', props: { scene: 'fremd' } }, passed)).toBe('engine');
  });
});
