// Feste Git-Identität, feste Commit-Daten und die Einstellungen, die den Commit beeinflussen – für jeden Commit eines
// Szenarios: die der Fixture-Repos und die des geprüften Servers (Merge eines Ticket-Branches). Nur so sind die
// Commit-Kennungen über Läufe und Rechner hinweg dieselben und dürfen als Klartext in den Golden-Dateien stehen.
// Git liest diese Variablen unabhängig davon, in welcher Sprache der Server geschrieben ist – jede Umsetzung bekommt
// dieselben Kennungen. `GIT_CONFIG_*` hängt die Einstellungen an, die sonst aus der Einstellung des Rechners kämen:
// eine Signatur und umgesetzte Zeilenenden würden die Kennungen verändern.
export const GIT_IDENTITY_ENV = {
  GIT_AUTHOR_NAME: 'Vertrag',
  GIT_AUTHOR_EMAIL: 'vertrag@example.invalid',
  GIT_COMMITTER_NAME: 'Vertrag',
  GIT_COMMITTER_EMAIL: 'vertrag@example.invalid',
  // Weit zurückliegend, damit der Normalisierer das Datum nicht als Grenze „vor n Tagen“ liest
  GIT_AUTHOR_DATE: '2020-03-02T08:00:00Z',
  GIT_COMMITTER_DATE: '2020-03-02T08:00:00Z',
  GIT_CONFIG_COUNT: '2',
  GIT_CONFIG_KEY_0: 'commit.gpgsign',
  GIT_CONFIG_VALUE_0: 'false',
  GIT_CONFIG_KEY_1: 'core.autocrlf',
  GIT_CONFIG_VALUE_1: 'false',
};
