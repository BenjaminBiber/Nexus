// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { DispatchRunShotDto } from '../src/api/dispatchTypes.js';
import { RunShots } from '../src/components/db/run/RunShots.js';
import { RunShotViewer } from '../src/components/db/run/RunShotViewer.js';
import { formatDateTime } from '../src/sessions/format.js';

afterEach(cleanup);

const shot = (file: string): DispatchRunShotDto => ({ file, toolUseId: `call-${file}`, at: '2026-10-16T08:16:00.000Z', mediaType: 'image/png', bytes: 1 });
const SHOTS = [shot('001.png'), shot('002.png'), shot('003.png')];
const url = (file: string) => `/shots/run/${file}`;

describe('RunShots', () => {
  it('rendert ohne Screenshots nichts', () => {
    const { container } = render(<RunShots shots={[]} url={url} onOpen={vi.fn()} />);
    expect(container.innerHTML).toBe('');
  });

  it('zeigt alle Screenshots mit ihrer Anzahl und öffnet den angeklickten', async () => {
    const onOpen = vi.fn();
    render(<RunShots shots={SHOTS} url={url} onOpen={onOpen} />);
    const strip = screen.getByRole('group', { name: 'Screenshots des Laufs' });
    expect(within(strip).getAllByRole('img')).toHaveLength(3);
    expect((within(strip).getByAltText('Screenshot 2 von 3') as HTMLImageElement).getAttribute('src')).toBe('/shots/run/002.png');
    expect(within(strip).getByText('3 Screenshots')).toBeTruthy();

    await userEvent.click(within(strip).getByRole('button', { name: 'Screenshot 2 von 3 vergrößern' }));
    expect(onOpen).toHaveBeenCalledWith('002.png');
  });

  it('zählt einen einzelnen Screenshot in der Einzahl', () => {
    render(<RunShots shots={[SHOTS[0]!]} url={url} onOpen={vi.fn()} />);
    expect(screen.getByText('1 Screenshot')).toBeTruthy();
  });
});

describe('RunShotViewer', () => {
  // Rendert die Großansicht für eine Datei
  function open(file: string) {
    const onChange = vi.fn();
    const onClose = vi.fn();
    const view = render(<RunShotViewer shots={SHOTS} file={file} url={url} onChange={onChange} onClose={onClose} />);
    return { ...view, onChange, onClose };
  }

  it('zeigt das Bild mit Zählung und Zeitpunkt', () => {
    open('002.png');
    const dialog = screen.getByRole('dialog', { name: 'Screenshot 2 von 3' });
    expect((within(dialog).getByAltText('Screenshot 2 von 3') as HTMLImageElement).getAttribute('src')).toBe('/shots/run/002.png');
    expect(within(dialog).getByText(formatDateTime(SHOTS[1]!.at))).toBeTruthy();
  });

  it('blättert mit Knöpfen und Pfeiltasten', async () => {
    const { onChange } = open('002.png');
    await userEvent.click(screen.getByRole('button', { name: 'Weiter' }));
    expect(onChange).toHaveBeenLastCalledWith('003.png');
    await userEvent.click(screen.getByRole('button', { name: 'Zurück' }));
    expect(onChange).toHaveBeenLastCalledWith('001.png');
    await userEvent.keyboard('{ArrowRight}');
    expect(onChange).toHaveBeenLastCalledWith('003.png');
    await userEvent.keyboard('{ArrowLeft}');
    expect(onChange).toHaveBeenLastCalledWith('001.png');
    expect(onChange).toHaveBeenCalledTimes(4);
  });

  it('sperrt das Blättern an den Enden', async () => {
    const first = open('001.png');
    expect((screen.getByRole('button', { name: 'Zurück' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Weiter' }) as HTMLButtonElement).disabled).toBe(false);
    await userEvent.keyboard('{ArrowLeft}');
    expect(first.onChange).not.toHaveBeenCalled();
    first.unmount();

    open('003.png');
    expect((screen.getByRole('button', { name: 'Weiter' }) as HTMLButtonElement).disabled).toBe(true);
  });

  it('schließt über den Knopf und über Esc', async () => {
    const { onClose } = open('002.png');
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(onClose).toHaveBeenCalledTimes(1);
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('rendert für eine unbekannte Datei nichts', () => {
    open('999.png');
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
