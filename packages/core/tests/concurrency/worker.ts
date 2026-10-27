import { HarnessProject } from '../../src/project/HarnessProject.js';

// Hilfsprozess für den Nebenläufigkeitstest: zieht N Session-Nummern und gibt sie als JSON aus.
// Aufruf: worker.ts <Arbeitsordner> <Anzahl>
async function main(): Promise<void> {
  const [cwd, countText] = process.argv.slice(2);
  const project = await HarnessProject.open(cwd);
  const numbers: number[] = [];
  for (let i = 0; i < Number(countText); i++) numbers.push(await project.counter.next('session'));
  console.log(JSON.stringify(numbers));
}

await main();
