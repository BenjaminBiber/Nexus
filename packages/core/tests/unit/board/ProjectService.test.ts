import { describe, expect, it } from 'vitest';
import { BoardFixture } from './helpers/BoardFixture.js';

describe('ProjectService', () => {
  it('legt ein Projekt mit normalisiertem Key an und listet es', async () => {
    const f = await BoardFixture.create();
    const p = await f.projects.create('inc', 'Incidents', f.actor);
    expect(p).toMatchObject({ key: 'INC', name: 'Incidents', nextTicketNumber: 1, createdBy: f.actor.userId });
    expect((await f.projects.list()).map((x) => x.key)).toEqual(['ARC', 'INC']);
  });

  it('lehnt einen doppelten Key ab', async () => {
    const f = await BoardFixture.create();
    await expect(f.projects.create('ARC', 'Nochmal', f.actor)).rejects.toMatchObject({ code: 'project-exists' });
  });

  it('lehnt einen leeren Namen ab', async () => {
    const f = await BoardFixture.create();
    await expect(f.projects.create('TLS', '  ', f.actor)).rejects.toMatchObject({ code: 'project-name-empty' });
  });

  it('benennt um und meldet unbekannte Projekte', async () => {
    const f = await BoardFixture.create();
    expect((await f.projects.rename('arc', 'Arches Shop')).name).toBe('Arches Shop');
    await expect(f.projects.rename('XYZ', 'n')).rejects.toMatchObject({ code: 'project-not-found' });
  });

  it('lehnt zu lange Projektnamen ab', async () => {
    const f = await BoardFixture.create();
    await expect(f.projects.create('INC', 'n'.repeat(201), f.actor)).rejects.toMatchObject({ code: 'value-too-long' });
    await expect(f.projects.rename('ARC', 'n'.repeat(201))).rejects.toMatchObject({ code: 'value-too-long' });
  });
});
