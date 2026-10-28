import { vi } from 'vitest';
import type { Platform } from '../../src/platform/Platform.js';

// Plattform-Doppel: merkt sich die Aufrufe, die Überschreibungen gewinnen.
// Ohne Überschreibung gilt die Erlaubnis für Meldungen als noch nicht erteilt.
export function fakePlatform(overrides: Partial<Platform> = {}): Platform {
  return {
    kind: 'tauri',
    pickFolder: vi.fn(async () => null),
    openDoc: vi.fn(async () => undefined),
    openExternal: vi.fn(async () => undefined),
    notificationsSupported: vi.fn(() => true),
    notificationPermission: vi.fn(async () => 'default' as const),
    requestNotificationPermission: vi.fn(async () => 'default' as const),
    notify: vi.fn(async () => undefined),
    ...overrides,
  } as Platform;
}
