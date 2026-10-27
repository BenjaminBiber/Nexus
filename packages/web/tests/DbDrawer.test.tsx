// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DbDrawer } from '../src/components/db/DbDrawer.js';

beforeEach(() => localStorage.clear());
afterEach(cleanup);

describe('DbDrawer', () => {
  it('klappt auf und zu, behält den Inhalt und merkt sich den Zustand je Bereich', async () => {
    const { unmount } = render(<DbDrawer id="steps" title="Schritte" meta="3/10" actions={<button type="button">Neu</button>}><input aria-label="Eingabe" /></DbDrawer>);
    const toggle = screen.getByRole('button', { name: /Schritte/ });
    expect(toggle.getAttribute('aria-expanded')).toBe('true');
    expect(toggle.textContent).toContain('3/10');
    await userEvent.type(screen.getByLabelText('Eingabe'), 'x');
    await userEvent.click(toggle);
    expect(toggle.getAttribute('aria-expanded')).toBe('false');
    expect((screen.getByLabelText('Eingabe') as HTMLInputElement).value).toBe('x');
    expect(screen.getByRole('button', { name: 'Neu' })).toBeTruthy();
    unmount();
    render(<DbDrawer id="steps" title="Schritte"><p>Inhalt</p></DbDrawer>);
    expect(screen.getByRole('button', { name: /Schritte/ }).getAttribute('aria-expanded')).toBe('false');
    expect(screen.getByRole('region', { name: 'Schritte' })).toBeTruthy();
  });

  it('startet ohne gemerkten Zustand mit defaultOpen', () => {
    render(<DbDrawer id="x" title="Zu" defaultOpen={false}><p>Inhalt</p></DbDrawer>);
    expect(screen.getByRole('button', { name: /Zu/ }).getAttribute('aria-expanded')).toBe('false');
  });
});
