import { Fragment, useState, type ReactNode } from 'react';
import { Link, useLocation, useParams } from 'react-router-dom';
import { BoardConfigPanel } from '../components/BoardConfigPanel.js';
import { DISPATCH_SECTIONS, DispatchPanel, type DispatchSection } from '../components/DispatchPanel.js';
import { CustomizationsPanel } from '../components/CustomizationsPanel.js';
import { ErrorState } from '../components/ErrorState.js';
import { NotificationPanel } from '../components/NotificationPanel.js';
import { UsagePanel } from '../components/UsagePanel.js';
import { LoadingState } from '../components/LoadingState.js';
import { ProjectsPanel } from '../components/ProjectsPanel.js';
import { TokenPanel } from '../components/TokenPanel.js';
import { TelegramPanel } from '../components/TelegramPanel.js';
import { SpeechPanel } from '../components/settings/SpeechPanel.js';
import { InterfacePanel } from '../components/settings/InterfacePanel.js';
import { ThemesPanel } from '../components/settings/ThemesPanel.js';
import { SettingsIcon } from '../components/NavIcons.js';
import { SaveStatusSlot } from '../components/settings/SettingsPanel.js';
import { useSettings } from '../hooks/useSettings.js';
import { AccountsPanel } from '../components/AccountsPanel.js';

interface SubSection { id: string; label: string; icon: string; hint?: string }
interface Section { id: string; label: string; icon: string; hint?: string; subs?: readonly SubSection[] }

// Unterbereiche von „Darstellung“; ohne Angabe im Hash gilt der erste
const APPEARANCE_SECTIONS: readonly SubSection[] = [
  { id: 'themes', label: 'Themes', icon: 'brush', hint: 'Farbschema und Theme-Pakete der ganzen Oberfläche.' },
  { id: 'interface', label: 'Oberfläche', icon: 'view_quilt', hint: 'Aussehen des Boards und eigene CSS-Schnipsel.' },
];

// Bereiche der Einstellungen, gruppiert; die ID ist zugleich der URL-Hash (z. B. #board-db).
const GROUPS: Array<{ title: string; sections: Section[] }> = [
  { title: 'Arbeitsbereich', sections: [
    { id: 'projects', label: 'Arbeitsverzeichnisse', icon: 'folder', hint: 'Lokale Repos, in denen eine Claude-Session lief, und ob sie erreichbar sind.' },
    { id: 'token', label: 'Zugriff', icon: 'key', hint: 'Token, mit dem sich Plugin und Terminal beim Harness anmelden.' },
    { id: 'accounts', label: 'Accounts', icon: 'group' },
  ] },
  { title: 'Oberfläche', sections: [
    { id: 'appearance', label: 'Darstellung', icon: 'palette', subs: APPEARANCE_SECTIONS },
    { id: 'notifications', label: 'Benachrichtigungen', icon: 'notifications', hint: 'Wie du erfährst, dass eine Session fertig ist oder deine Antwort braucht.' },
    { id: 'usage', label: 'Nutzungslimits', icon: 'donut_large', hint: 'Verbrauch deines Claude-Abos als Ring neben der Suche.' },
    { id: 'customizations', label: 'Anpassungen', icon: 'tune', hint: 'Eigene Workflows, Skills und Output-Styles.' },
  ] },
  { title: 'Board', sections: [
    { id: 'board-db', label: 'Board-Datenbank', icon: 'storage', hint: 'Wo die Boards gespeichert sind und wer du dort bist.' },
    { id: 'dispatch', label: 'Dispatcher', icon: 'send', hint: 'Lässt bereite Tickets automatisch von Claude umsetzen und mergen.', subs: DISPATCH_SECTIONS },
    { id: 'telegram', label: 'Telegram', icon: 'chat', hint: 'Bot für Meldungen, Freigaben und Ideen per Chat.' },
    { id: 'speech', label: 'Spracherkennung', icon: 'mic', hint: 'Dienst, der Sprachnachrichten in Text verwandelt.' },
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
  // Gewählter Unterbereich (Darstellung, Dispatcher); unbekannte oder fehlende fallen auf den ersten
  const sub = active.subs ? active.subs.find((s) => s.id === subId) ?? active.subs[0] : undefined;
  const hint = sub?.hint ?? active.hint;

  let content: ReactNode;
  if (settings.state.status === 'loading' || projects.state.status === 'loading') content = <LoadingState text="Einstellungen werden geladen …" />;
  else if (settings.state.status === 'error') content = <ErrorState message={settings.state.message} onRetry={settings.retry} />;
  else if (projects.state.status === 'error') content = <ErrorState message={projects.state.message} onRetry={projects.retry} />;
  else {
    const panels: Record<string, ReactNode> = {
      projects: <ProjectsPanel projects={projects.state.data} current={pid} />,
      token: <TokenPanel token={settings.state.data.token} />,
      accounts: <AccountsPanel />,
      appearance: sub?.id === 'interface' ? <InterfacePanel /> : <ThemesPanel />,
      notifications: <NotificationPanel />,
      usage: <UsagePanel />,
      customizations: <CustomizationsPanel />,
      'board-db': <BoardConfigPanel />,
      dispatch: <DispatchPanel sub={sub?.id as DispatchSection | undefined} />,
      telegram: <TelegramPanel />,
      speech: <SpeechPanel />,
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
              <Link to={{ hash: s.id }} replace className={s.id === active.id ? 'active' : undefined} aria-current={s.id === active.id && !sub ? 'page' : undefined}>
                <SettingsIcon name={s.icon} />
                <span>{s.label}</span>
              </Link>
              {s.subs && s.id === active.id && (
                <div className="settings-subnav">
                  {s.subs.map((d) => (
                    <Link key={d.id} to={{ hash: `${s.id}/${d.id}` }} replace className={d.id === sub?.id ? 'active' : undefined} aria-current={d.id === sub?.id ? 'page' : undefined}>
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
            <h2>{active.label}{sub && <span className="settings-head-sub"> / {sub.label}</span>}</h2>
            {hint && <p className="muted">{hint}</p>}
          </div>
          <div className="settings-status-slot" ref={setSlot} />
        </header>
        <SaveStatusSlot.Provider value={slot}>{content}</SaveStatusSlot.Provider>
      </div>
    </div>
  );
}
