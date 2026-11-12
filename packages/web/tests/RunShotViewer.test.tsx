// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { DispatchRunShotDto } from '../src/api/dispatchTypes.js';
import { RunShotViewer } from '../src/components/db/run/RunShotViewer.js';
import { formatDateTime } from '../src/sessions/format.js';

afterEach(cleanup);

const shot = (file: string, minute: number): DispatchRunShotDto =>
  ({ file, toolUseId: `call-${file}`, at: `2026-10-16T08:${String(minute).padStart(2, '0')}:00.000Z`, mediaType: 'image/png', bytes: 1 });
const SHOTS = [shot('001.png', 10), shot('002.png', 11), shot('003.png', 12), shot('004.png', 13), shot('005.png', 14)];
const url = (file: string) => `/shots/run/${file}`;
// Tool-Name je Screenshot; der dritte hat keinen bekannten Aufruf
const caption = (s: DispatchRunShotDto) => (s.file === '003.png' ? '' : 'Bash');

// Rendert die Großansicht für eine Datei
function open(file: string, shots = SHOTS) {
  const onChange = vi.fn();
  const onClose = vi.fn();
  const view = render(<RunShotViewer shots={shots} file={file} url={url} caption={caption} onChange={onChange} onClose={onClose} />);
  return { ...view, onChange, onClose };
}

describe('RunShotViewer', () => {
  it('zeigt das Bild groß mit Namen, Beschriftung und Zählung', () => {
    open('002.png');
    const dialog = screen.getByRole('dialog', { name: 'Screenshot 2 von 5' });
    expect(dialog.classList.contains('run-viewer')).toBe(true);
    expect(dialog.parentElement!.classList.contains('run-viewer-backdrop')).toBe(true);
    const image = within(dialog).getByAltText('Screenshot 2 von 5') as HTMLImageElement;
    expect(image.getAttribute('src')).toBe('/shots/run/002.png');
    expect(image.classList.contains('run-shot-full')).toBe(true);
    const label = dialog.querySelector('.run-viewer-caption')!;
    expect(within(label as HTMLElement).getByText('Bash')).toBeTruthy();
    expect(within(label as HTMLElement).getByText(formatDateTime(SHOTS[1]!.at))).toBeTruthy();
    expect(within(dialog).getByText('2 von 5')).toBeTruthy();
  });

  it('nennt ohne bekannten Aufruf nur den Zeitpunkt', () => {
    open('003.png');
    const label = document.querySelector('.run-viewer-caption')!;
    expect(label.textContent).toBe(formatDateTime(SHOTS[2]!.at));
  });

  it('gibt dem Knopf „Schließen“ beim Öffnen den Fokus', () => {
    open('002.png');
    const close = screen.getByRole('button', { name: 'Schließen' });
    expect(document.activeElement).toBe(close);
    expect(close.querySelector('.material-icons')!.textContent).toBe('close');
  });

  it('blättert mit Knöpfen und Pfeiltasten', async () => {
    const { onChange } = open('002.png');
    await userEvent.click(screen.getByRole('button', { name: 'Nächster Screenshot' }));
    expect(onChange).toHaveBeenLastCalledWith('003.png');
    await userEvent.click(screen.getByRole('button', { name: 'Vorheriger Screenshot' }));
    expect(onChange).toHaveBeenLastCalledWith('001.png');
    await userEvent.keyboard('{ArrowRight}');
    expect(onChange).toHaveBeenLastCalledWith('003.png');
    await userEvent.keyboard('{ArrowLeft}');
    expect(onChange).toHaveBeenLastCalledWith('001.png');
    expect(onChange).toHaveBeenCalledTimes(4);
  });

  it('sperrt das Blättern an den Enden', async () => {
    const first = open('001.png');
    expect((screen.getByRole('button', { name: 'Vorheriger Screenshot' }) as HTMLButtonElement).disabled).toBe(true);
    expect((screen.getByRole('button', { name: 'Nächster Screenshot' }) as HTMLButtonElement).disabled).toBe(false);
    await userEvent.keyboard('{ArrowLeft}');
    expect(first.onChange).not.toHaveBeenCalled();
    first.unmount();

    const last = open('005.png');
    expect((screen.getByRole('button', { name: 'Nächster Screenshot' }) as HTMLButtonElement).disabled).toBe(true);
    await userEvent.keyboard('{ArrowRight}');
    expect(last.onChange).not.toHaveBeenCalled();
  });

  it('schließt über den Knopf und über Esc', async () => {
    const { onClose } = open('002.png');
    await userEvent.click(screen.getByRole('button', { name: 'Schließen' }));
    expect(onClose).toHaveBeenCalledTimes(1);
    await userEvent.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it('schließt bei einem Klick auf die dunkle Fläche, nicht beim Klick aufs Bild oder die Beschriftung', () => {
    const { onClose } = open('002.png');
    const image = screen.getByAltText('Screenshot 2 von 5');
    fireEvent.mouseDown(image);
    fireEvent.click(image);
    const caption = document.querySelector('.run-viewer-caption')!;
    fireEvent.mouseDown(caption);
    fireEvent.click(caption);
    // Vom Bild auf die Fläche gezogen: schließt nicht
    const surface = screen.getByRole('dialog').querySelector('.run-viewer-surface')!;
    fireEvent.mouseDown(image);
    fireEvent.click(surface);
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.mouseDown(surface);
    fireEvent.click(surface);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('füllt die Fläche im Seitenverhältnis des geladenen Bildes, höchstens in dessen eigener Breite', () => {
    open('002.png');
    const image = screen.getByAltText('Screenshot 2 von 5') as HTMLImageElement;
    expect(image.classList.contains('fitted')).toBe(false);
    Object.defineProperty(image, 'naturalWidth', { configurable: true, value: 1600 });
    Object.defineProperty(image, 'naturalHeight', { configurable: true, value: 800 });
    fireEvent.load(image);
    expect(image.classList.contains('fitted')).toBe(true);
    expect(image.style.getPropertyValue('--shot-ratio')).toBe('2');
    expect(image.style.getPropertyValue('--shot-width')).toBe('1600px');
  });

  it('zeigt bei nur einem Screenshot keinen Blätterer', () => {
    open('001.png', [SHOTS[0]!]);
    expect(screen.getByRole('dialog', { name: 'Screenshot 1 von 1' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Vorheriger Screenshot' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Nächster Screenshot' })).toBeNull();
    expect(screen.queryByText('1 von 1')).toBeNull();
  });

  it('rendert für eine unbekannte Datei nichts', () => {
    open('999.png');
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});
