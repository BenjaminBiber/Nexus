// Schalter „Fertige anzeigen“ neben einer Specauswahl; der Zustand liegt beim Aufrufer.
export function ShowFinishedToggle({ checked, onChange }: { checked: boolean; onChange: (value: boolean) => void }) {
  return (
    <label className="inline-field">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      Fertige anzeigen
    </label>
  );
}
