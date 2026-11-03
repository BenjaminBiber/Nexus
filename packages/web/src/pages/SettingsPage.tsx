import { Fragment, useState, type ReactNode } from 'react';
import { Link, useLocation, useParams } from 'react-router-dom';
import { BoardConfigPanel } from '../components/BoardConfigPanel.js';
import { DISPATCH_SECTIONS, DispatchPanel } from '../components/DispatchPanel.js';
import { CustomizationsPanel } from '../components/CustomizationsPanel.js';
import { ErrorState } from '../components/ErrorState.js';
import { AppearancePanel } from '../components/AppearancePanel.js';
import { NotificationPanel } from '../components/NotificationPanel.js';
import { UsagePanel } from '../components/UsagePanel.js';
import { LoadingState } from '../components/LoadingState.js';
import { ProjectsPanel } from '../components/ProjectsPanel.js';
import { TokenPanel } from '../components/TokenPanel.js';
import { TelegramPanel } from '../components/TelegramPanel.js';
import { SettingsIcon } from '../components/NavIcons.js';
import { SaveStatusSlot } from '../components/settings/SettingsPanel.js';
import { useSettings } from '../hooks/useSettings.js';
import { AccountsPanel } from '../components/AccountsPanel.js';

interface Section { id: string; label: string; icon: string; hint?: string }

// Bereiche der Einstellungen, gruppiert; die ID ist zugleich der URL-Hash (z. B. #board-db).
const GROUPS: Array<{ title: string; sections: Section[] }> = [
  { title: 'Arbeitsbereich', sections: [
    { id: 'projects', label: 'Arbeitsverzeichnisse', icon: 'folder', hint: 'Lokale Repos, in denen eine Claude-Session lief, und ob sie erreichbar sind.' },
    { id: 'token', label: 'Zugriff', icon: 'key', hint: 'Token, mit dem sich Plugin und Terminal beim Harness anmelden.' },
    { id: 'accounts', label: 'Accounts', icon: 'group' },
  ] },
  { title: 'Oberfläche', sections: [
    { id: 'appearance', label: 'Darstellung', icon: 'palette', hint: 'Farbschema und Aussehen des Boards.' },
    { id: 'notifications', label: 'Benachrichtigungen', icon: 'notifications', hint: 'Wie du erfährst, dass eine Session fertig ist oder deine Antwort braucht.' },
    { id: 'usage', label: 'Nutzungslimits', icon: 'donut_large', hint: 'Verbrauch deines Claude-Abos als Ring neben der Suche.' },
    { id: 'customizations', label: 'Anpassungen', icon: 'tune', hint: 'Eigene Workflows, Skills und Output-Styles.' },
  ] },
  { title: 'Board', sections: [
    { id: 'board-db', label: 'Board-Datenbank', icon: 'storage', hint: 'Wo die Boards gespeichert sind und wer du dort bist.' },
    { id: 'dispatch', label: 'Dispatcher', icon: 'send', hint: 'Lässt bereite Tickets automatisch von Claude umsetzen und mergen.' },
    { id: 'telegram', label: 'Telegram', icon: 'chat', hint: 'Bot für Meldungen, Freigaben und Ideen per Chat.' },
  ] },
];
const SECTIONS = GROUPS.flatMap((g) => g.sections);

// Einstellungen mit eigener Bereichs-Navigation links; rechts nur der gewählte Bereich.
export function SettingsPage() {
  const { pid = '' } = useParams();
  const { hash } = useLocation();
  const { projects, settings } = useSettings(pid);
  const [slot, setSlot] = useState<HTMLElement | null>(null);
  const [mainId, subId] = hash.slice(1).split('/');
  const active = SECTIONS.find((s) => s.id === mainId) ?? SECTIONS[0]!;
  const dispatchSub = active.id === 'dispatch' ? DISPATCH_SECTIONS.find((s) => s.id === subId) ?? DISPATCH_SECTIONS[0] : undefined;

  let content: ReactNode;
  if (settings.state.status === 'loading' || projects.state.status === 'loading') content = <LoadingState text="Einstellungen werden geladen …" />;
  else if (settings.state.status === 'error') content = <ErrorState message={settings.state.message} onRetry={settings.retry} />;
  else if (projects.state.status === 'error') content = <ErrorState message={projects.state.message} onRetry={projects.retry} />;
  else {
    const panels: Record<string, ReactNode> = {
      projects: <ProjectsPanel projects={projects.state.data} current={pid} />,
      token: <TokenPanel token={settings.state.data.token} />,
      accounts: <AccountsPanel />,
      appearance: <AppearancePanel />,
      notifications: <NotificationPanel />,
      usage: <UsagePanel />,
      customizations: <CustomizationsPanel />,
      'board-db': <BoardConfigPanel />,
      dispatch: <DispatchPanel sub={dispatchSub?.id} />,
      telegram: <TelegramPanel />,
    };
    content = panels[active.id];
  }

  return (
    <div className="settings-layout">
      <nav className="settings-nav" aria-label="Einstellungsbereiche">
        <h1 className="settings-nav-title">Einstellungen</h1>
        {GROUPS.map((group) => (
          <div key={group.title} className="settings-nav-group">
            <span className="side-label">{group.title}</span>
            {group.sections.map((s) => (
              <Fragment key={s.id}>
              <Link to={{ hash: s.id }} replace className={s.id === active.id ? 'active' : undefined} aria-current={s.id === active.id && !dispatchSub ? 'page' : undefined}>
                <SettingsIcon name={s.icon} />
                <span>{s.label}</span>
              </Link>
              {s.id === 'dispatch' && s.id === active.id && (
                <div className="settings-subnav">
                  {DISPATCH_SECTIONS.map((d) => (
                    <Link key={d.id} to={{ hash: `dispatch/${d.id}` }} replace className={d.id === dispatchSub?.id ? 'active' : undefined} aria-current={d.id === dispatchSub?.id ? 'page' : undefined}>
                      <SettingsIcon name={d.icon} />
                      <span>{d.label}</span>
                    </Link>
                  ))}
                </div>
              )}
              </Fragment>
            ))}
          </div>
        ))}
      </nav>
      <div className="settings-content">
        <header className="settings-head">
          <div className="settings-head-text">
            <h2>{active.label}{dispatchSub && <span className="settings-head-sub"> / {dispatchSub.label}</span>}</h2>
            {active.hint && <p className="muted">{active.hint}</p>}
          </div>
          <div className="settings-status-slot" ref={setSlot} />
        </header>
        <SaveStatusSlot.Provider value={slot}>{content}</SaveStatusSlot.Provider>
      </div>
    </div>
  );
}
