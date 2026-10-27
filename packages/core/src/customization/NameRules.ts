// Regeln für Namen von Skills und Output-Styles: Kleinbuchstaben, Ziffern, einzelne Bindestriche, höchstens 64 Zeichen.
export class NameRules {
  private static readonly PATTERN = /^[a-z0-9]+(-[a-z0-9]+)*$/;

  // true, wenn der Name als Skill- oder Output-Style-Name zulässig ist
  isValid(name: string): boolean {
    return name.length <= 64 && NameRules.PATTERN.test(name);
  }
}
