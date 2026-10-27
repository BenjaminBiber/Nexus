// Fortschritt einer Spec oder eines Tickets: „x von y erledigt“ mit grünem Balken.
export function Progress({ done, total, label = 'Fortschritt der Spec' }: { done: number; total: number; label?: string }) {
  const percent = total === 0 ? 0 : Math.round((done / total) * 100);
  return (
    <div className="progress">
      <span className="progress-text">
        {done} von {total} erledigt
      </span>
      <div className="progress-bar" role="img" aria-label={label}>
        <span style={{ width: `${percent}%` }} />
      </div>
    </div>
  );
}
