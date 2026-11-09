import type { ReactNode } from 'react';
import { SlotBoundary } from '../theme/SlotBoundary.js';
import type { NavIconName } from '@nexus/theme-sdk';
import { useDisplayedTheme } from '../theme/useDisplayedTheme.js';
import { useSlot } from '../theme/useSlot.js';

// Linien-Symbol im 24er-Raster, Farbe aus currentColor
function LineIcon({ children }: { children: ReactNode }) {
  return (
    <svg className="nav-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      {children}
    </svg>
  );
}

// Symbol der Navigation; mit `name` darf der Slot „NavIcon“ des Themes es ersetzen
function Icon({ name, children }: { name?: NavIconName; children: ReactNode }) {
  const theme = useDisplayedTheme();
  const Custom = useSlot('NavIcon');
  const standard = <LineIcon>{children}</LineIcon>;
  if (!name || !Custom) return standard;
  return (
    <SlotBoundary fallback={standard} resetKey={theme}>
      <Custom name={name} />
    </SlotBoundary>
  );
}

export const BoardIcon = () => <Icon name="board"><path d="M4 4h6v16H4zM14 4h6v10h-6z" /></Icon>;
export const DashboardIcon = () => <Icon name="dashboard"><path d="M4 20V10M10 20V4M16 20v-7M21 20H3" /></Icon>;
export const InboxIcon = () => <Icon name="inbox"><path d="M4 13l2.5-8h11L20 13v6H4zM4 13h5l1 2h4l1-2h5" /></Icon>;
export const SessionsIcon = () => <Icon name="sessions"><path d="M4 5h16v14H4zM8 10l3 2-3 2M13 14h3" /></Icon>;
export const QuestionsIcon = () => <Icon name="questions"><path d="M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.6.3-1 .9-1 1.6v.6M12 17h.01" /></Icon>;
export const DocsIcon = () => <Icon name="docs"><path d="M6 3h8l4 4v14H6zM14 3v4h4M9 12h6M9 16h6" /></Icon>;
export const SearchIcon = () => <Icon name="search"><path d="M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14zM20 20l-4-4" /></Icon>;
export const GuideIcon = () => <Icon name="guide"><path d="M4 19V5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2zM9 7h6" /></Icon>;

// Offizielles Claude-Logo als Ersatz für die Pille „Agent“ auf der kompakten Ticket-Kachel
export function ClaudeIcon() {
  return (
    <svg viewBox="0 0 1200 1200" aria-hidden="true">
      <path
        fill="#d97757"
        d="M 233.959793 800.214905 L 468.644287 668.536987 L 472.590637 657.100647 L 468.644287 650.738403 L 457.208069 650.738403 L 417.986633 648.322144 L 283.892639 644.69812 L 167.597321 639.865845 L 54.926208 633.825623 L 26.577238 627.785339 L 3.3e-05 592.751709 L 2.73832 575.27533 L 26.577238 559.248352 L 60.724873 562.228149 L 136.187973 567.382629 L 249.422867 575.194763 L 331.570496 580.026978 L 453.261841 592.671082 L 472.590637 592.671082 L 475.328857 584.859009 L 468.724915 580.026978 L 463.570557 575.194763 L 346.389313 495.785217 L 219.543671 411.865906 L 153.100723 363.543762 L 117.181267 339.060425 L 99.060455 316.107361 L 91.248367 266.01355 L 123.865784 230.093994 L 167.677887 233.073853 L 178.872513 236.053772 L 223.248367 270.201477 L 318.040283 343.570496 L 441.825592 434.738342 L 459.946411 449.798706 L 467.194672 444.64447 L 468.080597 441.020203 L 459.946411 427.409485 L 392.617493 305.718323 L 320.778564 181.932983 L 288.80542 130.630859 L 280.348999 99.865845 C 277.369171 87.221436 275.194641 76.590698 275.194641 63.624268 L 312.322174 13.20813 L 332.8591 6.604126 L 382.389313 13.20813 L 403.248352 31.328979 L 434.013519 101.71814 L 483.865753 212.537048 L 561.181274 363.221497 L 583.812134 407.919434 L 595.892639 449.315491 L 600.40271 461.959839 L 608.214783 461.959839 L 608.214783 454.711609 L 614.577271 369.825623 L 626.335632 265.61084 L 637.771851 131.516846 L 641.718201 93.745117 L 660.402832 48.483276 L 697.530334 24.000122 L 726.52356 37.852417 L 750.362549 72 L 747.060486 94.067139 L 732.886047 186.201416 L 705.100708 330.52356 L 686.979919 427.167847 L 697.530334 427.167847 L 709.61084 415.087341 L 758.496704 350.174561 L 840.644348 247.490051 L 876.885925 206.738342 L 919.167847 161.71814 L 946.308838 140.29541 L 997.61084 140.29541 L 1035.38269 196.429626 L 1018.469849 254.416199 L 965.637634 321.422852 L 921.825562 378.201538 L 859.006714 462.765259 L 819.785278 530.41626 L 823.409424 535.812073 L 832.75177 534.92627 L 974.657776 504.724915 L 1051.328979 490.872559 L 1142.818848 475.167786 L 1184.214844 494.496582 L 1188.724854 514.147644 L 1172.456421 554.335693 L 1074.604126 578.496765 L 959.838989 601.449829 L 788.939636 641.879272 L 786.845764 643.409485 L 789.261841 646.389343 L 866.255127 653.637634 L 899.194702 655.409424 L 979.812134 655.409424 L 1129.932861 666.604187 L 1169.154419 692.537109 L 1192.671265 724.268677 L 1188.724854 748.429688 L 1128.322144 779.194641 L 1046.818848 759.865845 L 856.590759 714.604126 L 791.355774 698.335754 L 782.335693 698.335754 L 782.335693 703.731567 L 836.69812 756.885986 L 936.322205 846.845581 L 1061.073975 962.81897 L 1067.436279 991.490112 L 1051.409424 1014.120911 L 1034.496704 1011.704712 L 924.885986 929.234924 L 882.604126 892.107544 L 786.845764 811.48999 L 780.483276 811.48999 L 780.483276 819.946289 L 802.550415 852.241699 L 919.087341 1027.409424 L 925.127625 1081.127686 L 916.671204 1098.604126 L 886.469849 1109.154419 L 853.288696 1103.114136 L 785.073914 1007.355835 L 714.684631 899.516785 L 657.906067 802.872498 L 650.979858 806.81897 L 617.476624 1167.704834 L 601.771851 1186.147705 L 565.530212 1200 L 535.328857 1177.046997 L 519.302124 1139.919556 L 535.328857 1066.550537 L 554.657776 970.792053 L 570.362488 894.68457 L 584.536926 800.134277 L 592.993347 768.724976 L 592.429626 766.630859 L 585.503479 767.516968 L 514.22821 865.369263 L 405.825531 1011.865906 L 320.053711 1103.677979 L 299.516815 1111.812256 L 263.919525 1093.369263 L 267.221497 1060.429688 L 287.114136 1031.114136 L 405.825531 880.107361 L 477.422913 786.52356 L 523.651062 732.483276 L 523.328918 724.671265 L 520.590698 724.671265 L 205.288605 929.395935 L 149.154434 936.644409 L 124.993355 914.01355 L 127.973183 876.885986 L 139.409409 864.80542 L 234.201385 799.570435 L 233.879227 799.8927 Z"
      />
    </svg>
  );
}

// Zahnrad für den Link zu den Einstellungen; der Slot „NavIcon“ darf es als `settings` ersetzen
export function CogIcon() {
  return (
    <Icon name="settings">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h.1a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z" />
    </Icon>
  );
}

// Symbole der Einstellungs-Navigation, nach Namen
const SETTINGS_ICONS: Record<string, string> = {
  folder: 'M3 6a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z',
  group: 'M9 11a4 4 0 1 0 0-8 4 4 0 0 0 0 8zM2 21v-1a6 6 0 0 1 6-6h2a6 6 0 0 1 6 6v1M16 3.1a4 4 0 0 1 0 7.8M22 21v-1a6 6 0 0 0-4-5.7',
  key: 'M14 10a4 4 0 1 0-3.9 4H11v2h2v2h3v-3.2l-2.4-2.4A4 4 0 0 0 14 10zM8 9h.01',
  palette: 'M12 21a9 9 0 1 1 9-9c0 2.2-1.8 3-3.5 3H16a2 2 0 0 0-1.5 3.3A1.7 1.7 0 0 1 12 21zM7.5 11h.01M10 7h.01M14.5 7h.01',
  notifications: 'M6 16V11a6 6 0 1 1 12 0v5l2 2H4zM10 21h4',
  donut_large: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 16a4 4 0 1 0 0-8 4 4 0 0 0 0 8z',
  tune: 'M4 6h10M18 6h2M4 12h4M12 12h8M4 18h12M20 18h0M14 4v4M8 10v4M16 16v4',
  storage: 'M4 6c0-1.7 3.6-3 8-3s8 1.3 8 3-3.6 3-8 3-8-1.3-8-3zM4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3',
  send: 'M4 12l16-8-6 16-3-7zM11 13l9-9',
  power_settings_new: 'M12 3v8M7.1 6.3a7 7 0 1 0 9.8 0',
  schedule: 'M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18zM12 7v5l3 2',
  merge: 'M7 4v4a5 5 0 0 0 5 5h0a5 5 0 0 1 5 5v2M17 4v4a5 5 0 0 1-5 5M4 7l3-3 3 3M14 7l3-3 3 3',
  account_tree: 'M3 4h6v5H3zM15 15h6v5h-6zM15 4h6v5h-6zM6 9v8h9M9 6.5h6',
  travel_explore: 'M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM21 21l-4.3-4.3',
  terminal: 'M4 5h16v14H4zM8 10l3 2-3 2M13 14h3',
  insights: 'M4 19l5-6 4 3 7-9M20 7v4M20 7h-4',
  event_note:'M4 6h16v15H4zM4 10h16M8 3v4M16 3v4M8 14h8M8 17h5',
  description: 'M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8zM14 3v5h5M9 13h6M9 17h6',
  chat: 'M4 5h16v11H9l-5 4zM8 9h8M8 12h5',
  mic: 'M12 3a3 3 0 0 0-3 3v5a3 3 0 0 0 6 0V6a3 3 0 0 0-3-3zM5 11a7 7 0 0 0 14 0M12 18v3M9 21h6',
};
export const SettingsIcon = ({ name }: { name: string }) => <Icon><path d={SETTINGS_ICONS[name] ?? ''} /></Icon>;

// Animiertes Merge-Symbol (nach lucide-animated „git-merge“, ohne motion): Kreise und Bogen zeichnen sich gestaffelt
// und in Schleife, solange der Dispatcher das Ticket mergt; Animation in dbboard.css
export function GitMergeIcon() {
  return (
    <svg className="git-merge-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="18" cy="18" r="3" pathLength={1} />
      <path d="M6 21V9a9 9 0 0 0 9 9" pathLength={1} />
      <circle cx="6" cy="6" r="3" pathLength={1} />
    </svg>
  );
}
