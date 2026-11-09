# Minecraft-Theme

Referenz-Theme für Code-Themes mit fremden Ressourcen. Es ersetzt den Slot `Shell` und lädt
Schrift-Atlas, Button-Texturen und Panorama aus dem Minecraft-Client von Mojang.

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

Oben in der Shell steht dabei „Minecraft-Ressourcen werden geladen … n/9“. Jeder Schritt wird in
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
