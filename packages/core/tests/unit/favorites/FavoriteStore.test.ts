import { describe, it, expect } from 'vitest';
import { FavoriteStore } from '../../../src/favorites/FavoriteStore.js';
import { ResumeCommand } from '../../../src/favorites/ResumeCommand.js';

const snap = (id: string, extra: object = {}) => ({
  sessionId: id, tickets: ['T-0001'], cwd: 'D:/Mein Projekt/wt', title: 'Login', state: 'running' as const, lastEventAt: '2026-10-10T10:00:00.000Z', ...extra,
});

describe('FavoriteStore', () => {
  const store = () => new FavoriteStore([], () => new Date('2026-10-10T12:00:00.000Z'));

  it('markiert eine Session mit Momentaufnahme und Notiz', () => {
    const s = store();
    const f = s.add('p1', snap('s1'), 'Wichtig');
    expect(f).toMatchObject({ projectId: 'p1', sessionId: 's1', note: 'Wichtig', tickets: ['T-0001'], cwd: 'D:/Mein Projekt/wt', title: 'Login', lastState: 'running', resumedAs: [] });
    expect(f.markedAt).toBe('2026-10-10T12:00:00.000Z');
    expect(s.isFavorite('p1', 's1')).toBe(true);
    expect(s.isFavorite('p1', 'x')).toBe(false);
  });

  it('ist idempotent: erneutes Markieren behält die Notiz und aktualisiert die Momentaufnahme', () => {
    const s = store();
    s.add('p1', snap('s1'), 'Notiz');
    const again = s.add('p1', snap('s1', { state: 'idle', tickets: ['T-0001', 'T-0002'] }));
    expect(again.note).toBe('Notiz');
    expect(again.lastState).toBe('idle');
    expect(again.tickets).toEqual(['T-0001', 'T-0002']);
    expect(s.list('p1')).toHaveLength(1);
  });

  it('ändert die Notiz und entfernt Favoriten', () => {
    const s = store();
    s.add('p1', snap('s1'));
    expect(s.setNote('p1', 's1', 'Neu').note).toBe('Neu');
    expect(s.remove('p1', 's1')).toBe(true);
    expect(s.remove('p1', 's1')).toBe(false);
    expect(s.list('p1')).toEqual([]);
  });

  it('meldet eine unbekannte Notiz-Änderung verständlich', () => {
    expect(() => store().setNote('p1', 'gibtsnicht', 'x')).toThrow(/Favorit/);
  });

  it('hält Projekte getrennt und sortiert neueste Markierung zuerst', () => {
    let tick = 0;
    const s = new FavoriteStore([], () => new Date(2026, 9, 10, 12, 0, tick++));
    s.add('p1', snap('a'));
    s.add('p2', snap('b'));
    s.add('p1', snap('c'));
    expect(s.list('p1').map((f) => f.sessionId)).toEqual(['c', 'a']);
    expect(s.list('p2').map((f) => f.sessionId)).toEqual(['b']);
  });

  it('aktualisiert die Momentaufnahme eines Favoriten (refresh) und ignoriert Unbekannte', () => {
    const s = store();
    s.add('p1', snap('s1'));
    s.refresh('p1', snap('s1', { state: 'ended', lastEventAt: '2026-10-10T11:00:00.000Z' }));
    s.refresh('p1', snap('unbekannt'));
    expect(s.list('p1')[0]).toMatchObject({ lastState: 'ended', lastEventAt: '2026-10-10T11:00:00.000Z' });
    expect(s.list('p1')).toHaveLength(1);
  });

  describe('linkResumed', () => {
    it('ordnet eine fortgesetzte Session dem einzigen Favoriten im selben Arbeitsverzeichnis zu', () => {
      const s = store();
      s.add('p1', snap('s1'));
      const linked = s.linkResumed('p1', 'D:\\Mein Projekt\\wt', 'neu-1');
      expect(linked?.sessionId).toBe('s1');
      expect(s.list('p1')[0].resumedAs).toEqual(['neu-1']);
      s.linkResumed('p1', 'D:/Mein Projekt/wt', 'neu-1');
      expect(s.list('p1')[0].resumedAs).toEqual(['neu-1']);
    });

    it('ordnet bei mehreren passenden Favoriten oder anderem Verzeichnis nichts zu', () => {
      const s = store();
      s.add('p1', snap('a'));
      s.add('p1', snap('b'));
      expect(s.linkResumed('p1', 'D:/Mein Projekt/wt', 'x')).toBeNull();
      expect(s.linkResumed('p1', 'D:/anderswo', 'x')).toBeNull();
      expect(s.linkResumed('p2', 'D:/Mein Projekt/wt', 'x')).toBeNull();
    });

    it('ordnet die Session eines Favoriten selbst nicht sich selbst zu', () => {
      const s = store();
      s.add('p1', snap('s1'));
      expect(s.linkResumed('p1', 'D:/Mein Projekt/wt', 's1')).toBeNull();
    });
  });

  it('übernimmt gespeicherte Favoriten', () => {
    const s = store();
    s.add('p1', snap('s1'), 'N');
    const copy = new FavoriteStore(s.toJSON());
    expect(copy.list('p1')[0].note).toBe('N');
  });
});

describe('ResumeCommand', () => {
  it('baut den Befehl mit quotiertem Pfad', () => {
    expect(ResumeCommand.build('D:/Mein Projekt ä/wt', 'abc-123')).toBe('cd "D:/Mein Projekt ä/wt" && claude --resume abc-123');
  });

  it('maskiert Anführungszeichen im Pfad und quotiert ungewöhnliche Kennungen', () => {
    expect(ResumeCommand.build('D:/a"b', 'x')).toBe('cd "D:/a\\"b" && claude --resume x');
    expect(ResumeCommand.build('/tmp', 'a b;c')).toBe("cd \"/tmp\" && claude --resume 'a b;c'");
  });
});
