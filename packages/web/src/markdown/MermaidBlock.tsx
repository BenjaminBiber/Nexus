import { useEffect, useId, useRef, useState } from 'react';
import { getTheme } from '../theme/registry.js';
import { THEME_EVENT, currentTheme, displayedTheme } from '../theme/theme.js';
import { DiagramViewer } from './DiagramViewer.js';

type Result = { svg: string } | { error: string };

// Aussehen für das angezeigte Schema: vom Theme (ThemeDefinition.mermaid), sonst hell oder dunkel; ein scheiterndes Theme fällt zurück
function lookFor(themeId: string) {
  const standard = { theme: currentTheme() === 'dark' ? ('dark' as const) : ('default' as const) };
  const custom = getTheme(themeId)?.mermaid;
  if (!custom) return standard;
  try {
    return { ...custom() };
  } catch (error) {
    console.error('Theme: Mermaid-Aussehen gescheitert, Standard wird gezeigt.', error);
    return standard;
  }
}

// Zeichnet ein Mermaid-Diagramm (Bibliothek wird erst bei Bedarf geladen); Fehler zeigen Meldung und Quelltext.
// Ein gezeichnetes Diagramm lässt sich im Vollbild mit Zoom und Verschieben öffnen; bei Wechsel des Farbschemas wird neu gezeichnet.
// Beim Neuzeichnen (Schema oder Inhalt) bleibt das bisherige Bild stehen, bis das neue fertig ist; ein offenes Vollbild bleibt so offen.
export function MermaidBlock({ source, title = 'Diagramm' }: { source: string; title?: string }) {
  const id = 'm' + useId().replace(/[^a-zA-Z0-9]/g, '');
  const renders = useRef(0);
  const [result, setResult] = useState<Result | null>(null);
  const [full, setFull] = useState(false);
  const [theme, setTheme] = useState(displayedTheme);

  // Bei Wechsel des Farbschemas neu zeichnen (gleiches Schema löst nichts aus)
  useEffect(() => {
    const redraw = () => setTheme(displayedTheme());
    window.addEventListener(THEME_EVENT, redraw);
    return () => window.removeEventListener(THEME_EVENT, redraw);
  }, []);

  useEffect(() => {
    let cancelled = false;
    // Eigene id je Durchlauf: Mermaid entfernt vor dem Zeichnen Elemente mit dieser id, das stehende Bild trägt die alte
    const renderId = `${id}r${++renders.current}`;
    (async () => {
      try {
        const { default: mermaid } = await import('mermaid');
        mermaid.initialize({ ...lookFor(theme), startOnLoad: false, securityLevel: 'strict' });
        const { svg } = await mermaid.render(renderId, source);
        if (!cancelled) setResult({ svg });
      } catch (error) {
        if (!cancelled) setResult({ error: (error as Error).message });
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [id, source, theme]);

  if (!result) return <p className="muted">Diagramm wird gezeichnet …</p>;
  if ('error' in result) {
    return (
      <div role="alert" className="diagram-error">
        <strong>Diagramm kann nicht gezeichnet werden</strong>
        <p>{result.error}</p>
        <pre>{source}</pre>
      </div>
    );
  }
  return (
    <div className="diagram-wrap">
      <button type="button" className="btn small diagram-expand" aria-label="Diagramm im Vollbild öffnen" onClick={() => setFull(true)}>
        Vollbild
      </button>
      <div className="diagram" dangerouslySetInnerHTML={{ __html: result.svg }} />
      {full && <DiagramViewer svg={result.svg} title={title} onClose={() => setFull(false)} />}
    </div>
  );
}
