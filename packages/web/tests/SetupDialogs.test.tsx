// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiProvider } from '../src/api/ApiContext.js';
import type { BoardMigrationDto, PluginSetupDto } from '../src/api/types.js';
import { MigrationDialog } from '../src/components/setup/MigrationDialog.js';
import { PluginSetupDialog } from '../src/components/setup/PluginSetupDialog.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const missing: PluginSetupDto = { bundled: true, claude: true, state: 'missing', installedVersion: null, bundledVersion: '0.1.2' };

function openPlugin(setup: PluginSetupDto, overrides: Parameters<typeof fakeApi>[0] = {}) {
  const handlers = { onLater: vi.fn(), onNever: vi.fn(), onClose: vi.fn() };
  render(
    <ApiProvider api={fakeApi(overrides)}>
      <PluginSetupDialog setup={setup} {...handlers} />
    </ApiProvider>,
  );
  return handlers;
}

const legacy: BoardMigrationDto = { needed: true, repoPath: 'C:/repo', reasons: ['legacy-plugin', 'legacy-rules'] };

function openMigration(overrides: Parameters<typeof fakeApi>[0] = {}, pluginInstalled = true) {
  const handlers = { onLater: vi.fn(), onNever: vi.fn(), onClose: vi.fn() };
  render(
    <ApiProvider api={fakeApi(overrides)}>
      <MigrationDialog boardId={7} check={legacy} pluginInstalled={pluginInstalled} {...handlers} />
    </ApiProvider>,
  );
  return handlers;
}

describe('PluginSetupDialog', () => {
  it('erklärt ein fehlendes Plugin und installiert es', async () => {
    const installPlugin = vi.fn(async () => ({ ok: true, installed: true, steps: ['Plugin nexus@nexus-local installiert'], hints: [] }));
    openPlugin(missing, { installPlugin });
    expect(screen.getByText(/ist nicht installiert/)).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Installieren' }));
    expect(await screen.findByText(/\/reload-plugins/)).toBeTruthy();
    expect(installPlugin).toHaveBeenCalledOnce();
  });

  it('nennt bei veraltetem Plugin beide Versionen', () => {
    openPlugin({ ...missing, state: 'outdated', installedVersion: '0.1.1' });
    expect(screen.getByText(/installiert 0\.1\.1, mitgeliefert 0\.1\.2/)).toBeTruthy();
  });

  it('sperrt die Installation ohne claude', () => {
    openPlugin({ ...missing, claude: false });
    expect((screen.getByRole('button', { name: 'Installieren' }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.getByText(/Claude Code wurde nicht gefunden/)).toBeTruthy();
  });

  it('zeigt Hinweise und bietet erneut versuchen an', async () => {
    const installPlugin = vi.fn(async () => ({ ok: false, installed: false, steps: [], hints: ['claude plugin install nexus@nexus-local ist gescheitert.'] }));
    openPlugin(missing, { installPlugin });
    await userEvent.click(screen.getByRole('button', { name: 'Installieren' }));
    expect(await screen.findByText(/ist gescheitert/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Erneut versuchen' })).toBeTruthy();
  });

  it('sperrt die Knöpfe während der Installation', async () => {
    let finish: (value: unknown) => void = () => {};
    const installPlugin = vi.fn(() => new Promise((resolve) => { finish = resolve; }));
    const handlers = openPlugin(missing, { installPlugin });
    await userEvent.click(screen.getByRole('button', { name: 'Installieren' }));
    expect(screen.getByText(/wird bei Claude Code angemeldet/)).toBeTruthy();
    expect((screen.getByRole('button', { name: 'Nicht jetzt' }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.keyboard('{Escape}');
    expect(handlers.onLater).not.toHaveBeenCalled();
    finish({ ok: true, installed: true, steps: [], hints: [] });
  });

  it('meldet Nicht jetzt und Nicht mehr fragen', async () => {
    const handlers = openPlugin(missing);
    await userEvent.click(screen.getByRole('button', { name: 'Nicht jetzt' }));
    expect(handlers.onLater).toHaveBeenCalledOnce();
    await userEvent.click(screen.getByRole('button', { name: 'Nicht mehr fragen' }));
    expect(handlers.onNever).toHaveBeenCalledOnce();
  });
});

describe('MigrationDialog', () => {
  it('nennt Repo und Gründe in Klartext', () => {
    openMigration();
    expect(screen.getByText('C:/repo')).toBeTruthy();
    expect(screen.getByText('altes Plugin eingeschaltet')).toBeTruthy();
    expect(screen.getByText('alte Regeln vorhanden')).toBeTruthy();
  });

  it('migriert und listet geänderte und behaltene Dateien', async () => {
    const migrateBoard = vi.fn(async () => ({
      written: ['.claude/rules/nexus-board.md'], removed: ['.claude/rules/ai-harness-board.md'],
      kept: ['.claude/rules/ai-harness-docs.md'], pluginSwitched: true, importApproval: 'approved' as const,
    }));
    openMigration({ migrateBoard });
    await userEvent.click(screen.getByRole('button', { name: 'Migrieren' }));
    expect(await screen.findByText('.claude/rules/nexus-board.md')).toBeTruthy();
    expect(screen.getByText(/von Hand geändert/i)).toBeTruthy();
    expect(migrateBoard).toHaveBeenCalledWith(7);
  });

  it('weist ohne installiertes Plugin darauf hin', async () => {
    const migrateBoard = vi.fn(async () => ({ written: [], removed: [], kept: [], pluginSwitched: true, importApproval: 'unchanged' as const }));
    openMigration({ migrateBoard }, false);
    await userEvent.click(screen.getByRole('button', { name: 'Migrieren' }));
    expect(await screen.findByText(/erst mit installiertem Plugin/)).toBeTruthy();
  });

  it('zeigt Fehler und bietet erneut versuchen an', async () => {
    const migrateBoard = vi.fn(async () => { throw new Error('settings.local.json ist kein gültiges JSON'); });
    openMigration({ migrateBoard });
    await userEvent.click(screen.getByRole('button', { name: 'Migrieren' }));
    expect(await screen.findByText(/kein gültiges JSON/)).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Erneut versuchen' })).toBeTruthy();
  });

  it('meldet Für dieses Board nicht mehr fragen', async () => {
    const handlers = openMigration();
    await userEvent.click(screen.getByRole('button', { name: 'Für dieses Board nicht mehr fragen' }));
    expect(handlers.onNever).toHaveBeenCalledOnce();
  });
});
