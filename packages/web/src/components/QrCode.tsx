import { useMemo } from 'react';
import qrcode from 'qrcode-generator';

// Ruhezone um den Code in Modulen; der Standard verlangt vier
const QUIET_ZONE = 4;

// Ein QR-Code als SVG, gezeichnet aus den Modulen – ohne HTML aus der Bibliothek.
// Schwarz auf Weiß in beiden Themes, damit jede Kamera ihn liest.
export function QrCode({ value, size = 180, label }: { value: string; size?: number; label: string }) {
  const { path, extent } = useMemo(() => {
    const code = qrcode(0, 'M');
    code.addData(value);
    code.make();
    const count = code.getModuleCount();
    const parts: string[] = [];
    for (let row = 0; row < count; row++) {
      for (let col = 0; col < count; col++) {
        if (code.isDark(row, col)) parts.push(`M${col + QUIET_ZONE} ${row + QUIET_ZONE}h1v1h-1z`);
      }
    }
    return { path: parts.join(''), extent: count + QUIET_ZONE * 2 };
  }, [value]);

  return (
    <svg
      className="qr-code"
      role="img"
      aria-label={label}
      width={size}
      height={size}
      viewBox={`0 0 ${extent} ${extent}`}
      shapeRendering="crispEdges"
    >
      <rect width={extent} height={extent} fill="#fff" />
      <path d={path} fill="#000" />
    </svg>
  );
}
