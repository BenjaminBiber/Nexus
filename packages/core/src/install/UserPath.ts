// Trägt einen Ordner dauerhaft in den PATH des Benutzers ein (nur Benutzerebene, nie systemweit).
export interface UserPath {
  /** Liefert true, wenn der Eintrag neu hinzugefügt wurde, false, wenn er schon vorhanden war */
  add(dir: string): Promise<boolean>;
}
