// Einstellungen eines Laufs als Reihe kleiner Marken: Profil, Modell, Effort, Limit, Grund der Wahl (siehe `runChips`)
export function RunChips({ chips }: { chips: string[] }) {
  if (chips.length === 0) return null;
  return (
    <ul className="run-chips" aria-label="Einstellungen des Laufs">
      {chips.map((chip, i) => (
        <li key={i}>{chip}</li>
      ))}
    </ul>
  );
}
