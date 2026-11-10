// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { getTheme, registerTheme, unregisterTheme } from '../src/theme/registry.js';
import { displayedTheme } from '../src/theme/theme.js';

afterEach(() => {
  unregisterTheme('test-registry');
  delete document.documentElement.dataset.theme;
});

describe('Theme-Registry', () => {
  it('liefert ein registriertes Theme nach ID und vergisst es nach dem Abmelden', () => {
    const def = { id: 'test-registry', slots: {} };
    registerTheme(def);
    expect(getTheme('test-registry')).toBe(def);
    unregisterTheme('test-registry');
    expect(getTheme('test-registry')).toBeUndefined();
  });

  it('ersetzt ein Theme mit gleicher ID', () => {
    registerTheme({ id: 'test-registry', slots: {} });
    const second = { id: 'test-registry', slots: {} };
    registerTheme(second);
    expect(getTheme('test-registry')).toBe(second);
  });

  it('meldet auch Schemata außerhalb der eingebauten Liste als angezeigt', () => {
    document.documentElement.dataset.theme = 'test-registry';
    expect(displayedTheme()).toBe('test-registry');
  });

  it('fällt ohne gesetztes Schema auf hell zurück', () => {
    expect(displayedTheme()).toBe('light');
  });
});
