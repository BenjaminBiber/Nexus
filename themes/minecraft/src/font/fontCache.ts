// Erzeugte Schrift in der Cache API ablegen; ohne Cache API (alte WebView, jsdom ohne Stub) wird jedes Mal gebaut
const CACHE = 'nexus-theme-minecraft';

/** Schlüssel der Schrift: Spielversion und die Quell-URLs bestimmen das Ergebnis */
export function fontCacheKey(version: string, urls: string[]): string {
  let h = 0;
  for (const ch of urls.join('|')) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return `/font/${version}/${h.toString(16)}`;
}

export async function readCachedFont(key: string): Promise<ArrayBuffer | null> {
  try {
    const hit = await (await caches.open(CACHE)).match(key);
    return hit ? await hit.arrayBuffer() : null;
  } catch {
    return null;
  }
}

export async function writeCachedFont(key: string, bytes: ArrayBuffer): Promise<void> {
  try {
    await (await caches.open(CACHE)).put(key, new Response(bytes, { headers: { 'Content-Type': 'font/ttf' } }));
  } catch {
    /* ohne Cache kein Merken */
  }
}
