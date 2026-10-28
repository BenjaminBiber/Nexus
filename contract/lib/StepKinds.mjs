import { CommitStep } from './CommitStep.mjs';
import { HomeFileStep } from './HomeFileStep.mjs';
import { HttpStep } from './HttpStep.mjs';
import { HttpStubStep } from './HttpStubStep.mjs';

// Bekannte Schritt-Arten; eine neue Art ist ein eigenes Modul mit `run(step, context)` und `keys`
// und meldet sich hier an. Der Lader prüft damit die Felder, der Runner führt die Schritte aus.
export const stepKinds = { commit: new CommitStep(), homeFile: new HomeFileStep(), http: new HttpStep(), httpStub: new HttpStubStep() };
