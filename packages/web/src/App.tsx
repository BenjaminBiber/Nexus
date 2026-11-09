import './themes/builtin.js';
import { useEffect } from 'react';
import { Navigate, Route, Routes, useParams } from 'react-router-dom';
import { AppShell } from './components/AppShell.js';
import { DashboardPage } from './pages/DashboardPage.js';
import { DbBoardPage } from './pages/DbBoardPage.js';
import { DbQuestionDetailPage } from './pages/DbQuestionDetailPage.js';
import { DbQuestionsPage } from './pages/DbQuestionsPage.js';
import { DocsPage } from './pages/DocsPage.js';
import { GuidePage } from './pages/GuidePage.js';
import { InboxPage } from './pages/InboxPage.js';
import { SessionsPage } from './pages/SessionsPage.js';
import { SettingsPage } from './pages/SettingsPage.js';
import { OpenLinkPage } from './pages/OpenLinkPage.js';
import { RootPage } from './pages/RootPage.js';
import { DocQuestionJobsProvider } from './hooks/docQuestionJobs.js';
import { DocTicketJobsProvider } from './hooks/docTicketJobs.js';
import { ThemeSync } from './theme/ThemeSync.js';
import { StartAnimation } from './components/StartAnimation.js';
import { usePlatform } from './platform/PlatformContext.js';

// Alle Adressen der Oberfläche; Seiten unter /p/:pid teilen sich den Rahmen. ThemeSync setzt das Farbschema, StartAnimation zeigt die Startanimation des Themes.
// Board und Fragen kommen aus der Board-Datenbank; das Ticket-Panel liegt unter board/t/:id, die Details einer Session unter sessions/:sid.
// Ticket- und Fragen-Erzeugungen aus Dokumenten (DocTicketJobsProvider, DocQuestionJobsProvider) laufen über Seiten- und Projektwechsel weiter.
// /open/:kind/:id ist die Weiterleitung für nexus://-Links; sie ergänzt die fehlende Projekt-ID.
// Unbekannte Adressen eines Projekts (alte Lesezeichen wie board-alt, tickets/…, questions-alt) führen zum Board.
export function App() {
  return (
    <DocTicketJobsProvider>
      <DocQuestionJobsProvider>
        <ThemeSync />
        <StartAnimation />
        <ExternalLinks />
        <Routes>
          <Route path="/" element={<RootPage />} />
          <Route path="/open/:kind/:id" element={<OpenLinkPage />} />
          <Route path="/p/:pid" element={<AppShell />}>
            <Route path="board/*" element={<DbBoardPage />} />
            <Route path="dashboard" element={<DashboardPage />} />
            <Route path="inbox" element={<InboxPage />} />
            <Route path="sessions/*" element={<SessionsPage />} />
            <Route path="questions" element={<DbQuestionsPage />} />
            <Route path="questions/:id" element={<DbQuestionDetailPage />} />
            <Route path="docs/*" element={<DocsPage />} />
            <Route path="settings" element={<SettingsPage />} />
            <Route path="guide" element={<GuidePage />} />
            <Route path="*" element={<ToBoard />} />
          </Route>
        </Routes>
      </DocQuestionJobsProvider>
    </DocTicketJobsProvider>
  );
}

// In der Desktop-App öffnen Links auf fremde Adressen den Standardbrowser, statt das App-Fenster wegzunavigieren.
// Im Browser bleibt jeder Klick, wie er ist.
function ExternalLinks() {
  const platform = usePlatform();
  useEffect(() => {
    if (platform.kind !== 'tauri') return;
    const onClick = (event: MouseEvent) => {
      // Mittel- und Sondertasten öffnen schon von sich aus einen eigenen Tab bzw. ein Menü
      if (event.defaultPrevented || event.button !== 0 || event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
      const anchor = (event.target as Element | null)?.closest?.('a[href]');
      const href = anchor?.getAttribute('href');
      if (!href) return;
      let url: URL;
      try {
        url = new URL(href, window.location.href);
      } catch {
        return;
      }
      if (url.origin === window.location.origin) return;
      event.preventDefault();
      void platform.openExternal(url.href);
    };
    document.addEventListener('click', onClick);
    return () => document.removeEventListener('click', onClick);
  }, [platform]);
  return null;
}

// Leitet zum Board des Projekts weiter
function ToBoard() {
  const { pid } = useParams();
  return <Navigate to={`/p/${encodeURIComponent(pid ?? '')}/board`} replace />;
}
