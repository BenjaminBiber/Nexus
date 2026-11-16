import { useCallback, useState, useSyncExternalStore } from 'react';

// Laptop-Breite: breiter als die Leiste oben (bis 900 px), schmaler als ein großer Monitor. Hier ist die Seitenleiste
// von sich aus nur eine Symbolleiste
export const LAPTOP_QUERY = '(min-width: 901px) and (max-width: 1600px)';
export const SIDEBAR_KEY = 'nexus.sidebar';

type Choice = 'collapsed' | 'expanded';

function media() {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function' ? window.matchMedia(LAPTOP_QUERY) : null;
}

function subscribe(onChange: () => void) {
  const query = media();
  query?.addEventListener?.('change', onChange);
  return () => query?.removeEventListener?.('change', onChange);
}

const isLaptop = () => media()?.matches ?? false;

function readChoice(): Choice | null {
  try {
    const value = localStorage.getItem(SIDEBAR_KEY);
    return value === 'collapsed' || value === 'expanded' ? value : null;
  } catch {
    return null;
  }
}

function writeChoice(value: Choice | null) {
  try {
    if (value) localStorage.setItem(SIDEBAR_KEY, value);
    else localStorage.removeItem(SIDEBAR_KEY);
  } catch {
    // ohne Speicher gilt die Wahl nur bis zum Neuladen
  }
}

// Eingeklappt oder nicht: von Hand gewählt (gemerkt) oder nach Fensterbreite. Wer von Hand wieder auf das zurückstellt,
// was die Breite ohnehin vorgibt, gibt die Wahl ab; danach folgt die Leiste wieder der Fensterbreite
export function useSidebarRail() {
  const laptop = useSyncExternalStore(subscribe, isLaptop, () => false);
  const [choice, setChoice] = useState<Choice | null>(readChoice);
  const collapsed = choice ? choice === 'collapsed' : laptop;
  const toggle = useCallback(() => {
    const next: Choice = collapsed ? 'expanded' : 'collapsed';
    const value = next === (laptop ? 'collapsed' : 'expanded') ? null : next;
    writeChoice(value);
    setChoice(value);
  }, [collapsed, laptop]);
  return { collapsed, toggle };
}
