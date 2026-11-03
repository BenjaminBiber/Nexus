import { useEffect, useRef, useState } from 'react';
import { useApi } from '../../api/ApiContext.js';

export interface AccountChoice {
  id: string;
  name: string;
  enabled: boolean;
}

interface Props {
  label: string;
  /** Account-IDs, durch Komma getrennt (wie im Formular) */
  value: string;
  accounts: AccountChoice[];
  /** Anzeige ohne Auswahl, z. B. „alle“ */
  emptyText: string;
  onChange: (value: string) => void;
}

const split = (value: string) => value.split(',').map((s) => s.trim()).filter(Boolean);

// Dropdown mit Checkboxen für die Accounts eines Profils oder einer Regel. Die Reihenfolge der Auswahl bleibt erhalten (neue hinten an),
// weil „Erster freier der Liste“ danach wählt. IDs ohne passenden Account (entfernt) bleiben sichtbar und abwählbar.
export function AccountMultiSelect({ label, value, accounts, emptyText, onChange }: Props) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const selected = split(value);

  // Klick außerhalb und Escape schließen das Menü
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => { if (!ref.current?.contains(e.target as Node)) setOpen(false); };
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') setOpen(false); };
    document.addEventListener('mousedown', onDown);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('mousedown', onDown);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  const known = new Set(accounts.map((a) => a.id));
  const choices: AccountChoice[] = [...accounts, ...selected.filter((id) => !known.has(id)).map((id) => ({ id, name: id, enabled: false }))];
  const nameOf = (id: string) => choices.find((a) => a.id === id)?.name ?? id;
  const toggle = (id: string) => onChange((selected.includes(id) ? selected.filter((s) => s !== id) : [...selected, id]).join(', '));
  const summary = selected.length ? selected.map(nameOf).join(', ') : emptyText;

  return (
    <div className="account-select" ref={ref}>
      <button
        type="button"
        className={`account-select-btn${selected.length ? '' : ' empty'}`}
        aria-label={label}
        aria-haspopup="true"
        aria-expanded={open}
        title={summary}
        onClick={() => setOpen(!open)}
      >
        {summary}
      </button>
      {open && (
        <div className="account-select-menu" role="group" aria-label={label}>
          {choices.length === 0 && <p className="account-select-none">Keine Accounts angelegt (Einstellungen → Accounts).</p>}
          {choices.map((a) => (
            <label key={a.id}>
              <input type="checkbox" aria-label={a.name} checked={selected.includes(a.id)} onChange={() => toggle(a.id)} />
              <span>{a.name}</span>
              {!known.has(a.id) ? <small>unbekannt</small> : !a.enabled ? <small>aus</small> : a.name !== a.id ? <small>{a.id}</small> : null}
            </label>
          ))}
          {selected.length > 0 && <button type="button" className="account-select-clear" onClick={() => onChange('')}>Auswahl leeren ({emptyText})</button>}
        </div>
      )}
    </div>
  );
}

// Angelegte Accounts für die Auswahl; ohne Antwort des Servers eine leere Liste (gewählte IDs bleiben trotzdem sichtbar)
export function useAccountChoices(): AccountChoice[] {
  const api = useApi();
  const [accounts, setAccounts] = useState<AccountChoice[]>([]);
  useEffect(() => {
    let current = true;
    api.getAccounts().then(
      (list) => { if (current) setAccounts(list.accounts.map(({ id, name, enabled }) => ({ id, name, enabled }))); },
      () => undefined,
    );
    return () => { current = false; };
  }, [api]);
  return accounts;
}
