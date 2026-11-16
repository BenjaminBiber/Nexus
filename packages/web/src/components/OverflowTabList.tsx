import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from 'react';
import { fitTabs } from './fitTabs.js';

export type OverflowTab = {
  key: string;
  /** Reitername (Beschriftung des Menüeintrags für Screenreader) */
  label: string;
  /** Inhalt des Reiters und des Menüeintrags, z. B. Name mit Punkt */
  content: ReactNode;
  /** Ausgeblendet markiert der Reiter den ⋯-Knopf (z. B. laufender Agent, offene Fragen) */
  attention?: boolean;
};

const MORE = 'Weitere Reiter';

// Reiterleiste, die nie scrollt: Reiter, die nicht in die Breite passen, wandern in ein ⋯-Menü am Ende (fitTabs).
// Der aktive Reiter bleibt immer sichtbar. Gemessen wird an einer unsichtbaren Kopie aller Reiter (gleiche Klassen, damit
// Themes mitzählen); ein ResizeObserver rechnet bei jeder Größenänderung neu. Ohne Messung (jsdom) stehen alle Reiter da.
// className kommt an den äußeren Rahmen (z. B. „db-tablist“); die Reiter liegen in einem eigenen role="tablist".
export function OverflowTabList({ label, tabs, active, tabId, panelId, onSelect, className = '' }: {
  label: string;
  tabs: OverflowTab[];
  active: string;
  tabId: (key: string) => string;
  panelId: string;
  onSelect: (key: string) => void;
  className?: string;
}) {
  const root = useRef<HTMLDivElement>(null);
  const ruler = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState<number[] | null>(null);
  const [open, setOpen] = useState(false);
  // Nach einer Wahl im Menü bekommt der Reiter den Fokus, sobald er in der Leiste steht
  const focusKey = useRef<string | null>(null);
  const activeIndex = Math.max(0, tabs.findIndex((tab) => tab.key === active));

  // Misst Leiste und Reiterkopien und legt fest, welche Reiter sichtbar sind (nur bei Änderung neu setzen)
  const measure = useRef(() => {});
  measure.current = () => {
    const box = root.current;
    const row = ruler.current;
    if (!box || !row) return;
    const style = getComputedStyle(box);
    const available = box.clientWidth - (parseFloat(style.paddingLeft) || 0) - (parseFloat(style.paddingRight) || 0);
    const widths = [...row.querySelectorAll<HTMLElement>('[data-measure-tab]')].map((el) => el.getBoundingClientRect().width);
    const moreWidth = row.querySelector<HTMLElement>('[data-measure-more]')?.getBoundingClientRect().width ?? 0;
    const gap = parseFloat(style.columnGap) || 0;
    const next = fitTabs({ widths, gap, moreWidth, available, active: activeIndex }).visible;
    setVisible((current) => (current && current.join() === next.join() ? current : next));
  };

  // Nach jedem Zeichnen neu messen (Beschriftungen wie „Kommentare (3)“ ändern ihre Breite)
  useLayoutEffect(() => measure.current());

  useEffect(() => {
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(() => measure.current());
    if (root.current) observer.observe(root.current);
    if (ruler.current) observer.observe(ruler.current);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!focusKey.current) return;
    document.getElementById(tabId(focusKey.current))?.focus();
    focusKey.current = null;
  });

  // Klick außerhalb schließt das Menü; beim Öffnen bekommt der erste Eintrag den Fokus
  useEffect(() => {
    if (!open) return;
    menu.current?.querySelector<HTMLElement>('[role="menuitem"]')?.focus();
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside);
    return () => document.removeEventListener('pointerdown', outside);
  }, [open]);

  const shown = visible ?? tabs.map((_, index) => index);
  const hidden = tabs.map((_, index) => index).filter((index) => !shown.includes(index));
  const attention = hidden.some((index) => tabs[index].attention);
  // Der Knopf verschwindet, wenn alles passt; ein offenes Menü schließt dann mit
  useEffect(() => {
    if (hidden.length === 0) setOpen(false);
  }, [hidden.length]);

  const choose = (key: string) => {
    setOpen(false);
    focusKey.current = key;
    onSelect(key);
  };

  // Pfeiltasten wandern durch die Einträge, Escape und Tab schließen das Menü
  const onMenuKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const items = [...(menu.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];
    const at = items.indexOf(document.activeElement as HTMLElement);
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const step = event.key === 'ArrowDown' ? 1 : -1;
      items[(at + step + items.length) % items.length]?.focus();
    } else if (event.key === 'Home' || event.key === 'End') {
      event.preventDefault();
      items[event.key === 'Home' ? 0 : items.length - 1]?.focus();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      trigger.current?.focus();
    } else if (event.key === 'Tab') {
      setOpen(false);
    }
  };

  return (
    <div ref={root} className={`overflow-tabs ${className}`.trim()}>
      <div role="tablist" aria-label={label} className="overflow-tabs-list">
        {shown.map((index) => {
          const tab = tabs[index];
          return (
            <button
              key={tab.key}
              type="button"
              role="tab"
              id={tabId(tab.key)}
              aria-selected={tab.key === active}
              aria-controls={panelId}
              className={tab.key === active ? 'active' : ''}
              onClick={() => onSelect(tab.key)}
            >
              {tab.content}
            </button>
          );
        })}
      </div>
      {hidden.length > 0 && (
        <div className="overflow-tabs-more">
          <button
            ref={trigger}
            type="button"
            className="overflow-tabs-more-btn"
            aria-label={attention ? `${MORE} (mit Aktivität)` : MORE}
            title={MORE}
            aria-haspopup="menu"
            aria-expanded={open}
            onClick={() => setOpen((value) => !value)}
          >
            <span className="material-icons" aria-hidden="true">more_horiz</span>
            {attention && <span className="overflow-tabs-attention" aria-hidden="true" />}
          </button>
          {open && (
            <div ref={menu} role="menu" aria-label={MORE} className="overflow-tabs-menu" onKeyDown={onMenuKey}>
              {hidden.map((index) => (
                <button key={tabs[index].key} type="button" role="menuitem" aria-label={tabs[index].label} tabIndex={-1} onClick={() => choose(tabs[index].key)}>
                  {tabs[index].content}
                </button>
              ))}
            </div>
          )}
        </div>
      )}
      {/* Messkopie: unsichtbar, ohne Platzbedarf; dieselben Klassen wie die echten Reiter */}
      <div className="overflow-tabs-ruler" aria-hidden="true">
        <div ref={ruler} className="overflow-tabs-ruler-row">
          {tabs.map((tab) => (
            <button key={tab.key} type="button" role="tab" tabIndex={-1} data-measure-tab="" className={tab.key === active ? 'active' : ''}>
              {tab.content}
            </button>
          ))}
          <button type="button" tabIndex={-1} data-measure-more="" className="overflow-tabs-more-btn">
            <span className="material-icons">more_horiz</span>
          </button>
        </div>
      </div>
    </div>
  );
}
