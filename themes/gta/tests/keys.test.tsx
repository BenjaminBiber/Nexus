// @vitest-environment jsdom
// Cheats und Tasten: ein keydown-Lauscher auf window, Felder bleiben unberührt, nie preventDefault
import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { CHEATS, cheatProgress, matchCheat, useGameKeys, type CheatId } from '../src/shared/keys.js';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanup();
  document.body.innerHTML = '';
  vi.useRealTimers();
});

// Drückt Tasten auf target (Standard: body); ein String sind einzelne Zeichen, eine Liste benannte Tasten wie „Tab“
function press(keys: string | string[], init: KeyboardEventInit = {}, target: EventTarget = document.body): KeyboardEvent[] {
  return (typeof keys === 'string' ? [...keys] : keys).map((key) => {
    const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true, ...init });
    act(() => void target.dispatchEvent(event));
    return event;
  });
}

function setup(cheats = true, keys?: Record<string, () => void>) {
  const onCheat = vi.fn<(id: CheatId) => void>();
  const hook = renderHook((props: { cheats: boolean; keys?: Record<string, () => void> }) => useGameKeys({ cheats: props.cheats, onCheat, keys: props.keys }), {
    initialProps: { cheats, keys },
  });
  return { onCheat, ...hook };
}

describe('Cheat-Codes', () => {
  it('kennt alle Codes', () => {
    expect(CHEATS).toEqual({
      HESOYAM: 'money',
      TURTLE: 'money',
      AEZAKMI: 'clear',
      LAWYERUP: 'clear',
      BRINGITON: 'wanted',
      FUGITIVE: 'wanted',
    });
  });

  it('cheatProgress: längstes Ende, das Anfang eines Codes ist', () => {
    expect(cheatProgress('')).toBe(0);
    expect(cheatProgress('Q')).toBe(0);
    expect(cheatProgress('H')).toBe(1);
    expect(cheatProgress('XXHESO')).toBe(4);
    expect(cheatProgress('HESOYAM')).toBe(7);
    expect(cheatProgress('TURTL')).toBe(5);
    expect(cheatProgress('HESX')).toBe(0);
  });

  it('matchCheat: endet der Puffer mit einem Code?', () => {
    expect(matchCheat('QQHESOYAM')).toBe('money');
    expect(matchCheat('LAWYERUP')).toBe('clear');
    expect(matchCheat('FUGITIVE')).toBe('wanted');
    expect(matchCheat('HESOYA')).toBeNull();
    expect(matchCheat('')).toBeNull();
  });
});

describe('useGameKeys', () => {
  it('löst jeden Code aus, auch in Kleinbuchstaben, und leert danach den Puffer', () => {
    const { onCheat } = setup();
    for (const code of Object.keys(CHEATS)) press(code.toLowerCase());
    expect(onCheat.mock.calls.map((c) => c[0])).toEqual(['money', 'money', 'clear', 'clear', 'wanted', 'wanted']);
    // „TURTLE“ endet mit E, „E“ allein startet keinen neuen Code mit dem alten Puffer
    press('HESOYA');
    press('M');
    expect(onCheat).toHaveBeenCalledTimes(7);
  });

  it('mit cheats = false keine Cheats', () => {
    const { onCheat } = setup(false);
    press('HESOYAM');
    expect(onCheat).not.toHaveBeenCalled();
  });

  it('nach 3 s Pause ist der Puffer leer', () => {
    const { onCheat } = setup();
    press('HESO');
    act(() => vi.advanceTimersByTime(3000));
    press('YAM');
    expect(onCheat).not.toHaveBeenCalled();
    press('HESO');
    act(() => vi.advanceTimersByTime(2999));
    press('YAM');
    expect(onCheat).toHaveBeenCalledWith('money');
  });

  it('ignoriert Felder, Modifikatoren, Nicht-Buchstaben und bereits behandelte Ereignisse', () => {
    const { onCheat } = setup();
    const input = document.body.appendChild(document.createElement('input'));
    const area = document.body.appendChild(document.createElement('textarea'));
    const select = document.body.appendChild(document.createElement('select'));
    const editable = document.body.appendChild(document.createElement('div'));
    editable.setAttribute('contenteditable', 'true');
    const inner = editable.appendChild(document.createElement('span'));
    const textbox = document.body.appendChild(document.createElement('div'));
    textbox.setAttribute('role', 'textbox');
    for (const target of [input, area, select, editable, inner, textbox]) press('HESOYAM', {}, target);
    press('HESOYAM', { ctrlKey: true });
    press('HESOYAM', { altKey: true });
    press('HESOYAM', { metaKey: true });
    press('HESOYAM', { repeat: true });
    expect(onCheat).not.toHaveBeenCalled();
    // contenteditable="false" ist kein Feld
    const plain = document.body.appendChild(document.createElement('div'));
    plain.setAttribute('contenteditable', 'false');
    press('HESOYAM', {}, plain);
    expect(onCheat).toHaveBeenCalledTimes(1);
    onCheat.mockClear();
    // Bereits behandelt: zählt nicht, der Code bleibt unvollständig
    press('HESO');
    const handled = new KeyboardEvent('keydown', { key: 'Y', bubbles: true, cancelable: true });
    handled.preventDefault();
    act(() => void document.body.dispatchEvent(handled));
    press('AM');
    expect(onCheat).not.toHaveBeenCalled();
    // Nicht-Buchstaben unterbrechen nichts, zählen aber auch nicht
    press('HESO');
    press(['Shift', '1', 'ä', 'Enter']);
    press('YAM');
    expect(onCheat).toHaveBeenCalledTimes(1);
  });

  it('ruft nie preventDefault, auch nicht bei Cheats und Tasten', () => {
    const q = vi.fn();
    setup(true, { q });
    const events = [...press('HESOYAM'), ...press('q'), ...press(['Tab', 'ArrowLeft', 'Enter'])];
    expect(events.every((event) => !event.defaultPrevented)).toBe(true);
    expect(q).toHaveBeenCalledTimes(1);
  });

  it('Tasten: Kleinbuchstabe → Aktion, auch bei gedrückter Umschalttaste', () => {
    const q = vi.fn();
    const e = vi.fn();
    setup(true, { q, e });
    press('q');
    press('E', { shiftKey: true });
    expect(q).toHaveBeenCalledTimes(1);
    expect(e).toHaveBeenCalledTimes(1);
  });

  it('Ausnahme: Buchstaben mitten in einem Cheat lösen keine Aktion aus', () => {
    const q = vi.fn();
    const e = vi.fn();
    setup(true, { q, e });
    press('HESOYAM'); // E an Stelle 2
    press('AEZAKMI'); // E an Stelle 2
    press('TURTLE'); // E am Ende
    expect(e).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(3000));
    press('XE'); // kein Cheat-Anfang
    expect(e).toHaveBeenCalledTimes(1);
  });

  it('ohne Cheats gilt die Ausnahme nicht', () => {
    const e = vi.fn();
    setup(false, { e });
    press('HESOYAM');
    expect(e).toHaveBeenCalledTimes(1);
  });

  it('nimmt neue Optionen ohne neuen Lauscher an und räumt beim Abbauen auf', () => {
    const add = vi.spyOn(window, 'addEventListener');
    const remove = vi.spyOn(window, 'removeEventListener');
    const first = vi.fn();
    const second = vi.fn();
    const { rerender, unmount } = setup(true, { q: first });
    rerender({ cheats: true, keys: { q: second } });
    press('q');
    expect(first).not.toHaveBeenCalled();
    expect(second).toHaveBeenCalledTimes(1);
    expect(add.mock.calls.filter((c) => c[0] === 'keydown')).toHaveLength(1);
    unmount();
    expect(remove.mock.calls.filter((c) => c[0] === 'keydown')).toHaveLength(1);
    expect(vi.getTimerCount()).toBe(0);
    add.mockRestore();
    remove.mockRestore();
  });
});
