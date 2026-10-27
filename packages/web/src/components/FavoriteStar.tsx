// Stern einer Session: zeigt, ob sie Favorit ist, und schaltet um.
export function FavoriteStar({ active, onToggle }: { active: boolean; onToggle: () => void }) {
  return (
    <button
      type="button"
      className="star"
      aria-pressed={active}
      aria-label={active ? 'Aus Favoriten entfernen' : 'Zu Favoriten hinzufügen'}
      onClick={onToggle}
    >
      <svg width="22" height="22" viewBox="0 0 24 24" fill={active ? '#e8a317' : 'none'} stroke={active ? '#b7791f' : 'currentColor'} strokeWidth="1.5" strokeLinejoin="round" aria-hidden="true">
        <path d="M12 3l2.7 5.6 6.1.8-4.5 4.2 1.1 6.1L12 16.8 6.6 19.7l1.1-6.1L3.2 9.4l6.1-.8z" />
      </svg>
    </button>
  );
}
