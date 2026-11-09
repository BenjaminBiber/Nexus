import type { ComponentType, MouseEvent, ReactNode } from 'react';

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
  /** Einzelne Links der Hauptnavigation im Standard-Aussehen, frei anordenbar */
  navItems: Partial<Record<NavIconName, ReactNode>>;
  /** Nur die aktuelle Seite, ohne Standard-Wrapper und Verbindungshinweis */
  outlet: ReactNode;
  /** Verbindungshinweis (Banner bei getrennter Live-Verbindung) */
  connection: ReactNode;
  /** Verbindungspunkt der Live-Verbindung */
  status: ReactNode;
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

/** Eingaben des Slots „Button“: ein Knopf; das Theme rendert das `<button>` selbst und reicht die Eingaben weiter */
export interface ButtonProps {
  /** Gewicht des Knopfs; Standard „secondary“ */
  variant?: 'primary' | 'secondary' | 'danger' | 'ghost';
  type?: 'button' | 'submit';
  disabled?: boolean;
  onClick?: (event: MouseEvent<HTMLButtonElement>) => void;
  title?: string;
  autoFocus?: boolean;
  'aria-label'?: string;
  'aria-pressed'?: boolean;
  'aria-expanded'?: boolean;
  /** Zusätzliche Klassen des Kerns, z. B. „small“ */
  className?: string;
  children?: ReactNode;
}

/** Eingaben des Slots „Card“: das Innere einer Karte; das äußere Element (Klasse, Rolle, Position, Ereignisse) bleibt im Kern und trägt `data-slot="Card"` */
export interface CardProps {
  title?: ReactNode;
  actions?: ReactNode;
  /** Klassen des äußeren Elements, z. B. „option-card recommended“ */
  className?: string;
  children?: ReactNode;
}

/** Eingaben des Slots „Panel“: das Innere eines Bereichs (z. B. Einstellungen); das äußere Element bleibt im Kern und trägt `data-slot="Panel"` */
export interface PanelProps {
  title?: ReactNode;
  className?: string;
  children?: ReactNode;
}

/** Eingaben des Slots „Dialog“: nur die sichtbare Hülle im Dialog; Rolle, Fokusfalle, Esc und Fokusrückgabe bleiben im Kern (Element mit `data-slot="Dialog"`) */
export interface DialogProps {
  /** id der Überschrift, die den Dialog benennt */
  labelledBy: string;
  role: 'dialog' | 'alertdialog';
  className?: string;
  children?: ReactNode;
}

/** Eingaben des Slots „Input“: ein Eingabefeld (`multiline` = mehrzeilig); das Theme rendert `<input>` bzw. `<textarea>` selbst */
export interface InputProps {
  value: string;
  onChange: (value: string) => void;
  id?: string;
  name?: string;
  type?: 'text' | 'search' | 'password' | 'email' | 'url' | 'number';
  placeholder?: string;
  disabled?: boolean;
  required?: boolean;
  autoFocus?: boolean;
  multiline?: boolean;
  /** Zeilen eines mehrzeiligen Felds */
  rows?: number;
  'aria-label'?: string;
  'aria-describedby'?: string;
  className?: string;
}

/** Stellen der Oberfläche, die ein Theme durch eigene Komponenten ersetzen kann; fehlt ein Slot, gilt die Standard-Komponente */
export interface ThemeSlots {
  Shell?: ComponentType<ShellProps>;
  NavIcon?: ComponentType<{ name: NavIconName }>;
  Progress?: ComponentType<ProgressProps>;
  Button?: ComponentType<ButtonProps>;
  Card?: ComponentType<CardProps>;
  Panel?: ComponentType<PanelProps>;
  Dialog?: ComponentType<DialogProps>;
  Input?: ComponentType<InputProps>;
}

/** Ein Theme mit ID (Wert von data-theme) und seinen Slots */
export interface ThemeDefinition {
  id: string;
  slots: ThemeSlots;
}
