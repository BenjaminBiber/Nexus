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
