import { cloneElement, useId, type ReactElement, type ReactNode } from 'react';
import { InfoTip } from './InfoTip.js';

type Control = ReactElement<{ id?: string; 'aria-describedby'?: string }>;

interface SettingRowProps {
  label: string;
  /** Ein kurzer Satz unter der Beschriftung; Längeres gehört in info */
  hint?: ReactNode;
  /** Ausführliche Erklärung hinter dem (i) */
  info?: ReactNode;
  /** Bedienelement unter der Beschriftung über die volle Breite (Textfelder, Tabellen) */
  wide?: boolean;
  /** Kein einzelnes Eingabefeld (Tabelle, Knopfgruppe): die Zeile wird zur Gruppe mit der Beschriftung als Name */
  group?: boolean;
  children: ReactNode;
}

// Eine Einstellung als Zeile: links Beschriftung, kurzer Hinweis und (i), rechts das Bedienelement.
// Ein einzelnes Eingabefeld bekommt id und aria-describedby, damit Beschriftung und Hinweis zu ihm gehören.
export function SettingRow({ label, hint, info, wide, group, children }: SettingRowProps) {
  const id = useId();
  const hintId = hint ? `${id}-hint` : undefined;
  const labelId = `${id}-label`;
  const title = group ? <span id={labelId} className="setting-label">{label}</span> : <label htmlFor={(children as Control).props.id ?? id}><span>{label}</span></label>;
  const control = group ? children : cloneControl(children as Control, id, hintId);
  return (
    <div className={`setting-row${wide ? ' wide' : ''}`} {...(group ? { role: 'group', 'aria-labelledby': labelId, 'aria-describedby': hintId } : {})}>
      <div className="setting-text">
        <div className="setting-title">
          {title}
          {info && <InfoTip label={label}>{info}</InfoTip>}
        </div>
        {hint && <p className="setting-hint" id={hintId}>{hint}</p>}
      </div>
      <div className="setting-control">{control}</div>
    </div>
  );
}

// Eingabefeld mit eigener oder vergebener id; der Hinweis kommt zu einer vorhandenen Beschreibung dazu
function cloneControl(control: Control, id: string, hintId: string | undefined) {
  const describedBy = [control.props['aria-describedby'], hintId].filter(Boolean).join(' ') || undefined;
  return cloneElement(control, { id: control.props.id ?? id, 'aria-describedby': describedBy });
}

interface SettingsSectionProps {
  title?: string;
  /** Höchstens ein Satz; Längeres gehört in info */
  description?: ReactNode;
  info?: ReactNode;
  /** Ohne Fläche: für Inhalte mit eigener Gestaltung (Auswahlkacheln, Editoren) */
  bare?: boolean;
  children: ReactNode;
}

// Gruppe zusammengehöriger Einstellungen: Überschrift mit (i) und einem Satz, darunter die Zeilen auf einer Fläche.
export function SettingsSection({ title, description, info, bare, children }: SettingsSectionProps) {
  return (
    <div className="settings-section">
      {(title || description) && (
        <div className="settings-section-head">
          {title && (
            <div className="settings-section-title">
              <h3>{title}</h3>
              {info && <InfoTip label={title}>{info}</InfoTip>}
            </div>
          )}
          {description && <p>{description}</p>}
        </div>
      )}
      {bare ? children : <div className="settings-card">{children}</div>}
    </div>
  );
}
