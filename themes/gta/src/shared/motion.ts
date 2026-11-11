/** true, wenn das System reduzierte Bewegung wünscht; ohne matchMedia (Tests, alte Browser) false */
export function reducedMotion(): boolean {
  try {
    return window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  } catch {
    return false;
  }
}
