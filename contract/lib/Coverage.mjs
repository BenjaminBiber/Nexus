// Ordnet die aufgezeichneten Schritte den Routen aus `api/routes.json` zu und meldet abgedeckt/offen.
export class Coverage {
  constructor(routes) {
    this.routes = routes.map((route) => ({ ...route, regex: Coverage.regex(route.pattern), hits: 0 }));
    this.unmatched = [];
  }

  // Muster einer Route als regulärer Ausdruck (`:id` → ein Pfadabschnitt, `*` → Rest des Pfades)
  static regex(pattern) {
    const parts = pattern.split('/').map((part) => {
      if (part.startsWith(':')) return '[^/]+';
      if (part === '*') return '.*';
      return part.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    });
    return new RegExp(`^${parts.join('/')}$`);
  }

  // Zählt einen aufgezeichneten Schritt; Abfragezeichen hinter `?` zählen nicht zum Pfad
  add(method, path) {
    const url = path.split('?')[0];
    const candidates = this.routes.filter((route) => route.method === method);
    // Ein Muster mit `*` (der Rückfall der Oberfläche) deckt alles ab und zählt erst, wenn keine genauere Route passt
    const specific = candidates.filter((route) => !route.pattern.includes('*'));
    const route = candidates.find((entry) => entry.pattern === url)
      ?? specific.find((entry) => entry.regex.test(url))
      ?? candidates.find((entry) => entry.regex.test(url));
    if (!route) {
      this.unmatched.push(`${method} ${url}`);
      return false;
    }
    route.hits += 1;
    return true;
  }

  // Alle Routen ohne Schritt
  get open() {
    return this.routes.filter((route) => route.hits === 0);
  }

  // Tabelle je Quelldatei: `[x]` abgedeckt, `[ ]` offen
  report() {
    const lines = [];
    const covered = this.routes.length - this.open.length;
    lines.push(`Abdeckung: ${covered} von ${this.routes.length} Routen`);
    for (const file of [...new Set(this.routes.map((route) => route.file))]) {
      const group = this.routes.filter((route) => route.file === file);
      const done = group.filter((route) => route.hits > 0).length;
      lines.push('', `${file}  ${done}/${group.length}`);
      for (const route of group) {
        lines.push(`  ${route.hits > 0 ? '[x]' : '[ ]'} ${route.method.padEnd(6)} ${route.pattern}`);
      }
    }
    if (this.unmatched.length) {
      lines.push('', 'Ohne Route in routes.json (Liste mit `node contract/lib/extract-routes.mjs` neu erzeugen):');
      for (const entry of [...new Set(this.unmatched)].sort()) lines.push(`  ? ${entry}`);
    }
    return lines.join('\n');
  }
}
