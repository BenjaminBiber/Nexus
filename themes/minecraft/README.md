# Minecraft-Theme

Referenz-Theme für Code-Themes mit fremden Ressourcen. Es ersetzt die Slots `Shell` und `NavIcon`
und lädt Schrift-Atlas, Button-Texturen, Panorama, Hotbar, Logo und Item-Symbole aus dem
Minecraft-Client von Mojang.

## Menü-Layout

Die Shell sieht aus wie der Titelbildschirm des Spiels:

- **Hintergrund:** die 6 Panorama-Bilder als Würfel um den Betrachter, langsam drehend. Bei
  „Bewegung reduzieren“ (`prefers-reduced-motion`) steht das Bild still. Ohne Ressourcen ein ruhiger
  Erdbraun-Verlauf.
- **Hotbar** (unten, `navigation` „Hauptnavigation“): 9 Felder, eins je Navigationsziel, mit einem
  Item-Symbol (Karte = Board, Uhr = Dashboard, Bündel = Meine Aufgaben, Buch und Feder = Sessions,
  Buch = Fragen, beschriebenes Buch = Dokumente, Fernrohr = Suche, Kompass = Anleitung, Redstone =
  Einstellungen). Das aktive Feld trägt den Auswahl-Rahmen, sein Name steht darüber.
  - Klick auf ein Feld öffnet die Seite.
  - Tasten **1–9** wählen das Feld (nicht beim Tippen in Feldern, nicht mit Strg/Alt).
  - **Mausrad** über der Hotbar wechselt zum Nachbarfeld, am Rand rundherum.
- **Titelmenü** (`menu` „Hauptmenü“): öffnet über den Knopf „Menü“ oben links oder mit **Esc**
  (außerhalb von Feldern und Dialogen) und erscheint auch, wenn keine Seite offen ist. Die
  Navigationsziele stehen als große Minecraft-Knöpfe darin, dazu „Suchen …“ für die globale Suche.
  Pfeiltasten wandern, Enter öffnet, Esc schließt und gibt den Fokus an den Knopf „Menü“ zurück.
- **Kopfzeile:** Suche, Board-Auswahl, Claude-Verbrauch und Verbindungsstatus des Hosts.
- **Seite:** im Panel mit halbtransparentem dunklem Grund; Fließtext in normaler Schrift.

Knöpfe, Hotbar-Namen und Überschriften nutzen die Bitmap-Schrift aus `ascii.png`
(`src/BitmapText.tsx`). Der Text bleibt im DOM (unsichtbar über den Glyphen), Screenreader und
Kopieren funktionieren also. Zeichen außerhalb von ASCII (Umlaute, „…“) und alles vor dem Laden des
Atlas erscheinen in einer Rückfallschrift.

## Keine Mojang-Dateien im Paket

Texturen, Schriften und das Client-JAR gehören Mojang (EULA) und werden **nie** mitverteilt –
weder im Repo noch im `.nexus`-Paket, auch nicht als Test-Fixture. Das Paket enthält nur
`manifest.json`, `theme.css` und `main.js`. Ein Test (`tests/package.test.ts`) prüft das.

## Laden beim ersten Start

`src/resources.ts` fragt alles über den Asset-Proxy von Nexus an (`/api/themes/minecraft/fetch`):

1. Versionsliste `piston-meta.mojang.com/mc/game/version_manifest_v2.json`
2. Versions-JSON der festen Version (`MINECRAFT_VERSION`, derzeit 1.21.4) → URL des Client-JARs
3. Je Eintrag des JARs (`entry=…`) nacheinander: der erste lädt das JAR (rund 30 MiB) in den
   Proxy-Cache, die weiteren lesen daraus

Oben in der Shell steht dabei „Minecraft-Ressourcen werden geladen … n/21“. Jeder Schritt wird in
der Browser-Konsole als `[minecraft] … { cache: 'miss' | 'hit' }` geloggt.

## Cache

Der Proxy legt die Antworten unter `~/.nexus/themes/minecraft/cache/` ab. Ab dem zweiten Start
kommt alles aus dem Cache (`cache: hit`), es geht nichts mehr ins Netz. Wer den Ordner löscht,
lädt beim nächsten Start neu.

## Offline

Ohne Netz (oder wenn Mojang nicht antwortet, Proxy-Status 502) zeigt die Shell den Hinweis
„Keine Verbindung zu Mojang …“ mit dem Knopf „Erneut versuchen“. Die Seite bleibt bedienbar,
nur die Texturen fehlen; dann gelten die Rückfall-Farben aus `theme.css`. Antwortet der Proxy
mit 403, fehlt die Domain-Freigabe im Manifest – der Hinweis nennt das.

## Freigegebene Domains

`permissions.network`: `piston-meta.mojang.com`, `piston-data.mojang.com`,
`launchermeta.mojang.com`, `launcher.mojang.com`. Andere Domains lehnt der Proxy ab.

## Bauen

```
cd themes/minecraft
npm run build
nexus theme pack dist
```
