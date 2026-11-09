// Test-Theme mit eigener Shell: reines ESM ohne Build, React und JSX-Runtime kommen über die Import-Map der App
import { jsx, jsxs } from 'react/jsx-runtime';

// Eigene Shell: Kopfzeile mit Suche und Boardwechsel, Navigation und Inhalt ohne Standard-Seitenleiste
function Shell({ nav, content, parts }) {
  return jsxs('div', {
    'data-testid': 'e2e-shell',
    className: 'e2e-shell',
    children: [
      jsxs('header', { className: 'e2e-shell-header', children: [jsx('strong', { children: 'E2E-Shell' }), parts.search, parts.boardSwitcher] }),
      jsx('nav', { 'aria-label': 'E2E-Navigation', children: nav }),
      content,
    ],
  });
}

export default { id: 'e2e-shell', slots: { Shell } };
