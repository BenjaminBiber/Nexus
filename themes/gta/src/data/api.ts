// Lesender Zugriff auf die Server-API. Im Browser gleiche Herkunft ohne Token; in der Desktop-App legt Nexus Adresse und
// Token als window.__NEXUS__ ins Fenster (wie packages/web/src/api/nexusBridge.ts). EventSource kann keine Köpfe setzen,
// dort geht das Token in die Adresse.
import { parseBoardView, type BoardView } from './types.js';

interface Bridge {
  base: string;
  token: string;
}

function bridge(): Bridge | undefined {
  const values = typeof window === 'undefined' ? undefined : (window as Window & { __NEXUS__?: Bridge }).__NEXUS__;
  return values?.base && values.token ? values : undefined;
}

/** Adresse des Servers ohne Schrägstrich am Ende; leer bei gleicher Herkunft */
export function apiBase(): string {
  return bridge()?.base ?? '';
}

/** Köpfe für Anfragen: in der Desktop-App das Token */
export function apiHeaders(): Record<string, string> {
  const token = bridge()?.token;
  return token ? { 'x-nexus-token': token } : {};
}

export function boardViewUrl(boardId: number): string {
  return `${apiBase()}/api/db/boards/${boardId}/view`;
}

export function eventsUrl(pid: string, boardId: number): string {
  const token = bridge()?.token;
  return `${apiBase()}/api/projects/${encodeURIComponent(pid)}/events?dbBoard=${boardId}${token ? `&token=${encodeURIComponent(token)}` : ''}`;
}

/** Lädt den Board-Stand; wirft nie, bei Fehlern null */
export async function fetchBoardView(boardId: number, signal: AbortSignal): Promise<BoardView | null> {
  try {
    const res = await fetch(boardViewUrl(boardId), { headers: apiHeaders(), signal });
    if (!res.ok) return null;
    return parseBoardView(await res.json());
  } catch {
    return null;
  }
}
