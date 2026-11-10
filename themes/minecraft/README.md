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
- **Seite:** im Panel mit halbtransparentem dunklem Grund; Code und Diffs bleiben in der
  Mono-Schrift des Hosts.

## Schrift

Die Shell schreibt alles in der Schrift des Spiels: Seitentitel, Knöpfe, Hotbar-Namen und der
Seiteninhalt des Hosts. Nur Code und Diffs (`pre`, `code`, `.diff`) behalten die Mono-Schrift.
Texte in der Spielschrift tragen den Schatten des Spiels (2 px nach rechts unten in einem Viertel
der Textfarbe), Code nicht.

- **Erzeugung:** `src/font/useMinecraftFont.ts` lädt die Schrift-Definition (`default.json`,
  `space.json`) und die drei Atlanten (`ascii.png`, `accented.png`, `nonlatin_european.png`) über
  den Asset-Proxy, legt die Atlanten per `Image` und Canvas als Pixel offen und baut daraus mit
  `src/font/buildFont.ts` eine TrueType-Schrift. Sie wird als `FontFace('Minecraft')` in
  `document.fonts` eingetragen; `--font-sans` nennt sie an erster Stelle.
- **Maße:** 16 px Schriftgröße und 18 px Zeilenhöhe entsprechen den 8 px bzw. 9 px des Spiels bei
  GUI-Maßstab 2. Je Spielpixel stehen so genau 2×2 gleichfarbige Bildschirmpixel.
- **Cache:** die gebaute Schrift liegt in der Cache API (`src/font/fontCache.ts`, Cache
  `nexus-theme-minecraft`) unter `/font/<Spielversion>/<Hash der Quell-URLs>`. Ab dem zweiten Start
  wird sie nicht neu gebaut. Ändern sich Version oder URLs, entsteht ein neuer Schlüssel.
- **Rückfall:** bis die Schrift steht und wenn etwas fehlschlägt (offline, kein Canvas, keine Cache
  API) gilt `'Courier New', ui-monospace, monospace`; der Schatten entfällt dann. Die Shell trägt
  dazu `data-mc-font="idle" | "building" | "ready" | "failed"`, Fehler stehen als
  `[minecraft] Schrift` in der Browser-Konsole.

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

## Sounds

Knöpfe klicken, ein Feldwechsel der Hotbar (Klick, Tasten 1–9, Mausrad) macht „pop“, das
Öffnen des Titelmenüs spielt einen eigenen Ton. Die Töne liegen nicht im Client-JAR, sondern
im Asset-Index der Version: Versions-JSON → `assetIndex.url` → Hash → 
`resources.download.minecraft.net/<ab>/<hash>`, alles über den Asset-Proxy. Fehlt der Index
oder ein Eintrag, bleibt das Theme still und voll bedienbar.

Der Schalter „Sounds“ unter Einstellungen → Darstellung (Theme-Einstellung `sounds`,
standardmäßig an) schaltet die Töne ab; der Wert wird je Theme gespeichert.

## Freigegebene Domains

`permissions.network`: `piston-meta.mojang.com`, `piston-data.mojang.com`,
`launchermeta.mojang.com`, `launcher.mojang.com`, `resources.download.minecraft.net` (Sounds).
Andere Domains lehnt der Proxy ab.

## Bauen

```
cd themes/minecraft
npm run build
nexus theme pack dist
```
