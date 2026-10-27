// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DiagramViewer } from '../src/markdown/DiagramViewer.js';
import { MermaidBlock } from '../src/markdown/MermaidBlock.js';
import { applyTheme } from '../src/theme/theme.js';
import { clampScale, fitView, naturalSize, wheelFactor, zoomAt } from '../src/markdown/diagramView.js';

vi.mock('mermaid', () => ({ default: { initialize: vi.fn(), render: vi.fn(async () => ({ svg: '<svg viewBox="0 0 400 200"><text>A</text></svg>' })) } }));
afterEach(cleanup);

describe('diagramView', () => {
  it('begrenzt den Maßstab', () => {
    expect(clampScale(0.01)).toBe(0.1);
    expect(clampScale(20)).toBe(8);
    expect(clampScale(Number.NaN)).toBe(1);
  });

  it('passt ein Diagramm mittig mit Rand ein', () => {
    expect(fitView({ w: 200, h: 100 }, { w: 448, h: 448 })).toEqual({ scale: 2, x: 24, y: 124 });
  });

  it('zoomt um einen Punkt, der dabei stehen bleibt', () => {
    expect(zoomAt({ scale: 1, x: 0, y: 0 }, 2, { x: 100, y: 50 })).toEqual({ scale: 2, x: -100, y: -50 });
  });

  it('rechnet Radwege in Zoomfaktoren um', () => {
    expect(wheelFactor(-100)).toBeCloseTo(1.15);
    expect(wheelFactor(100)).toBeCloseTo(1 / 1.15);
    expect(wheelFactor(-3, 1)).toBeCloseTo(1.15);
  });

  it('liest die natürliche Größe aus viewBox oder width/height', () => {
    expect(naturalSize('<svg viewBox="0 0 400 200"></svg>')).toEqual({ w: 400, h: 200 });
    expect(naturalSize('<svg width="300" height="150"></svg>')).toEqual({ w: 300, h: 150 });
    expect(naturalSize('<svg></svg>')).toEqual({ w: 800, h: 600 });
  });
});

describe('DiagramViewer', () => {
  it('öffnet ein gezeichnetes Diagramm im Vollbild mit Zoom und schließt mit Esc', async () => {
    render(<MermaidBlock source="graph TD; A-->B" title="ARC-0001 – Diagramm" />);
    await userEvent.click(await screen.findByRole('button', { name: 'Diagramm im Vollbild öffnen' }));
    const dialog = screen.getByRole('dialog', { name: 'Diagramm: ARC-0001 – Diagramm' });
    await userEvent.click(within(dialog).getByRole('button', { name: '1:1' }));
    expect(within(dialog).getByText('100 %')).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Vergrößern' }));
    expect(within(dialog).getByText('125 %')).toBeTruthy();
    await userEvent.keyboard('-');
    expect(within(dialog).getByText('100 %')).toBeTruthy();
    await userEvent.click(within(dialog).getByRole('button', { name: 'Verkleinern' }));
    expect(within(dialog).getByText('80 %')).toBeTruthy();
    await userEvent.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('gibt Esc nicht an umgebende Elemente weiter', async () => {
    const outer = vi.fn();
    render(
      <div onKeyDown={(e) => e.key === 'Escape' && outer()}>
        <MermaidBlock source="graph TD; A-->B" />
      </div>,
    );
    await userEvent.click(await screen.findByRole('button', { name: 'Diagramm im Vollbild öffnen' }));
    fireEvent.keyDown(screen.getByRole('dialog', { name: 'Diagramm: Diagramm' }), { key: 'Escape' });
    expect(outer).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog')).toBeNull();
  });
});

describe('DiagramViewer – Bühne', () => {
  const SVG = '<svg viewBox="0 0 400 200"><text>A</text></svg>';
  let stageSize = { w: 448, h: 448 };

  beforeEach(() => {
    stageSize = { w: 448, h: 448 };
    // jsdom kennt kein PointerEvent; ohne es fehlen clientX/clientY
    if (!('PointerEvent' in window)) {
      class PointerEventStub extends MouseEvent {
        pointerId: number;
        constructor(type: string, init: PointerEventInit = {}) {
          super(type, init);
          this.pointerId = init.pointerId ?? 0;
        }
      }
      vi.stubGlobal('PointerEvent', PointerEventStub);
    }
    vi.spyOn(HTMLElement.prototype, 'getBoundingClientRect').mockImplementation(function (this: HTMLElement) {
      const stage = this.classList.contains('diagram-viewer-stage');
      const w = stage ? stageSize.w : 0;
      const h = stage ? stageSize.h : 0;
      return { width: w, height: h, left: 0, top: 0, right: w, bottom: h, x: 0, y: 0, toJSON: () => ({}) } as DOMRect;
    });
  });
  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  const canvas = () => document.querySelector<HTMLElement>('.diagram-viewer-canvas')!;
  const stage = () => document.querySelector<HTMLElement>('.diagram-viewer-stage')!;

  it('passt beim Öffnen ein und beim Ändern der Fenstergröße neu ein', () => {
    render(<DiagramViewer svg={SVG} title="T" onClose={() => undefined} />);
    expect(screen.getByText('100 %')).toBeTruthy();
    stageSize = { w: 848, h: 848 };
    act(() => {
      window.dispatchEvent(new Event('resize'));
    });
    expect(screen.getByText('200 %')).toBeTruthy();
  });

  it('passt beim Wechsel ins Browser-Vollbild neu ein und ändert die Beschriftung', () => {
    render(<DiagramViewer svg={SVG} title="T" onClose={() => undefined} />);
    expect(screen.getByRole('button', { name: 'Browser-Vollbild' })).toBeTruthy();
    stageSize = { w: 848, h: 848 };
    Object.defineProperty(document, 'fullscreenElement', { configurable: true, get: () => screen.getByRole('dialog') });
    try {
      act(() => {
        document.dispatchEvent(new Event('fullscreenchange'));
      });
      expect(screen.getByText('200 %')).toBeTruthy();
      expect(screen.getByRole('button', { name: 'Vollbild beenden' })).toBeTruthy();
    } finally {
      delete (document as { fullscreenElement?: unknown }).fullscreenElement;
    }
    act(() => {
      document.dispatchEvent(new Event('fullscreenchange'));
    });
    expect(screen.getByRole('button', { name: 'Browser-Vollbild' })).toBeTruthy();
  });

  it('beendet das Ziehen bei pointercancel und lostpointercapture', () => {
    render(<DiagramViewer svg={SVG} title="T" onClose={() => undefined} />);
    const before = canvas().style.transform;
    for (const end of [fireEvent.pointerCancel, fireEvent.lostPointerCapture]) {
      fireEvent.pointerDown(stage(), { clientX: 0, clientY: 0, pointerId: 1 });
      end(stage(), { pointerId: 1 });
      fireEvent.pointerMove(stage(), { clientX: 50, clientY: 50, pointerId: 1 });
      expect(canvas().style.transform).toBe(before);
    }
  });

  it('zoomt mit Strg+Mausrad und verhindert dabei den Browser-Zoom', () => {
    render(<DiagramViewer svg={SVG} title="T" onClose={() => undefined} />);
    const notCancelled = fireEvent.wheel(stage(), { ctrlKey: true, deltaY: -100, clientX: 224, clientY: 224 });
    expect(notCancelled).toBe(false);
    expect(screen.getByText('115 %')).toBeTruthy();
  });
});

describe('MermaidBlock – Neuzeichnen', () => {
  it('behält beim Neuzeichnen das bisherige Diagramm, damit ein offenes Vollbild offen bleibt', async () => {
    const mermaid = (await import('mermaid')).default as unknown as { render: ReturnType<typeof vi.fn> };
    render(<MermaidBlock source="graph TD; A-->B" title="T" />);
    await userEvent.click(await screen.findByRole('button', { name: 'Diagramm im Vollbild öffnen' }));
    expect(screen.getByRole('dialog', { name: 'Diagramm: T' })).toBeTruthy();
    let finish: (value: { svg: string }) => void = () => undefined;
    mermaid.render.mockImplementationOnce(() => new Promise((resolve) => (finish = resolve)));
    const calls = mermaid.render.mock.calls.length;
    act(() => applyTheme('dark'));
    await waitFor(() => expect(mermaid.render.mock.calls.length).toBe(calls + 1));
    expect(screen.queryByText('Diagramm wird gezeichnet …')).toBeNull();
    expect(screen.getByRole('dialog', { name: 'Diagramm: T' })).toBeTruthy();
    await act(async () => finish({ svg: '<svg viewBox="0 0 400 200"><text>Neu</text></svg>' }));
    const dialog = screen.getByRole('dialog', { name: 'Diagramm: T' });
    expect(within(dialog).getByText('Neu')).toBeTruthy();
    delete document.documentElement.dataset.theme;
    delete document.documentElement.dataset.themeMode;
  });
});
