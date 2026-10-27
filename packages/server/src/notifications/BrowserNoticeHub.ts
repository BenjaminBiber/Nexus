import type { ToastKind } from './ToastModel.js';

// Meldung, die ein offener Tab als Browser-Benachrichtigung zeigt; `url` ist ein Pfad der Oberfläche
export interface BrowserNotice {
  type: 'notice';
  kind: ToastKind;
  tag: string;
  title: string;
  body: string;
  url: string;
}

type Listener = (notice: BrowserNotice) => void;

// Offene Tabs, die Benachrichtigungen zeigen dürfen (Ereignisstrom mit ?notify=1), projektübergreifend.
// Solange einer offen ist, zeigt er die Meldung statt des Windows-Toasts; ein Klick bleibt dann im selben Tab.
export class BrowserNoticeHub {
  private readonly tabs = new Set<Listener>();

  // Meldet einen Tab an; der Rückgabewert meldet ihn wieder ab
  attach(listener: Listener): () => void {
    this.tabs.add(listener);
    return () => void this.tabs.delete(listener);
  }

  // Schickt die Meldung an alle Tabs (gleiche `tag` ersetzen sich im Browser); false: kein Tab offen
  deliver(notice: BrowserNotice): boolean {
    if (this.tabs.size === 0) return false;
    this.tabs.forEach((listener) => listener(notice));
    return true;
  }
}
