#!/usr/bin/env node
// Signiert eine Windows-Datei über Azure Trusted Signing. Tauri ruft dieses Skript über
// bundle.windows.signCommand auf – ohne Shell, darum lässt sich dort keine Umgebungsvariable
// einsetzen und die Werte werden hier gelesen. Fehlt eine Angabe (lokaler Build, Fork ohne
// Secrets), bleibt die Datei unsigniert und das Skript endet mit 0.
import { spawnSync } from 'node:child_process';

// Konfiguration aus der Umgebung; endpoint/account/profile kommen als Variablen, der Rest als Secret
function leseKonfiguration() {
  const endpoint = process.env.AZURE_SIGNING_ENDPOINT;
  const account = process.env.AZURE_SIGNING_ACCOUNT;
  const profile = process.env.AZURE_SIGNING_PROFILE;
  const fehlend = [
    ['AZURE_SIGNING_ENDPOINT', endpoint],
    ['AZURE_SIGNING_ACCOUNT', account],
    ['AZURE_SIGNING_PROFILE', profile],
    ['AZURE_CLIENT_ID', process.env.AZURE_CLIENT_ID],
    ['AZURE_CLIENT_SECRET', process.env.AZURE_CLIENT_SECRET],
    ['AZURE_TENANT_ID', process.env.AZURE_TENANT_ID],
  ]
    .filter(([, wert]) => !wert)
    .map(([name]) => name);
  return { endpoint, account, profile, fehlend };
}

// Befehlszeile von trusted-signing-cli für eine Datei. Nicht gesetzte Werte werden im Probelauf
// als Platzhalter sichtbar, damit der Befehl auch ohne Secrets nachvollziehbar ist.
function baueBefehl({ endpoint, account, profile }, datei) {
  const wert = (name, inhalt) => inhalt ?? `<${name}>`;
  return [
    'trusted-signing-cli',
    '-e',
    wert('AZURE_SIGNING_ENDPOINT', endpoint),
    '-a',
    wert('AZURE_SIGNING_ACCOUNT', account),
    '-c',
    wert('AZURE_SIGNING_PROFILE', profile),
    '-d',
    'Nexus',
    datei,
  ];
}

const argumente = process.argv.slice(2);
const probelauf = argumente.includes('--dry-run');
const datei = argumente.find((wert) => wert !== '--dry-run');

if (!datei) {
  console.error('Aufruf: sign-windows.mjs [--dry-run] <datei>');
  process.exit(2);
}

const konfiguration = leseKonfiguration();

// Der Probelauf zeigt nur den Befehl und signiert nichts – auch ohne vollständige Konfiguration
if (probelauf) {
  if (konfiguration.fehlend.length > 0) {
    console.error(`Hinweis: ${konfiguration.fehlend.join(', ')} nicht gesetzt.`);
  }
  console.log(baueBefehl(konfiguration, datei).join(' '));
  process.exit(0);
}

if (konfiguration.fehlend.length > 0) {
  console.error(
    `Keine Signatur: ${konfiguration.fehlend.join(', ')} nicht gesetzt. Die Datei bleibt unsigniert.`,
  );
  process.exit(0);
}

const befehl = baueBefehl(konfiguration, datei);

const ergebnis = spawnSync(befehl[0], befehl.slice(1), { stdio: 'inherit', windowsHide: true });

if (ergebnis.error) {
  console.error(`trusted-signing-cli konnte nicht gestartet werden: ${ergebnis.error.message}`);
  process.exit(1);
}

process.exit(ergebnis.status ?? 1);
