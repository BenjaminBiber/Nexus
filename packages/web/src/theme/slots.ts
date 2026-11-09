import type { ComponentType, ReactNode } from 'react';

/** Namen der Navigationssymbole, die ein Theme ersetzen kann */
export type NavIconName = 'board' | 'dashboard' | 'inbox' | 'sessions' | 'questions' | 'docs' | 'search' | 'guide' | 'settings';

/** Fertige Bausteine des Rahmens, die eine eigene Shell frei anordnet */
export interface ShellParts {
  /** Knopf der globalen Suche (öffnet auch mit Strg+K) */
  search: ReactNode;
  /** Board-Auswahl samt Verbindungspunkt */
  boardSwitcher: ReactNode;
  /** Claude-Nutzung (Ring/Balken) */
  usage: ReactNode;
}

/** Eingaben des Slots „Shell“: das ganze Layout um die Seiten */
export interface ShellProps {
  /** Projekt-ID aus der Adresse */
  pid: string;
  /** Links der Hauptnavigation (NavLinks mit Symbol und Beschriftung) */
  nav: ReactNode;
  /** Inhaltsbereich mit Verbindungshinweis und der aktuellen Seite */
  content: ReactNode;
  /** Weitere Bausteine des Rahmens */
  parts: ShellParts;
}

/** Eingaben des Slots „Progress“: Fortschritt aus erledigten und allen Schritten */
export interface ProgressProps {
  done: number;
  total: number;
  /** Zugängliche Bezeichnung, z. B. „Fortschritt der Schritte“ */
  label: string;
  /** Beschriftung neben der Zahl, z. B. „Schritte erledigt“ */
  caption: string;
}

/** Stellen der Oberfläche, die ein Theme durch eigene Komponenten ersetzen kann; fehlt ein Slot, gilt die Standard-Komponente */
export interface ThemeSlots {
  Shell?: ComponentType<ShellProps>;
  NavIcon?: ComponentType<{ name: NavIconName }>;
  Progress?: ComponentType<ProgressProps>;
}

/** Ein Theme mit ID (Wert von data-theme) und seinen Slots */
export interface ThemeDefinition {
  id: string;
  slots: ThemeSlots;
}
