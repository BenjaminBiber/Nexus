import { WORDMARKS, type WordmarkName } from './wordmarks.js';

// Rand um den Pfad in Einheiten der viewBox, damit die Kontur aus theme.css nicht abgeschnitten wird (wie in den Previews)
const PAD = 9;

interface WordmarkProps {
  name: WordmarkName;
  /** Zugänglicher Name; ohne ist die Wortmarke reiner Schmuck */
  label?: string;
  className?: string;
}

// Wortmarke als SVG in der Textfarbe; Kontur und Schatten setzt theme.css über .gta-wordmark und data-wordmark
export function Wordmark({ name, label, className }: WordmarkProps) {
  const { d, box } = WORDMARKS[name];
  const [x, y, w, h] = box;
  const a11y = label ? { role: 'img', 'aria-label': label } : { 'aria-hidden': true };
  return (
    <svg
      className={className ? `gta-wordmark ${className}` : 'gta-wordmark'}
      data-wordmark={name}
      viewBox={`${x - PAD} ${y - PAD} ${w + 2 * PAD} ${h + 2 * PAD}`}
      fill="currentColor"
      {...a11y}
    >
      <path d={d} />
    </svg>
  );
}
