// Werte, die die Desktop-App vor jedem Skript ins Fenster legt: Die Oberfläche kommt dort aus den
// Dateien der App und weiß von sich aus nicht, wo der Server hört und wie sie sich ausweist.
// Im Browser gibt es die Werte nicht; dann bleibt alles wie bisher (gleiche Herkunft, kein Token).
export interface NexusBridge {
  // Adresse des Servers ohne Schrägstrich am Ende, z. B. http://127.0.0.1:4710
  base: string;
  // Start-Token dieses Serverlaufs
  token: string;
}

declare global {
  interface Window {
    __NEXUS__?: NexusBridge;
  }
}

// Die Werte der App, falls die Oberfläche in ihrem Fenster läuft
export function nexusBridge(): NexusBridge | undefined {
  const values = typeof window === 'undefined' ? undefined : window.__NEXUS__;
  return values?.base && values.token ? values : undefined;
}

// Adresse einer Datei oder Route des Servers für Stellen ohne ApiClient (Stylesheets, Bilder):
// in der App mit der Adresse des Servers davor, im Browser unverändert (gleiche Herkunft).
// Ohne sie landet die Anfrage bei der App, die für unbekannte Adressen ihre index.html liefert.
export function serverUrl(path: string): string {
  return `${nexusBridge()?.base ?? ''}${path}`;
}
