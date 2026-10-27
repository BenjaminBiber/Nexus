import type { ReactNode } from 'react';
import { CopyCommand } from './CopyCommand.js';

// Leerzustand mit Symbol, Überschrift, Erklärung und auf Wunsch einem kopierbaren Befehl.
export function EmptyState({ title, children, command }: { title: string; children?: ReactNode; command?: string }) {
  return (
    <div className="state empty">
      <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" aria-hidden="true">
        <rect x="3" y="3" width="7" height="18" rx="1.5" />
        <rect x="14" y="3" width="7" height="11" rx="1.5" />
      </svg>
      <h2>{title}</h2>
      {children && <p className="muted">{children}</p>}
      {command && <CopyCommand command={command} />}
    </div>
  );
}
