// Item-Symbol je Navigationsziel: Einträge im Client-JAR (gegen 1.21.4 geprüft)
import type { NavIconName } from '@nexus/theme-sdk';

const item = (name: string) => `assets/minecraft/textures/item/${name}.png`;

/** Textur des Items, das in der Hotbar für ein Navigationsziel steht */
export const NAV_ITEM_TEXTURE: Record<NavIconName, string> = {
  board: item('map'),
  dashboard: item('clock_00'),
  inbox: item('bundle'),
  sessions: item('writable_book'),
  questions: item('book'),
  docs: item('written_book'),
  search: item('spyglass'),
  guide: item('compass_00'),
  settings: item('redstone'),
};
