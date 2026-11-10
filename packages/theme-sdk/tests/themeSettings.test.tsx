// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { ThemeHostProvider, useThemeSettings } from '@nexus/theme-sdk';
import type { ThemeDefinition, ThemeHost, ThemeSettingsSchema } from '@nexus/theme-sdk';

afterEach(cleanup);

const schema = {
  sounds: { type: 'boolean', label: 'Sounds', default: true },
  style: { type: 'select', label: 'Stil', options: [{ value: 'pixel', label: 'Pixel' }], default: 'pixel' },
} satisfies ThemeSettingsSchema;

// Theme-Komponente nur mit Importen aus dem SDK und react
function Sounds() {
  const { values, set } = useThemeSettings<typeof schema>();
  const on: boolean = values.sounds;
  const style: string = values.style;
  return <button onClick={() => void set('sounds', false)}>{`${on ? 'an' : 'aus'}|${style}`}</button>;
}

describe('useThemeSettings', () => {
  it('liest die Werte und speichert eine Änderung über den Host', () => {
    const set = vi.fn(async () => {});
    const host = { themeSettings: { values: { sounds: true, style: 'pixel' }, loading: false, set } } as unknown as ThemeHost;
    render(
      <ThemeHostProvider value={host}>
        <Sounds />
      </ThemeHostProvider>,
    );
    fireEvent.click(screen.getByRole('button', { name: 'an|pixel' }));
    expect(set).toHaveBeenCalledWith('sounds', false);
  });

  it('wirft ohne Provider mit Nennung des Hooks', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    expect(() => render(<Sounds />)).toThrow(/useThemeSettings braucht den ThemeHostProvider/);
  });

  it('ThemeDefinition nimmt ein settingsSchema auf', () => {
    const theme = { id: 'mit-schema', slots: {}, settingsSchema: schema } satisfies ThemeDefinition;
    expect(Object.keys(theme.settingsSchema)).toEqual(['sounds', 'style']);
  });
});
