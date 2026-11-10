/** Version als [major, minor, patch]; fehlende Teile zählen als 0 */
type Version = [number, number, number];

/** Eine Version mit ein bis drei Teilen, optional mit führendem „v“ */
const VERSION_PATTERN = /^v?(\d+)(?:\.(\d+))?(?:\.(\d+))?$/;

// Zerlegt eine Version; ungültig → undefined
function parseVersion(text: string): { version: Version; parts: number } | undefined {
  const m = VERSION_PATTERN.exec(text);
  if (!m) return undefined;
  const parts = m.slice(1).filter((p) => p !== undefined).length;
  return { version: [Number(m[1]), Number(m[2] ?? 0), Number(m[3] ?? 0)], parts };
}

// Vergleicht zwei Versionen: negativ, 0 oder positiv
function compare(a: Version, b: Version): number {
  return a[0] - b[0] || a[1] - b[1] || a[2] - b[2];
}

// Prüft eine einzelne Bedingung wie `^1.2`, `~1.0.3`, `>=1.0.0` oder `1.1.0`
function satisfiesComparator(version: Version, comparator: string): boolean {
  if (comparator === '*' || comparator === 'x') return true;
  const m = /^(\^|~|>=|<=|>|<|=)?(.+)$/.exec(comparator);
  if (!m) return false;
  const parsed = parseVersion(m[2]);
  if (!parsed) return false;
  const { version: target, parts } = parsed;
  const c = compare(version, target);
  switch (m[1]) {
    case '>=': return c >= 0;
    case '<=': return c <= 0;
    case '>': return c > 0;
    case '<': return c < 0;
    case '^': {
      if (c < 0) return false;
      // ^0.x bleibt in derselben Minor, ^0.0.x in derselben Patch-Version
      if (target[0] > 0 || parts === 1) return version[0] === target[0];
      if (target[1] > 0 || parts === 2) return version[0] === 0 && version[1] === target[1];
      return compare(version, target) === 0;
    }
    case '~':
      if (c < 0) return false;
      return parts === 1 ? version[0] === target[0] : version[0] === target[0] && version[1] === target[1];
    default:
      return c === 0;
  }
}

// Erfüllt `version` den semver-Bereich `range`? Bedingungen mit Leerzeichen müssen alle gelten; Ungültiges → false
export function satisfiesSdkRange(version: string, range: string): boolean {
  const parsed = parseVersion(version.trim());
  const comparators = range.trim().split(/\s+/).filter(Boolean);
  if (!parsed || comparators.length === 0) return false;
  return comparators.every((c) => satisfiesComparator(parsed.version, c));
}
