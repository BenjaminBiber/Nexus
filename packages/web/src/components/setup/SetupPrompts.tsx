import { useBoardProject } from '../../dbboard/boardProject.js';
import { MigrationDialog } from './MigrationDialog.js';
import { PluginSetupDialog } from './PluginSetupDialog.js';
import { useSetupPrompts } from './useSetupPrompts.js';

// Setup-Dialoge nach dem Start: Plugin installieren (nur App), danach das Repo des angezeigten Boards migrieren.
export function SetupPrompts() {
  const shown = useBoardProject()?.shown;
  const boardId = typeof shown?.board === 'number' ? shown.board : null;
  const setup = useSetupPrompts(boardId);
  const { prompt } = setup;
  if (prompt?.kind === 'plugin') {
    return <PluginSetupDialog setup={prompt.setup} onLater={setup.pluginLater} onNever={setup.pluginNever} onClose={setup.pluginClose} />;
  }
  if (prompt?.kind === 'migration') {
    return (
      <MigrationDialog
        key={prompt.boardId}
        boardId={prompt.boardId}
        check={prompt.check}
        pluginInstalled={setup.pluginInstalled}
        onLater={setup.migrationLater}
        onNever={setup.migrationNever}
        onClose={setup.migrationClose}
      />
    );
  }
  return null;
}
