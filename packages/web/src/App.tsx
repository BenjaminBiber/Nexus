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
import { RootPage } from './pages/RootPage.js';
import { DocQuestionJobsProvider } from './hooks/docQuestionJobs.js';
import { DocTicketJobsProvider } from './hooks/docTicketJobs.js';
import { ThemeSync } from './theme/ThemeSync.js';

// Alle Adressen der Oberfläche; Seiten unter /p/:pid teilen sich den Rahmen. ThemeSync setzt das Farbschema.
// Board und Fragen kommen aus der Board-Datenbank; das Ticket-Panel liegt unter board/t/:id.
// Ticket- und Fragen-Erzeugungen aus Dokumenten (DocTicketJobsProvider, DocQuestionJobsProvider) laufen über Seiten- und Projektwechsel weiter.
// Unbekannte Adressen eines Projekts (alte Lesezeichen wie board-alt, tickets/…, questions-alt) führen zum Board.
export function App() {
  return (
    <DocTicketJobsProvider>
      <DocQuestionJobsProvider>
        <ThemeSync />
        <Routes>
          <Route path="/" element={<RootPage />} />
          <Route path="/p/:pid" element={<AppShell />}>
            <Route path="board/*" element={<DbBoardPage />} />
            <Route path="dashboard" element={<DashboardPage />} />
            <Route path="inbox" element={<InboxPage />} />
            <Route path="sessions" element={<SessionsPage />} />
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

// Leitet zum Board des Projekts weiter
function ToBoard() {
  const { pid } = useParams();
  return <Navigate to={`/p/${encodeURIComponent(pid ?? '')}/board`} replace />;
}
