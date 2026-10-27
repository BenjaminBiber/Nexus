import type { DbBoardViewDto, DbStatus } from '../api/dbTypes.js';

// Verschiebt eine Karte sofort sichtbar ans Ende der Zielspalte (die genaue Reihenfolge liefert der Server danach)
export function moveDbCard(view: DbBoardViewDto, id: string, status: DbStatus): DbBoardViewDto {
  const card = view.columns.flatMap((c) => c.tickets).find((c) => c.id === id);
  if (!card) return view;
  const moved = { ...card, status, column: status, waiting: false };
  return {
    ...view,
    columns: view.columns.map((column) => ({
      ...column,
      tickets: [...column.tickets.filter((c) => c.id !== id), ...(column.id === status ? [moved] : [])],
    })),
  };
}
