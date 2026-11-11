// Alles, was die Rahmen anzeigen, an einer Stelle: Board-Stand, Spielwerte, Banner, Anruf, Einblendungen, Cheats und
// Tasten. Die Rahmen lesen nur über useGame(); Logik und Daten bleiben hier.
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import { useBoards, useNav, useNavigate, useUsage, type NavItem } from '@nexus/theme-sdk';
import { useActiveStyle, useGtaSettings, type GtaSettings } from '../settings.js';
import { useBanners } from '../shared/banners.js';
import { useGameKeys, type CheatId } from '../shared/keys.js';
import { useChanged, useFlash, useIncrease } from '../shared/transient.js';
import { useClock } from '../shared/useClock.js';
import { deriveGame, type GameState } from './derive.js';
import { boardEvents, quotaEvent, type GameEvent } from './events.js';
import { useBoardFeed } from './feed.js';
import type { BoardView } from './types.js';

export const CHEAT_MONEY = 250_000;
export const MAX_WANTED = 6;

const PHONE_MS = 15_000;
const ZONE_MS = 2500;
const STATION_MS = 3000;
const CHEAT_MS = 3000;
/** So lange meldet `declined` einen abgelehnten Anruf (ms) */
export const DECLINED_MS = 4000;

export interface Game extends GameState {
  pid: string;
  settings: GtaSettings & { loading: boolean };
  style: 'v' | 'sa';
  boardName: string | null; // Name des aktuellen Boards
  nav: NavItem[];
  active: NavItem | undefined;
  now: Date;
  wanted: number; // angezeigte Sterne: blocked, von Cheats überschrieben, bis sich blocked ändert
  moneyShown: number; // money + Cheat-Bonus (bis zum Neuladen)
  banner: GameEvent | null;
  dismissBanner(): void;
  phone: { count: number } | null; // neue Fragen; nur wenn settings.banners
  declined: boolean; // kurz nach dem Ablehnen eines Anrufs
  dismissPhone(): void; // ablehnen
  answerPhone(): void; // zur Seite „Fragen“ und schließen
  zone: string | null; // Name der Seite nach einem Seitenwechsel
  station: string | null; // Name des Boards nach einem Board-Wechsel
  cheat: CheatId | null; // zuletzt ausgelöster Cheat
  statsOpen: boolean;
  setStatsOpen(open: boolean): void;
}

export const GameContext = createContext<Game | null>(null);

/** Spiel-Zustand des Rahmens; nur innerhalb des GameProvider */
export function useGame(): Game {
  const game = useContext(GameContext);
  if (!game) throw new Error('useGame braucht den GameProvider des GTA-Themes – die Komponente läuft außerhalb des Rahmens.');
  return game;
}

// Banner aus Änderungen des Board-Stands und des 5-Stunden-Kontingents
function useGameEvents(view: BoardView | null, percent: number | null, push: (events: GameEvent[]) => void): void {
  const prevView = useRef<BoardView | null>(null);
  useEffect(() => {
    const prev = prevView.current;
    prevView.current = view;
    if (view) push(boardEvents(prev, view));
  }, [view, push]);

  const prevPercent = useRef<number | null>(null);
  useEffect(() => {
    const prev = prevPercent.current;
    prevPercent.current = percent;
    const event = quotaEvent(prev, percent);
    if (event) push([event]);
  }, [percent, push]);
}

// Cheat-Bonus und überschriebene Sterne; die Sterne gelten nur, bis sich die Zahl blockierter Tickets ändert
function useCheats(blocked: number) {
  const [bonus, setBonus] = useState(0);
  const [override, setOverride] = useState<{ wanted: number; blocked: number } | null>(null);
  const [cheat, setCheat] = useState<{ id: CheatId } | null>(null);

  if (override && override.blocked !== blocked) setOverride(null);

  useEffect(() => {
    if (!cheat) return;
    const timer = setTimeout(() => setCheat(null), CHEAT_MS);
    return () => clearTimeout(timer);
  }, [cheat]);

  const onCheat = useCallback(
    (id: CheatId) => {
      if (id === 'money') setBonus((b) => b + CHEAT_MONEY);
      if (id === 'clear') setOverride({ wanted: 0, blocked });
      if (id === 'wanted') setOverride({ wanted: MAX_WANTED, blocked });
      setCheat({ id });
    },
    [blocked],
  );

  const wanted = override && override.blocked === blocked ? override.wanted : Math.min(blocked, MAX_WANTED);
  return { bonus, wanted, cheat: cheat?.id ?? null, onCheat };
}

// Nächster bzw. vorheriger Eintrag der Navigation, am Rand umlaufend
function neighbour(nav: NavItem[], step: 1 | -1): NavItem | undefined {
  if (nav.length === 0) return undefined;
  const at = nav.findIndex((item) => item.active);
  const from = at < 0 ? (step === 1 ? -1 : 0) : at;
  return nav[(from + step + nav.length) % nav.length];
}

export function GameProvider({ pid, children }: { pid: string; children: ReactNode }) {
  const boards = useBoards();
  const usage = useUsage();
  const nav = useNav();
  const navigate = useNavigate();
  const settings = useGtaSettings();
  const style = useActiveStyle();
  const now = useClock();

  const boardId = boards.current?.board ?? null;
  const boardName = boardId === null ? null : (boards.boards.find((b) => b.id === boardId)?.name ?? null);
  const active = nav.find((item) => item.active);
  const questionsHref = nav.find((item) => item.id === 'questions')?.href ?? null;

  const feed = useBoardFeed(pid, boardId);
  const state = useMemo(() => deriveGame(feed.view, usage, nav), [feed.view, usage, nav]);

  const banners = useBanners(settings.banners);
  const percent = usage.available ? (usage.fiveHour?.percent ?? null) : null;
  useGameEvents(feed.view, percent, banners.push);

  const call = useIncrease(state.questions, PHONE_MS, settings.banners);
  const phoneCount = call.value;
  const declined = useFlash(DECLINED_MS);
  const { clear: hangUp } = call;
  const { fire: decline } = declined;
  const dismissPhone = useCallback(() => {
    if (phoneCount !== null) decline();
    hangUp();
  }, [phoneCount, decline, hangUp]);
  const answerPhone = useCallback(() => {
    if (questionsHref) navigate(questionsHref);
    hangUp();
  }, [questionsHref, navigate, hangUp]);

  const zone = useChanged(active?.label, ZONE_MS);
  const station = useChanged(boardName, STATION_MS);
  const cheats = useCheats(state.blocked);
  const [statsOpen, setStatsOpen] = useState(false);

  const go = (step: 1 | -1) => () => {
    const target = neighbour(nav, step);
    if (target) navigate(target.href);
  };
  const keys: Record<string, () => void> =
    style === 'v' ? { q: go(-1), e: go(1) } : phoneCount !== null ? { y: answerPhone, n: dismissPhone } : {};
  useGameKeys({ cheats: settings.cheats, onCheat: cheats.onCheat, keys });

  const game = useMemo<Game>(
    () => ({
      ...state,
      pid,
      settings,
      style,
      boardName,
      nav,
      active,
      now,
      wanted: cheats.wanted,
      moneyShown: state.money + cheats.bonus,
      banner: banners.current,
      dismissBanner: banners.dismiss,
      phone: phoneCount === null ? null : { count: phoneCount },
      declined: declined.on,
      dismissPhone,
      answerPhone,
      zone,
      station,
      cheat: cheats.cheat,
      statsOpen,
      setStatsOpen,
    }),
    [state, pid, settings, style, boardName, nav, active, now, cheats.wanted, cheats.bonus, cheats.cheat, banners.current, banners.dismiss, phoneCount, declined.on, dismissPhone, answerPhone, zone, station, statsOpen],
  );

  return <GameContext.Provider value={game}>{children}</GameContext.Provider>;
}
