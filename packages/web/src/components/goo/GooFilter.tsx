// SVG-Filter für den Goo-Effekt: weichzeichnen, dann die Deckkraft mit einer steilen Kennlinie schwellen, so verschmelzen
// nahe Formen. `blur` bestimmt, wie weit sie sich anziehen. Der Filter steht im selben Baum wie die Ebene, die ihn nutzt.
export function GooFilter({ id, blur }: { id: string; blur: number }) {
  return (
    <svg className="goo-defs" width="0" height="0" aria-hidden="true" focusable="false">
      <defs>
        <filter id={id} colorInterpolationFilters="sRGB">
          <feGaussianBlur in="SourceGraphic" stdDeviation={blur} result="blur" />
          <feColorMatrix in="blur" type="matrix" values="1 0 0 0 0  0 1 0 0 0  0 0 1 0 0  0 0 0 22 -10" />
        </filter>
      </defs>
    </svg>
  );
}
