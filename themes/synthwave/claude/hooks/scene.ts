// Szene „Synthwave“ für Claude: Nachthimmel mit Sternen, große gestreifte Sonne rechts vor einem rosa beleuchteten
// Grat, Drahtgitter-Berge mit Neonkanten links, Perspektiv-Gitter am Boden und Dunst über Horizont und Mitte.
// Das Bild ist 1600 × 1000 und wird mit „slice“ beschnitten; im Hochformat rücken Berge und Sonne per @media an die
// sichtbaren Ränder. Bewegung nur über CSS im eigenen <style>, kein Skript, keine fremden Adressen.

/** Grundfarben eines Farbschemas, wie in theme.css (--synth-*) */
export type SceneColors = {
  bg: string
  deep: string
  fg: string
  violet: string
  magenta: string
  cyan: string
  sunTop: string
  sunBottom: string
}

export type SkyPhase = 'dawn' | 'day' | 'dusk' | 'night'
export type HorizonMode = 'animated' | 'still' | 'off'

const W = 1600
const H = 1000
/** Horizontlinie */
const HY = 640

/** Mischt zwei Hex-Farben wie color-mix(in srgb, a p, b) */
export function mix(a: string, b: string, p: number): string {
  const ch = (hex: string, i: number) => parseInt(hex.slice(1 + i * 2, 3 + i * 2), 16)
  let out = '#'
  for (let i = 0; i < 3; i++) out += Math.round(ch(a, i) * p + ch(b, i) * (1 - p)).toString(16).padStart(2, '0')
  return out
}

/** Gekürzte Zahl für Koordinaten */
const n = (v: number) => String(Math.round(v * 10) / 10)

/** Fester Zufall, damit Sterne und Berge bei jedem Start gleich liegen */
function seeded(seed: number): () => number {
  let s = seed >>> 0
  return () => {
    s = (s + 0x6d2b79f5) >>> 0
    let t = s
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

type Pt = [number, number]

/**
 * Low-Poly-Gebirge aus einer Gratlinie: Flächen abwechselnd hell und dunkel, dazu alle Kanten als ein Pfad.
 * Zwischen je zwei Gratpunkten liegt ein Mittelpunkt auf halber Höhe zum Horizont, von dort fällt eine Kante zum Fuß.
 */
function mountains(ridge: Pt[], light: string, dark: string, seed: number): { faces: string; edges: string; ridge: string } {
  const rnd = seeded(seed)
  const mids: Pt[] = []
  for (let i = 0; i < ridge.length - 1; i++) {
    const [x1, y1] = ridge[i]!
    const [x2, y2] = ridge[i + 1]!
    const top = (y1 + y2) / 2
    mids.push([(x1 + x2) / 2 + (rnd() - 0.5) * 30, top + (HY - top) * (0.45 + rnd() * 0.2)])
  }
  const p = (pts: Pt[]) => pts.map(([x, y]) => `${n(x)} ${n(y)}`).join(' ')
  let faces = ''
  let edges = ''
  for (let i = 0; i < mids.length; i++) {
    const a = ridge[i]!
    const b = ridge[i + 1]!
    const m = mids[i]!
    faces += `<polygon points="${p([a, b, m])}" fill="${i % 2 ? dark : light}"/>`
    edges += `M${p([a])}L${p([m])}L${p([b])}M${p([m])}L${n(m[0] + (rnd() - 0.5) * 40)} ${HY}`
    if (i < mids.length - 1) {
      const next = mids[i + 1]!
      faces += `<polygon points="${p([m, b, next])}" fill="${i % 2 ? light : dark}"/>`
      edges += `M${p([m])}L${p([next])}`
    }
  }
  const outline = `${p(ridge)} ${n(ridge[ridge.length - 1]![0])} ${HY + 4} ${n(ridge[0]![0])} ${HY + 4}`
  faces = `<polygon points="${outline}" fill="${dark}"/>${faces}`
  return { faces, edges, ridge: `M${ridge.map(([x, y]) => `${n(x)} ${n(y)}`).join('L')}` }
}

/** Himmel je Phase: Verlauf oben → Mitte → Horizont, Sichtbarkeit der Sterne, Lage und Helligkeit der Sonne */
function skyOf(c: SceneColors, phase: SkyPhase) {
  switch (phase) {
    case 'dawn':
      return {
        top: mix(c.violet, c.deep, 0.22),
        mid: mix(c.magenta, c.bg, 0.3),
        low: mix(mix(c.sunBottom, c.sunTop, 0.65), c.bg, 0.62),
        stars: 0.18,
        sunY: 560,
        sunGlow: 0.75,
        grid: c.magenta,
        gridOpacity: 0.85,
      }
    case 'day':
      return {
        top: mix(c.cyan, c.bg, 0.3),
        mid: mix(c.violet, c.bg, 0.38),
        low: mix(c.magenta, c.violet, 0.4),
        stars: 0,
        sunY: 380,
        sunGlow: 0.6,
        grid: c.magenta,
        gridOpacity: 0.55,
      }
    case 'night':
      return {
        top: c.deep,
        mid: mix(c.violet, c.deep, 0.16),
        low: mix(c.violet, c.bg, 0.42),
        stars: 1,
        sunY: 640,
        sunGlow: 0.45,
        grid: c.cyan,
        gridOpacity: 0.85,
      }
    default:
      return {
        top: mix(c.violet, c.deep, 0.12),
        mid: mix(c.violet, c.bg, 0.3),
        low: mix(c.magenta, c.violet, 0.55),
        stars: 0.7,
        sunY: 450,
        sunGlow: 0.9,
        grid: c.magenta,
        gridOpacity: 1,
      }
  }
}

/** Waagerechte Gitterlinien: so viele laufen gleichzeitig, eine Runde dauert DRIVE Sekunden */
const LINES = 14
const DRIVE = 16
/** Tiefe einer Linie am Horizont und beim Verlassen des Bildes; der Abstand zum Horizont ist 360 / Tiefe */
const Z_FAR = 14
const Z_NEAR = 0.85

/**
 * Keyframes einer Gitterlinie: die Tiefe sinkt gleichmäßig, die Linie wird zum Betrachter hin schneller.
 * Die Stützstellen liegen gegen Ende dichter, damit die lineare Interpolation der Kurve folgt.
 */
function driveKeyframes(): string {
  const steps = 16
  let frames = ''
  for (let i = 0; i <= steps; i++) {
    const t = 1 - Math.pow(1 - i / steps, 2)
    const z = Z_FAR - (Z_FAR - Z_NEAR) * t
    const opacity = Math.min(1, t / 0.15)
    frames += `${Math.round(t * 1000) / 10}%{transform:translateY(${n(360 / z)}px);opacity:${n(opacity)}}`
  }
  return `@keyframes drive{${frames}}`
}

/** Baut das SVG der Szene */
export function buildScene(c: SceneColors, phase: SkyPhase, horizon: HorizonMode): string {
  const sky = skyOf(c, phase)
  const rnd = seeded(84)
  const glowPink = mix(c.magenta, c.sunBottom, 0.5)

  // Sterne über die ganze Breite (auch für das Hochformat), ein Teil funkelt
  let stars = ''
  for (let i = 0; i < 90; i++) {
    const x = -300 + rnd() * 2200
    const y = 10 + Math.pow(rnd(), 1.4) * 540
    const r = 0.7 + rnd() * 1.3
    const fill = rnd() < 0.2 ? c.cyan : c.fg
    const tw = i % 4 === 0 ? ` class="tw t${i % 3}"` : ''
    stars += `<circle cx="${n(x)}" cy="${n(y)}" r="${n(r)}" fill="${fill}"${tw}/>`
  }

  // Sonne: Streifenlücken in der unteren Hälfte, nach unten breiter (Anteile des Radius)
  const R = 245
  const gaps: [number, number][] = [
    [0.05, 0.028],
    [0.18, 0.042],
    [0.32, 0.058],
    [0.47, 0.074],
    [0.62, 0.092],
    [0.78, 0.11],
  ]
  const stripes = gaps.map(([y, h]) => `<rect x="-${R}" y="${n(y * R)}" width="${2 * R}" height="${n(h * R)}"/>`).join('')

  // Linke Berge: vorne mit Cyan-Kanten, dahinter flacher in Violett
  const front = mountains(
    [
      [-520, 330], [-410, 370], [-300, 300], [-190, 360], [-90, 290], [10, 340], [110, 268], [205, 352], [300, 318],
      [395, 410], [480, 392], [565, 478], [640, 530], [720, 596], [800, 640],
    ],
    mix(c.violet, c.deep, 0.24),
    mix(c.violet, c.deep, 0.12),
    7,
  )
  const back = mountains(
    [
      [-520, 470], [-380, 430], [-250, 470], [-120, 420], [0, 455], [140, 400], [270, 450], [390, 430], [520, 490],
      [640, 470], [760, 530], [880, 560], [980, 640],
    ],
    mix(c.violet, c.bg, 0.2),
    mix(c.violet, c.bg, 0.12),
    11,
  )
  // Grat hinter der Sonne: rosa beleuchtet
  const ridge = mountains(
    [
      [860, 640], [930, 596], [1000, 566], [1080, 580], [1160, 526], [1240, 552], [1330, 506], [1420, 534], [1500, 476],
      [1580, 522], [1660, 490], [1760, 540], [1880, 512], [2000, 556], [2120, 600],
    ],
    mix(c.magenta, c.bg, 0.26),
    mix(c.magenta, c.bg, 0.14),
    23,
  )

  // Gitter: senkrechte Linien zum Fluchtpunkt, waagerechte laufen auf den Betrachter zu
  let grid = ''
  if (horizon !== 'off') {
    let rays = ''
    for (let k = -16; k <= 16; k++) rays += `M800 ${HY}L${800 + k * 150 * 1.25} 1090`
    let lines = ''
    for (let i = 0; i < LINES; i++) {
      const delay = -((i * DRIVE) / LINES)
      lines += `<line x1="-500" y1="${HY}" x2="2100" y2="${HY}" style="animation-delay:${n(delay)}s"/>`
    }
    grid =
      `<g mask="url(#gm)" opacity="${sky.gridOpacity}"><g mask="url(#gv)" stroke="${sky.grid}" fill="none">` +
      `<path d="${rays}" stroke-width="5" opacity=".18"/><path d="${rays}" stroke-width="1.6"/>` +
      `<g class="hl" stroke-width="5" opacity=".2">${lines}</g><g class="hl" stroke-width="1.8">${lines}</g>` +
      `</g></g>`
  }

  // Mond nur nachts, über der versunkenen Sonne; liegt in der rechten Gruppe und wandert im Hochformat mit
  const moon =
    phase === 'night'
      ? `<g transform="translate(1240 150)"><circle r="70" fill="url(#mg)"/>` +
        `<circle r="30" fill="${c.fg}" mask="url(#mm)"/></g>`
      : ''

  const css =
    `.tw{animation:tw 3.6s ease-in-out infinite}.t1{animation-duration:5.2s;animation-delay:-1.7s}` +
    `.t2{animation-duration:4.4s;animation-delay:-3.1s}@keyframes tw{0%,100%{opacity:1}50%{opacity:.2}}` +
    `.hl line{animation:drive ${DRIVE}s linear infinite}${driveKeyframes()}` +
    `.glow{animation:pulse 7s ease-in-out infinite}@keyframes pulse{0%,100%{opacity:1}50%{opacity:.8}}` +
    // Hochformat: Berge an den linken, Sonne und Grat an den rechten sichtbaren Rand
    `#L,#R{transform-box:view-box}#L{transform-origin:0 ${HY}px}#R{transform-origin:${W}px ${HY}px}` +
    `@media (max-aspect-ratio:1/1){#L{transform:translate(400px,0) scale(.8)}#R{transform:translate(-230px,0) scale(.8)}}` +
    `@media (max-aspect-ratio:3/5){#L{transform:translate(560px,0) scale(.56)}#R{transform:translate(-420px,0) scale(.6)}}` +
    `@media (prefers-reduced-motion:reduce){*{animation-play-state:paused!important}}`

  return (
    `<svg viewBox="0 0 ${W} ${H}" preserveAspectRatio="xMidYMid slice" xmlns="http://www.w3.org/2000/svg">` +
    `<style>${css}</style>` +
    `<defs>` +
    `<linearGradient id="sky" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${sky.top}"/>` +
    `<stop offset=".6" stop-color="${sky.mid}"/><stop offset="1" stop-color="${sky.low}"/></linearGradient>` +
    `<linearGradient id="sun" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${c.sunTop}"/>` +
    `<stop offset=".45" stop-color="${mix(c.sunTop, c.sunBottom, 0.45)}"/><stop offset=".8" stop-color="${c.sunBottom}"/>` +
    `<stop offset="1" stop-color="${c.magenta}"/></linearGradient>` +
    `<radialGradient id="sg"><stop offset=".55" stop-color="${glowPink}" stop-opacity=".55"/>` +
    `<stop offset=".75" stop-color="${glowPink}" stop-opacity=".18"/><stop offset="1" stop-color="${glowPink}" stop-opacity="0"/></radialGradient>` +
    `<mask id="ss" maskUnits="userSpaceOnUse" x="-${R}" y="-${R}" width="${2 * R}" height="${2 * R}">` +
    `<rect x="-${R}" y="-${R}" width="${2 * R}" height="${2 * R}" fill="#fff"/><g fill="#000">${stripes}</g></mask>` +
    `<linearGradient id="rf" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${mix(c.magenta, c.bg, 0.3)}"/>` +
    `<stop offset="1" stop-color="${c.bg}" stop-opacity="0"/></linearGradient>` +
    `<linearGradient id="gd" x1="0" y1="${HY}" x2="0" y2="${H}" gradientUnits="userSpaceOnUse">` +
    `<stop offset="0" stop-color="${mix(sky.grid, c.deep, 0.22)}"/><stop offset=".35" stop-color="${c.deep}"/></linearGradient>` +
    `<linearGradient id="gh" x1="-500" y1="0" x2="2100" y2="0" gradientUnits="userSpaceOnUse">` +
    `<stop offset=".33" stop-color="#fff"/><stop offset=".41" stop-color="#fff" stop-opacity=".18"/>` +
    `<stop offset=".5" stop-color="#fff" stop-opacity=".08"/><stop offset=".59" stop-color="#fff" stop-opacity=".18"/>` +
    `<stop offset=".67" stop-color="#fff"/></linearGradient>` +
    `<linearGradient id="gvv" x1="0" y1="${HY}" x2="0" y2="${H}" gradientUnits="userSpaceOnUse">` +
    `<stop offset="0" stop-color="#fff" stop-opacity="0"/><stop offset=".25" stop-color="#fff" stop-opacity=".7"/>` +
    `<stop offset="1" stop-color="#fff"/></linearGradient>` +
    `<mask id="gm" maskUnits="userSpaceOnUse" x="-500" y="${HY}" width="2600" height="460">` +
    `<rect x="-500" y="${HY}" width="2600" height="460" fill="url(#gh)"/></mask>` +
    `<mask id="gv" maskUnits="userSpaceOnUse" x="-500" y="${HY}" width="2600" height="460">` +
    `<rect x="-500" y="${HY}" width="2600" height="460" fill="url(#gvv)"/></mask>` +
    `<linearGradient id="fog" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${sky.low}" stop-opacity="0"/>` +
    `<stop offset=".5" stop-color="${mix(c.violet, c.magenta, 0.5)}" stop-opacity=".42"/>` +
    `<stop offset="1" stop-color="${c.deep}" stop-opacity="0"/></linearGradient>` +
    `<radialGradient id="hz"><stop offset="0" stop-color="${c.bg}" stop-opacity=".62"/>` +
    `<stop offset=".6" stop-color="${c.bg}" stop-opacity=".38"/><stop offset="1" stop-color="${c.bg}" stop-opacity="0"/></radialGradient>` +
    `<radialGradient id="mg"><stop offset=".4" stop-color="${c.cyan}" stop-opacity=".3"/>` +
    `<stop offset="1" stop-color="${c.cyan}" stop-opacity="0"/></radialGradient>` +
    `<mask id="mm" maskUnits="userSpaceOnUse" x="-40" y="-40" width="80" height="80">` +
    `<rect x="-40" y="-40" width="80" height="80" fill="#fff"/><circle cx="14" cy="-10" r="27" fill="#000"/></mask>` +
    `</defs>` +
    // Himmel und Sterne
    `<rect x="-600" y="-200" width="2800" height="${HY + 200}" fill="url(#sky)"/>` +
    `<g opacity="${sky.stars}">${stars}</g>` +
    // Rechts: Grat hinter der Sonne, Sonne mit Schein
    `<g id="R">${moon}` +
    `<g opacity="${sky.sunGlow}"><ellipse class="glow" cx="1320" cy="${sky.sunY}" rx="${R * 1.85}" ry="${R * 1.85}" fill="url(#sg)"/></g>` +
    `<g>${ridge.faces}</g><path d="${ridge.edges}" stroke="${glowPink}" stroke-width="1" opacity=".4" fill="none"/>` +
    `<path d="${ridge.ridge}" stroke="${c.magenta}" stroke-width="6" opacity=".25" fill="none"/>` +
    `<path d="${ridge.ridge}" stroke="${glowPink}" stroke-width="2" fill="none"/>` +
    `<g transform="translate(1320 ${sky.sunY})"><circle r="${R}" fill="url(#sun)" mask="url(#ss)"/></g>` +
    `</g>` +
    // Links: Drahtgitter-Berge
    `<g id="L">` +
    `<g opacity=".85">${back.faces}<path d="${back.edges}" stroke="${c.violet}" stroke-width="1" opacity=".45" fill="none"/>` +
    `<path d="${back.ridge}" stroke="${mix(c.violet, c.cyan, 0.5)}" stroke-width="1.6" opacity=".8" fill="none"/></g>` +
    `${front.faces}<path d="${front.edges}" stroke="${c.cyan}" stroke-width="1.1" opacity=".5" fill="none"/>` +
    `<path d="${front.ridge}" stroke="${c.cyan}" stroke-width="7" opacity=".18" fill="none"/>` +
    `<path d="${front.ridge}" stroke="${c.cyan}" stroke-width="2" fill="none"/>` +
    `</g>` +
    // Boden mit Gitter, Horizontlinie, Dunst am Horizont und in der Mitte
    `<rect x="-600" y="${HY}" width="2800" height="${H - HY + 100}" fill="url(#gd)"/>${grid}` +
    `<rect x="-600" y="${HY - 90}" width="2800" height="180" fill="url(#fog)"/>` +
    `<rect x="-600" y="${HY - 1}" width="2800" height="2" fill="${sky.grid}" opacity="${n(0.55 * sky.gridOpacity)}"/>` +
    `<ellipse cx="800" cy="540" rx="560" ry="560" fill="url(#hz)"/>` +
    `</svg>`
  )
}
