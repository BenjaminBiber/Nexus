import { Component, type ErrorInfo, type ReactNode } from 'react';
import { getThemeNotice, setThemeNotice } from './themeNoticeStore.js';

interface Props {
  /** Standard-Komponente, falls die des Themes scheitert */
  fallback: ReactNode;
  /** Wechselt er (anderes Theme), wird die Theme-Komponente erneut versucht */
  resetKey: string;
  children: ReactNode;
}

interface State {
  failedFor: string | null;
}

// Fängt Fehler einer Theme-Komponente ab und zeigt dann die Standard-Komponente; ein kaputtes Theme macht Nexus nie unbedienbar
export class SlotBoundary extends Component<Props, State> {
  state: State = { failedFor: null };

  // Merkt sich, für welches Theme die Komponente gescheitert ist
  static getDerivedStateFromError(): Partial<State> {
    return { failedFor: '__pending__' };
  }

  // Ordnet den Fehler dem aktuellen Theme zu und meldet ihn in der Konsole und im Hinweisbalken (einmal je Theme)
  componentDidCatch(error: Error, info: ErrorInfo): void {
    this.setState({ failedFor: this.props.resetKey });
    const message = `Theme „${this.props.resetKey}“: Komponente gescheitert, Standard wird gezeigt.`;
    console.error(message, error, info.componentStack);
    if (getThemeNotice() !== message) setThemeNotice({ kind: 'error', message });
  }

  // Rückfall nur für das Theme, bei dem es gescheitert ist
  render(): ReactNode {
    const { failedFor } = this.state;
    if (failedFor === '__pending__' || failedFor === this.props.resetKey) return this.props.fallback;
    return this.props.children;
  }
}
