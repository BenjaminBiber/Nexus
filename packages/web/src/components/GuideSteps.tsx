import { useState } from 'react';
import type { GuideStep } from '../guide/guideContent.js';
import { CopyCommand } from './CopyCommand.js';

// Umschalter für gleichwertige Alternativen eines Schritts (z. B. Grill oder Wayfinder).
function OptionSwitch({ labels, selected, onSelect }: { labels: string[]; selected: number; onSelect: (index: number) => void }) {
  return (
    <span className="step-options" role="group" aria-label="Alternative wählen">
      {labels.map((label, index) => (
        <button key={label} type="button" className="step-option" aria-pressed={index === selected} onClick={() => onSelect(index)}>
          {label}
        </button>
      ))}
    </span>
  );
}

// Ein Schritt: Titel, kopierbarer Befehl und Erklärung; bei Alternativen mit Umschalter.
function StepRow({ row, index }: { row: GuideStep; index: number }) {
  const [selected, setSelected] = useState(0);
  const shown = row.options?.[selected] ?? row;
  return (
    <li className="step">
      <span className="num">{index + 1}</span>
      <span className="step-title">
        {row.step}
        {row.options && <OptionSwitch labels={row.options.map((o) => o.label)} selected={selected} onSelect={setSelected} />}
      </span>
      {shown.command ? <CopyCommand command={shown.command} /> : <span className="step-place">{row.place}</span>}
      <span className="step-when muted">{shown.when}</span>
    </li>
  );
}

// Nummerierte Schrittliste der Anleitung.
export function GuideSteps({ steps, label }: { steps: GuideStep[]; label: string }) {
  return (
    <ol className="steps" aria-label={label}>
      {steps.map((row, index) => (
        <StepRow key={row.step} row={row} index={index} />
      ))}
    </ol>
  );
}
