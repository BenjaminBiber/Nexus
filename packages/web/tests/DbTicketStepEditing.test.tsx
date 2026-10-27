// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ApiError } from '../src/api/ApiError.js';
import type { DbStepDto, DbTicketDetailDto } from '../src/api/dbTypes.js';
import { openPanel, openStepsTab } from './helpers/dbPanel.js';
import { dbCard, dbDetail } from './helpers/fakeApi.js';
import { FakeEventSource } from './helpers/FakeEventSource.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg></svg>' })) } }));

afterEach(cleanup);
beforeEach(() => FakeEventSource.reset());

const pause = () => new Promise((resolve) => setTimeout(resolve, 20));

const STEPS: DbStepDto[] = [{ text: 'Test schreiben', done: false }, { text: 'Umsetzen', done: false, detail: 'npm test' }];
const WITH = dbDetail(dbCard('ARC-0001', 'Login bauen', 'in_progress', { stepsTotal: 2 }), { steps: STEPS });

// Stand des Tickets mit neuen Schritten und nächster Version
const withSteps = (steps: DbStepDto[]): DbTicketDetailDto => ({ ...WITH, rev: WITH.rev + 1, steps, stepsTotal: steps.length, stepsDone: steps.filter((s) => s.done).length });

describe('Schritte im Ticket-Panel bearbeiten', () => {
  it('legt einen Schritt mit Anleitung an (Enter speichert) und zeigt den neuen Stand', async () => {
    const addDbStep = vi.fn(async () => withSteps([{ text: 'Committen', done: false, detail: 'git commit' }]));
    const { onChanged } = openPanel({ addDbStep });
    await openStepsTab();
    await userEvent.click(await screen.findByRole('button', { name: '+ Schritt' }));
    const form = screen.getByRole('form', { name: 'Neuer Schritt' });
    expect(within(form).getByRole('button', { name: 'Hinzufügen' })).toHaveProperty('disabled', true);
    await userEvent.type(within(form).getByLabelText('Anleitung (optional, Markdown)'), 'git commit');
    await userEvent.type(within(form).getByLabelText('Text'), '  Committen {Enter}');
    expect(addDbStep).toHaveBeenCalledWith('ARC-0001', { text: 'Committen', detail: 'git commit' });
    expect(await screen.findByRole('checkbox', { name: 'Schritt 1: Committen' })).toBeTruthy();
    expect(screen.queryByRole('form', { name: 'Neuer Schritt' })).toBeNull();
    expect(onChanged).toHaveBeenCalled();
  });

  it('bricht die Eingabe mit Esc ab, ohne das Panel zu schließen', async () => {
    const addDbStep = vi.fn();
    const { onClose } = openPanel({ addDbStep });
    await openStepsTab();
    await userEvent.click(await screen.findByRole('button', { name: '+ Schritt' }));
    await userEvent.type(screen.getByLabelText('Text'), 'Halb{Escape}');
    expect(screen.queryByRole('form', { name: 'Neuer Schritt' })).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    expect(addDbStep).not.toHaveBeenCalled();
  });

  it('bearbeitet Text und entfernt eine leere Anleitung', async () => {
    const editDbStep = vi.fn(async () => withSteps([STEPS[0], { text: 'Umsetzen und prüfen', done: false }]));
    openPanel({ editDbStep }, WITH);
    await openStepsTab();
    await userEvent.click(await screen.findByRole('button', { name: 'Schritt 2 bearbeiten' }));
    const form = screen.getByRole('form', { name: 'Schritt 2 bearbeiten' });
    const text = within(form).getByLabelText('Text');
    expect((text as HTMLInputElement).value).toBe('Umsetzen');
    await userEvent.clear(text);
    await userEvent.type(text, 'Umsetzen und prüfen');
    await userEvent.clear(within(form).getByLabelText('Anleitung (optional, Markdown)'));
    await userEvent.click(within(form).getByRole('button', { name: 'Speichern' }));
    expect(editDbStep).toHaveBeenCalledWith('ARC-0001', 2, { text: 'Umsetzen und prüfen', detail: null }, 'Umsetzen');
    expect(await screen.findByRole('checkbox', { name: 'Schritt 2: Umsetzen und prüfen' })).toBeTruthy();
    // Fokus zurück auf den auslösenden Knopf
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Schritt 2 bearbeiten' })));
  });

  it('löscht erst nach Rückfrage; Abbrechen lässt den Schritt stehen', async () => {
    const removeDbStep = vi.fn(async () => withSteps([STEPS[1]]));
    openPanel({ removeDbStep }, WITH);
    await openStepsTab();
    await userEvent.click(await screen.findByRole('button', { name: 'Schritt 1 löschen' }));
    const confirm = screen.getByRole('alertdialog', { name: 'Schritt 1 löschen' });
    // Der Fokus steht auf „Abbrechen“, nicht auf „Löschen“
    expect(document.activeElement).toBe(within(confirm).getByRole('button', { name: 'Abbrechen' }));
    await userEvent.click(within(confirm).getByRole('button', { name: 'Abbrechen' }));
    expect(removeDbStep).not.toHaveBeenCalled();
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Schritt 1 löschen' })));
    await userEvent.click(screen.getByRole('button', { name: 'Schritt 1 löschen' }));
    await userEvent.click(within(screen.getByRole('alertdialog', { name: 'Schritt 1 löschen' })).getByRole('button', { name: 'Löschen' }));
    expect(removeDbStep).toHaveBeenCalledWith('ARC-0001', 1, 'Test schreiben');
    expect(await screen.findByRole('checkbox', { name: 'Schritt 1: Umsetzen' })).toBeTruthy();
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('button', { name: '+ Schritt' })));
  });

  it('schiebt Schritte nach oben und unten; Ränder sind gesperrt', async () => {
    const moveDbStep = vi.fn(async () => withSteps([STEPS[1], STEPS[0]]));
    openPanel({ moveDbStep }, WITH);
    await openStepsTab();
    expect(await screen.findByRole('button', { name: 'Schritt 1 nach oben' })).toHaveProperty('disabled', true);
    expect(screen.getByRole('button', { name: 'Schritt 2 nach unten' })).toHaveProperty('disabled', true);
    await userEvent.click(screen.getByRole('button', { name: 'Schritt 2 nach oben' }));
    expect(moveDbStep).toHaveBeenCalledWith('ARC-0001', 2, 1, 'Umsetzen');
    expect(await screen.findByRole('checkbox', { name: 'Schritt 1: Umsetzen' })).toBeTruthy();
  });

  it('zeigt den Fehler des Servers und lässt die Eingabe offen', async () => {
    const addDbStep = vi.fn(async () => { throw new Error('Ein Schritt braucht einen Text.'); });
    openPanel({ addDbStep });
    await openStepsTab();
    await userEvent.click(await screen.findByRole('button', { name: '+ Schritt' }));
    await userEvent.type(screen.getByLabelText('Text'), 'X{Enter}');
    expect(await screen.findByText('Ein Schritt braucht einen Text.')).toBeTruthy();
    expect(screen.getByRole('form', { name: 'Neuer Schritt' })).toBeTruthy();
  });

  it('sperrt bei offenem Formular Verschieben, Bearbeiten und Löschen der Schritte; ein geänderter Entwurf hält Live-Nachladen und Schließen auf', async () => {
    const getDbTicket = vi.fn().mockResolvedValueOnce(WITH).mockResolvedValue(withSteps([STEPS[1], STEPS[0]]));
    const { onClose } = openPanel({ getDbTicket }, WITH);
    await openStepsTab();
    await userEvent.click(await screen.findByRole('button', { name: 'Schritt 2 bearbeiten' }));
    for (const name of ['Schritt 1 nach unten', 'Schritt 1 bearbeiten', 'Schritt 1 löschen', '+ Schritt']) {
      expect(screen.getByRole('button', { name })).toHaveProperty('disabled', true);
    }
    const text = within(screen.getByRole('form', { name: 'Schritt 2 bearbeiten' })).getByLabelText('Text');
    await userEvent.type(text, ' und prüfen');
    act(() => FakeEventSource.current().emit({ type: 'dbboard', boardId: WITH.boardId }));
    await pause();
    expect(getDbTicket).toHaveBeenCalledTimes(1);
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(await screen.findByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeTruthy();
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole('button', { name: 'Weiter bearbeiten' }));
    // Abbrechen: nichts mehr offen, der aufgeschobene Stand wird nachgeladen und die Sperre fällt
    await userEvent.click(within(screen.getByRole('form', { name: 'Schritt 2 bearbeiten' })).getByRole('button', { name: 'Abbrechen' }));
    await waitFor(() => expect(getDbTicket).toHaveBeenCalledTimes(2));
    expect(await screen.findByRole('checkbox', { name: 'Schritt 1: Umsetzen' })).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Schritt 1 löschen' })).toHaveProperty('disabled', false);
  });

  it('speichert nach Verschieben durch andere den richtigen Schritt (Formular folgt seinem Schritt)', async () => {
    const getDbTicket = vi.fn().mockResolvedValueOnce(WITH).mockResolvedValue(withSteps([STEPS[1], STEPS[0]]));
    const editDbStep = vi.fn(async () => withSteps([{ ...STEPS[1], text: 'Umsetzen und prüfen' }, STEPS[0]]));
    openPanel({ getDbTicket, editDbStep }, WITH);
    await openStepsTab();
    await userEvent.click(await screen.findByRole('button', { name: 'Schritt 2 bearbeiten' }));
    // Unveränderter Entwurf hält das Nachladen nicht auf; ein anderer hat Schritt 2 nach oben geschoben
    act(() => FakeEventSource.current().emit({ type: 'dbboard', boardId: WITH.boardId }));
    const form = await screen.findByRole('form', { name: 'Schritt 1 bearbeiten' });
    const text = within(form).getByLabelText('Text') as HTMLInputElement;
    expect(text.value).toBe('Umsetzen');
    await userEvent.type(text, ' und prüfen{Enter}');
    expect(editDbStep).toHaveBeenCalledWith('ARC-0001', 1, { text: 'Umsetzen und prüfen', detail: 'npm test' }, 'Umsetzen');
    expect(await screen.findByRole('checkbox', { name: 'Schritt 1: Umsetzen und prüfen' })).toBeTruthy();
    expect(screen.getByRole('checkbox', { name: 'Schritt 2: Test schreiben' })).toBeTruthy();
  });

  it('übernimmt bei 409 den aktuellen Stand ohne Konflikt-Dialog; der Entwurf bleibt und folgt dem Schritt', async () => {
    const moved = withSteps([STEPS[1], STEPS[0]]);
    const editDbStep = vi.fn()
      .mockRejectedValueOnce(new ApiError('conflict', 'ARC-0001: Schritt 2 ist inzwischen „Test schreiben“ – die Schritte wurden geändert.', 409, moved))
      .mockResolvedValueOnce(withSteps([{ ...STEPS[1], text: 'Umsetzen!' }, STEPS[0]]));
    const { patchDbTicket } = openPanel({ editDbStep }, WITH);
    await openStepsTab();
    await userEvent.click(await screen.findByRole('button', { name: 'Schritt 2 bearbeiten' }));
    await userEvent.type(within(screen.getByRole('form', { name: 'Schritt 2 bearbeiten' })).getByLabelText('Text'), '!{Enter}');
    expect(await screen.findByText('Die Schritte wurden inzwischen geändert – bitte erneut versuchen.')).toBeTruthy();
    expect(screen.queryByRole('alertdialog', { name: /zwischenzeitlich geändert/ })).toBeNull();
    const form = await screen.findByRole('form', { name: 'Schritt 1 bearbeiten' });
    expect((within(form).getByLabelText('Text') as HTMLInputElement).value).toBe('Umsetzen!');
    await userEvent.click(within(form).getByRole('button', { name: 'Speichern' }));
    expect(editDbStep).toHaveBeenLastCalledWith('ARC-0001', 1, { text: 'Umsetzen!', detail: 'npm test' }, 'Umsetzen');
    expect(await screen.findByRole('checkbox', { name: 'Schritt 1: Umsetzen!' })).toBeTruthy();
    expect(patchDbTicket).not.toHaveBeenCalled();
  });

  it('schließt die Lösch-Rückfrage mit Esc ohne das Panel zu schließen; offen zählt sie als ungespeichert', async () => {
    const removeDbStep = vi.fn();
    const { onClose } = openPanel({ removeDbStep }, WITH);
    await openStepsTab();
    await userEvent.click(await screen.findByRole('button', { name: 'Schritt 2 löschen' }));
    expect(screen.getByRole('button', { name: 'Schritt 1 nach unten' })).toHaveProperty('disabled', true);
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(await screen.findByRole('alertdialog', { name: 'Ungespeicherte Änderungen' })).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Weiter bearbeiten' }));
    await userEvent.click(within(screen.getByRole('alertdialog', { name: 'Schritt 2 löschen' })).getByRole('button', { name: 'Abbrechen' }));
    await userEvent.click(screen.getByRole('button', { name: 'Schritt 2 löschen' }));
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('alertdialog', { name: 'Schritt 2 löschen' })).toBeNull();
    expect(onClose).not.toHaveBeenCalled();
    expect(removeDbStep).not.toHaveBeenCalled();
    await waitFor(() => expect(document.activeElement).toBe(screen.getByRole('button', { name: 'Schritt 2 löschen' })));
    expect(screen.getByRole('button', { name: 'Schritt 1 nach unten' })).toHaveProperty('disabled', false);
  });
});
