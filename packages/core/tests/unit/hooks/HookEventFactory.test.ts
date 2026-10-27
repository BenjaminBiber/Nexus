import { describe, it, expect } from 'vitest';
import { HookEventFactory } from '../../../src/hooks/HookEventFactory.js';

describe('HookEventFactory', () => {
  const factory = new HookEventFactory();

  it('bildet die Eingabe von Claude Code auf ein normalisiertes Ereignis ab', () => {
    const event = factory.create('subagent-start', {
      session_id: 'abc', cwd: 'D:\\Entwicklung\\Projekt', agent_id: 'a1', agent_type: 'Explore', model: 'm', session_title: 'Titel',
    }, 'C:/fallback', ['T-0001']);
    expect(event).toEqual({
      kind: 'subagent-start', sessionId: 'abc', cwd: 'D:/Entwicklung/Projekt', tickets: ['T-0001'],
      agentId: 'a1', agentType: 'Explore', model: 'm', title: 'Titel',
    });
  });

  it('liest die Art der Benachrichtigung aus notification_type oder type', () => {
    expect(factory.create('notification', { session_id: 's', notification_type: 'permission_prompt' }, '/x', [])?.notificationType).toBe('permission_prompt');
    expect(factory.create('notification', { session_id: 's', type: 'idle_prompt' }, '/x', [])?.notificationType).toBe('idle_prompt');
  });

  it('übernimmt die Quelle des Sessionstarts', () => {
    expect(factory.create('session-start', { session_id: 's', source: 'resume' }, '/x', [])?.source).toBe('resume');
    expect(factory.create('session-start', { session_id: 's' }, '/x', [])).not.toHaveProperty('source');
  });

  it('nimmt ohne cwd das Ausweichverzeichnis', () => {
    expect(factory.create('stop', { session_id: 's' }, 'D:\\wt', [])?.cwd).toBe('D:/wt');
  });

  it('liefert ohne Session-Kennung nichts', () => {
    expect(factory.create('stop', {}, '/x', [])).toBeNull();
    expect(factory.create('stop', { session_id: '' }, '/x', [])).toBeNull();
    expect(factory.create('stop', { session_id: 42 as unknown as string }, '/x', [])).toBeNull();
  });

  it('weist unbekannte Ereignisse ab', () => {
    expect(factory.create('quatsch' as never, { session_id: 's' }, '/x', [])).toBeNull();
  });

  it('ignoriert Eingaben, die kein Objekt sind', () => {
    expect(factory.create('stop', null as never, '/x', [])).toBeNull();
    expect(factory.create('stop', 'text' as never, '/x', [])).toBeNull();
  });
});
