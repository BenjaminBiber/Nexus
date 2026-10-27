// Optionen einer Profilauswahl; ein Wert ohne passendes Profil erscheint als „(ungültig: <name>)“ statt still die erste Option zu zeigen.
export function ProfileOptions({ names, value }: { names: string[]; value: string }) {
  const unknown = value.trim() !== '' && !names.includes(value.trim());
  return (
    <>
      {unknown && <option value={value}>(ungültig: {value})</option>}
      {names.map((n) => <option key={n} value={n}>{n}</option>)}
    </>
  );
}
