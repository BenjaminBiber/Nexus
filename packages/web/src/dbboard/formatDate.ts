// Datum und Uhrzeit im deutschen Kurzformat (lokale Zeit)
export function formatDate(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString('de-DE', { dateStyle: 'short', timeStyle: 'short' });
}
