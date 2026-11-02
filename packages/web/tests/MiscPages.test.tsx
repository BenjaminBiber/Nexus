// @vitest-environment jsdom
import { describe, it, expect, afterEach, vi } from 'vitest';
import { cleanup, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { GuidePage } from '../src/pages/GuidePage.js';
import { SessionsPage } from '../src/pages/SessionsPage.js';
import { SettingsPage } from '../src/pages/SettingsPage.js';
import { fakeApi, projects } from './helpers/fakeApi.js';
import { renderWithApp } from './helpers/renderWithApp.js';

afterEach(cleanup);

const TOKEN = 'a'.repeat(64);

// Rendert die Einstellungen mit austauschbaren Aufrufen
function openSettings(overrides: Parameters<typeof fakeApi>[0] = {}) {
  const api = fakeApi({
    getProjects: async () => [projects[0], { ...projects[1], reachable: false }],
    getSettings: async () => ({ project: projects[0], token: TOKEN }),
    ...overrides,
  });
  renderWithApp(<SettingsPage />, { api, path: '/p/p1/settings', route: '/p/:pid/settings' });
}

describe('SettingsPage', () => {
  it('listet die Projekte mit Pfad und kennzeichnet nicht erreichbare mit Text', async () => {
    openSettings();
    const section = await screen.findByRole('region', { name: 'Arbeitsverzeichnisse' });
    expect(within(section).getByText('Demo')).toBeTruthy();
    expect(within(section).getByText('/demo')).toBeTruthy();
    expect(within(section).getByText('Zweites')).toBeTruthy();
    expect(within(section).getByText('nicht erreichbar')).toBeTruthy();
    expect(within(section).getByText('aktuell')).toBeTruthy();
  });

  it('zeigt das Token erst auf Wunsch und kopiert es', async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    openSettings();
    await screen.findByRole('region', { name: 'Arbeitsverzeichnisse' });
    await userEvent.click(screen.getByRole('link', { name: /Zugriff/ }));
    expect(screen.queryByText(TOKEN)).toBeNull();
    await userEvent.click(screen.getByRole('button', { name: 'Token anzeigen' }));
    expect(screen.getByText(TOKEN)).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: 'Token kopieren' }));
    expect(writeText).toHaveBeenCalledWith(TOKEN);
  });

  it('hat eine Bereichs-Navigation und zeigt nur den gewählten Bereich', async () => {
    openSettings();
    await screen.findByRole('region', { name: 'Arbeitsverzeichnisse' });
    const nav = screen.getByRole('navigation', { name: 'Einstellungsbereiche' });
    for (const name of ['Arbeitsverzeichnisse', 'Zugriff', 'Darstellung', 'Benachrichtigungen', 'Anpassungen', 'Board-Datenbank', 'Dispatcher']) {
      expect(within(nav).getByRole('link', { name: new RegExp(name) }), name).toBeTruthy();
    }
    expect(screen.queryByRole('region', { name: 'Darstellung' })).toBeNull();
    await userEvent.click(within(nav).getByRole('link', { name: /Darstellung/ }));
    expect(screen.getByRole('region', { name: 'Darstellung' })).toBeTruthy();
    expect(screen.queryByRole('region', { name: 'Arbeitsverzeichnisse' })).toBeNull();
  });

  it('zeigt beim Dispatcher ein Untermenü und je Unterbereich nur dessen Felder', async () => {
    openSettings();
    await screen.findByRole('region', { name: 'Arbeitsverzeichnisse' });
    const nav = screen.getByRole('navigation', { name: 'Einstellungsbereiche' });
    expect(within(nav).queryByRole('link', { name: /Recherchen/ })).toBeNull();
    await userEvent.click(within(nav).getByRole('link', { name: /Dispatcher/ }));
    for (const name of ['Allgemein', 'Zeitplan & Grenzen', 'Merge & Worktrees', 'Profile & Regeln', 'Recherchen', 'Claude-Aufruf']) {
      expect(within(nav).getByRole('link', { name: new RegExp(name) }), name).toBeTruthy();
    }
    await userEvent.click(within(nav).getByRole('link', { name: /Claude-Aufruf/ }));
    expect(await screen.findByText('Claude-Aufruf', { selector: 'label span' })).toBeTruthy();
    expect(screen.queryByText('Dispatcher eingeschaltet')).toBeNull();
  });

  it('bietet weder Spalten noch Export noch Sicherung an', async () => {
    openSettings();
    await screen.findByRole('region', { name: 'Arbeitsverzeichnisse' });
    for (const name of ['Spalten', 'Export', 'Sicherung']) expect(screen.queryByRole('region', { name }), name).toBeNull();
  });
});

describe('GuidePage', () => {
  const open = () => renderWithApp(<GuidePage />, { api: fakeApi(), path: '/p/p1/guide', route: '/p/:pid/guide' });

  it('hat die Abschnitte Ablauf, Dispatcher, Im Terminal, Einrichten, Aktualisieren, Anpassen, Begriffe und Häufige Fragen', () => {
    open();
    const nav = screen.getByRole('navigation', { name: 'Abschnitte' });
    expect(within(nav).getByRole('link', { name: 'Dispatcher' }).getAttribute('href')).toBe('#dispatcher');
    for (const name of ['Ablauf', 'Dispatcher', 'Im Terminal', 'Einrichten', 'Aktualisieren', 'Anpassen', 'Begriffe', 'Häufige Fragen']) {
      expect(screen.getByRole('heading', { name }), name).toBeTruthy();
    }
  });

  it('erklärt unter Häufige Fragen schnelle Aufgaben ohne Workflow', () => {
    open();
    expect(screen.getByText('Kann ich kleine Aufgaben ohne Workflow erledigen?')).toBeTruthy();
    expect(screen.getByText(/Ohne Workflow direkt umsetzen\?/)).toBeTruthy();
  });

  it('erklärt im Abschnitt Anpassen Skill und setup', () => {
    open();
    const section = screen.getByRole('heading', { name: 'Anpassen' }).closest('section')!;
    expect(within(section).getByText(/Workflow, Skills und Output-Styles/)).toBeTruthy();
    expect(within(section).getByRole('button', { name: 'Befehl kopieren: /nexus:harness-customize' })).toBeTruthy();
    expect(within(section).getByRole('button', { name: 'Befehl kopieren: packages/plugin/dist/nexus/bin/nexus install' })).toBeTruthy();
  });

  it('zeigt den Ablauf in Phasen mit kopierbaren Claude-Befehlen', async () => {
    open();
    const phases: Array<[string, string[]]> = [
      ['Neue Funktion planen', ['/superpowers:brainstorming', '/superpowers:writing-plans']],
      ['Ticket umsetzen', ['/nexus:harness-implement']],
    ];
    await screen.findByRole('list', { name: 'Neue Funktion planen' });
    for (const [title, commands] of phases) {
      const list = screen.getByRole('list', { name: title });
      for (const command of commands) {
        expect(within(list).getByText(command), command).toBeTruthy();
        expect(within(list).getByRole('button', { name: `Befehl kopieren: ${command}` }), command).toBeTruthy();
      }
    }
    const planning = screen.getByRole('list', { name: 'Neue Funktion planen' });
    expect(within(planning).queryByText(/\/speckit-|harness-tasks/)).toBeNull();
    expect(within(planning).getByText('In der Session sagen: „Lege für die Tasks des Plans Tickets an“')).toBeTruthy();
    expect(within(planning).getByText('In der Session sagen: „Bereite ARC-0042 für den Dispatcher vor“')).toBeTruthy();
  });

  it('bietet beim Umsetzen ein Ticket oder den ganzen Plan an und verweist auf den Dispatcher', async () => {
    open();
    const list = await screen.findByRole('list', { name: 'Ticket umsetzen' });
    await userEvent.click(within(list).getByRole('button', { name: 'Ganzer Plan' }));
    expect(within(list).getByRole('button', { name: 'Befehl kopieren: /superpowers:subagent-driven-development' })).toBeTruthy();
    expect(within(list).getByText(/Abschnitt „Dispatcher“ der Anleitung/)).toBeTruthy();
    expect(within(list).getByText(/mit harness board done auf Erledigt/)).toBeTruthy();
  });

  it('erklärt im Abschnitt Dispatcher Bereitschaft, Ausgänge, Einrichten und Befehle', () => {
    open();
    const section = screen.getByRole('heading', { name: 'Dispatcher', level: 2 }).closest('section')!;
    expect(within(section).getByRole('heading', { name: 'Bereit ist ein Ticket, wenn' })).toBeTruthy();
    expect(within(section).getByText(/Plan, Handbuch und Abnahme sind ausgefüllt/)).toBeTruthy();
    for (const outcome of ['Erledigt', 'Frage', 'Abbruch', 'Merge blockiert', 'Nutzungslimit']) expect(within(section).getByText(outcome), outcome).toBeTruthy();
    expect(within(section).getByText(/~\/\.harness\/dispatch\.yaml/)).toBeTruthy();
    const commands = ['harness board boards repo 7 C:\\Projekte\\Demo', 'harness board dispatch status', 'harness board dispatch run', 'harness board dispatch pause --minutes 30',
      'harness board dispatch resume', 'harness board dispatch enable', 'harness board merge ARC-0042'];
    for (const command of commands) expect(within(section).getByRole('button', { name: `Befehl kopieren: ${command}` }), command).toBeTruthy();
    // Kein doppelter Schritt in „Nebenbei“
    expect(within(screen.getByRole('list', { name: 'Nebenbei' })).queryByText('Automatisch umsetzen lassen')).toBeNull();
  });

  it('bietet Brainstorming, Grill und Wayfinder als Umschalter in Schritt 1', async () => {
    open();
    const list = await screen.findByRole('list', { name: 'Neue Funktion planen' });
    expect(within(list).getByText('/superpowers:brainstorming')).toBeTruthy();
    await userEvent.click(within(list).getByRole('button', { name: 'Grill' }));
    expect(within(list).getByText('/nexus:harness-grill')).toBeTruthy();
    expect(within(list).queryByText('/superpowers:brainstorming')).toBeNull();
    await userEvent.click(within(list).getByRole('button', { name: 'Wayfinder' }));
    expect(within(list).getByText('/nexus:harness-wayfinder')).toBeTruthy();
    expect(within(list).queryByText('/nexus:harness-grill')).toBeNull();
  });

  it('zeigt die Terminal-Befehle getrennt vom Ablauf', async () => {
    open();
    await screen.findByRole('list', { name: 'Ticket umsetzen' });
    const list = screen.getByRole('list', { name: 'Terminal' });
    for (const command of ['harness board', 'harness board list', 'harness spec comments spec-docs-rework']) {
      expect(within(list).getByText(command), command).toBeTruthy();
    }
    expect(within(list).queryByText(/harness (ticket|export|spec (new|import))/)).toBeNull();
    expect(within(screen.getByRole('list', { name: 'Ticket umsetzen' })).queryByText(/^harness /)).toBeNull();
  });

  it('beschreibt Board-Einrichtung, Update mit harness init und neue Terminal-Befehle', () => {
    open();
    const setup = screen.getByRole('heading', { name: 'Einrichten' }).closest('section')!;
    expect(within(setup).getByText(/Doku- und Board-Regel/)).toBeTruthy();
    expect(within(setup).getByRole('button', { name: /Befehl kopieren: harness board boards create ARC/ })).toBeTruthy();
    const update = screen.getByRole('heading', { name: 'Aktualisieren' }).closest('section')!;
    expect(within(update).queryByText(/nichts zu tun/)).toBeNull();
    expect(within(update).getByRole('button', { name: 'Befehl kopieren: harness init' })).toBeTruthy();
    const list = screen.getByRole('list', { name: 'Terminal' });
    for (const command of ['harness init', 'harness board merge ARC-0042', 'harness board step add ARC-0042 "Text"', 'harness board question research-start ARC-F0007']) {
      expect(within(list).getByText(command), command).toBeTruthy();
    }
  });

  it('beschreibt in „Nebenbei“ den Weg vom markierten Text zu Bearbeiten und Claude', () => {
    open();
    const list = screen.getByRole('list', { name: 'Nebenbei' });
    for (const step of ['Abschnitt bearbeiten', 'Mit Claude ändern', 'Tickets aus einem Dokument', 'Stelle kommentieren']) {
      expect(within(list).getByText(step), step).toBeTruthy();
    }
    expect(within(list).getByText(/„Bearbeiten“/)).toBeTruthy();
    expect(within(list).getByText(/„Mit Claude ändern“/)).toBeTruthy();
  });

  it('beantwortet, warum der Dispatcher ein Ticket nicht nimmt', () => {
    open();
    expect(screen.getByText('Warum nimmt der Dispatcher mein Ticket nicht?')).toBeTruthy();
  });

  it('erklärt die Begriffe des Glossars', () => {
    open();
    const terms = screen.getByRole('region', { name: 'Begriffe' });
    for (const term of ['Ticket', 'Spec', 'Session', 'Subagent', 'Kommentar', 'Kommentar (Docs)', 'Verlauf', 'Favorit', 'Board', 'Schritt', 'Projekt', 'Dispatcher', 'Worktree', 'Planungsbedarf', 'Label auto']) {
      expect(within(terms).getByText(term), term).toBeTruthy();
    }
  });
});

describe('SessionsPage', () => {
  it('zeigt einen Leerzustand mit Hinweisen und leere Favoriten mit Erklärung', async () => {
    renderWithApp(<SessionsPage />, { api: fakeApi(), path: '/p/p1/sessions', route: '/p/:pid/sessions' });
    expect(await screen.findByText('Keine Sessions')).toBeTruthy();
    expect(screen.getByText('Starte ein Ticket, dann erscheint die Session hier.')).toBeTruthy();
    expect(screen.getByText('Noch keine Hook-Meldungen empfangen')).toBeTruthy();
    expect(screen.getByText('harness init')).toBeTruthy();
    await userEvent.click(screen.getByRole('button', { name: /^Favoriten/ }));
    expect(screen.getByText('Noch keine Favoriten')).toBeTruthy();
    expect(screen.queryByText(/Etappe 5/)).toBeNull();
  });
});
