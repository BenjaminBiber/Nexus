import { useId } from 'react';
import { SCENE_H, SCENE_W, sceneData, type Rect } from './scene.js';

const rects = (list: Rect[]) => list.map((r, i) => <rect key={i} x={r.x} y={r.y} width={r.w} height={r.h} />);

/** Abendhimmel mit Sonne, Skyline und Palmen als SVG; füllt seine Fläche (wie object-fit: cover) */
export function SceneArt({ className }: { className?: string }) {
  const id = useId();
  const sky = `${id}-sky`;
  const sun = `${id}-sun`;
  const { far, near, windows, palms } = sceneData();
  return (
    <svg className={className} viewBox={`0 0 ${SCENE_W} ${SCENE_H}`} preserveAspectRatio="xMidYMid slice" aria-hidden="true" focusable="false">
      <defs>
        <linearGradient id={sky} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#10163a" />
          <stop offset=".32" stopColor="#4b2a70" />
          <stop offset=".56" stopColor="#c94f77" />
          <stop offset=".72" stopColor="#ff9450" />
          <stop offset=".84" stopColor="#ffd27a" />
        </linearGradient>
        <radialGradient id={sun}>
          <stop offset="0" stopColor="#fff0c0" stopOpacity=".9" />
          <stop offset=".35" stopColor="#ffb86a" stopOpacity=".45" />
          <stop offset="1" stopColor="#ff8a5a" stopOpacity="0" />
        </radialGradient>
      </defs>
      <rect width={SCENE_W} height={SCENE_H} fill={`url(#${sky})`} />
      <circle cx="470" cy="612" r="340" fill={`url(#${sun})`} />
      <circle cx="470" cy="612" r="104" fill="#ffecb5" />
      <path d="M0 646C150 566 330 600 500 624S860 540 1080 606 1340 584 1440 612V900H0Z" fill="#7a3a6c" opacity=".8" />
      <g fill="#452860">{rects(far)}</g>
      <g fill="#1e1332">{rects(near)}</g>
      <g fill="#ffd98a" opacity=".85">{rects(windows)}</g>
      <rect y="742" width={SCENE_W} height="158" fill="#0a0712" />
      <g stroke="#05030a" fill="none" strokeLinecap="round">
        {palms.map((p, i) => (
          <g key={i}>
            <path d={p.trunk} strokeWidth={p.width} />
            {p.leaves.map((d) => (
              <path key={d} d={d} fill="#05030a" strokeWidth="2" />
            ))}
            <circle cx={p.crown.x} cy={p.crown.y} r={p.crown.r} fill="#05030a" />
          </g>
        ))}
      </g>
    </svg>
  );
}

// Szene fest hinter dem Rahmen, unscharf und abgedunkelt wie das Pausenmenü; rein dekorativ
export function VScene() {
  return (
    <div className="gta-v-scene" aria-hidden="true">
      <SceneArt className="gta-v-world" />
      <div className="gta-v-shade" />
    </div>
  );
}
