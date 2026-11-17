// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { ApiProvider } from '../src/api/ApiContext.js';
import { BoardProjectSwitcher } from '../src/components/BoardProjectSwitcher.js';
import { BoardProjectProvider } from '../src/dbboard/boardProject.js';
import { boardPickerTrigger } from './helpers/boardPicker.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

function renderSwitcher() {
  render(
    <ApiProvider api={fakeApi()}>
      <MemoryRouter initialEntries={['/p/p1/board']}>
        <BoardProjectProvider>
          <BoardProjectSwitcher pid="p1" status={<span className="live" data-testid="live" />} />
        </BoardProjectProvider>
      </MemoryRouter>
    </ApiProvider>,
  );
}

describe('BoardProjectSwitcher', () => {
  it('trägt das Projekt-Kürzel direkt hinter dem Verbindungspunkt (für die eingeklappte Leiste)', async () => {
    renderSwitcher();
    await waitFor(() => expect(boardPickerTrigger().getAttribute('aria-label')).toMatch(/^Board wählen: Arches/));
    const field = document.querySelector('.bp-field')!;
    const key = field.querySelector('.bp-rail-key')!;
    expect(key.textContent).toBe('ARC');
    // Nur zum Ansehen: der Auswahlknopf nennt Projekt und Board schon vollständig
    expect(key.getAttribute('aria-hidden')).toBe('true');
    expect(key.previousElementSibling?.classList.contains('bp-status')).toBe(true);
  });

  describe('Überfahren auf dem Weg zu den Boards (Dreieck zur Board-Spalte)', () => {
    // Die Board-Spalte steht rechts der Projekte: linke Kante bei x = 240, von y = 60 bis 400
    beforeEach(() => {
      vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(() => DOMRect.fromRect({ x: 240, y: 60, width: 220, height: 340 }));
    });
    afterEach(() => {
      vi.useRealTimers();
      vi.restoreAllMocks();
    });

    async function openPicker() {
      renderSwitcher();
      await waitFor(() => expect(boardPickerTrigger().getAttribute('aria-label')).toMatch(/^Board wählen: Arches/));
      fireEvent.click(boardPickerTrigger());
      const dialog = screen.getByRole('dialog', { name: 'Board wählen' });
      vi.useFakeTimers();
      return {
        dialog,
        title: () => dialog.querySelector('#bp-boards')!.textContent,
        project: (key: string) => within(dialog).getByRole('button', { name: new RegExp(`^${key}\\b`) }),
        projects: () => within(dialog).getByRole('list', { name: 'Projekte' }),
      };
    }

    it('wechselt nicht, solange der Zeiger schräg zur Board-Spalte zieht, und verlässt er die Projekte, bleibt es dabei', async () => {
      const picker = await openPicker();
      fireEvent.mouseMove(picker.dialog, { clientX: 100, clientY: 200 });
      fireEvent.mouseMove(picker.dialog, { clientX: 120, clientY: 190 });
      fireEvent.mouseEnter(picker.project('OTH'));
      expect(picker.title()).toBe('Boards in ARC');
      fireEvent.mouseMove(picker.dialog, { clientX: 140, clientY: 180 });
      fireEvent.mouseLeave(picker.projects());
      act(() => vi.advanceTimersByTime(1000));
      expect(picker.title()).toBe('Boards in ARC');
    });

    it('wechselt, wenn der Zeiger auf dem überfahrenen Projekt stehen bleibt', async () => {
      const picker = await openPicker();
      fireEvent.mouseMove(picker.dialog, { clientX: 100, clientY: 200 });
      fireEvent.mouseMove(picker.dialog, { clientX: 120, clientY: 190 });
      fireEvent.mouseEnter(picker.project('OTH'));
      act(() => vi.advanceTimersByTime(1000));
      expect(picker.title()).toBe('Boards in OTH');
    });

    it('wechselt sofort, wenn der Zeiger nicht zur Board-Spalte zieht', async () => {
      const picker = await openPicker();
      fireEvent.mouseMove(picker.dialog, { clientX: 100, clientY: 180 });
      fireEvent.mouseMove(picker.dialog, { clientX: 100, clientY: 200 });
      fireEvent.mouseEnter(picker.project('OTH'));
      expect(picker.title()).toBe('Boards in OTH');
    });
  });
});
