// Icon-Knopf, der den Dialog für einen neuen Kommentar öffnet (Reiter „Kommentare“ und „Verlauf“)
export function DbCommentButton({ onOpen, busy }: { onOpen: () => void; busy: boolean }) {
  return (
    <button type="button" className="icon-btn" aria-label="Kommentar hinzufügen" title="Kommentar hinzufügen" disabled={busy} onClick={onOpen}>
      <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" aria-hidden="true">
        <path d="M12 5v14M5 12h14" />
      </svg>
    </button>
  );
}
