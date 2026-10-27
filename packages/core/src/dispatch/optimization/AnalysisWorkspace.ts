import { mkdir } from 'node:fs/promises';
import type { HarnessHome } from '../../registry/HarnessHome.js';

// Arbeitsordner der Claude-Analyse (~/.harness/dispatch/analysis): absichtlich leer, damit Claude dort kein Projekt vorfindet.
export class AnalysisWorkspace {
  constructor(private readonly home: HarnessHome) {}

  // Legt den Ordner an, falls er fehlt, und liefert seinen Pfad
  async dir(): Promise<string> {
    await mkdir(this.home.dispatchAnalysisDir, { recursive: true });
    return this.home.dispatchAnalysisDir;
  }
}
