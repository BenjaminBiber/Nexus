// Baut latest.json für den Tauri-Updater im Format von tauri-action: je Plattform die Signatur
// (Inhalt der .sig-Datei) und die Download-Adresse am GitHub-Release. Die App fragt
// https://github.com/<repo>/releases/latest/download/latest.json ab (tauri.conf.json → plugins.updater).

// Updater-Plattformen und die Datei am Release, auf die sie zeigen. Die Schlüssel mit Bundle-Art
// (…-app, …-nsis, …-appimage) erwartet der Updater ab Tauri 2, die kurzen sind der Rückfall
export function updaterPlatforms(version) {
  const mac = 'Nexus_aarch64.app.tar.gz';
  const windows = `Nexus_${version}_x64-setup.exe`;
  const linux = `Nexus_${version}_amd64.AppImage`;
  return {
    'darwin-aarch64': mac,
    'darwin-aarch64-app': mac,
    'windows-x86_64': windows,
    'windows-x86_64-nsis': windows,
    'linux-x86_64': linux,
    'linux-x86_64-appimage': linux,
  };
}

// latest.json als Objekt. `signatures` bildet Dateinamen am Release auf den Inhalt ihrer .sig ab;
// fehlt eine, bricht die Funktion ab, statt eine Plattform ohne Update auszuliefern
export function updaterJson({ version, tag, repo, notes = '', pubDate, signatures }) {
  const platforms = {};
  const missing = [];
  for (const [key, file] of Object.entries(updaterPlatforms(version))) {
    const signature = signatures[file];
    if (!signature) {
      missing.push(`${file}.sig`);
      continue;
    }
    platforms[key] = {
      signature: signature.trim(),
      url: `https://github.com/${repo}/releases/download/${tag}/${encodeURIComponent(file)}`,
    };
  }
  if (missing.length > 0) throw new Error(`Für latest.json fehlen Signaturen: ${[...new Set(missing)].join(', ')}`);
  return { version, notes: notes.trim(), pub_date: pubDate, platforms };
}
