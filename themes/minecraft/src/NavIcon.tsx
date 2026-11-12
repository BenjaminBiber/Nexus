// Slot „NavIcon“: Item-Symbol aus dem Client-JAR; ohne Ressourcen ein Pixel-Feld mit dem Anfangsbuchstaben
import type { NavIconName } from '@nexus/theme-sdk';
import { useResourceUrl } from './context.js';
import { NAV_ITEM_TEXTURE } from './navIcons.js';

export function NavIcon({ name }: { name: NavIconName }) {
  const url = useResourceUrl(NAV_ITEM_TEXTURE[name]);
  if (url) return <img className="mc-nav-icon" src={url} alt="" width={16} height={16} draggable={false} />;
  return (
    <span className="mc-nav-icon mc-nav-icon--fallback" aria-hidden="true">
      {name.charAt(0).toUpperCase()}
    </span>
  );
}
