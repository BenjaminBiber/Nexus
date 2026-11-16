// Szenen des Claude-Themes „GTA“: eigene Kunst aus Rechtecken und Kurven, keine Logos, Figuren, Karten oder
// Schriften der Spiele. GTA V: Küstenstadt in der Abenddämmerung (Skyline rechts, Palmen links, tiefe Sonne im
// Hitzeflimmern, ein Flugzeug zieht oben vorbei). San Andreas: dunstiger, orange-grüner Smog der Neunziger mit
// Hügeln links, flacher Skyline rechts und leichtem Filmkorn.
//
// Aufbau: Zeichenfläche 1600 × 1000 mit `xMidYMid slice`. Ein verschachteltes <svg> mit eigenem
// preserveAspectRatio hilft hier nicht, weil `slice` die Ränder der Zeichenfläche selbst abschneidet. Die Randmotive
// liegen deshalb in Gruppen .L (links unten), .R (rechts unten) und .B (Boden über die ganze Breite), die per
// @media (aspect-ratio) so verschoben und verkleinert werden, dass sie vom hohen Handy bis zum breiten Fenster im
// sichtbaren Ausschnitt bleiben. Die Mitte (Text) und der Fuß (Eingabefeld) tragen nur Himmel, Glühen und Dunst.
// Bewegung nur über CSS im eigenen <style>; prefers-reduced-motion hält alles an.
import type { NexusScene } from './nexus-look'

export type Style = 'v' | 'sa'

/** Zufallszahlen mit festem Startwert (mulberry32), damit jede Variante bei jedem Start dasselbe SVG liefert */
function rng(seed: number): () => number {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

/** Zahl mit höchstens einer Nachkommastelle, damit das SVG klein bleibt */
const n = (v: number): string => String(Math.round(v * 10) / 10)

type Point = readonly [number, number]

/** Geschlossener Hügelzug durch die Punkte (Mittelpunkt-Glättung), unten bis y = 1200 aufgefüllt */
function ridge(points: readonly Point[]): string {
  const first = points[0]!
  const last = points[points.length - 1]!
  let d = `M${n(first[0])} 1200V${n(first[1])}`
  for (let i = 1; i < points.length - 1; i++) {
    const [x, y] = points[i]!
    const [nx, ny] = points[i + 1]!
    d += `Q${n(x)} ${n(y)} ${n((x + nx) / 2)} ${n((y + ny) / 2)}`
  }
  return `${d}L${n(last[0])} ${n(last[1])}V1200Z`
}

/** Gebäude: linke Kante, Breite, Oberkante; unten reicht es bis y = 1200 */
type Building = readonly [number, number, number]

function blocks(list: readonly Building[]): string {
  return list.map(([x, w, top]) => `M${x} 1200V${top}H${x + w}V1200Z`).join('')
}

/**
 * Fenster der Gebäude, nach Eimern getrennt: je Eimer ein Pfad aus kleinen Rechtecken. `twinkle` Eimer mit den
 * Klassen t0…t(n-1) gehen langsam an und aus; die übrigen leuchten still.
 */
function windows(
  list: readonly Building[],
  rand: () => number,
  opts: { lit: number; twinkle: number; cool: number; dim: number; floor: number },
): { warm: string; cool: string; dim: string; tw: string[] } {
  const out = { warm: '', cool: '', dim: '', tw: Array.from({ length: opts.twinkle }, () => '') }
  for (const [x, w, top] of list) {
    for (let y = top + 9; y < opts.floor; y += 13) {
      for (let cx = x + 5; cx + 4 <= x + w - 4; cx += 9) {
        if (rand() > opts.lit) continue
        const rect = `M${cx} ${y}h4v6h-4z`
        const pick = rand()
        if (pick < 0.3 && opts.twinkle > 0) out.tw[Math.floor(rand() * opts.twinkle)] += rect
        else if (pick < 0.3 + opts.dim) out.dim += rect
        else if (pick < 0.3 + opts.dim + opts.cool) out.cool += rect
        else out.warm += rect
      }
    }
  }
  return out
}

/** CSS für die Fenster-Klassen: verschiedene Längen und Startpunkte, damit nie zwei Gruppen im Gleichtakt blinken */
function twinkleCss(count: number, base: number): string {
  const lengths = [9, 13, 17, 11, 19, 23, 15, 21, 27, 12]
  let css = ''
  for (let i = 0; i < count; i++) {
    const kf = i % 2 === 0 ? 'tw' : 'tu'
    css += `.t${i}{animation:${kf} ${n(lengths[i % lengths.length]! * base)}s ${-((i * 3.7) % 11).toFixed(1)}s infinite}`
  }
  // tw: meist an, kurz aus; tu: meist aus, kurz an
  css += '@keyframes tw{0%,70%{opacity:1}75%,92%{opacity:.08}97%,100%{opacity:1}}'
  css += '@keyframes tu{0%,62%{opacity:.08}67%,90%{opacity:1}95%,100%{opacity:.08}}'
  return css
}

/** Palme: Stamm von (bx, 1200) zur Krone (cx, cy), Wedel mit Schwanken um die Krone */
function palm(bx: number, cx: number, cy: number, size: number, bend: number, sway: string, fill: string): string {
  const by = 1200
  const wb = 9 * size
  const wt = 4 * size
  const kx = bx + (cx - bx) * 0.3 + bend
  const ky = (by + cy) / 2
  const trunk =
    `M${n(bx - wb)} ${by}Q${n(kx - wb * 0.7)} ${n(ky)} ${n(cx - wt)} ${n(cy)}` +
    `L${n(cx + wt)} ${n(cy)}Q${n(kx + wb * 0.7)} ${n(ky)} ${n(bx + wb)} ${by}Z`
  // Winkel in Grad, 0 = rechts, positiv nach unten; Länge relativ zur Größe
  const fronds: [number, number][] = [
    [-172, 1], [-150, 1.05], [-128, 0.9], [-104, 0.7], [-80, 0.72], [-56, 0.92], [-32, 1.05], [-8, 1],
    [18, 0.85], [160, 0.85], [196, 0.7],
  ]
  let leaves = ''
  for (const [deg, len] of fronds) {
    const a = (deg * Math.PI) / 180
    const L = 92 * size * len
    const ca = Math.cos(a)
    const sa = Math.sin(a)
    // Spitze hängt umso tiefer, je waagerechter der Wedel steht
    const tx = cx + ca * L
    const ty = cy + sa * L + Math.abs(ca) * L * 0.42
    const mx = cx + ca * L * 0.55
    const my = cy + sa * L * 0.55 - L * 0.22
    const px = -sa * L * 0.16
    const py = ca * L * 0.16
    leaves +=
      `M${n(cx)} ${n(cy)}Q${n(mx + px)} ${n(my + py - L * 0.04)} ${n(tx)} ${n(ty)}` +
      `Q${n(mx - px * 0.35)} ${n(my - py * 0.35 + L * 0.08)} ${n(cx)} ${n(cy)}Z`
  }
  const nuts = `<circle cx="${n(cx)}" cy="${n(cy + 5 * size)}" r="${n(7 * size)}"/>`
  return (
    `<path d="${trunk}"/>` +
    `<g class="${sway}" style="transform-origin:${n(cx)}px ${n(cy)}px"><path d="${leaves}"/>${nuts}</g>`
  ).replace(/<path /g, `<path fill="${fill}" `).replace('<circle ', `<circle fill="${fill}" `)
}

/** Feste Lage der Randmotive je Seitenverhältnis (siehe Kopf). Werte für 1600 × 1000 */
function pinCss(): string {
  // Schmaler als 8:5: die Zeichenfläche verliert links und rechts je 800 − 500·a; a = untere Grenze der Stufe.
  // Die erste Stufe endet knapp unter 8:5, damit genau 8:5 (1440 × 900) unverschoben bleibt
  const narrow: [string, number, number][] = [
    ['159/100', 150, 1],
    ['13/10', 300, 0.9],
    ['1/1', 425, 0.78],
    ['3/4', 520, 0.62],
    ['14/25', 590, 0.5],
  ]
  // Breiter als 8:5: unten fehlen 500 − 800/a; der Boden rückt so weit hoch, wie an der unteren Grenze fehlt
  const wide: [string, number][] = [
    ['19/10', 79],
    ['23/10', 152],
  ]
  let css =
    '.L,.R,.B{transform-box:view-box}.L{transform-origin:0 1000px}.R{transform-origin:1600px 1000px}'
  for (const [ratio, dx, s] of narrow) {
    const scale = s === 1 ? '' : ` scale(${s})`
    css += `@media (max-aspect-ratio:${ratio}){.L{transform:translate(${dx}px,0)${scale}}.R{transform:translate(-${dx}px,0)${scale}}}`
  }
  for (const [ratio, dy] of wide) css += `@media (min-aspect-ratio:${ratio}){.L,.R,.B{transform:translate(0,-${dy}px)}}`
  return css
}

const STILL = '@media (prefers-reduced-motion:reduce){*{animation:none!important}}'

const ROOT =
  '<svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice" xmlns="http://www.w3.org/2000/svg">'

// ==== GTA V: Küstenstadt in der Abenddämmerung ====

const V_SKYLINE: Building[] = [
  [1030, 50, 905],
  [1078, 38, 872],
  [1114, 60, 836],
  [1172, 44, 790],
  [1214, 70, 700],
  [1282, 40, 762],
  [1320, 56, 640],
  [1374, 48, 734],
  [1420, 64, 682],
  [1482, 36, 792],
  [1516, 58, 720],
  [1572, 52, 812],
  // Jenseits des Rands: füllen die Lücke, die zwischen zwei Stufen der Verschiebung entsteht
  [1624, 60, 758],
  [1684, 46, 846],
  [1730, 90, 704],
]

function sceneV(accent: string): string {
  const rand = rng(1985)

  // Sterne nur im oberen Drittel, schwach; einige funkeln
  let stars = ''
  for (let i = 0; i < 34; i++) {
    const x = rand() * 1600
    const y = 20 + rand() * rand() * 300
    const cls = i % 6 === 0 ? ` class="t${i % 4}"` : ''
    stars += `<circle cx="${n(x)}" cy="${n(y)}" r="${n(0.6 + rand() * 0.9)}" opacity="${n(0.2 + rand() * 0.45)}"${cls}/>`
  }

  const win = windows(V_SKYLINE, rand, { lit: 0.24, twinkle: 10, cool: 0.2, dim: 0.18, floor: 1000 })
  const tw = win.tw.map((d, i) => (d ? `<path class="t${i}" d="${d}"/>` : '')).join('')

  // Spiegelung der Sonne auf dem Wasser
  let glints = ''
  for (let i = 0; i < 7; i++) {
    const y = 938 + i * 8
    const half = 46 - i * 5 + rand() * 10
    glints += `<path class="r${i % 3}" d="M${n(1040 - half)} ${y}H${n(1040 + half)}"/>`
  }

  const palms =
    palm(40, 70, 572, 1, 26, 's0', '#05070c') +
    palm(150, 124, 474, 1.15, -30, 's1', '#04060a') +
    palm(238, 266, 660, 0.82, 18, 's2', '#070910')

  const css =
    twinkleCss(10, 1) +
    '.s0,.s1,.s2{animation:sw 11s ease-in-out infinite alternate}.s1{animation-duration:14s;animation-delay:-5s}' +
    '.s2{animation-duration:9s;animation-delay:-3s}@keyframes sw{from{transform:rotate(-2.2deg)}to{transform:rotate(2.6deg)}}' +
    '.h0,.h1,.h2{animation:hz 18s ease-in-out infinite alternate}.h1{animation-duration:23s;animation-delay:-8s}' +
    '.h2{animation-duration:29s;animation-delay:-14s}@keyframes hz{from{transform:translate(-50px,0)}to{transform:translate(50px,2px)}}' +
    '.r0,.r1,.r2{animation:rf 4s ease-in-out infinite}.r1{animation-delay:-1.3s}.r2{animation-delay:-2.6s}' +
    '@keyframes rf{0%,100%{opacity:.2}50%{opacity:.75}}' +
    '.pl{animation:fl 110s linear infinite}@keyframes fl{0%{transform:translate(-80px,0)}70%,100%{transform:translate(1700px,-46px)}}' +
    '.bk{animation:bk 1.6s steps(1,end) infinite}@keyframes bk{0%{opacity:1}12%,100%{opacity:.12}}' +
    '.bc{animation:bc 2.8s ease-in-out infinite}@keyframes bc{0%,100%{opacity:.15}45%,55%{opacity:1}}' +
    pinCss() +
    STILL

  return (
    ROOT +
    `<style>${css}</style>` +
    '<defs>' +
    '<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#060d18"/>' +
    '<stop offset=".38" stop-color="#0b2431"/><stop offset=".62" stop-color="#143c47"/>' +
    '<stop offset=".8" stop-color="#2f3f4b"/><stop offset="1" stop-color="#5a4248"/></linearGradient>' +
    '<linearGradient id="glow" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#e2733a" stop-opacity="0"/>' +
    '<stop offset=".42" stop-color="#d9693a" stop-opacity=".42"/><stop offset=".55" stop-color="#ef8a40" stop-opacity=".85"/>' +
    '<stop offset=".6" stop-color="#f6a650"/><stop offset="1" stop-color="#f6a650"/></linearGradient>' +
    '<radialGradient id="halo"><stop offset="0" stop-color="#ffb36a" stop-opacity=".6"/>' +
    '<stop offset=".35" stop-color="#ff8f4f" stop-opacity=".22"/><stop offset="1" stop-color="#ff8f4f" stop-opacity="0"/></radialGradient>' +
    '<radialGradient id="sun"><stop offset="0" stop-color="#fff2c8"/><stop offset=".55" stop-color="#ffc977"/>' +
    '<stop offset="1" stop-color="#ff9d52"/></radialGradient>' +
    '<linearGradient id="sea" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2b4752"/>' +
    '<stop offset=".25" stop-color="#11252e"/><stop offset="1" stop-color="#050b10"/></linearGradient>' +
    '<linearGradient id="bld" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#171b28"/>' +
    '<stop offset=".45" stop-color="#0b0e17"/><stop offset="1" stop-color="#05070c"/></linearGradient>' +
    '</defs>' +
    '<rect width="1600" height="1000" fill="url(#sky)"/>' +
    `<g fill="#fff">${stars}</g>` +
    // Glühen über dem Horizont und Wasser liegen am Boden
    '<rect class="B" x="-200" y="560" width="2000" height="640" fill="url(#glow)"/>' +
    '<g class="R"><circle cx="1040" cy="806" r="330" fill="url(#halo)"/><circle cx="1040" cy="806" r="56" fill="url(#sun)"/>' +
    `<path fill="#3b2b46" fill-opacity=".6" d="${ridge([[560, 912], [700, 884], [840, 896], [980, 868], [1120, 886], [1270, 856], [1420, 874], [1660, 860], [1880, 872]])}"/></g>` +
    '<g class="B" fill="#ffd9a8"><ellipse class="h0" cx="800" cy="884" rx="980" ry="5" opacity=".14"/>' +
    '<ellipse class="h1" cx="760" cy="900" rx="900" ry="4" opacity=".12"/><ellipse class="h2" cx="860" cy="914" rx="1000" ry="6" opacity=".1"/></g>' +
    '<g class="B"><rect x="-200" y="926" width="2000" height="300" fill="url(#sea)"/>' +
    '<path d="M-200 926H1800" stroke="#f6b06a" stroke-opacity=".45" stroke-width="1.5"/></g>' +
    `<g class="R" stroke="#ffc07a" stroke-width="2.2">${glints}</g>` +
    // Skyline mit Dachaufbauten, Neonkanten in der Akzentfarbe und Fenstern
    '<g class="R">' +
    `<path fill="url(#bld)" d="${blocks(V_SKYLINE)}M1226 702V676H1272V702ZM1240 678V660H1258V678ZM1328 642V618H1368V642ZM1344 620L1348 556L1352 620ZM1530 722V708H1548V722Z"/>` +
    '<path d="M1440 684V640M1466 684V656M1182 792V770" stroke="#0b0e17" stroke-width="2"/>' +
    `<g fill="none" stroke="${accent}"><path d="M1320 640H1376M1328 618H1368M1214 714H1284" stroke-width="7" stroke-opacity=".22"/>` +
    '<path d="M1320 640H1376M1328 618H1368M1214 714H1284" stroke-width="1.8" stroke-opacity=".9"/></g>' +
    `<path fill="#ffd38a" fill-opacity=".85" d="${win.warm}"/><path fill="#a9dcee" fill-opacity=".7" d="${win.cool}"/>` +
    `<path fill="#ffb46a" fill-opacity=".32" d="${win.dim}"/><g fill="#ffd38a" fill-opacity=".9">${tw}</g>` +
    '<circle class="bc" cx="1348" cy="556" r="8" fill="#ff4b3a" opacity=".3"/><circle class="bc" cx="1348" cy="556" r="2.6" fill="#ff6a50"/>' +
    '</g>' +
    `<g class="L">${palms}</g>` +
    // Flugzeug: rotes Positionslicht, weißer Blitz
    '<g class="pl"><circle cx="0" cy="232" r="2.1" fill="#ff5a48" opacity=".85"/><circle class="bk" cx="-7" cy="232" r="1.6" fill="#ffffff"/></g>' +
    '</svg>'
  )
}

// ==== San Andreas: Smog der Neunziger ====

const SA_BACK: Building[] = [
  [990, 70, 820],
  [1060, 56, 790],
  [1116, 40, 806],
  [1160, 52, 700],
  [1212, 76, 772],
  [1288, 46, 668],
  [1334, 80, 760],
  [1414, 60, 732],
  [1474, 74, 786],
  [1548, 70, 754],
  [1618, 84, 782],
  [1702, 120, 742],
]

const SA_FRONT: Building[] = [
  [1030, 64, 896],
  [1094, 90, 868],
  [1184, 52, 884],
  [1236, 84, 846],
  [1320, 60, 878],
  [1380, 96, 832],
  [1476, 58, 864],
  [1534, 76, 848],
  [1610, 92, 874],
  [1702, 120, 852],
]

/** Kachel des Filmkorns: helle und dunkle Punkte, je ein Pfad */
function grainTile(id: string, size: number, rand: () => number): string {
  let light = ''
  let dark = ''
  for (let i = 0; i < size; i++) {
    const x = Math.floor(rand() * size)
    const y = Math.floor(rand() * size)
    const dot = `M${x} ${y}h1v1h-1z`
    if (rand() < 0.5) light += dot
    else dark += dot
  }
  return (
    `<pattern id="${id}" width="${size}" height="${size}" patternUnits="userSpaceOnUse">` +
    `<path fill="#fff1d2" fill-opacity=".4" d="${light}"/><path fill="#000000" fill-opacity=".55" d="${dark}"/></pattern>`
  )
}

function sceneSa(accent: string): string {
  const rand = rng(1992)

  const win = windows(SA_FRONT, rand, { lit: 0.2, twinkle: 6, cool: 0.08, dim: 0.3, floor: 1000 })
  const tw = win.tw.map((d, i) => (d ? `<path class="t${i}" d="${d}"/>` : '')).join('')

  const palms = palm(70, 94, 432, 0.62, 10, 's0', '#17120a') + palm(128, 116, 496, 0.56, -8, 's1', '#1b150c')

  // Strommast links mit drei durchhängenden Leitungen, die zum Rand laufen
  const pole =
    '<g fill="#17120a"><path d="M245 1200V556H252V1200Z"/><path d="M224 574H274V579H224ZM230 592H268V596H230Z"/></g>' +
    '<path fill="none" stroke="#17120a" stroke-width="1.4" d="M226 574Q60 660 -260 600M249 572Q80 650 -260 576M272 574Q90 680 -260 628"/>'

  const css =
    twinkleCss(6, 1.6) +
    '.s0,.s1{animation:sw 13s ease-in-out infinite alternate}.s1{animation-duration:16s;animation-delay:-6s}' +
    '@keyframes sw{from{transform:rotate(-1.8deg)}to{transform:rotate(2deg)}}' +
    '.h0,.h1,.h2,.h3{animation:hz 70s ease-in-out infinite alternate}.h1{animation-duration:94s;animation-delay:-30s}' +
    '.h2{animation-duration:58s;animation-delay:-20s}.h3{animation-duration:120s;animation-delay:-60s}' +
    '@keyframes hz{from{transform:translate(-140px,0)}to{transform:translate(140px,-6px)}}' +
    '.gn{animation:gn .9s steps(1,end) infinite}' +
    '@keyframes gn{0%{transform:translate(0,0)}25%{transform:translate(-17px,9px)}50%{transform:translate(11px,-23px)}75%{transform:translate(-29px,-13px)}}' +
    '.ne{animation:ne 9s steps(1,end) infinite}@keyframes ne{0%,61%,66%,100%{opacity:.9}62%,65%{opacity:.25}63%{opacity:.7}}' +
    '.bc{animation:bc 3.2s ease-in-out infinite}@keyframes bc{0%,100%{opacity:.15}45%,55%{opacity:1}}' +
    pinCss() +
    STILL

  return (
    ROOT +
    `<style>${css}</style>` +
    '<defs>' +
    '<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#15110b"/>' +
    '<stop offset=".34" stop-color="#271e13"/><stop offset=".6" stop-color="#45321c"/>' +
    '<stop offset=".78" stop-color="#704c25"/><stop offset="1" stop-color="#8e602f"/></linearGradient>' +
    '<linearGradient id="glow" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#c7803a" stop-opacity="0"/>' +
    '<stop offset=".45" stop-color="#c7803a" stop-opacity=".5"/><stop offset=".7" stop-color="#d4954c" stop-opacity=".85"/>' +
    '<stop offset="1" stop-color="#b48d50"/></linearGradient>' +
    '<linearGradient id="smog" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#7f9150" stop-opacity="0"/>' +
    '<stop offset=".6" stop-color="#7f9150" stop-opacity=".16"/><stop offset="1" stop-color="#6c7e46" stop-opacity=".1"/></linearGradient>' +
    '<radialGradient id="halo"><stop offset="0" stop-color="#f0c27a" stop-opacity=".45"/>' +
    '<stop offset=".4" stop-color="#d79a52" stop-opacity=".16"/><stop offset="1" stop-color="#d79a52" stop-opacity="0"/></radialGradient>' +
    '<radialGradient id="hg"><stop offset="0" stop-color="#9aa863" stop-opacity=".34"/><stop offset="1" stop-color="#9aa863" stop-opacity="0"/></radialGradient>' +
    '<radialGradient id="ho"><stop offset="0" stop-color="#d6a463" stop-opacity=".3"/><stop offset="1" stop-color="#d6a463" stop-opacity="0"/></radialGradient>' +
    grainTile('g1', 73, rand) +
    grainTile('g2', 113, rand) +
    '</defs>' +
    '<rect width="1600" height="1000" fill="url(#sky)"/>' +
    '<rect class="B" x="-200" y="540" width="2000" height="660" fill="url(#glow)"/>' +
    // Sonne als blasse Scheibe im Dunst über den Hügeln
    '<g class="L"><circle cx="390" cy="610" r="260" fill="url(#halo)"/><circle cx="390" cy="610" r="44" fill="#f3d79c" opacity=".5"/>' +
    // Hügelzüge laufen nach rechts sanft unter den Rand aus, damit keine senkrechte Kante stehen bleibt
    `<path fill="#5d4a2b" fill-opacity=".8" d="${ridge([[-260, 690], [-60, 702], [80, 668], [220, 690], [380, 652], [540, 690], [700, 722], [860, 772], [1000, 830], [1120, 930], [1200, 1080], [1230, 1200]])}"/>` +
    `<path fill="#3d311d" d="${ridge([[-260, 730], [-60, 738], [110, 714], [260, 744], [420, 732], [600, 792], [780, 860], [900, 950], [970, 1090], [990, 1200]])}"/></g>` +
    '<g class="R">' +
    `<path fill="#5c4a2b" fill-opacity=".72" d="${blocks(SA_BACK)}M1296 670V652H1322V670Z"/>` +
    '</g>' +
    '<g class="B">' +
    '<ellipse class="h0" cx="560" cy="780" rx="760" ry="80" fill="url(#hg)"/>' +
    '<ellipse class="h1" cx="1120" cy="850" rx="820" ry="90" fill="url(#ho)"/>' +
    '<ellipse class="h3" cx="900" cy="560" rx="900" ry="70" fill="url(#hg)" opacity=".55"/>' +
    '</g>' +
    // Grünlicher Smog liegt über Ferne und Mitte, der Vordergrund bleibt satt
    '<rect class="B" x="-200" y="300" width="2000" height="900" fill="url(#smog)"/>' +
    `<g class="L"><path fill="#231b10" d="${ridge([[-260, 790], [-60, 806], [90, 784], [240, 818], [420, 884], [580, 960], [660, 1060], [690, 1200]])}"/>${pole}${palms}</g>` +
    '<g class="R">' +
    `<path fill="#1c160d" d="${blocks(SA_FRONT)}M1496 866L1503 700L1510 866Z"/>` +
    '<path fill="none" stroke="#1c160d" stroke-width="1.2" d="M1498 820H1508M1500 780H1506M1501 740H1505"/>' +
    `<path fill="#f0c878" fill-opacity=".62" d="${win.warm}"/><path fill="#c9d6a8" fill-opacity=".5" d="${win.cool}"/>` +
    `<path fill="#e0a85c" fill-opacity=".28" d="${win.dim}"/><g fill="#f0c878" fill-opacity=".7">${tw}</g>` +
    `<rect class="ne" x="1108" y="880" width="44" height="14" fill="none" stroke="${accent}" stroke-width="1.6"/>` +
    `<circle class="bc" cx="1503" cy="700" r="8" fill="${accent}" opacity=".3"/><circle class="bc" cx="1503" cy="700" r="2.6" fill="${accent}"/>` +
    '</g>' +
    '<g class="B"><ellipse class="h2" cx="820" cy="940" rx="980" ry="60" fill="url(#ho)" opacity=".6"/></g>' +
    '<rect class="gn" x="-40" y="-40" width="1680" height="1080" fill="url(#g1)" opacity=".5"/>' +
    '<rect x="0" y="0" width="1600" height="1000" fill="url(#g2)" opacity=".35"/>' +
    '</svg>'
  )
}

const ALT: Record<Style, string> = {
  v: 'Küstenstadt in der Abenddämmerung: Skyline mit funkelnden Fenstern rechts, Palmen links, tiefe Sonne über dem Wasser',
  sa: 'Dunstiger, orange-grüner Smog über der Stadt: Hügel und Strommast links, flache Skyline rechts, leichtes Filmkorn',
}

/** Szene des Stils; `accent` färbt Neon und Leuchtfeuer, `tint` ist surface-0 für das Glas der Panels */
export function sceneFor(style: Style, accent: string, tint: string): NexusScene {
  return {
    svg: style === 'sa' ? sceneSa(accent) : sceneV(accent),
    alt: ALT[style],
    glass: { blur: 16, tint, opacity: style === 'sa' ? 0.6 : 0.55 },
  }
}
