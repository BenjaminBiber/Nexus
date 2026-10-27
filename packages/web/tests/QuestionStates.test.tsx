// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, screen, within } from '@testing-library/react';
import { GuidePage } from '../src/pages/GuidePage.js';
import { fakeApi } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

afterEach(cleanup);

// Rendert die Anleitung (sie liest den angewendeten Workflow über die API)
const openGuide = () => renderWithApp(<GuidePage />, { api: fakeApi(), path: '/p/p1/guide', route: '/p/:pid/guide' });

describe('Anleitung: Fragen', () => {
  it('nennt die Schritte mit kopierbarem Befehl auf dem Board', () => {
    openGuide();
    const steps = screen.getByRole('list', { name: 'Fragen' });
    expect(within(steps).getByText('Entscheidung aufschieben')).toBeTruthy();
    expect(within(steps).getByText('Recherchieren lassen')).toBeTruthy();
    expect(within(steps).getByRole('button', { name: /Befehl kopieren: harness board question add/ })).toBeTruthy();
    expect(within(steps).getByRole('button', { name: 'Befehl kopieren: /ai-harness:board-question ARC-F0007' })).toBeTruthy();
  });

  it('erklärt die Begriffe Frage, Option, Recherche und Entscheidung', () => {
    openGuide();
    const terms = screen.getByRole('region', { name: 'Begriffe' });
    for (const term of ['Frage', 'Option', 'Recherche', 'Entscheidung']) {
      expect(within(terms).getByText(term, { selector: 'dt' })).toBeTruthy();
    }
  });
});
