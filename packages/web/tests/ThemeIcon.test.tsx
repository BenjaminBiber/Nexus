// @vitest-environment jsdom
// Slot „Icon“: Symbole im Kern, die ein Theme ersetzen darf; ohne Slot oder bei einem Fehler bleibt das Standard-Symbol
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render } from '@testing-library/react';
import type { IconName } from '@nexus/theme-sdk';
import { registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { ThemeIcon } from '../src/theme/ThemeIcon.js';

afterEach(() => {
  cleanup();
  unregisterTheme('symbole');
  delete document.documentElement.dataset.theme;
});

describe('ThemeIcon', () => {
  it('zeigt ohne Slot das Standard-Symbol, ohne Standard nichts', () => {
    const { container } = render(
      <>
        <ThemeIcon name="doc"><svg className="standard" /></ThemeIcon>
        <ThemeIcon name="folder" />
      </>,
    );
    expect(container.querySelectorAll('svg')).toHaveLength(1);
    expect(container.querySelector('.standard')).toBeTruthy();
  });

  it('zeigt im Theme mit Slot „Icon“ dessen Symbol mit dem Namen', () => {
    registerTheme({ id: 'symbole', slots: { Icon: ({ name }: { name: IconName }) => <i data-name={name} /> } });
    document.documentElement.dataset.theme = 'symbole';
    const { container } = render(<ThemeIcon name="folderOpen"><svg className="standard" /></ThemeIcon>);
    expect(container.querySelector('i')?.dataset.name).toBe('folderOpen');
    expect(container.querySelector('.standard')).toBeNull();
  });

  it('fällt auf das Standard-Symbol zurück, wenn der Slot scheitert', () => {
    const error = vi.spyOn(console, 'error').mockImplementation(() => {});
    registerTheme({ id: 'symbole', slots: { Icon: () => { throw new Error('kaputt'); } } });
    document.documentElement.dataset.theme = 'symbole';
    const { container } = render(<ThemeIcon name="empty"><svg className="standard" /></ThemeIcon>);
    expect(container.querySelector('.standard')).toBeTruthy();
    error.mockRestore();
  });
});
