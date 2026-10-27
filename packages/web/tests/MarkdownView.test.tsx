// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { MarkdownView } from '../src/markdown/MarkdownView.js';

const mermaid = vi.hoisted(() => ({
  initialize: vi.fn(),
  render: vi.fn(async (_id: string, source: string) => {
    if (source.includes('KAPUTT')) throw new Error('Parse error on line 1');
    return { svg: '<svg data-testid="diagramm"><text>gezeichnet</text></svg>' };
  }),
}));
vi.mock('mermaid', () => ({ default: mermaid }));

afterEach(cleanup);

describe('MarkdownView', () => {
  it('stellt Überschriften, Listen und Codeblöcke dar', () => {
    const { container } = render(<MarkdownView source={'# Titel\n\n- eins\n- zwei\n\n```ts\nconst a = 1;\n```'} />);
    expect(container.querySelector('h1')?.textContent).toBe('Titel');
    expect(container.querySelectorAll('li')).toHaveLength(2);
    expect(container.querySelector('pre code')?.textContent).toContain('const a = 1;');
  });

  it('entfernt Skripte und Ereignisattribute aus dem HTML', () => {
    const { container } = render(
      <MarkdownView source={'Text <script>window.hacked = 1</script> <img src="x" onerror="window.hacked = 2"> <a href="javascript:alert(1)">klick</a>'} />,
    );
    expect(container.querySelector('script')).toBeNull();
    expect(container.querySelector('img')?.getAttribute('onerror')).toBeNull();
    expect(container.querySelector('a')?.getAttribute('href') ?? '').not.toContain('javascript');
    expect((window as unknown as { hacked?: number }).hacked).toBeUndefined();
  });

  it('öffnet Links sicher in neuem Tab', () => {
    const { container } = render(<MarkdownView source="[Doku](https://example.com)" />);
    const link = container.querySelector('a')!;
    expect(link.getAttribute('target')).toBe('_blank');
    expect(link.getAttribute('rel')).toContain('noopener');
  });

  it('zeichnet Mermaid-Blöcke als Grafik und lässt den Text drumherum stehen', async () => {
    render(<MarkdownView source={'Vorher\n\n```mermaid\nflowchart LR\n  A --> B\n```\n\nNachher'} />);
    expect(await screen.findByText('gezeichnet')).toBeTruthy();
    expect(screen.getByText('Vorher')).toBeTruthy();
    expect(screen.getByText('Nachher')).toBeTruthy();
    expect(mermaid.initialize).toHaveBeenCalledWith(expect.objectContaining({ securityLevel: 'strict', startOnLoad: false }));
  });

  it('zeigt bei fehlerhaftem Diagramm Meldung samt Quelltext, die Seite bleibt benutzbar', async () => {
    render(<MarkdownView source={'```mermaid\nKAPUTT --> ???\n```\n\nRest bleibt'} />);
    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('Diagramm kann nicht gezeichnet werden');
    expect(alert.textContent).toContain('Parse error on line 1');
    expect(alert.textContent).toContain('KAPUTT --> ???');
    expect(screen.getByText('Rest bleibt')).toBeTruthy();
  });
});
