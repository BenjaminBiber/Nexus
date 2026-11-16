import { useLayoutEffect, useRef } from 'react';
import type { NavIconName } from '@nexus/theme-sdk';
import { NAV_ICONS } from './defs.js';
import { go, jump, stop, type Els, type MotionInst } from './motion.js';

// Linien-Symbol der Navigation (18 px, currentColor, Strich 1.75 wie LineIcon), das sich beim Überfahren oder bei
// Tastaturfokus des umgebenden Links bewegt. Berührungen lösen nichts aus, damit auf Touch nichts hängen bleibt
export function AnimatedNavIcon({ name }: { name: NavIconName }) {
  const ref = useRef<SVGSVGElement>(null);
  const def = NAV_ICONS[name];

  useLayoutEffect(() => {
    const svg = ref.current;
    if (!svg) return;
    const e: Els = {};
    svg.querySelectorAll<SVGElement>('[data-n]').forEach((el) => {
      if (el.dataset.n) e[el.dataset.n] = el;
    });
    const inst: MotionInst = { motion: def.motion, e, cur: {}, raf: 0, hover: false };
    jump(inst);

    const host = svg.closest('a, button') ?? svg.parentElement;
    if (!host) return () => stop(inst);
    let pointer = false;
    let focus = false;
    const sync = () => {
      const hover = pointer || focus;
      if (hover === inst.hover) return;
      inst.hover = hover;
      go(inst);
    };
    const onEnter = (ev: Event) => {
      if ((ev as PointerEvent).pointerType === 'touch') return;
      pointer = true;
      sync();
    };
    const onLeave = () => {
      pointer = false;
      sync();
    };
    const onFocusIn = () => {
      try {
        focus = host.matches(':focus-visible');
      } catch {
        focus = false;
      }
      sync();
    };
    const onFocusOut = () => {
      focus = false;
      sync();
    };
    host.addEventListener('pointerenter', onEnter);
    host.addEventListener('pointerleave', onLeave);
    host.addEventListener('focusin', onFocusIn);
    host.addEventListener('focusout', onFocusOut);
    return () => {
      stop(inst);
      host.removeEventListener('pointerenter', onEnter);
      host.removeEventListener('pointerleave', onLeave);
      host.removeEventListener('focusin', onFocusIn);
      host.removeEventListener('focusout', onFocusOut);
    };
  }, [def]);

  return (
    <svg ref={ref} className="nav-icon" data-icon={name} width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {def.body}
    </svg>
  );
}
