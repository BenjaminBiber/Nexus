import { Navigate } from 'react-router-dom';
import { EmptyState } from '../components/EmptyState.js';
import { ErrorState } from '../components/ErrorState.js';
import { LoadingState } from '../components/LoadingState.js';
import { useProjects } from '../hooks/useProjects.js';

// Startadresse: zum Board des ersten Projekts oder Hinweis, wie man ein Projekt anmeldet.
export function RootPage() {
  const { state, retry } = useProjects();
  if (state.status === 'loading') return <div className="page"><LoadingState text="Projekte werden geladen …" /></div>;
  if (state.status === 'error') return <ErrorState message={state.message} onRetry={retry} />;
  if (state.data.length === 0) {
    return (
      <div className="page">
        <EmptyState title="Noch kein Projekt angemeldet" command="nexus board">
          Führe im Projektordner den Befehl aus, dann erscheint das Projekt hier.
        </EmptyState>
      </div>
    );
  }
  return <Navigate to={`/p/${state.data[0].id}/board`} replace />;
}
