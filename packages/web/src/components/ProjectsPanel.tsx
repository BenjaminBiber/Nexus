import type { ProjectDto } from '../api/types.js';
import { Badge } from './Badge.js';
import { SettingsPanel } from './settings/SettingsPanel.js';
import { SettingsSection } from './settings/SettingRow.js';

// Angemeldete Projekte mit Pfad; nicht erreichbare sind mit Text gekennzeichnet.
export function ProjectsPanel({ projects, current }: { projects: ProjectDto[]; current: string }) {
  return (
    <SettingsPanel label="Arbeitsverzeichnisse">
      <SettingsSection info="Ein Ordner kommt dazu, sobald dort eine Claude-Session mit dem Harness-Plugin läuft. „Nicht erreichbar“ heißt: Der Pfad existiert auf diesem Rechner nicht mehr.">
        <ul className="plain">
          {projects.map((p) => (
            <li key={p.id} className="project-row">
              <span className="project-name">
                <strong>{p.name}</strong>
                {p.id === current && <Badge kind="running">aktuell</Badge>}
                {!p.reachable && <Badge kind="blocked">nicht erreichbar</Badge>}
              </span>
              <span className="mono muted project-path">{p.path}</span>
            </li>
          ))}
        </ul>
      </SettingsSection>
    </SettingsPanel>
  );
}
