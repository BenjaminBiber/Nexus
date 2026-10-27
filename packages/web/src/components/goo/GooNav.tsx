import { useLayoutEffect, useRef, useState, type ComponentProps } from 'react';
import { useLocation } from 'react-router-dom';
import { GooFilter } from './GooFilter.js';
import { prefersReducedMotion } from './goo.js';

interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

const KEYS: Array<keyof Box> = ['x', 'y', 'w', 'h'];
// Feder des Kopfes (leicht überschwingend) und Nachlauf des Schwanzes (Zeitkonstante in 1/s)
const STIFFNESS = 300;
const DAMPING = 24;
const TAIL_RATE = 11;

// Navigation mit Goo-Markierung: Der aktive Eintrag wird nicht an den Links gezeichnet, sondern von Flächen unter ihnen
// (Kopf, Schwanz und ein schmaler Steg dazwischen, durch den Goo-Filter verbunden). Beim Seitenwechsel springt der Kopf per
// Feder zum neuen Eintrag, der Schwanz läuft nach: die Markierung zieht sich zu einem Faden und fließt hinüber. Ohne Bewegung
// (oder wenn der Nutzer sie abwählt) bleibt die gewöhnliche Hervorhebung am Link. Die Links liegen über der Ebene.
export function GooNav({ children, className, ...props }: ComponentProps<'nav'>) {
  const nav = useRef<HTMLElement>(null);
  const head = useRef<HTMLSpanElement>(null);
  const tail = useRef<HTMLSpanElement>(null);
  const bridge = useRef<HTMLSpanElement>(null);
  const target = useRef<Box | null>(null);
  const location = useLocation();
  const [goo] = useState(() => !prefersReducedMotion());

  // Zustand der Animation; lebt über Seitenwechsel hinweg, nur die Zielbox ändert sich
  const motion = useRef<{ head: Box; tail: Box; velocity: Box; frame: number; last: number } | null>(null);

  useLayoutEffect(() => {
    if (!goo) return;
    const blobs = [head.current, tail.current, bridge.current];
    const draw = () => {
      const m = motion.current;
      if (!m) return;
      const { head: a, tail: b } = m;
      const set = (el: HTMLElement | null, box: Box) => {
        if (!el) return;
        el.style.width = `${Math.max(0, box.w)}px`;
        el.style.height = `${Math.max(0, box.h)}px`;
        el.style.transform = `translate(${box.x}px, ${box.y}px)`;
      };
      set(head.current, a);
      set(tail.current, b);
      // Steg: das Rechteck zwischen beiden, quer zur Bewegung schmal; bei ruhender Markierung liegt es unter ihr
      const left = Math.min(a.x, b.x);
      const top = Math.min(a.y, b.y);
      const right = Math.max(a.x + a.w, b.x + b.w);
      const bottom = Math.max(a.y + a.h, b.y + b.h);
      const vertical = bottom - top - Math.min(a.h, b.h) >= right - left - Math.min(a.w, b.w);
      const side = 0.3 * Math.min(a.w, b.w);
      const end = 0.5 * Math.min(a.h, b.h);
      set(
        bridge.current,
        vertical
          ? { x: left + side, y: top + end, w: right - left - 2 * side, h: bottom - top - 2 * end }
          : { x: left + end, y: top + side, w: right - left - 2 * end, h: bottom - top - 2 * side },
      );
    };
    const step = (now: number) => {
      const m = motion.current;
      const goal = target.current;
      if (!m || !goal) return;
      const dt = Math.min(0.032, (now - m.last) / 1000);
      m.last = now;
      let moving = false;
      for (const key of KEYS) {
        const accel = STIFFNESS * (goal[key] - m.head[key]) - DAMPING * m.velocity[key];
        m.velocity[key] += accel * dt;
        m.head[key] += m.velocity[key] * dt;
        m.tail[key] += (m.head[key] - m.tail[key]) * (1 - Math.exp(-TAIL_RATE * dt));
        if (Math.abs(goal[key] - m.head[key]) > 0.3 || Math.abs(m.velocity[key]) > 2 || Math.abs(m.head[key] - m.tail[key]) > 0.3) moving = true;
      }
      if (!moving) {
        for (const key of KEYS) {
          m.head[key] = goal[key];
          m.tail[key] = goal[key];
          m.velocity[key] = 0;
        }
      }
      draw();
      m.frame = moving ? requestAnimationFrame(step) : 0;
    };
    // `animate`: nur beim Seitenwechsel federt der Kopf; ändert sich das Layout, rastet die Markierung sofort ein
    const place = (animate: boolean) => {
      const link = nav.current?.querySelector<HTMLElement>('a.active');
      for (const el of blobs) if (el) el.style.opacity = link ? '1' : '0';
      if (!link) {
        target.current = null;
        return;
      }
      const goal = { x: link.offsetLeft, y: link.offsetTop, w: link.offsetWidth, h: link.offsetHeight };
      target.current = goal;
      let m = motion.current;
      if (!m) {
        m = motion.current = { head: { ...goal }, tail: { ...goal }, velocity: { x: 0, y: 0, w: 0, h: 0 }, frame: 0, last: performance.now() };
        draw();
      }
      if (!animate) {
        if (m.frame) cancelAnimationFrame(m.frame);
        m.frame = 0;
        for (const key of KEYS) {
          m.head[key] = goal[key];
          m.tail[key] = goal[key];
          m.velocity[key] = 0;
        }
        draw();
      } else if (!m.frame) {
        m.last = performance.now();
        m.frame = requestAnimationFrame(step);
      }
    };
    place(true);
    // Ändert sich das Layout (Fenstergröße, ein Eintrag erscheint oder verschwindet), liegt die Markierung gleich richtig
    const relayout = () => place(false);
    // Ein ResizeObserver meldet beim Start sofort einmal; das ist keine Änderung und darf den Seitenwechsel nicht abbrechen
    let measured = false;
    const observer =
      typeof ResizeObserver === 'function' && nav.current
        ? new ResizeObserver(() => {
            if (measured) relayout();
            measured = true;
          })
        : null;
    if (observer && nav.current) observer.observe(nav.current);
    const mutations = typeof MutationObserver === 'function' && nav.current ? new MutationObserver(relayout) : null;
    if (mutations && nav.current) mutations.observe(nav.current, { childList: true, subtree: true });
    window.addEventListener('resize', relayout);
    return () => {
      observer?.disconnect();
      mutations?.disconnect();
      window.removeEventListener('resize', relayout);
      if (motion.current?.frame) cancelAnimationFrame(motion.current.frame);
      if (motion.current) motion.current.frame = 0;
    };
  }, [goo, location.pathname]);

  return (
    <nav ref={nav} className={`${className ?? ''}${goo ? ' goo-nav' : ''}`} {...props}>
      {goo && (
        <span className="goo-nav-layer" aria-hidden="true">
          <GooFilter id="goo-nav" blur={7} />
          <span ref={bridge} className="goo-nav-blob" />
          <span ref={tail} className="goo-nav-blob" />
          <span ref={head} className="goo-nav-blob" />
        </span>
      )}
      {children}
    </nav>
  );
}
