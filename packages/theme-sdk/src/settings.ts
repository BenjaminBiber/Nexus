/** Eigene Einstellung eines Themes: Schalter */
export interface ThemeSettingBoolean {
  type: 'boolean';
  label: string;
  hint?: string;
  default: boolean;
}

/** Eigene Einstellung eines Themes: Auswahl aus festen Optionen */
export interface ThemeSettingSelect {
  type: 'select';
  label: string;
  hint?: string;
  options: { value: string; label: string }[];
  default: string;
}

/** Eine eigene Einstellung eines Themes */
export type ThemeSetting = ThemeSettingBoolean | ThemeSettingSelect;

/** Eigene Einstellungen eines Themes je Schlüssel (`^[a-zA-Z][a-zA-Z0-9_-]{0,39}$`, höchstens 50) */
export type ThemeSettingsSchema = Record<string, ThemeSetting>;

/** Werte zu einem Schema: Schalter als boolean, Auswahl als string */
export type ThemeSettingValues<S extends ThemeSettingsSchema> = { [K in keyof S]: S[K] extends ThemeSettingBoolean ? boolean : string };

/** Gespeicherte Werte des angezeigten Themes (fehlende oder ungültige ergeben den Default) */
export interface ThemeSettingsState {
  values: Record<string, boolean | string>;
  loading: boolean;
  /** Speichert einen Wert; das Panel und alle Hooks sehen ihn danach */
  set(key: string, value: boolean | string): Promise<void>;
}
