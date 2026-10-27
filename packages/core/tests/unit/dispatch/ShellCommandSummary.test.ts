import { describe, expect, it } from 'vitest';
import { ShellCommandSummary } from '../../../src/dispatch/ShellCommandSummary.js';

describe('ShellCommandSummary', () => {
  const s = new ShellCommandSummary();

  it('erkennt Lesen, Schreiben, Ändern per Skript, Suchen und Tests', () => {
    expect(s.summarize("cat src/a.tsx; echo ===; sed -n '1,130p' src/b.tsx")).toEqual({ kind: 'read', text: 'liest a.tsx, b.tsx' });
    expect(s.summarize("cat > src/c.tsx <<'EOF'\nexport {}\nEOF")).toEqual({ kind: 'edit', text: 'schreibt c.tsx' });
    expect(s.summarize("node - <<'EOF'\nconst fs=require('fs');let p='src/d.tsx';fs.writeFileSync(p,'x');\nEOF")).toEqual({ kind: 'edit', text: 'ändert per Skript: d.tsx' });
    expect(s.summarize('sed -i "s/a/b/" src/e.ts')).toEqual({ kind: 'edit', text: 'ändert e.ts' });
    expect(s.summarize('grep -n "db-composer" src/styles/x.css | head -40')).toEqual({ kind: 'search', text: 'sucht „db-composer“ in x.css' });
    expect(s.summarize('cd ../.. && npx vitest run packages/web/tests/A.test.tsx 2>&1 | tail -60')).toEqual({ kind: 'test', text: 'Tests: A.test.tsx' });
    expect(s.summarize('npm test 2>&1 | tail -25')).toEqual({ kind: 'test', text: 'Tests (alle)' });
  });

  it('kennt Installation, git und harness, sonst bleibt der gekürzte Befehl', () => {
    expect(s.summarize('npm ci 2>&1 | tail -5')).toEqual({ kind: 'run', text: 'installiert Abhängigkeiten' });
    expect(s.summarize('git commit -m "feat: Dialog"')).toEqual({ kind: 'run', text: 'git commit „feat: Dialog“' });
    expect(s.summarize('harness board show AIH-0043 2>&1 | head -200')).toEqual({ kind: 'run', text: 'liest Ticket AIH-0043' });
    expect(s.summarize('docker ps')).toEqual({ kind: 'run', text: 'docker ps' });
  });

  it('nimmt die Beschreibung des Agenten als Text, behält aber die Art', () => {
    expect(s.summarize('npm test', 'Alle Tests')).toEqual({ kind: 'test', text: 'Alle Tests' });
  });
});
