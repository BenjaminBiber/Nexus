// Versionen aus den Git-Tags `v*`. Die Auswahl selbst ist eine reine Funktion; Git steckt nur in `listTags`.

import { execFileSync } from 'node:child_process';
import { format, highest, tryParse } from './SemVer.mjs';

// Tags `v*` des Repos in `root`; `null`, wenn es dort kein Git-Repo gibt (oder kein Git)
export function listTags(root) {
  try {
    const output = execFileSync('git', ['tag', '--list', 'v*'], {
      cwd: root, encoding: 'utf8', windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
    });
    return output.split('\n').map((line) => line.trim()).filter(Boolean);
  } catch {
    return null;
  }
}

// Teilt Tags in brauchbare Versionen und übersprungene auf. Vorabversionen (`v1.0.0-rc.1`) zählen nicht als
// Release und landen in `prerelease`; alles, was keine Version ist, in `skipped`.
export function tagVersions(tags) {
  const versions = [];
  const prerelease = [];
  const skipped = [];
  for (const tag of tags) {
    const parsed = tryParse(tag.startsWith('v') ? tag.slice(1) : tag);
    if (!parsed) skipped.push(tag);
    else if (parsed.prerelease) prerelease.push(tag);
    // Der Build-Anhang gehört nicht zur Version (SemVer 10)
    else versions.push(format({ ...parsed, build: null }));
  }
  return { versions, prerelease, skipped };
}

// Höchste Version der Tags; `null`, wenn keines eine Release-Version ist
export function highestTag(tags) {
  return highest(tagVersions(tags).versions);
}
