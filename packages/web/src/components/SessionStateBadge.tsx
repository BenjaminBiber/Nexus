import type { SessionDisplayState } from '../api/types.js';
import { STATE_LABEL } from '../sessions/format.js';
import { Badge, type BadgeKind } from './Badge.js';

const KIND: Record<SessionDisplayState, BadgeKind> = { running: 'running', waiting: 'waiting', idle: 'plain', ended: 'plain', stale: 'plain' };

// Zustand einer Session als Marke mit Text.
export function SessionStateBadge({ state }: { state: SessionDisplayState }) {
  return (
    <span data-testid="session-state">
      <Badge kind={KIND[state]}>{STATE_LABEL[state]}</Badge>
    </span>
  );
}
