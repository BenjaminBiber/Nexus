import type { ReactNode } from 'react';

export type BadgeKind = 'plain' | 'blocked' | 'orphan' | 'unread' | 'running' | 'waiting';

// Kleine Marke; trägt immer Text, Farbe unterstützt nur (Zustände nie nur durch Farbe).
export function Badge({ kind = 'plain', children }: { kind?: BadgeKind; children: ReactNode }) {
  return <span className={`badge ${kind === 'plain' ? '' : kind}`.trim()}>{children}</span>;
}
