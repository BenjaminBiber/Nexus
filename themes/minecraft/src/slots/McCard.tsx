// Slot „Card“: Karten des Hosts als Listeneintrag wie in der Weltenliste – Rahmen 1 px Grau außen,
// 1 px Schwarz innen (bei Hover und Fokus weiß) auf Schwarz 50 %; Maße in theme.css.
import type { CardProps } from '@nexus/theme-sdk';

export function McCard({ title, actions, className, children }: CardProps) {
  return (
    <div className={['mc-card', className].filter(Boolean).join(' ')}>
      {(title || actions) && (
        <div className="mc-card-head">
          {title && <div className="mc-card-title mc-shadow">{title}</div>}
          {actions && <div className="mc-card-actions">{actions}</div>}
        </div>
      )}
      <div className="mc-card-body">{children}</div>
    </div>
  );
}
