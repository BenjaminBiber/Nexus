const PATHS = {
  check: 'M20 6 9 17l-5-5',
  undo: 'M3 7v6h6M3 13a9 9 0 1 0 3-7',
  edit: 'M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z',
  trash: 'M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6',
  plus: 'M12 5v14M5 12h14',
  folder: 'M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
} as const;

// Kleines Linien-Symbol für die Aktionen eines Kommentars
export function CommentIcon({ name }: { name: keyof typeof PATHS }) {
  return (
    <svg className="comment-icon" viewBox="0 0 24 24" aria-hidden="true">
      <path d={PATHS[name]} />
    </svg>
  );
}
