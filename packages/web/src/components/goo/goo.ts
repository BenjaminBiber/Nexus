// Gemeinsame Hilfe der Goo-Effekte (Blur + Alpha-Schwelle): sie laufen nur, wenn der Nutzer Bewegung zulässt.
// Ohne matchMedia (z. B. in Tests) gibt es keine Animation, die Oberfläche reagiert dann sofort.
export function prefersReducedMotion(): boolean {
  return typeof window.matchMedia !== 'function' || window.matchMedia('(prefers-reduced-motion: reduce)').matches;
}
