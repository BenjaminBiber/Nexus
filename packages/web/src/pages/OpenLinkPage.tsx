import { Navigate, useParams } from 'react-router-dom';
import { EmptyState } from '../components/EmptyState.js';
import { ErrorState } from '../components/ErrorState.js';
import { LoadingState } from '../components/LoadingState.js';
import { useProjects } from '../hooks/useProjects.js';

const TICKET_ID = /^[A-Z][A-Z0-9]*-\d+$/;
const QUESTION_ID = /^[A-Z][A-Z0-9]*-F\d+$/;

// Zieladresse im ersten Projekt für eine geprüfte Art und ID; null bei unbekannter Art oder ungültiger ID.
function targetPath(pid: string, kind: string | undefined, id: string | undefined): string | null {
  if (!id) return null;
  if (kind === 'ticket' && TICKET_ID.test(id)) return `/p/${pid}/board/t/${id}`;
  if (kind === 'question' && QUESTION_ID.test(id)) return `/p/${pid}/questions/${id}`;
  return null;
}

// Weiterleitungsseite für nexus://-Links: /open/ticket/ARC-0042 führt zum Ticket im ersten Projekt.
// Die Adressen der Oberfläche brauchen eine Projekt-ID, der Link kennt sie nicht – darum dieser Zwischenschritt.
export function OpenLinkPage() {
  const { kind, id } = useParams();
  const { state, retry } = useProjects();
  if (state.status === 'loading') return <div className="page"><LoadingState text="Projekte werden geladen …" /></div>;
  if (state.status === 'error') return <ErrorState message={state.message} onRetry={retry} />;
  if (state.data.length === 0) {
    return (
      <div className="page">
        <EmptyState title="Noch kein Projekt angemeldet" command="harness board">
          Führe im Projektordner den Befehl aus, dann erscheint das Projekt hier.
        </EmptyState>
      </div>
    );
  }
  const path = targetPath(state.data[0].id, kind, id);
  if (!path) {
    return (
      <div className="page">
        <ErrorState message="Dieser Link führt nirgendwohin." detail="Erwartet werden Links der Form nexus://ticket/ARC-0042 oder nexus://question/ARC-F0001." />
      </div>
    );
  }
  return <Navigate to={path} replace />;
}
