import { memo, useId } from 'react';
import { SCENE_H, SCENE_W, type Rect } from '../shared/scene.js';
import { saSceneData } from './scene.js';

const rects = (list: Rect[]) => list.map((r, i) => <rect key={i} x={r.x} y={r.y} width={r.w} height={r.h} />);

/** Vorort bei Sonnenuntergang als SVG; füllt seine Fläche (wie object-fit: cover) */
export function SaSceneArt({ className }: { className?: string }) {
  const id = useId();
  const sky = `${id}-sky`;
  const sun = `${id}-sun`;
  const haze = `${id}-haze`;
  const { far, houses, windows, poles, dashes, palms } = saSceneData();
  return (
    <svg className={className} viewBox={`0 0 ${SCENE_W} ${SCENE_H}`} preserveAspectRatio="xMidYMid slice" aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id={sky} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#27365c" />
          <stop offset=".26" stopColor="#6b5b78" />
          <stop offset=".47" stopColor="#c88555" />
          <stop offset=".6" stopColor="#f2aa50" />
          <stop offset=".72" stopColor="#ffd98a" />
        </linearGradient>
        <radialGradient id={sun}>
          <stop offset="0" stopColor="#fff3c8" stopOpacity=".95" />
          <stop offset=".3" stopColor="#ffc56e" stopOpacity=".5" />
          <stop offset="1" stopColor="#ff9a4a" stopOpacity="0" />
        </radialGradient>
        <linearGradient id={haze} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#ffb060" stopOpacity="0" />
          <stop offset="1" stopColor="#ffb060" stopOpacity=".34" />
        </linearGradient>
      </defs>
      <rect width={SCENE_W} height={SCENE_H} fill={`url(#${sky})`} />
      <circle cx="1030" cy="572" r="330" fill={`url(#${sun})`} />
      <circle cx="1030" cy="572" r="66" fill="#fff1c4" />
      <g fill="#9a6f5a" opacity=".78">{rects(far)}</g>
      <path d="M0 640C190 586 370 628 560 612S900 566 1100 602 1320 574 1440 598V900H0Z" fill="#70503f" />
      <rect y="470" width={SCENE_W} height="250" fill={`url(#${haze})`} />
      <g fill="#241a17">
        {houses.map((d, i) => (
          <path key={i} d={d} />
        ))}
      </g>
      <g fill="#ffcf7a" opacity=".9">{rects(windows)}</g>
      <g stroke="#17100e" fill="none">
        {poles.map((p, i) => (
          <path key={i} d={p.d} strokeWidth={p.width} />
        ))}
      </g>
      {/* Basketballkorb */}
      <g fill="none" stroke="#17100e" strokeWidth="5">
        <path d="M706 724V552M706 560H680" />
        <rect x="646" y="534" width="36" height="28" strokeWidth="4" />
        <ellipse cx="664" cy="568" rx="11" ry="3" strokeWidth="3" />
      </g>
      <rect y="716" width={SCENE_W} height="184" fill="#16100e" />
      <rect y="716" width={SCENE_W} height="12" fill="#3a2c25" />
      <g fill="#e0b566" opacity=".4">{rects(dashes)}</g>
      {/* Auto am Straßenrand */}
      <g transform="translate(930 741)">
        <path d="M0 44L6 30Q10 25 30 24L84 22 104 6Q108 2 118 2H178Q190 2 196 8L214 24 262 28Q274 30 276 38L278 46Z" fill="#0e0908" />
        <path d="M112 8H146V22H92ZM152 8H178Q184 8 190 14L198 22H152Z" fill="#c98a4a" opacity=".55" />
        <path d="M4 38H276" stroke="#e8b15a" strokeWidth="1.6" opacity=".6" />
        <circle cx="58" cy="46" r="15" fill="#070505" />
        <circle cx="58" cy="46" r="8" fill="#b88a4a" opacity=".75" />
        <circle cx="222" cy="46" r="15" fill="#070505" />
        <circle cx="222" cy="46" r="8" fill="#b88a4a" opacity=".75" />
      </g>
      <g stroke="#120c0a" fill="none" strokeLinecap="round">
        {palms.map((p, i) => (
          <g key={i}>
            <path d={p.trunk} strokeWidth={p.width} />
            {p.leaves.map((d) => (
              <path key={d} d={d} fill="#120c0a" strokeWidth="2" />
            ))}
            <circle cx={p.crown.x} cy={p.crown.y} r={p.crown.r} fill="#120c0a" />
          </g>
        ))}
      </g>
    </svg>
  );
}

// Szene fest hinter dem Rahmen: abgedunkelt, mit dem Verlauf der Preview an den Rändern und feiner Körnung; rein
// dekorativ. Ohne Props und mit memo, damit Uhr und Feed die vielen SVG-Elemente nicht jedes Mal neu abgleichen.
export const SaScene = memo(function SaScene() {
  return (
    <div className="gta-sa-scene" aria-hidden="true">
      <SaSceneArt className="gta-sa-world" />
      <div className="gta-sa-shade" />
      <div className="gta-sa-grain" />
    </div>
  );
});
