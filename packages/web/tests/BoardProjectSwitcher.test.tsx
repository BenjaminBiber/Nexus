// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, waitFor } from '@testing-library/react';
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
});
