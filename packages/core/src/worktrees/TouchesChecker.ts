// Prüft, ob sich die betroffenen Bereiche (`touches`) zweier Tickets überschneiden.
export class TouchesChecker {
  // Überschneidung: gleicher Pfad oder einer liegt innerhalb des anderen
  overlaps(a: string[], b: string[]): boolean {
    const left = a.map((p) => this.normalize(p));
    const right = b.map((p) => this.normalize(p));
    return left.some((x) => right.some((y) => this.contains(x, y) || this.contains(y, x)));
  }

  // `outer` ist gleich `inner` oder ein Oberordner davon (ganze Pfadteile, kein bloßer Namensanfang)
  private contains(outer: string, inner: string): boolean {
    return outer === inner || inner.startsWith(`${outer}/`);
  }

  // Vereinheitlicht Schrägstriche, entfernt „./“ am Anfang und „/“ am Ende
  private normalize(path: string): string {
    return path.replace(/\\/g, '/').replace(/^\.\//, '').replace(/\/+$/, '');
  }
}
