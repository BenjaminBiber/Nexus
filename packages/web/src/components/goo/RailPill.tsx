import { useEffect, useRef, type RefObject } from 'react';
import { prefersReducedMotion } from './goo.js';

// Einträge der eingeklappten Seitenleiste, bei denen die Leiste beim Überfahren ausbeult
export const RAIL_PILL_SOURCES = '.nav a, .global-search, .rail-toggle, .bp-field';
// Nur wo die Leiste wirklich eine Symbolspalte ist; schmaler wird sie zur Leiste oben (base.css)
export const RAIL_QUERY = '(min-width: 901px)';
// Farben übernimmt die Ausbeulung vom Eintrag: Themes färben Teile der Leiste eigens (z. B. dunkel auf hellem Inhalt)
const COLORS = ['--sidebar-bg', '--nav-active-bg', '--surface', '--text', '--text-soft', '--muted', '--line', '--line-soft', '--idle-bg', '--primary'];
// Art des Eintrags; danach richtet sich, wie seine Zeile in der Ausbeulung hervorgehoben ist (base.css)
const KINDS: Array<[string, string]> = [['.nav a', 'nav'], ['.global-search', 'search'], ['.rail-toggle', 'toggle'], ['.bp-field', 'field']];

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}
interface Point {
  x: number;
  y: number;
}

const KEYS: Array<keyof Box> = ['x', 'y', 'w', 'h'];
// Körper federt leicht über; der Tropfen läuft beim Öffnen schneller voraus und hängt beim Schließen nach, so zieht
// sich zwischen beiden ein Hals. Der Nachlauf (Zeitkonstante in 1/s) streckt die Pille beim Wechsel zum Nachbarn.
const BODY = { stiffness: 220, damping: 19 };
const DROP_OUT = { stiffness: 520, damping: 31 };
const DROP_BACK = { stiffness: 150, damping: 16 };
const TAIL_RATE = 12;
const DROP_SIZE = 0.58;
const NECK = 0.36;
const PAD = 24;
const CLOSE_DELAY = 90;
// Ausbeulung um die Zeile: oben und unten etwas Luft, rechts so viel Rand wie die offene Leiste hat
const ROW_PAD = 6;
const REACH = 12;
// Streifen an der Kante der Leiste, mit dem die Ausbeulung verschmilzt (breit und hoch genug für die Weichzeichnung)
const STRIP = 40;
const STRIP_REACH = 48;
const BLUR = 7;
// Eine Zeile ist ab diesem Anteil Bedeckung zu sehen und ab FADE_FROM + FADE_SPAN ganz
const FADE_FROM = 0.1;
const FADE_SPAN = 0.5;

const zero = (): Box => ({ x: 0, y: 0, w: 0, h: 0 });
const boxOf = (rect: DOMRect): Box => ({ x: rect.left, y: rect.top, w: rect.width, h: rect.height });
const centerOf = (box: Box): Point => ({ x: box.x + box.w / 2, y: box.y + box.h / 2 });
const px = (value: string) => parseFloat(value) || 0;

// Ausbeulung der eingeklappten Seitenleiste: Statt die Leiste aufzuklappen, wächst sie neben dem überfahrenen (oder per
// Tastatur fokussierten) Eintrag flüssig nach rechts, gerade so weit, dass der Eintrag ganz Platz hat. Die Goo-Fläche hat
// Farbe und Rand der Leiste und verschmilzt mit einem Streifen an ihrer Kante; zu sehen ist nur, was über die Kante
// hinausragt. Ein Tropfen läuft voraus, der Körper federt nach und legt dabei den Text frei; beim Wechsel zum Nachbarn
// fließt die Beule an der Kante entlang hinüber, beim Verlassen zieht sie sich in die Leiste zurück. Der Text ist eine
// Kopie des Eintrags (Klick darauf löst den Eintrag selbst aus) und liegt unter den Einträgen, deren Symbole bleiben also
// oben. Ohne Bewegung erscheint die Beule gleich in voller Breite.
export function RailPill({ rail }: { rail: RefObject<HTMLElement | null> }) {
  const root = useRef<HTMLDivElement>(null);
  const layer = useRef<HTMLSpanElement>(null);
  const strip = useRef<HTMLSpanElement>(null);
  const tail = useRef<HTMLSpanElement>(null);
  const bridge = useRef<HTMLSpanElement>(null);
  const neck = useRef<HTMLSpanElement>(null);
  const body = useRef<HTMLSpanElement>(null);
  const drop = useRef<HTMLSpanElement>(null);
  const content = useRef<HTMLAnchorElement>(null);
  const outgoing = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const aside = rail.current;
    const pill = root.current;
    const label = content.current;
    const ghost = outgoing.current;
    if (!aside || !pill || !label || !ghost) return;
    const animate = !prefersReducedMotion();
    const wide = () => typeof window.matchMedia === 'function' && window.matchMedia(RAIL_QUERY).matches;

    const m = { body: zero(), velocity: zero(), tail: zero(), drop: { x: 0, y: 0 }, dropVelocity: { x: 0, y: 0 }, frame: 0, last: 0 };
    let goal = zero();
    let dropGoal: Point = { x: 0, y: 0 };
    let labelBox = zero();
    let radius = 10;
    // Linke Seite des rechten Randes der Leiste; was links davon liegt, verdeckt die Leiste selbst
    let edge = 0;
    // Beim Wechsel zum Nachbarn bleibt der vorige Eintrag als Kopie stehen, bis der Nachlauf seine Zeile verlassen hat
    let ghostBox: Box | null = null;
    // `source`: Eintrag, zu dem die Pille gerade gehört (beim Zurückziehen keiner mehr); `open`: Pille sichtbar
    let source: HTMLElement | null = null;
    let filled: HTMLElement | null = null;
    let open = false;
    // Nach einem Klick auf einen Knopf (Suche, Board-Auswahl) bleibt die Pille zu, bis der Zeiger den Eintrag verlässt
    let suppressed: HTMLElement | null = null;
    let closeTimer = 0;
    let hovering = false;
    let watcher: MutationObserver | null = null;
    // Solange die Leiste ihre Breite ändert (Einklappen per Knopf), stimmen Kante und Lage der Einträge noch nicht: der
    // Eintrag unter dem Zeiger wartet bis zum Ende, sonst stünde seine Kopie ohne Beule neben der Leiste
    let waiting: HTMLElement | null = null;
    const resizing = () =>
      typeof aside.getAnimations === 'function' &&
      aside.getAnimations().some((a) => (a as Animation & { transitionProperty?: string }).transitionProperty === 'width' && a.playState === 'running');

    const place = (el: HTMLElement | null, box: Box | null, origin: Point) => {
      if (!el) return;
      el.style.width = `${box ? Math.max(0, box.w) : 0}px`;
      el.style.height = `${box ? Math.max(0, box.h) : 0}px`;
      el.style.transform = box ? `translate(${box.x - origin.x}px, ${box.y - origin.y}px)` : '';
    };

    const draw = () => {
      const a = m.body;
      const t = m.tail;
      const size = DROP_SIZE * a.h;
      const dropBox = { x: m.drop.x - size / 2, y: m.drop.y - size / 2, w: size, h: size };
      // Steg zwischen Körper und Nachlauf (wie GooNav), quer zur Bewegung schmaler
      const top = Math.min(a.y, t.y);
      const bottom = Math.max(a.y + a.h, t.y + t.h);
      const side = 0.3 * a.w;
      const end = 0.5 * Math.min(a.h, t.h);
      const bridgeBox = { x: a.x + side, y: top + end, w: a.w - 2 * side, h: Math.max(0, bottom - top - 2 * end) };
      // Hals vom rechten Rand des Körpers zum Tropfen, solange der draußen ist
      const front = a.x + a.w;
      const neckH = NECK * a.h;
      const neckBox = m.drop.x > front - 6 ? { x: front - 12, y: (a.y + a.h / 2 + m.drop.y) / 2 - neckH / 2, w: m.drop.x - front + 12, h: neckH } : null;
      // Streifen an der Kante der Leiste, über die ganze Höhe der Beule und ein Stück darüber hinaus: mit ihm verschmilzt
      // die Beule, so entstehen die weichen Kehlen zur Kante. Sein Rand fällt auf den der Leiste.
      const high = Math.min(a.y, t.y, dropBox.y);
      const low = Math.max(a.y + a.h, t.y + t.h, dropBox.y + dropBox.h);
      const stripBox = { x: edge - STRIP, y: high - STRIP_REACH, w: STRIP - 0.5, h: low - high + 2 * STRIP_REACH };
      const shapes = [a, t, dropBox, stripBox, ...(neckBox ? [neckBox] : [])];
      const left = Math.min(...shapes.map((s) => s.x)) - PAD;
      const upper = Math.min(...shapes.map((s) => s.y)) - PAD;
      const right = Math.max(...shapes.map((s) => s.x + s.w)) + PAD;
      const lower = Math.max(...shapes.map((s) => s.y + s.h)) + PAD;
      if (layer.current) {
        const style = layer.current.style;
        style.left = `${left}px`;
        style.top = `${upper}px`;
        style.width = `${right - left}px`;
        style.height = `${lower - upper}px`;
        // Sichtbar nur jenseits der Kante; in der Leiste zeigt sich die Zeile über die Kopie des Eintrags
        style.clipPath = `inset(0 0 0 ${edge - left}px)`;
      }
      const origin = { x: left, y: upper };
      place(strip.current, stripBox, origin);
      place(body.current, a, origin);
      place(tail.current, t, origin);
      place(bridge.current, bridgeBox, origin);
      place(neck.current, neckBox, origin);
      place(drop.current, dropBox, origin);
      // Der Text steht nur, wo schon Fläche ist: der neue im Körper, der vorige im Nachlauf, der ihn mitnimmt
      // Schmale Reste einer Zeile (beim Wechsel) blenden mit aus, statt als Streifen stehen zu bleiben
      label.style.clipPath = reveal(labelBox, a);
      label.style.opacity = fade(covered(labelBox, a));
      if (ghostBox) {
        const g = ghostBox;
        const share = covered(g, t);
        if (share <= FADE_FROM || t.x + t.w <= g.x) dropGhost();
        else {
          ghost.style.clipPath = reveal(g, t);
          ghost.style.opacity = fade(share);
        }
      }
    };

    // Beschnitt einer Zeile auf die Zeile einer Fläche (ohne deren Luft oben und unten)
    const reveal = (p: Box, f: Box) => {
      const top = f.y + ROW_PAD;
      const bottom = f.y + f.h - ROW_PAD;
      const inset = [top - p.y, p.x + p.w - (f.x + f.w), p.y + p.h - bottom, f.x - p.x].map((v) => `${Math.max(0, v)}px`);
      return `inset(${inset.join(' ')} round ${radius}px)`;
    };
    // Anteil der Zeile, den die Zeile der Fläche in der Höhe bedeckt
    const covered = (p: Box, f: Box) => {
      const top = Math.max(p.y, f.y + ROW_PAD);
      const bottom = Math.min(p.y + p.h, f.y + f.h - ROW_PAD);
      return p.h > 0 ? Math.max(0, bottom - top) / p.h : 1;
    };
    const fade = (share: number) => `${Math.min(1, Math.max(0, (share - FADE_FROM) / FADE_SPAN))}`;

    const dropGhost = () => {
      ghostBox = null;
      ghost.hidden = true;
      ghost.replaceChildren();
    };

    const settle = () => {
      if (source || !open) return;
      open = false;
      pill.classList.remove('is-open');
      dropGhost();
    };

    const step = (now: number) => {
      const dt = Math.min(0.032, (now - m.last) / 1000);
      m.last = now;
      let moving = false;
      for (const key of KEYS) {
        const accel = BODY.stiffness * (goal[key] - m.body[key]) - BODY.damping * m.velocity[key];
        m.velocity[key] += accel * dt;
        m.body[key] += m.velocity[key] * dt;
        if (Math.abs(goal[key] - m.body[key]) > 0.3 || Math.abs(m.velocity[key]) > 2) moving = true;
      }
      const follow = 1 - Math.exp(-TAIL_RATE * dt);
      for (const key of ['x', 'y', 'h'] as const) {
        m.tail[key] += (m.body[key] - m.tail[key]) * follow;
        if (Math.abs(m.body[key] - m.tail[key]) > 0.3) moving = true;
      }
      m.tail.w = m.body.w;
      const spring = source ? DROP_OUT : DROP_BACK;
      for (const key of ['x', 'y'] as const) {
        const accel = spring.stiffness * (dropGoal[key] - m.drop[key]) - spring.damping * m.dropVelocity[key];
        m.dropVelocity[key] += accel * dt;
        m.drop[key] += m.dropVelocity[key] * dt;
        if (Math.abs(dropGoal[key] - m.drop[key]) > 0.3 || Math.abs(m.dropVelocity[key]) > 2) moving = true;
      }
      if (!moving) snap();
      draw();
      m.frame = moving ? requestAnimationFrame(step) : 0;
      // Beim Zurückziehen blendet die Beule aus, sobald Körper und Tropfen wieder in der Leiste sind; das letzte
      // Nachfedern muss niemand mehr sehen
      const home = m.body.x + m.body.w <= edge + 2 && m.drop.x + (DROP_SIZE * m.body.h) / 2 <= edge;
      if (!moving || home) settle();
    };

    const snap = () => {
      m.body = { ...goal };
      m.tail = { ...goal };
      m.velocity = zero();
      m.drop = { ...dropGoal };
      m.dropVelocity = { x: 0, y: 0 };
    };

    const run = () => {
      if (!animate) {
        snap();
        draw();
        settle();
        return;
      }
      if (!m.frame) {
        m.last = performance.now();
        m.frame = requestAnimationFrame(step);
      }
    };

    // Ruhelage der Beule ganz in der Leiste: so breit wie der Eintrag, oben und unten mit Luft
    const inside = (rect: Box): Box => ({ x: rect.x, y: rect.y - ROW_PAD, w: rect.w, h: rect.h + 2 * ROW_PAD });

    // Kopie des Eintrags in die Beule: gleiche Abstände und Schrift, damit das Symbol der Kopie genau unter dem echten
    // liegt und der Text daneben steht; die Breite ergibt sich aus dem Inhalt
    const fill = (el: HTMLElement) => {
      const rect = el.getBoundingClientRect();
      const style = getComputedStyle(el);
      const frame = aside.getBoundingClientRect();
      edge = frame.right - px(getComputedStyle(aside).borderRightWidth);
      label.dataset.kind = KINDS.find(([selector]) => el.matches(selector))?.[1] ?? '';
      for (const name of COLORS) {
        const value = style.getPropertyValue(name).trim();
        if (value) pill.style.setProperty(name, value);
        else pill.style.removeProperty(name);
      }
      if (style.colorScheme) pill.style.colorScheme = style.colorScheme;
      label.replaceChildren(...Array.from(el.childNodes, (node) => node.cloneNode(true)));
      for (const node of label.querySelectorAll('[id], [title], a, button, input')) {
        node.removeAttribute('id');
        node.removeAttribute('title');
        if (node.matches('a, button, input')) node.setAttribute('tabindex', '-1');
      }
      // Das erste Kind der Kopie beginnt dort, wo es im Eintrag steht, auch wenn der Eintrag seinen Inhalt mittig
      // setzt (Board-Auswahl mit Kürzel)
      const first = el.firstElementChild;
      const lead = first
        ? first.getBoundingClientRect().left - rect.left - px(getComputedStyle(first).marginLeft)
        : px(style.paddingLeft) + px(style.borderLeftWidth);
      const href = el instanceof HTMLAnchorElement ? el.getAttribute('href') : null;
      if (href) label.setAttribute('href', href);
      else label.removeAttribute('href');
      label.classList.toggle('is-active', el.classList.contains('active'));
      // Der Fuß der Leiste (Umschalter, Nutzung) liegt mit eigener Fläche über dem, was dahinter scrollt; dort muss die
      // Pille darüber, das Symbol zeigt dann ihre Kopie
      pill.classList.toggle('is-raised', !!el.closest('.usage-dock'));
      Object.assign(label.style, {
        left: `${rect.left}px`,
        top: `${rect.top}px`,
        height: `${rect.height}px`,
        width: 'max-content',
        paddingLeft: `${Math.max(0, lead)}px`,
        paddingRight: `${Math.max(14, px(style.paddingRight)) + px(style.borderRightWidth)}px`,
        columnGap: style.columnGap,
        alignItems: style.alignItems,
        fontFamily: style.fontFamily,
        fontSize: style.fontSize,
        fontWeight: style.fontWeight,
        letterSpacing: style.letterSpacing,
      });
      radius = px(style.borderTopLeftRadius) || radius;
      pill.style.setProperty('--rail-item-radius', `${radius}px`);
      pill.style.setProperty('--rail-pill-radius', `${radius + ROW_PAD}px`);
      const width = Math.max(rect.width, Math.min(label.getBoundingClientRect().width, window.innerWidth - rect.left - 12 - REACH));
      label.style.width = `${width}px`;
      labelBox = { x: rect.left, y: rect.top, w: width, h: rect.height };
      filled = el;
      goal = { x: rect.left, y: rect.top - ROW_PAD, w: width + REACH, h: rect.height + 2 * ROW_PAD };
      dropGoal = { x: goal.x + goal.w - goal.h / 2, y: goal.y + goal.h / 2 };
      return boxOf(rect);
    };

    // Ändert sich der Eintrag, solange die Pille offen ist (Zähler, aktiv geworden), zieht die Kopie nach
    const watch = (el: HTMLElement | null) => {
      watcher?.disconnect();
      watcher = null;
      if (!el || typeof MutationObserver !== 'function') return;
      watcher = new MutationObserver(() => {
        if (source !== el) return;
        fill(el);
        run();
      });
      watcher.observe(el, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: ['class'] });
    };

    const show = (el: HTMLElement) => {
      clearTimeout(closeTimer);
      if (el === source) return;
      if (resizing()) {
        waiting = el;
        return;
      }
      waiting = null;
      source?.removeAttribute('data-rail-pill');
      source = el;
      el.setAttribute('data-rail-pill', '');
      if (open && filled && filled !== el && animate) {
        // Der vorige Eintrag fließt mit dem Nachlauf ab, statt schlagartig zu verschwinden
        ghost.replaceChildren(...Array.from(label.childNodes));
        ghost.style.cssText = label.style.cssText;
        ghost.classList.toggle('is-active', label.classList.contains('is-active'));
        ghost.dataset.kind = label.dataset.kind ?? '';
        ghost.hidden = false;
        ghostBox = { ...labelBox };
      }
      const rect = fill(el);
      watch(el);
      if (!open) {
        // Aus der Leiste heraus: die Fläche beginnt unter dem Eintrag, der Tropfen in seiner Mitte
        m.body = inside(rect);
        m.tail = inside(rect);
        m.velocity = zero();
        m.drop = centerOf(rect);
        m.dropVelocity = { x: 0, y: 0 };
        open = true;
        pill.classList.add('is-open');
        draw();
      }
      run();
    };

    const hide = () => {
      clearTimeout(closeTimer);
      waiting = null;
      if (!source) return;
      const el = source;
      el.removeAttribute('data-rail-pill');
      source = null;
      watch(null);
      const rect = boxOf(el.getBoundingClientRect());
      goal = inside(rect);
      dropGoal = centerOf(rect);
      run();
    };

    const hideLater = () => {
      clearTimeout(closeTimer);
      closeTimer = window.setTimeout(hide, CLOSE_DELAY);
    };

    // Sofort weg, ohne Rückzug: Leiste gescrollt oder Fenster verändert, die Lage stimmt nicht mehr
    const reset = () => {
      clearTimeout(closeTimer);
      waiting = null;
      source?.removeAttribute('data-rail-pill');
      source = null;
      watch(null);
      if (m.frame) cancelAnimationFrame(m.frame);
      m.frame = 0;
      open = false;
      pill.classList.remove('is-open');
      dropGhost();
    };

    const sourceOf = (target: EventTarget | null) => {
      const el = target instanceof Element ? target.closest<HTMLElement>(RAIL_PILL_SOURCES) : null;
      return el && aside.contains(el) && !pill.contains(el) ? el : null;
    };
    const onPill = (target: EventTarget | null) => target instanceof Node && pill.contains(target);

    const onOver = (event: Event) => {
      hovering = true;
      if (onPill(event.target)) return clearTimeout(closeTimer);
      const el = sourceOf(event.target);
      if (!el || !wide()) return hideLater();
      if (el === suppressed) return;
      suppressed = null;
      show(el);
    };
    const onLeave = () => {
      hovering = false;
      suppressed = null;
      hideLater();
    };
    const onFocusIn = (event: FocusEvent) => {
      const el = sourceOf(event.target);
      if (el && wide() && focusVisible(event.target)) show(el);
    };
    const onFocusOut = () => {
      if (!hovering) hideLater();
    };
    // Klick auf den Eintrag selbst: Knöpfe öffnen etwas (Suche, Board-Auswahl), dann zieht sich die Pille zurück
    const onClick = (event: MouseEvent) => {
      if (onPill(event.target)) return;
      const el = sourceOf(event.target);
      if (el && el === source && !(el instanceof HTMLAnchorElement)) {
        suppressed = el;
        hide();
      }
    };
    // Klick auf die Kopie: an das entsprechende Element des Eintrags weiterreichen. Mit Strg/Mittelklick öffnet die
    // Kopie eines Links (gleiches href) selbst einen neuen Tab.
    const onLabelClick = (event: MouseEvent) => {
      const el = source ?? filled;
      if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      if (!el) return;
      const target = counterpart(event.target, label, el);
      if (!(el instanceof HTMLAnchorElement)) {
        suppressed = el;
        hide();
      }
      target?.click();
    };

    // Leiste hat ihre Breite erreicht: der wartende Eintrag (noch unter dem Zeiger) bekommt jetzt seine Beule
    const onResized = (event: TransitionEvent) => {
      if (event.target !== aside || event.propertyName !== 'width' || !waiting) return;
      const el = waiting;
      waiting = null;
      if (el.isConnected && (hovering || el.contains(document.activeElement)) && wide()) show(el);
    };

    aside.addEventListener('transitionend', onResized);
    aside.addEventListener('transitioncancel', onResized);
    aside.addEventListener('pointerover', onOver);
    aside.addEventListener('pointerleave', onLeave);
    aside.addEventListener('focusin', onFocusIn);
    aside.addEventListener('focusout', onFocusOut);
    aside.addEventListener('click', onClick);
    aside.addEventListener('scroll', reset, { passive: true });
    label.addEventListener('click', onLabelClick);
    window.addEventListener('resize', reset);
    return () => {
      reset();
      aside.removeEventListener('transitionend', onResized);
      aside.removeEventListener('transitioncancel', onResized);
      aside.removeEventListener('pointerover', onOver);
      aside.removeEventListener('pointerleave', onLeave);
      aside.removeEventListener('focusin', onFocusIn);
      aside.removeEventListener('focusout', onFocusOut);
      aside.removeEventListener('click', onClick);
      aside.removeEventListener('scroll', reset);
      label.removeEventListener('click', onLabelClick);
      window.removeEventListener('resize', reset);
    };
  }, [rail]);

  return (
    <div ref={root} className="rail-pill" aria-hidden="true">
      <span ref={layer} className="rail-pill-goo">
        {/* Goo wie GooFilter, dazu ein ein Pixel breiter Rand in der Farbe der Leistenkante um die verschmolzene Form */}
        <svg className="goo-defs" width="0" height="0" aria-hidden="true" focusable="false">
          <defs>
            <filter id="goo-rail" colorInterpolationFilters="sRGB">
              <feGaussianBlur in="SourceGraphic" stdDeviation={BLUR} result="blur" />
              <feColorMatrix in="blur" type="matrix" values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 22 -10" result="goo" />
              <feMorphology in="goo" operator="dilate" radius="1" result="grown" />
              <feFlood className="rail-pill-ink" result="ink" />
              <feComposite in="ink" in2="grown" operator="in" result="outline" />
              <feMerge>
                <feMergeNode in="outline" />
                <feMergeNode in="goo" />
              </feMerge>
            </filter>
          </defs>
        </svg>
        <span ref={strip} className="rail-pill-blob is-strip" />
        <span ref={tail} className="rail-pill-blob" />
        <span ref={bridge} className="rail-pill-blob" />
        <span ref={neck} className="rail-pill-blob" />
        <span ref={body} className="rail-pill-blob" />
        <span ref={drop} className="rail-pill-blob is-drop" />
      </span>
      <span ref={outgoing} className="rail-pill-content is-ghost" hidden />
      <a ref={content} className="rail-pill-content" tabIndex={-1} />
    </div>
  );
}

function focusVisible(target: EventTarget | null) {
  try {
    return target instanceof Element && target.matches(':focus-visible');
  } catch {
    return false;
  }
}

// Gegenstück eines Knotens der Kopie im Eintrag (gleicher Pfad); geklickt wird der Link oder Knopf darum, sonst der
// Eintrag selbst bzw. sein erster Knopf (Board-Auswahl: die Auswahl)
function counterpart(target: EventTarget | null, copy: Element, source: HTMLElement): HTMLElement | null {
  const path: number[] = [];
  let node = target instanceof Node ? target : null;
  while (node && node !== copy) {
    const parent: ParentNode | null = node.parentNode;
    if (!parent) return null;
    path.unshift(Array.prototype.indexOf.call(parent.childNodes, node));
    node = parent as Node;
  }
  let match: Node | null = source;
  for (const index of path) match = match?.childNodes[index] ?? null;
  const element = match instanceof Element ? match : (match?.parentElement ?? null);
  const control = element?.closest<HTMLElement>('a, button');
  if (control && source.contains(control)) return control;
  return source.matches('a, button') ? source : source.querySelector<HTMLElement>('a[href], button:not(:disabled)');
}
