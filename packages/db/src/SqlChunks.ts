// Teilt lange ID-Listen in Blöcke, damit eine Abfrage unter der Parametergrenze von SQL Server (2100) bleibt.
export class SqlChunks {
  // Blöcke zu höchstens `size` Einträgen; eine leere Liste ergibt keine Blöcke
  static of<T>(items: T[], size = 500): T[][] {
    const chunks: T[][] = [];
    for (let i = 0; i < items.length; i += size) chunks.push(items.slice(i, i + size));
    return chunks;
  }

  // Parameter @<prefix>0, @<prefix>1 … und die passende IN-Liste für einen Block
  static params<T extends string | number>(prefix: string, values: T[]): { list: string; params: Record<string, T> } {
    const params: Record<string, T> = {};
    values.forEach((v, i) => { params[`${prefix}${i}`] = v; });
    return { list: values.map((_, i) => `@${prefix}${i}`).join(', '), params };
  }
}
