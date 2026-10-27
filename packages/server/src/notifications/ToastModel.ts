export type ToastKind = 'bereit' | 'wartet' | 'gesammelt' | 'frage-neu' | 'recherche';

// Alles, was der Toast zeigt; Texte sind fertig formuliert, Adressen vollständig.
export interface ToastModel {
  kind: ToastKind;
  title: string;
  line1: string;
  line2: string;
  /** Adresse, die ein Klick oder „Board öffnen“ aufruft */
  openUrl: string;
}

// Bildpfade und Voreinstellungen für den Toast
export interface ToastOptions {
  assetsDir: string;
  heroImages: boolean;
}
