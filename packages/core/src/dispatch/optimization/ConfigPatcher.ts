import { HarnessError } from '../../errors/HarnessError.js';
import type { DispatchConfig, Profile, RawDispatchConfig } from '../DispatchConfig.js';
import type { ConfigPatchOp } from './SuggestionTypes.js';

// Pfade, die ein Vorschlag ändern darf
const ALLOWED = [/^profiles\.[^.]+\.(maxTurns|timeoutMinutes|model|effort)$/, /^defaultProfile$/, /^rules\.\d+\.profile$/, /^escalation$/];

// Wendet die Änderungen eines Vorschlags auf eine Kopie der rohen Dispatcher-Einstellung an.
// Geprüft wird nur Pfad und Ausgangswert; ob das Ergebnis gültig ist, entscheidet DispatchConfigStore beim Schreiben.
export class ConfigPatcher {
  allowed(path: string): boolean {
    return ALLOWED.some((re) => re.test(path));
  }

  apply(raw: RawDispatchConfig, config: DispatchConfig, ops: ConfigPatchOp[]): RawDispatchConfig {
    const next = structuredClone(raw);
    for (const op of ops) {
      if (!this.allowed(op.path)) throw new HarnessError('input-invalid', `${op.path} darf ein Vorschlag nicht ändern.`);
      if (JSON.stringify(this.current(config, op.path)) !== JSON.stringify(op.from)) {
        throw new HarnessError('conflict', `${op.path} ist nicht mehr ${JSON.stringify(op.from)} – der Vorschlag ist veraltet.`, { reason: 'patch-stale' });
      }
      this.set(next, op.path, op.to);
    }
    return next;
  }

  // Wirksamer Wert (nach Normalisierung) – gegen ihn wurde der Vorschlag berechnet
  private current(config: DispatchConfig, path: string): unknown {
    const [head, a = '', b = ''] = path.split('.');
    if (head === 'profiles') return config.profiles[a]?.[b as keyof Profile];
    if (head === 'defaultProfile') return config.defaultProfile;
    if (head === 'rules') return config.rules[Number(a)]?.profile;
    return config.ladder;
  }

  private set(raw: RawDispatchConfig, path: string, value: unknown): void {
    const [head, a = '', b = ''] = path.split('.');
    if (head === 'profiles') {
      raw.profiles = { ...raw.profiles, [a]: { ...raw.profiles?.[a], [b]: value } };
    } else if (head === 'defaultProfile') {
      raw.defaultProfile = value as string;
    } else if (head === 'rules') {
      const rules = [...(raw.rules ?? [])];
      const i = Number(a);
      if (!rules[i]) throw new HarnessError('input-invalid', `Regel ${i + 1} gibt es nicht.`);
      rules[i] = { ...rules[i], profile: value as string };
      raw.rules = rules;
    } else {
      raw.escalation = value as string[];
    }
  }
}
