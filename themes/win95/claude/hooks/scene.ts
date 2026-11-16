// Szene „Windows 95“: Petrol-Fläche wie ein Desktop der Neunziger, nach oben fein gerastert, darüber
// Pixelwolken, die langsam und verschieden schnell von links nach rechts ziehen, und ein paar Funkel-Pixel.
// Eigene Kunst im Geist der Zeit, keine Vorlage. Die Wolken stehen unten als ASCII-Karten und werden
// zu <rect> auf einem Pixelraster; jede Form liegt einmal als <symbol> vor und wird mit <use> gesetzt.
// Mitte (Text) und Fuß (Eingabefeld) bleiben ruhig: Wolken nur im oberen Band, zwei kleine an den Rändern.
// Klassen, IDs und Keyframes tragen das Präfix w95-, falls das SVG eingebettet statt als Bild gezeichnet wird.

/** Pixelkarte: '#' Weiß, '+' Schatten, alles andere leer; Zeilen dürfen kürzer sein */
type PixelMap = readonly string[]

const BIG: PixelMap = [
  '                 ########',
  '              ##############',
  '            ##################     ######',
  '           ####################  ##########',
  '      #######################################',
  '    ###########################################',
  '  ##############################################',
  ' ################################################',
  '##################################################',
  '##################################################',
  '+################################################+',
  ' ++############################################++',
  '   ++++######++++++++####################++++++',
  '       ++++++        ++++++++++++++++++++',
]

const MID: PixelMap = [
  '           ########',
  '        ##############   ####',
  '      ##########################',
  '  ##################################',
  ' ####################################',
  '######################################',
  '######################################',
  '+####################################+',
  ' ++################################++',
  '   ++++++########++++++++######+++',
  '         ++++++++        ++++++',
]

const SMALL: PixelMap = [
  '      ######',
  '   ############ ###',
  ' ####################',
  '######################',
  '+####################+',
  ' +++########++++++++',
  '    ++++++++',
]

/** Funkel-Pixel: kleines Kreuz mit hellem Kern */
const SPARK: PixelMap = [' + ', '+#+', ' + ']

const WHITE = '#ffffff'
const SHADE = '#bfdede'
const TEAL = '#008080'
const DITHER = '#1a9292'

type Run = { x: number; y: number; w: number; h: number }

/** Läufe gleicher Pixel je Zeile, gleich breite Läufe untereinander zu einem Rechteck zusammengefasst */
function runs(map: PixelMap, pixel: string): Run[] {
  const done: Run[] = []
  let open: Run[] = []
  map.forEach((row, y) => {
    const here: Run[] = []
    for (let x = 0; x < row.length; x++) {
      if (row[x] !== pixel) continue
      let end = x
      while (row[end + 1] === pixel) end++
      here.push({ x, y, w: end - x + 1, h: 1 })
      x = end
    }
    const next: Run[] = []
    for (const run of here) {
      const above = open.find(o => o.x === run.x && o.w === run.w)
      if (above) {
        above.h++
        next.push(above)
      } else next.push(run)
    }
    done.push(...open.filter(o => !next.includes(o)))
    open = next
  })
  return [...done, ...open]
}

function rects(map: PixelMap, pixel: string, fill: string): string {
  const body = runs(map, pixel)
    .map(r => `<rect x="${r.x}" y="${r.y}" width="${r.w}" height="${r.h}"/>`)
    .join('')
  return `<g fill="${fill}">${body}</g>`
}

function symbol(id: string, map: PixelMap): string {
  const width = Math.max(...map.map(row => row.length))
  return `<symbol id="${id}" viewBox="0 0 ${width} ${map.length}">${rects(map, '+', SHADE)}${rects(map, '#', WHITE)}</symbol>`
}

/** Eine ziehende Wolke: Form, Pixelgröße, Höhe, Dauer in Sekunden, Startversatz und Deckkraft */
type Cloud = { map: PixelMap; id: string; px: number; y: number; dur: number; delay: number; opacity?: number }

const DRIFT: Cloud[] = [
  { map: SMALL, id: 'w95-small', px: 5, y: 46, dur: 260, delay: -40, opacity: 0.75 },
  { map: MID, id: 'w95-mid', px: 6, y: 22, dur: 230, delay: -170, opacity: 0.8 },
  { map: BIG, id: 'w95-big', px: 8, y: 112, dur: 170, delay: -95 },
  { map: MID, id: 'w95-mid', px: 8, y: 236, dur: 135, delay: -30 },
  { map: BIG, id: 'w95-big', px: 7, y: 168, dur: 195, delay: -160, opacity: 0.92 },
  { map: SMALL, id: 'w95-small', px: 8, y: 292, dur: 115, delay: -75 },
]

function size(map: PixelMap, px: number): { w: number; h: number } {
  return { w: Math.max(...map.map(row => row.length)) * px, h: map.length * px }
}

function drifting(cloud: Cloud): string {
  const { w, h } = size(cloud.map, cloud.px)
  const opacity = cloud.opacity === undefined ? '' : ` opacity="${cloud.opacity}"`
  const style = `animation-duration:${cloud.dur}s;animation-delay:${cloud.delay}s`
  return `<use class="w95-drift" style="${style}" href="#${cloud.id}" x="0" y="${cloud.y}" width="${w}" height="${h}"${opacity}/>`
}

/** Kleine Wolken an den Rändern, die nur sacht hin und her schweben (auf schmalen Schirmen abgeschnitten) */
function bobbing(x: number, y: number, px: number, delay: number): string {
  const { w, h } = size(SMALL, px)
  return `<use class="w95-bob" style="animation-delay:${delay}s" href="#w95-small" x="${x}" y="${y}" width="${w}" height="${h}" opacity="0.85"/>`
}

function spark(x: number, y: number, delay: number): string {
  return `<use class="w95-spark" style="animation-delay:${delay}s" href="#w95-spark" x="${x}" y="${y}" width="12" height="12"/>`
}

/** Rasterband: Muster mit Anteil der hellen Pixel; Bänder werden nach unten lichter */
function ditherPatterns(): string {
  const p = 6
  const pattern = (id: string, tile: number, cells: [number, number][]) =>
    `<pattern id="${id}" width="${tile}" height="${tile}" patternUnits="userSpaceOnUse">` +
    cells.map(([x, y]) => `<rect x="${x * p}" y="${y * p}" width="${p}" height="${p}" fill="${DITHER}"/>`).join('') +
    '</pattern>'
  return (
    pattern('w95-d50', 2 * p, [[0, 0], [1, 1]]) +
    pattern('w95-d25', 2 * p, [[0, 0]]) +
    pattern('w95-d12', 4 * p, [[0, 0], [2, 2]]) +
    pattern('w95-d06', 4 * p, [[0, 0]]) +
    pattern('w95-d03', 8 * p, [[0, 0], [4, 4]])
  )
}

const STYLE = [
  '.w95-drift{animation-name:w95-drift;animation-timing-function:linear;animation-iteration-count:infinite}',
  '@keyframes w95-drift{from{transform:translateX(-440px)}to{transform:translateX(1640px)}}',
  '.w95-bob{animation:w95-bob 22s ease-in-out infinite alternate}',
  '@keyframes w95-bob{from{transform:translateX(-18px)}to{transform:translateX(18px)}}',
  '.w95-spark{opacity:0;animation:w95-spark 7s steps(1,end) infinite}',
  '@keyframes w95-spark{0%{opacity:0}62%{opacity:.9}70%{opacity:.45}78%{opacity:0}}',
  // Breite Fenster schneiden oben ab (slice): das obere Band rückt so weit nach unten, wie verdeckt ist
  '@media (min-aspect-ratio:16/9){.w95-top{transform:translateY(50px)}}',
  '@media (min-aspect-ratio:2/1){.w95-top{transform:translateY(100px)}}',
  '@media (min-aspect-ratio:7/3){.w95-top{transform:translateY(157px)}}',
  '@media (min-aspect-ratio:12/5){.w95-top{transform:translateY(167px)}}',
  '@media (prefers-reduced-motion:reduce){.w95-drift,.w95-bob,.w95-spark{animation-play-state:paused}}',
].join('')

/** Das fertige SVG-Dokument der Szene */
export const SCENE_SVG =
  '<svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice" shape-rendering="crispEdges" xmlns="http://www.w3.org/2000/svg">' +
  `<style>${STYLE}</style>` +
  '<defs>' +
  ditherPatterns() +
  symbol('w95-big', BIG) +
  symbol('w95-mid', MID) +
  symbol('w95-small', SMALL) +
  symbol('w95-spark', SPARK) +
  '</defs>' +
  `<rect x="-200" y="-200" width="2000" height="1400" fill="${TEAL}"/>` +
  '<g class="w95-top">' +
  '<rect x="-200" y="-200" width="2000" height="296" fill="url(#w95-d50)"/>' +
  '<rect x="-200" y="96" width="2000" height="108" fill="url(#w95-d25)"/>' +
  '<rect x="-200" y="204" width="2000" height="120" fill="url(#w95-d12)"/>' +
  '<rect x="-200" y="324" width="2000" height="120" fill="url(#w95-d06)"/>' +
  '<rect x="-200" y="444" width="2000" height="96" fill="url(#w95-d03)"/>' +
  spark(252, 84, -1) +
  spark(1312, 60, -3.5) +
  spark(930, 30, -5.2) +
  spark(118, 362, -2.4) +
  spark(1488, 340, -6.1) +
  DRIFT.map(drifting).join('') +
  '</g>' +
  bobbing(36, 540, 7, -4) +
  bobbing(1404, 640, 6, -13) +
  '</svg>'
