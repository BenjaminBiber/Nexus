// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { App } from '../src/App.js';
import { ApiProvider } from '../src/api/ApiContext.js';
import { dbQuestion, dbQuestionList, fakeApi } from './helpers/fakeApi.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

afterEach(cleanup);

describe('Routen der Fragen', () => {
  it('zeigt unter „Fragen“ die Fragen der Board-DB und nur diesen Eintrag in der Navigation', async () => {
    render(
      <ApiProvider api={fakeApi({ getDbQuestions: async () => dbQuestionList([dbQuestion('ARC-F0001', 'DB-Frage')]) })}>
        <MemoryRouter initialEntries={['/p/p1/questions']}>
          <App />
        </MemoryRouter>
      </ApiProvider>,
    );
    expect(await screen.findByText('DB-Frage')).toBeTruthy();
    expect(screen.getByRole('link', { name: 'DB-Frage' }).getAttribute('href')).toBe('/p/p1/questions/ARC-F0001');
    const nav = screen.getByRole('navigation', { name: 'Hauptnavigation' });
    expect((await within(nav).findByRole('link', { name: 'Fragen, 1 eigene offen' })).getAttribute('href')).toBe('/p/p1/questions');
    expect(within(nav).queryByRole('link', { name: /\(alt\)/ })).toBeNull();
  });
});
