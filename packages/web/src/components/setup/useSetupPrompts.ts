import { useCallback, useEffect, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';
import type { BoardMigrationDto, PluginSetupDto, SetupPromptsDto } from '../../api/types.js';
import { isTauri } from '../../platform/createPlatform.js';
import { migrationLater, pluginLater, rememberMigrationLater, rememberPluginLater } from './setupSession.js';

export type SetupPrompt =
  | { kind: 'plugin'; setup: PluginSetupDto }
  | { kind: 'migration'; boardId: number; check: BoardMigrationDto }
  | null;

const NONE: SetupPromptsDto = { pluginDismissed: false, migrationDismissed: [] };

// Entscheidet, welcher Setup-Dialog dran ist: zuerst das Plugin (nur in der App), danach die Migration des angezeigten
// Boards. Fehler beim Laden lassen die Dialoge einfach weg.
export function useSetupPrompts(boardId: number | null) {
  const api = useApi();
  const [prompts, setPrompts] = useState<SetupPromptsDto | null>(null);
  const [plugin, setPlugin] = useState<PluginSetupDto | null>(null);
  const [pluginOpen, setPluginOpen] = useState(false);
  const [pluginDone, setPluginDone] = useState(false);
  const [migration, setMigration] = useState<{ boardId: number; check: BoardMigrationDto } | null>(null);

  useEffect(() => {
    let alive = true;
    void Promise.all([
      api.getUserSettings().then((s) => s.setupPrompts ?? NONE, () => NONE),
      api.getPluginSetup().catch(() => null),
    ]).then(([loaded, setup]) => {
      if (!alive) return;
      setPrompts(loaded);
      setPlugin(setup);
      const ask = isTauri() && setup !== null && setup.bundled && setup.state !== 'installed' && !loaded.pluginDismissed && !pluginLater();
      setPluginOpen(ask);
      setPluginDone(!ask);
    });
    return () => {
      alive = false;
    };
  }, [api]);

  const loaded = prompts !== null;
  useEffect(() => {
    setMigration(null);
    if (!pluginDone || prompts === null || boardId === null) return;
    if (prompts.migrationDismissed.includes(boardId) || migrationLater(boardId)) return;
    let alive = true;
    api.getBoardMigration(boardId).then(
      (check) => {
        if (alive && check.needed) setMigration({ boardId, check });
      },
      () => {},
    );
    return () => {
      alive = false;
    };
    // prompts absichtlich nur über loaded: „Nicht mehr fragen“ schließt den Dialog selbst
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [api, pluginDone, boardId, loaded]);

  const save = useCallback(
    (next: SetupPromptsDto) => {
      setPrompts(next);
      void api.saveUserSettings({ setupPrompts: next }).catch(() => {});
    },
    [api],
  );

  const closePlugin = useCallback(() => {
    setPluginOpen(false);
    setPluginDone(true);
  }, []);

  const prompt: SetupPrompt = pluginOpen && plugin ? { kind: 'plugin', setup: plugin } : migration ? { kind: 'migration', ...migration } : null;

  return {
    prompt,
    pluginInstalled: plugin?.state === 'installed',
    pluginLater: () => {
      rememberPluginLater();
      closePlugin();
    },
    pluginNever: () => {
      save({ ...(prompts ?? NONE), pluginDismissed: true });
      closePlugin();
    },
    pluginClose: () => {
      setPlugin((current) => (current ? { ...current, state: 'installed' } : current));
      closePlugin();
    },
    migrationLater: () => {
      if (migration) rememberMigrationLater(migration.boardId);
      setMigration(null);
    },
    migrationNever: () => {
      if (migration) {
        const current = prompts ?? NONE;
        save({ ...current, migrationDismissed: [...current.migrationDismissed, migration.boardId] });
      }
      setMigration(null);
    },
    migrationClose: () => setMigration(null),
  };
}
