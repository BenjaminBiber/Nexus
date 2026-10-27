import {
  DISPATCH_PROMPT_DEFAULTS, DISPATCH_PROMPT_PLACEHOLDERS, PROMPT_KINDS, PromptTemplateCheck,
  type DispatchPromptKind, type PromptPlaceholder,
} from '@harness/core';

export interface DispatchPromptInfo {
  kind: DispatchPromptKind;
  /** Beschriftung der Laufart für die Oberfläche */
  label: string;
  /** Standardvorlage, die ohne eigene Vorlage gilt */
  default: string;
  placeholders: PromptPlaceholder[];
  /** Beschriftungen der Pflichtteile dieser Laufart */
  required: string[];
}

const LABELS: Record<DispatchPromptKind, string> = {
  work: 'Umsetzung', plan: 'Planung', research: 'Recherche', merge: 'Merge',
};

// Was die Oberfläche zum Bearbeiten einer Vorlage braucht. Die Inhalte kommen vollständig aus core –
// hier wird kein Vorlagentext wiederholt, sonst liefe die Anzeige dem Standard hinterher.
export class PromptCatalog {
  private readonly check = new PromptTemplateCheck();

  list(): DispatchPromptInfo[] {
    return PROMPT_KINDS.map((kind) => ({
      kind,
      label: LABELS[kind],
      default: DISPATCH_PROMPT_DEFAULTS[kind],
      placeholders: DISPATCH_PROMPT_PLACEHOLDERS[kind],
      required: this.check.parts(kind).map((part) => part.label),
    }));
  }
}
