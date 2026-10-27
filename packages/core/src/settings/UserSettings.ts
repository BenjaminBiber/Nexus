export type NotificationMode = 'off' | 'browser' | 'windows';

export const NOTIFICATION_MODES: NotificationMode[] = ['off', 'browser', 'windows'];

export type ThemeMode = 'system' | 'light' | 'dark' | 'signal' | 'pastel' | 'win95';

export const THEME_MODES: ThemeMode[] = ['system', 'light', 'dark', 'signal', 'pastel', 'win95'];

// Benutzerweite Einstellungen des Harness (gelten für alle Projekte).
export interface UserSettings {
  /** Art der Meldung, wenn eine Session bereit ist oder auf Eingabe wartet */
  notifications: NotificationMode;
  /** Großes Bild in der Windows-Meldung „braucht deine Antwort“ */
  heroImages: boolean;
  /** Farbschema der Oberfläche; „system“ folgt dem Betriebssystem, „signal“ und „pastel“ sind farbigere helle Schemata, „win95“ ist Windows 95 samt Desktop, Fenster und Taskleiste */
  theme: ThemeMode;
  /** Claude-Nutzungslimits über das Anmelde-Token auslesen (Ring in der Seitenleiste); bewusst aus, bis es jemand einschaltet */
  claudeUsage: boolean;
  /** Kompakte Ticket-Kacheln auf dem Board: Tags eingeklappt, Agent-Pille nur als Claude-Logo */
  compactCards: boolean;
  /** Spalte „Nicht mehr nötig“ auf dem Board ausblenden */
  hideNotNeeded: boolean;
  /** Spalte „In Planung“ auf dem Board ausblenden */
  hideInPlanning: boolean;
  /** Meldungen zusätzlich als Push über ntfy schicken (unabhängig von `notifications`) */
  ntfy: boolean;
  /** Adresse des ntfy-Servers, z. B. https://ntfy.sh */
  ntfyServer: string;
  /** Thema, das die ntfy-App abonniert; leer: kein Push */
  ntfyTopic: string;
  /** Zugriffstoken für geschützte Themen; leer: ohne Anmeldung */
  ntfyToken: string;
}

export const DEFAULT_NTFY_SERVER = 'https://ntfy.sh';

// Erlaubte Zeichen eines ntfy-Themas (wie beim ntfy-Server)
export const NTFY_TOPIC_PATTERN = /^[-_A-Za-z0-9]{1,64}$/;

export const DEFAULT_USER_SETTINGS: UserSettings = {
  notifications: 'off',
  heroImages: false,
  theme: 'system',
  claudeUsage: false,
  compactCards: false,
  hideNotNeeded: false,
  hideInPlanning: false,
  ntfy: false,
  ntfyServer: DEFAULT_NTFY_SERVER,
  ntfyTopic: '',
  ntfyToken: '',
};
