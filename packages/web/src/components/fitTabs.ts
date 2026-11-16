// Verteilt Reiter auf die Leiste und das ⋯-Menü: von vorn so viele, wie neben den ⋯-Knopf passen. Der aktive Reiter bleibt
// immer sichtbar; liegt er dahinter, verdrängt er die letzten sichtbaren, bis er passt. Ohne gemessene Breite (available ≤ 0,
// z. B. vor dem ersten Messen oder in jsdom) und wenn alles passt, stehen alle Reiter in der Leiste.
export function fitTabs({ widths, gap, moreWidth, available, active }: {
  /** Breite jedes Reiters in Leistenreihenfolge */
  widths: number[];
  /** Abstand zwischen zwei Elementen der Leiste */
  gap: number;
  /** Breite des ⋯-Knopfs */
  moreWidth: number;
  /** Breite der Leiste */
  available: number;
  /** Index des aktiven Reiters */
  active: number;
}): { visible: number[]; hidden: number[] } {
  const all = widths.map((_, index) => index);
  const total = widths.reduce((sum, width) => sum + width, 0) + gap * Math.max(0, widths.length - 1);
  if (available <= 0 || total <= available) return { visible: all, hidden: [] };

  // Platz, den eine Auswahl von Reitern samt Abständen und ⋯-Knopf braucht
  const need = (indices: number[]) => indices.reduce((sum, index) => sum + widths[index] + gap, 0) + moreWidth;

  let visible: number[] = [];
  for (const index of all) {
    if (need([...visible, index]) > available) break;
    visible.push(index);
  }
  if (!visible.includes(active)) {
    while (visible.length > 0 && need([...visible, active]) > available) visible = visible.slice(0, -1);
    visible = [...visible, active].sort((a, b) => a - b);
  }
  return { visible, hidden: all.filter((index) => !visible.includes(index)) };
}
