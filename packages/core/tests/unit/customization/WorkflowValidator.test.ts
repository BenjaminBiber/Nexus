import { describe, expect, it } from 'vitest';
import { WorkflowValidator } from '../../../src/customization/WorkflowValidator.js';
import { WORKFLOW_PHASES, type WorkflowPhase } from '../../../src/workflow/WorkflowPhases.js';

const SKILLS = ['harness-grill', 'harness-wayfinder', 'harness-tasks', 'harness-implement'];
const validator = new WorkflowValidator();
const codes = (list: { code: string }[]) => list.map((p) => p.code);
const withStep = (step: WorkflowPhase['steps'][number]): WorkflowPhase[] =>
  WORKFLOW_PHASES.map((p) => (p.id === 'umsetzen' ? { ...p, steps: [...p.steps, step] } : p));

describe('WorkflowValidator', () => {
  it('nimmt den Standard ohne Befund an', () => {
    expect(validator.validate(WORKFLOW_PHASES, SKILLS)).toEqual({ problems: [], warnings: [] });
  });

  it('verlangt eine Liste von Phasen mit Texten', () => {
    expect(validator.validate({}, SKILLS).problems[0].message).toBe('Der Workflow muss eine Liste von Phasen sein.');
    const result = validator.validate([{ id: 'planen', hint: 'h', steps: [] }], SKILLS);
    expect(codes(result.problems)).toContain('shape-invalid');
    expect(result.problems[0].message).toContain('phases[0].title');
  });

  it('verlangt die Pflicht-Phasen und eindeutige IDs', () => {
    const onlyPlan = WORKFLOW_PHASES.filter((p) => p.id === 'planen');
    expect(validator.validate(onlyPlan, SKILLS).problems).toEqual([{ code: 'required-phase-missing', message: 'Die Pflicht-Phase umsetzen fehlt.' }]);
    expect(codes(validator.validate([...WORKFLOW_PHASES, WORKFLOW_PHASES[0]], SKILLS).problems)).toEqual(['phase-id-duplicate']);
  });

  it('reserviert die Phasen-IDs der Anleitung im Board', () => {
    const extra = (id: string): WorkflowPhase => ({ id, title: 'Eigene Phase', hint: 'h', steps: [] });
    for (const id of ['fragen', 'nebenbei']) {
      expect(validator.validate([...WORKFLOW_PHASES, extra(id)], SKILLS).problems).toEqual([
        { code: 'phase-id-reserved', message: `Die Phasen-ID ${id} ist für die Anleitung reserviert.` },
      ]);
    }
    // Andere eigene IDs bleiben erlaubt
    expect(validator.validate([...WORKFLOW_PHASES, extra('qualitaet')], SKILLS).problems).toEqual([]);
  });

  it('warnt einmal je fehlendem Harness-Skill und prüft fremde Befehle nicht', () => {
    const result = validator.validate(withStep({ step: 'Review', command: '/ai-harness:mein-review', when: 'Vorher.', options: [{ label: 'R', command: '/ai-harness:mein-review', when: 'x' }] }), SKILLS);
    expect(result.problems).toEqual([]);
    expect(result.warnings).toEqual([{ code: 'skill-ref-missing', message: '/ai-harness:mein-review gibt es nicht (Schritt „Review“).' }]);
    expect(validator.validate(withStep({ step: 'Ideen', command: '/superpowers:brainstorming', when: 'x' }), SKILLS).warnings).toEqual([]);
  });
});
