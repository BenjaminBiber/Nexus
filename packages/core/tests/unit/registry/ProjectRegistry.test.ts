import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { mkdtempSync, rmSync, writeFileSync, existsSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { TempRepo } from '../helpers/TempRepo.js';
import { HarnessProject } from '../../../src/project/HarnessProject.js';
import { HarnessHome } from '../../../src/registry/HarnessHome.js';
import { ProjectRegistry } from '../../../src/registry/ProjectRegistry.js';
import { ServerInfoFile } from '../../../src/registry/ServerInfoFile.js';

describe('HarnessHome', () => {
  it('nutzt HARNESS_HOME, sonst .harness im Benutzerordner', () => {
    expect(new HarnessHome({ HARNESS_HOME: '/x/y' }).dir.replace(/\\/g, '/')).toBe('/x/y');
    expect(new HarnessHome({}).dir.replace(/\\/g, '/')).toMatch(/\/\.harness$/);
  });
});

describe('ProjectRegistry', () => {
  let home: string;
  let repo: TempRepo;
  let registry: ProjectRegistry;

  beforeEach(async () => {
    home = mkdtempSync(join(tmpdir(), 'harness-home-'));
    repo = new TempRepo();
    await HarnessProject.init(repo.path);
    registry = new ProjectRegistry(new HarnessHome({ HARNESS_HOME: home }));
  });
  afterEach(() => {
    rmSync(home, { recursive: true, force: true });
    repo.dispose();
  });

  it('ist anfangs leer', async () => {
    expect(await registry.list()).toEqual([]);
  });

  it('meldet ein Projekt an, mit stabiler ID und Ordnername', async () => {
    const result = await registry.add(repo.path);
    expect(result.created).toBe(true);
    expect(result.project.id).toMatch(/^[0-9a-f]{8}$/);
    expect(result.project.name).toBe('Mein Projekt ä');
    expect((await registry.add(repo.path)).project.id).toBe(result.project.id);
  });

  it('meldet dasselbe Projekt aus einem Worktree unter derselben ID an und legt keinen zweiten Eintrag an', async () => {
    const first = await registry.add(repo.path);
    const worktree = repo.addWorktree('wt');
    const second = await registry.add(worktree);
    expect(second.created).toBe(false);
    expect(second.project.id).toBe(first.project.id);
    expect(await registry.list()).toHaveLength(1);
  });

  it('lehnt einen Ordner ohne Git oder ohne harness init ab', async () => {
    const plain = mkdtempSync(join(tmpdir(), 'harness-plain-'));
    try {
      await expect(registry.add(plain)).rejects.toThrow(/Git/);
      const bare = new TempRepo();
      try {
        await expect(registry.add(bare.path)).rejects.toThrow(/harness init/);
      } finally {
        bare.dispose();
      }
    } finally {
      rmSync(plain, { recursive: true, force: true });
    }
  });

  it('verliert bei gleichzeitigen Anmeldungen nichts', async () => {
    const others = await Promise.all(Array.from({ length: 4 }, async () => {
      const r = new TempRepo();
      await HarnessProject.init(r.path);
      return r;
    }));
    try {
      await Promise.all(others.map((r) => registry.add(r.path)));
      expect(await registry.list()).toHaveLength(4);
    } finally {
      others.forEach((r) => r.dispose());
    }
  });

  it('markiert Projekte mit fehlendem Ordner als nicht erreichbar, ohne sie zu löschen', async () => {
    await registry.add(repo.path);
    repo.dispose();
    const [entry] = await registry.listWithState();
    expect(entry.reachable).toBe(false);
    expect(await registry.list()).toHaveLength(1);
    repo = new TempRepo(); // für afterEach
  });

  it('meldet eine beschädigte Projektliste verständlich', async () => {
    writeFileSync(join(home, 'projects.json'), '{kaputt');
    await expect(registry.list()).rejects.toThrow(/projects\.json/);
  });
});

describe('ServerInfoFile', () => {
  it('schreibt, liest und löscht die Prozessangaben', async () => {
    const home = mkdtempSync(join(tmpdir(), 'harness-info-'));
    try {
      const file = new ServerInfoFile(new HarnessHome({ HARNESS_HOME: home }));
      expect(await file.read()).toBeNull();
      await file.write({ pid: 42, port: 4710, startedAt: '2026-10-09T10:00:00.000Z' });
      expect(await file.read()).toEqual({ pid: 42, port: 4710, startedAt: '2026-10-09T10:00:00.000Z' });
      expect(JSON.parse(readFileSync(join(home, 'server.json'), 'utf8')).port).toBe(4710);
      await file.remove();
      expect(existsSync(join(home, 'server.json'))).toBe(false);
    } finally {
      rmSync(home, { recursive: true, force: true });
    }
  });
});
