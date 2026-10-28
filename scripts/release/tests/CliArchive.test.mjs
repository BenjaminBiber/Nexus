import { describe, expect, it } from 'vitest';
import { archiveExtension, archiveName, archiveStem, binaryName, isWindowsTarget } from '../lib/CliArchive.mjs';

describe('CliArchive', () => {
  it('benennt das Archiv nach Version und Ziel', () => {
    expect(archiveName('0.2.0', 'aarch64-apple-darwin')).toBe('nexus-cli-0.2.0-aarch64-apple-darwin.tar.gz');
    expect(archiveName('1.0.0', 'x86_64-unknown-linux-gnu')).toBe('nexus-cli-1.0.0-x86_64-unknown-linux-gnu.tar.gz');
  });

  it('packt für Windows ein Zip', () => {
    expect(archiveName('0.2.0', 'x86_64-pc-windows-msvc')).toBe('nexus-cli-0.2.0-x86_64-pc-windows-msvc.zip');
    expect(archiveExtension('x86_64-pc-windows-msvc')).toBe('.zip');
    expect(archiveExtension('aarch64-apple-darwin')).toBe('.tar.gz');
  });

  it('nimmt die Version so, wie sie kommt – auch eine Vorabversion', () => {
    expect(archiveName('0.3.0-rc.1', 'aarch64-apple-darwin')).toBe('nexus-cli-0.3.0-rc.1-aarch64-apple-darwin.tar.gz');
  });

  it('nennt den Ordner im Archiv wie das Archiv ohne Endung', () => {
    expect(archiveStem('0.2.0', 'aarch64-apple-darwin')).toBe('nexus-cli-0.2.0-aarch64-apple-darwin');
  });

  it('hängt nur unter Windows .exe an das Programm', () => {
    expect(binaryName('x86_64-pc-windows-msvc')).toBe('harness.exe');
    expect(binaryName('aarch64-pc-windows-msvc')).toBe('harness.exe');
    expect(binaryName('x86_64-unknown-linux-gnu')).toBe('harness');
  });

  it('erkennt Windows-Ziele an ihrem Namen', () => {
    expect(isWindowsTarget('x86_64-pc-windows-msvc')).toBe(true);
    expect(isWindowsTarget('x86_64-apple-darwin')).toBe(false);
  });
});
