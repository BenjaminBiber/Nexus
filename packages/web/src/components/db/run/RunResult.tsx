import { MarkdownView } from '../../../markdown/MarkdownView.js';

// Antwort des Agenten am Ende eines Laufs: Trennzeile „Ergebnis“ und darunter der Text als Markdown auf einer Karte
export function RunResult({ text }: { text: string }) {
  return (
    <section className="run-result" aria-label="Ergebnis">
      <div className="run-result-head">
        <span className="material-icons" aria-hidden="true">
          check
        </span>
        <span className="run-result-title">Ergebnis</span>
      </div>
      <article className="run-answer">
        <MarkdownView source={text} />
      </article>
    </section>
  );
}
