// Szene „Terminal“: fast schwarzer Grund der Palette mit leisem Phosphorglühen, gedimmte Protokollzeilen ziehen am
// rechten Rand langsam nach oben, oben links blinkt ein Block-Cursor. Mitte (Gesprächstext) und Fuß (Eingabefeld)
// bleiben ruhig. `crt` legt Scanlines, Vignette und ein leichtes Flimmern darüber.
//
// Bildschirmformate: Die Wurzel schneidet mit `xMidYMid slice` eine 1600 × 1000-Fläche zu. Wie viel davon sichtbar
// bleibt, hängt vom Seitenverhältnis a der Szenenbox ab (schmal: x von 800 − 500a bis 800 + 500a, breit: y von
// 500 − 800/a bis 500 + 800/a). Media-Regeln im <style> schieben die Logspalte an den sichtbaren rechten Rand, den
// Cursor in die sichtbare linke obere Ecke und die Vignette auf den sichtbaren Ausschnitt. Animiert wird nur über
// CSS; `prefers-reduced-motion` hält alles an.

/** Farben, die die Szene braucht; rohe Werte aus theme.css */
export type SceneColors = {
  bg: string
  fg: string
  muted: string
  accent: string
  ok: string
  warn: string
  hex: string
  path: string
}

const W = 1600
const H = 1000
/** Breite der Logspalte in Einheiten der Wurzel */
const COL = 230
const LINE = 21
const LINES = 48
const BLOCK = LINE * LINES

function esc(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
}

/** Kleiner fester Zufall (mulberry32), damit jede Variante dieselben Zeilen zeigt */
function random(seed: number): () => number {
  let a = seed >>> 0
  return () => {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const COMMANDS = ['make test', 'git pull', 'ls -la', 'cat notes.txt', 'npm run build', 'tail -n 40 app.log', 'cd src', 'df -h', 'top -n 1', 'history | wc -l', 'git log -3', 'du -sh .']

/** Protokollzeilen der Spalte, je Zeile ein <text> mit Spalten über tspan x (kein Leerraum, der zusammenfällt) */
function logLines(c: SceneColors): string {
  const rnd = random(0x7e41)
  const pick = (list: readonly string[]): string => list[Math.floor(rnd() * list.length)]!
  const hex = (digits: number) => Array.from({ length: digits }, () => Math.floor(rnd() * 16).toString(16)).join('')
  const num = (lo: number, hi: number) => lo + Math.floor(rnd() * (hi - lo + 1))
  const messages = [
    () => `fetch ${hex(6)}`,
    () => `sync ${num(12, 480)}`,
    () => 'cache hit',
    () => `build ${num(1, 9)}.${num(0, 9)}s`,
    () => { const n = num(20, 96); return `tests ${n}/${n}` },
    () => `write ${num(1, 64)} kB`,
    () => 'lint clean',
    () => 'watch src',
    () => `load 0.${num(10, 89)}`,
    () => `gc ${num(1, 9)} ms`,
    () => `mem ${num(120, 640)} MiB`,
    () => `commit ${hex(6)}`,
    () => 'idle',
    () => `ping ${num(4, 40)} ms`,
    () => 'ready',
  ]
  let seconds = 9 * 3600 + 41 * 60 + 7
  const out: string[] = []
  for (let i = 0; i < LINES; i++) {
    const y = (i + 1) * LINE
    const dim = ['', ' class="d1"', ' class="d2"'][i % 3]
    const roll = rnd()
    if (roll < 0.15) {
      out.push(`<text y="${y}"${dim}><tspan x="8" fill="${c.accent}">$</tspan><tspan x="22" fill="${c.fg}">${esc(pick(COMMANDS))}</tspan></text>`)
      continue
    }
    if (roll < 0.32) {
      const bytes = Array.from({ length: 5 }, () => hex(2)).join(' ')
      out.push(`<text y="${y}"${dim}><tspan x="8" fill="${c.hex}">0x${hex(4)}</tspan><tspan x="60">${bytes}</tspan></text>`)
      continue
    }
    seconds += num(0, 4)
    const time = [Math.floor(seconds / 3600), Math.floor(seconds / 60) % 60, seconds % 60].map((n) => String(n).padStart(2, '0')).join(':')
    const level = rnd()
    const [word, color] = level < 0.6 ? ['ok', c.ok] : level < 0.85 ? ['info', c.path] : level < 0.95 ? ['..', c.muted] : ['warn', c.warn]
    out.push(`<text y="${y}"${dim}><tspan x="8">${time}</tspan><tspan x="72" fill="${color}">${word}</tspan><tspan x="110">${esc(messages[Math.floor(rnd() * messages.length)]!())}</tspan></text>`)
  }
  return out.join('')
}

/**
 * Lage eines Seitenverhältnis-Bereichs: sichtbarer rechter Rand R, linker Rand L, oberer Rand T (Wurzeleinheiten);
 * `k` verkleinert die Logspalte um ihren rechten Rand und die Bildmitte
 */
function place(r: number, l: number, t: number, k = 1): string {
  const sx = ((r - l) / W).toFixed(4)
  const sy = ((H - 2 * t) / H).toFixed(4)
  const pr = k === 1 ? `translate(${r - W}px,0)` : `translate(${r - k * W}px,${(H / 2) * (1 - k)}px) scale(${k})`
  return `.pr{transform:${pr}}.pt{transform:translate(${l}px,${t}px)}.fit{transform:translate(${l}px,${t}px) scale(${sx},${sy})}`
}

function style(crt: boolean): string {
  return [
    '.logs{opacity:.42}.d1{opacity:.7}.d2{opacity:.45}',
    `.scroll{animation:up 160s linear infinite}@keyframes up{to{transform:translateY(-${BLOCK}px)}}`,
    '.cur{animation:blink 1.1s steps(1,end) infinite}@keyframes blink{50%{opacity:0}}',
    crt
      ? '.flick{opacity:0;animation:flick 7s steps(1,end) infinite}@keyframes flick{0%{opacity:0}31%{opacity:.035}32%{opacity:0}67%{opacity:.025}68%{opacity:0}69%{opacity:.04}70%{opacity:0}}'
      : '',
    placements(),
    // Hochkant liegt die Spalte zwangsläufig neben dem Text: dort leiser
    '@media (max-aspect-ratio:1/1){.logs{opacity:.26}}',
    '@media (prefers-reduced-motion:reduce){.scroll,.cur,.flick{animation:none}}',
  ].join('')
}

/**
 * Media-Regeln in Stufen von 0,05 (schmal) bzw. 0,1 (breit) im Seitenverhältnis. Je Stufe gilt der Rand, der für
 * jedes a der Stufe noch sichtbar ist; spätere Regeln überschreiben frühere.
 */
function placements(): string {
  const rules: string[] = []
  const right = (a: number) => Math.min(W, W / 2 + (H / 2) * a)
  const top = (a: number) => Math.max(0, H / 2 - W / 2 / a)
  // Grundlage: a zwischen 1,6 und 1,7
  rules.push(place(W, 0, Math.ceil(top(1.7))))
  // Breiter: oben wird mehr abgeschnitten
  const wide = [1.7, 1.8, 1.9, 2.0, 2.1, 2.2, 2.4, 2.6, 2.9, 3.2]
  wide.forEach((a, i) => {
    const next = wide[i + 1] ?? 3.6
    rules.push(`@media (min-aspect-ratio:${Math.round(a * 100)}/100){${place(W, 0, Math.ceil(top(next)))}}`)
  })
  // Schmaler: links und rechts wird abgeschnitten; die Schwelle knapp unter a, damit genau a noch zur breiteren Stufe zählt
  for (let step = 32; step >= 9; step--) {
    const a = step / 20
    const lo = (step - 1) / 20
    const r = Math.floor(right(lo))
    // Hochkant kleiner, sonst nimmt die Spalte die halbe Breite ein
    rules.push(`@media (max-aspect-ratio:${Math.round(a * 1000) - 1}/1000){${place(r, W - r, 0, a <= 1 ? 0.8 : 1)}}`)
  }
  return rules.join('')
}

/** Das SVG der Szene für eine Palette; `crt` mit Scanlines, Vignette und Flimmern */
export function sceneFor(c: SceneColors, crt: boolean): string {
  const lines = logLines(c)
  const defs = [
    `<radialGradient id="glow" cx="50%" cy="42%" r="70%"><stop offset="0" stop-color="${c.accent}" stop-opacity=".075"/><stop offset=".6" stop-color="${c.accent}" stop-opacity=".02"/><stop offset="1" stop-color="${c.accent}" stop-opacity="0"/></radialGradient>`,
    `<radialGradient id="cglow"><stop offset="0" stop-color="${c.accent}" stop-opacity=".16"/><stop offset="1" stop-color="${c.accent}" stop-opacity="0"/></radialGradient>`,
    '<linearGradient id="fade" x1="0" y1="0" x2="0" y2="1"><stop offset=".04" stop-color="#fff" stop-opacity="0"/><stop offset=".18" stop-color="#fff"/><stop offset=".58" stop-color="#fff"/><stop offset=".8" stop-color="#fff" stop-opacity="0"/></linearGradient>',
    `<mask id="colmask" maskUnits="userSpaceOnUse" x="0" y="0" width="${COL}" height="${H}"><rect width="${COL}" height="${H}" fill="url(#fade)"/></mask>`,
    crt
      ? '<pattern id="scan" width="4" height="4" patternUnits="userSpaceOnUse"><rect width="4" height="1.6" fill="#000"/></pattern>' +
        '<radialGradient id="vig" gradientUnits="userSpaceOnUse" cx="800" cy="500" r="1152" gradientTransform="translate(0 500) scale(1 .625) translate(0 -500)"><stop offset=".55" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#000" stop-opacity=".62"/></radialGradient>'
      : '',
  ].join('')

  const column =
    `<g class="pr"><svg x="${W - COL - 16}" y="0" width="${COL}" height="${H}" viewBox="0 0 ${COL} ${H}" preserveAspectRatio="xMaxYMid meet">` +
    `<g class="logs" mask="url(#colmask)" fill="${c.muted}" font-family="ui-monospace,Menlo,Consolas,monospace" font-size="12">` +
    `<g class="scroll">${lines}<g transform="translate(0 ${BLOCK})">${lines}</g></g></g></svg></g>`

  const cursor =
    `<g class="pt"><circle cx="118" cy="128" r="70" fill="url(#cglow)"/>` +
    `<text x="64" y="136" fill="${c.muted}" opacity=".6" font-family="ui-monospace,Menlo,Consolas,monospace" font-size="20">~ $</text>` +
    `<rect class="cur" x="112" y="118" width="12" height="23" fill="${c.fg}" opacity=".85"/></g>`

  const tube = crt
    ? `<rect width="${W}" height="${H}" fill="url(#scan)" opacity=".28"/>` +
      `<rect class="fit" x="-${W}" y="-${H}" width="${3 * W}" height="${3 * H}" fill="url(#vig)"/>` +
      `<rect class="flick" width="${W}" height="${H}" fill="${c.accent}"/>`
    : ''

  return (
    `<svg viewBox="0 0 ${W} ${H}" preserveAspectRatio="xMidYMid slice" xmlns="http://www.w3.org/2000/svg">` +
    `<style>${style(crt)}</style><defs>${defs}</defs>` +
    `<rect width="${W}" height="${H}" fill="${c.bg}"/><rect width="${W}" height="${H}" fill="url(#glow)"/>` +
    column +
    cursor +
    tube +
    '</svg>'
  )
}
