import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// Auslöser der Board-Auswahl in der Seitenleiste (Name: „Board wählen: Projektname › Board“)
export function boardPickerTrigger(): HTMLElement {
  return screen.getByRole('button', { name: /^Board wählen/ });
}

// Öffnet die Board-Auswahl und gibt das Popover zurück
async function openPicker(): Promise<HTMLElement> {
  await userEvent.click(boardPickerTrigger());
  return screen.getByRole('dialog', { name: 'Board wählen' });
}

// Klickt in der Board-Auswahl ein Projekt: wählt dessen erstes Board (ohne Board: öffnet das Projekt)
export async function pickProject(project: string): Promise<void> {
  const dialog = await openPicker();
  await userEvent.click(within(dialog).getByRole('button', { name: new RegExp(`^${project}\\b`) }));
}

// Wählt in der Board-Auswahl ein bestimmtes Board; mit project zeigt erst das Überfahren des Projekts dessen Boards
export async function pickBoard(board: string, project?: string): Promise<void> {
  const dialog = await openPicker();
  if (project) await userEvent.hover(within(dialog).getByRole('button', { name: new RegExp(`^${project}\\b`) }));
  await userEvent.click(within(dialog).getByRole('button', { name: new RegExp(`^${board}\\b`) }));
}
