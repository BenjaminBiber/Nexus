// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiClient } from '../src/api/ApiClient.js';
import type { DispatchRunLogDto, DispatchRunShotDto } from '../src/api/dispatchTypes.js';
import type { RunItem, ToolEntry } from '../src/dbboard/runItems.js';
import { RunThumb } from '../src/components/db/run/RunThumb.js';
import { fakeApi } from './helpers/fakeApi.js';

const NAME = 'ARC-0001-20261016-101500.log';
const SHOT: DispatchRunShotDto = { file: '001.png', toolUseId: 'toolu_1', at: '2026-10-16T08:16:00.000Z', mediaType: 'image/png', bytes: 12 };

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

// Ersetzt fetch und merkt sich Methode und Adresse jedes Aufrufs
function stubFetch(body: unknown): string[] {
  const calls: string[] = [];
  vi.stubGlobal('fetch', vi.fn(async (url: string, init?: RequestInit) => {
    calls.push(`${init?.method ?? 'GET'} ${url}`);
    return new Response(JSON.stringify(body), { status: 200 });
  }));
  return calls;
}

describe('API-Client für Läufe', () => {
  it('holt den Verlauf eines Laufs, auf Wunsch ab einem Cursor', async () => {
    const calls = stubFetch({ entries: [], cursor: '0', skipped: 0, available: false });
    const api = new ApiClient();
    await api.getDispatchRunTranscript(NAME);
    await api.getDispatchRunTranscript(NAME, '120', 500);
    expect(calls).toEqual([
      `GET /api/dispatch/runs/${NAME}/transcript`,
      `GET /api/dispatch/runs/${NAME}/transcript?after=120&limit=500`,
    ]);
  });

  it('holt die Screenshots eines Laufs', async () => {
    const calls = stubFetch([SHOT]);
    expect(await new ApiClient().getDispatchRunShots(NAME)).toEqual([SHOT]);
    expect(calls).toEqual([`GET /api/dispatch/runs/${NAME}/shots`]);
  });

  it('nennt die Adresse eines Screenshots, in der Desktop-App mit Start-Token', () => {
    expect(new ApiClient().dispatchRunShotUrl(NAME, '001.png')).toBe(`/api/dispatch/runs/${NAME}/shots/001.png`);
    expect(new ApiClient('http://127.0.0.1:4810', 'geheim es').dispatchRunShotUrl(NAME, '001.png')).toBe(
      `http://127.0.0.1:4810/api/dispatch/runs/${NAME}/shots/001.png?token=geheim%20es`,
    );
  });
});

describe('Test-Attrappe', () => {
  it('kennt ohne Angabe weder Verlauf noch Screenshots', async () => {
    const api = fakeApi();
    expect((await api.getDispatchRunTranscript(NAME)).available).toBe(false);
    expect(await api.getDispatchRunShots(NAME)).toEqual([]);
    expect(api.dispatchRunShotUrl(NAME, '001.png')).toBe(`/shots/${NAME}/001.png`);
  });
});

describe('RunThumb', () => {
  it('zeigt das Vorschaubild und meldet den Klick mit dem Dateinamen', async () => {
    const onOpen = vi.fn();
    render(<RunThumb shot={SHOT} url="/shots/x/001.png" label="Screenshot 1 von 3" onOpen={onOpen} />);
    const image = screen.getByAltText('Screenshot 1 von 3') as HTMLImageElement;
    expect(image.getAttribute('src')).toBe('/shots/x/001.png');
    expect(image.getAttribute('loading')).toBe('lazy');
    await userEvent.click(screen.getByRole('button', { name: 'Screenshot 1 von 3 vergrößern' }));
    expect(onOpen).toHaveBeenCalledWith('001.png');
  });
});

describe('Formen', () => {
  it('beschreiben Lauf und Verlaufszeilen', () => {
    const run: DispatchRunLogDto = { name: NAME, kind: 'work', started: null, running: false, outcome: 'done', abortReason: null, endedAt: null, durationMs: 720000, model: 'claude-opus-5-5' };
    const tool: ToolEntry = { kind: 'tool', id: 'toolu_1', at: SHOT.at, name: 'Read', summary: 'a.ts', input: null, result: null };
    const items: RunItem[] = [
      { type: 'entry', key: tool.id, entry: tool, shots: [SHOT] },
      { type: 'group', key: 'group:toolu_1', entries: [tool], tools: 1, label: '1 Schritt · 1× Read', open: false },
    ];
    expect(run.durationMs).toBe(720000);
    expect(items.map((item) => item.type)).toEqual(['entry', 'group']);
  });
});
