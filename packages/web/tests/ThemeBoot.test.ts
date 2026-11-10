// @vitest-environment jsdom
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

// Erstes Inline-Skript aus index.html: setzt Schema, theme.css-Link und Startfarbe vor dem ersten Rendern
const html = readFileSync(join(import.meta.dirname, '..', 'index.html'), 'utf8');
const bootScript = /<script>([\s\S]*?)<\/script>/.exec(html)![1];

function boot(cache: Record<string, string>) {
  for (const [key, value] of Object.entries(cache)) window.localStorage.setItem(key, value);
  new Function(bootScript)();
}

const link = () => document.getElementById('nexus-theme-css') as HTMLLinkElement | null;

afterEach(() => {
  window.localStorage.clear();
  link()?.remove();
  document.documentElement.removeAttribute('style');
  delete document.documentElement.dataset.theme;
  delete document.documentElement.dataset.themeMode;
});

describe('index.html – früher Start', () => {
  it('setzt Link, data-theme und Startfarbe eines installierten Themes', () => {
    boot({ 'harness-theme-mode': 'ocean', 'harness-theme-css': '/themes/ocean/theme.css', 'harness-theme-start-color': '#123456' });
    expect(link()?.getAttribute('href')).toBe('/themes/ocean/theme.css');
    expect(link()?.rel).toBe('stylesheet');
    expect(link()?.parentNode).toBe(document.head);
    expect(document.documentElement.dataset.theme).toBe('ocean');
    expect(document.documentElement.style.getPropertyValue('--start-color')).toBe('#123456');
  });

  it('nutzt den Startfarben-Fallback im Style der Startanimation', () => {
    expect(html).toContain('background: var(--start-color, #f7f7f8)');
  });

  it('fällt ohne theme.css-URL auf „system“ zurück', () => {
    boot({ 'harness-theme-mode': 'ocean' });
    expect(link()).toBeNull();
    expect(document.documentElement.dataset.themeMode).toBe('system');
  });

  it('fällt bei unpassender oder fremder URL auf „system“ zurück', () => {
    for (const css of ['/themes/wald/theme.css', 'https://evil.example/theme.css']) {
      boot({ 'harness-theme-mode': 'ocean', 'harness-theme-css': css });
      expect(link()).toBeNull();
      expect(document.documentElement.dataset.themeMode).toBe('system');
    }
  });

  it('fällt bei ungültiger ID auf „system“ zurück', () => {
    boot({ 'harness-theme-mode': '../x', 'harness-theme-css': '/themes/../x/theme.css' });
    expect(link()).toBeNull();
    expect(document.documentElement.dataset.themeMode).toBe('system');
  });

  it('ignoriert eine ungültige Startfarbe', () => {
    boot({ 'harness-theme-mode': 'ocean', 'harness-theme-css': '/themes/ocean/theme.css', 'harness-theme-start-color': 'red;x' });
    expect(document.documentElement.style.getPropertyValue('--start-color')).toBe('');
  });

  it('setzt für eingebaute Themes keinen Link', () => {
    boot({ 'harness-theme-mode': 'dark', 'harness-theme-css': '/themes/dark/theme.css' });
    expect(link()).toBeNull();
    expect(document.documentElement.dataset.theme).toBe('dark');
  });

  it('startet „Windows 95“ als installiertes Theme mit theme.css und grauer Startfarbe', () => {
    boot({ 'harness-theme-mode': 'win95', 'harness-theme-css': '/themes/win95/theme.css', 'harness-theme-start-color': '#c0c0c0' });
    expect(link()?.getAttribute('href')).toBe('/themes/win95/theme.css');
    expect(document.documentElement.dataset.theme).toBe('win95');
    expect(document.documentElement.style.getPropertyValue('--start-color')).toBe('#c0c0c0');
  });
});
