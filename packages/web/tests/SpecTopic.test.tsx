// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { ApiProvider } from '../src/api/ApiContext.js';
import { specTopic } from '../src/board/specTopic.js';
import { DbSpecsDialog } from '../src/components/db/DbSpecsDialog.js';
import { fakeApi } from './helpers/fakeApi.js';

afterEach(cleanup);

const SPEC = 'docs/superpowers/specs/2026-10-22-spec-docs-rework-design.md';

describe('specTopic', () => {
  it('macht aus einem Spec-Pfad das Thema', () => {
    expect(specTopic(SPEC)).toBe('spec-docs-rework');
  });

  it('macht aus einem Plan-Pfad dasselbe Thema', () => {
    expect(specTopic('docs/superpowers/plans/2026-10-22-spec-docs-rework-plan.md')).toBe('spec-docs-rework');
  });

  it('kommt ohne Datum und mit Rückstrichen im Pfad aus', () => {
    expect(specTopic('docs\\superpowers\\specs\\ohne-datum-design.md')).toBe('ohne-datum');
  });

  it('lässt alte Spec-Werte unverändert', () => {
    expect(specTopic('001-login')).toBe('001-login');
  });

  it('behält den Eingabewert, wenn nach dem Kürzen nichts übrig bleibt', () => {
    expect(specTopic('docs/superpowers/plans/-plan.md')).toBe('docs/superpowers/plans/-plan.md');
  });
});

describe('DbSpecsDialog', () => {
  it('beschriftet die Spec mit dem Thema und nennt den Pfad im Hinweis', async () => {
    const api = fakeApi({
      getDbSpecProgress: async () => [{ spec: SPEC, total: 2, done: 1, blocked: 0, waiting: 0, inProgress: 0, open: 1, notNeeded: 0 }],
    });
    render(
      <ApiProvider api={api}>
        <DbSpecsDialog boardId={1} onClose={() => undefined} onOpenSpec={() => undefined} />
      </ApiProvider>,
    );
    const button = await screen.findByRole('button', { name: /spec-docs-rework/ });
    expect(button.textContent).toBe('spec-docs-rework');
    expect(button.getAttribute('title')).toContain(SPEC);
  });
});
