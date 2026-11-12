import { CliStep } from './CliStep.mjs';
import { CommitStep } from './CommitStep.mjs';
import { HomeFileStep } from './HomeFileStep.mjs';
import { HttpStep } from './HttpStep.mjs';
import { HttpStubStep } from './HttpStubStep.mjs';
import { SseCloseStep, SseExpectStep, SseOpenStep } from './SseSteps.mjs';
import { WaitStep } from './WaitStep.mjs';

// Bekannte Schritt-Arten; eine neue Art ist ein eigenes Modul mit `run(step, context)` und `keys`
// und meldet sich hier an. Der Lader prüft damit die Felder, der Runner führt die Schritte aus.
// Nennt die Art zusätzlich ein `parse`, prüft sie ihre Felder schon beim Lesen des Szenarios;
// ohne `parse` bekommt sie den rohen Schritt als `raw` und prüft ihn beim Ausführen.
export const stepKinds = {
  http: new HttpStep(),
  cli: new CliStep(),
  httpStub: new HttpStubStep(),
  sseOpen: new SseOpenStep(),
  sseExpect: new SseExpectStep(),
  sseClose: new SseCloseStep(),
  commit: new CommitStep(),
  homeFile: new HomeFileStep(),
  wait: new WaitStep(),
};
