import { describe, expect, it } from 'vitest';
import { matches, pick } from '../pick-runners.mjs';

const runner = (name, status, ...labels) => ({ name, status, labels: labels.map((label) => ({ name: label })) });

describe('pick-runners', () => {
  it('nimmt einen eigenen Runner, der online ist und alle Labels trägt', () => {
    const result = pick([runner('WS-023', 'online', 'self-hosted', 'Windows', 'X64')]);
    expect(JSON.parse(result.windows.runsOn)).toEqual(['self-hosted', 'Windows', 'X64']);
    expect(result.windows.names).toEqual(['WS-023']);
  });

  it('fällt ohne passenden Runner auf den gehosteten zurück', () => {
    const result = pick([runner('WS-023', 'online', 'self-hosted', 'Windows', 'X64')]);
    expect(JSON.parse(result.macos.runsOn)).toBe('macos-14');
  });

  it('übergeht Runner, die offline sind', () => {
    const result = pick([runner('WS-023', 'offline', 'self-hosted', 'Windows', 'X64')]);
    expect(JSON.parse(result.windows.runsOn)).toBe('windows-latest');
  });

  it('verlangt die Architektur', () => {
    expect(matches(runner('mac-intel', 'online', 'self-hosted', 'macOS', 'X64'), ['self-hosted', 'macOS', 'ARM64'])).toBe(false);
  });

  it('vergleicht Labels ohne Groß- und Kleinschreibung', () => {
    expect(matches(runner('mac', 'online', 'Self-Hosted', 'macos', 'arm64'), ['self-hosted', 'macOS', 'ARM64'])).toBe(true);
  });

  it('nimmt ohne Runner-Liste überall die gehosteten', () => {
    const result = pick([]);
    expect(JSON.parse(result.windows.runsOn)).toBe('windows-latest');
    expect(JSON.parse(result.macos.runsOn)).toBe('macos-14');
  });
});
