// Umschalter mit Segmenten (Lauf-Art, Zeitraum); deaktivierte Segmente bleiben sichtbar
export function Segmented<T extends string>({ label, options, value, onChange }: {
  label: string; options: Array<{ value: T; label: string; disabled?: boolean; title?: string }>; value: T; onChange: (value: T) => void;
}) {
  return (
    <div className="segmented" role="group" aria-label={label}>
      {options.map((o) => (
        <button
          key={o.value} type="button" className={o.value === value ? 'on' : undefined} aria-pressed={o.value === value}
          disabled={o.disabled} title={o.title} onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
