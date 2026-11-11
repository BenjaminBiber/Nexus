import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';
import { ThemeHostProvider } from '@nexus/theme-sdk';
import type { BoardsState, ColorSchemeState, SettingsState, ShellParts, ThemeHost, ThemeSettings, UsageState } from '@nexus/theme-sdk';
import { useApi } from '../api/ApiContext.js';
import type { ThemeMode, UserSettingsDto } from '../api/types.js';
import type { UsageOverviewDto } from '../api/usageTypes.js';
import { REFRESH_MS, USAGE_SETTING_EVENT } from '../components/ClaudeUsageMeter.js';
import { useBoardProject } from '../dbboard/boardProject.js';
import { currentBoard, useBoardChoices } from '../dbboard/useBoardChoices.js';
import { useNavCounts } from '../hooks/useNavCounts.js';
import { NAV_ITEMS } from './navItems.js';
import { applyTheme, currentTheme } from './theme.js';
import { useDisplayedTheme } from './useDisplayedTheme.js';
import { useThemeSettingsState } from './useThemeSettingsState.js';

const NO_USAGE: UsageState = { enabled: false, available: false, reason: null, fiveHour: null, sevenDay: null };

// Verbrauch des ersten aktiven Accounts; lädt im Takt der Seitenleiste und sofort nach Änderung der Einstellung
function useHostUsage(): UsageState {
  const api = useApi();
  const [overview, setOverview] = useState<UsageOverviewDto | null>(null);
  useEffect(() => {
    let alive = true;
    const load = () => {
      Promise.resolve(api.getUsageOverview()).then((value) => alive && setOverview(value), () => {});
    };
    load();
    const timer = setInterval(load, REFRESH_MS);
    window.addEventListener(USAGE_SETTING_EVENT, load);
    return () => {
      alive = false;
      clearInterval(timer);
      window.removeEventListener(USAGE_SETTING_EVENT, load);
    };
  }, [api]);
  return useMemo(() => {
    const usage = overview?.enabled ? overview.accounts[0]?.usage : undefined;
    if (!overview || !usage) return { ...NO_USAGE, enabled: Boolean(overview?.enabled) };
    return { enabled: true, available: usage.available, reason: usage.reason, fiveHour: usage.fiveHour, sevenDay: usage.sevenDay };
  }, [overview]);
}

// Lesbarer Ausschnitt der Nutzereinstellungen
function pick(dto: UserSettingsDto): ThemeSettings {
  return { theme: dto.theme, claudeUsage: dto.claudeUsage, compactCards: dto.compactCards, heroImages: dto.heroImages };
}

// Nutzereinstellungen; ein geändertes Farbschema wird sofort angezeigt, ein geänderter Verbrauchs-Schalter sofort neu geladen
function useHostSettings(): SettingsState {
  const api = useApi();
  const [values, setValues] = useState<ThemeSettings | null>(null);
  useEffect(() => {
    let alive = true;
    api.getUserSettings().then((dto) => alive && setValues(pick(dto)), () => {});
    return () => {
      alive = false;
    };
  }, [api]);
  const update = useCallback(
    async (patch: Partial<ThemeSettings>) => {
      const saved = await api.saveUserSettings(patch as Partial<UserSettingsDto>);
      setValues(pick(saved));
      if (patch.theme !== undefined) applyTheme(saved.theme as ThemeMode);
      if (patch.claudeUsage !== undefined) window.dispatchEvent(new Event(USAGE_SETTING_EVENT));
    },
    [api],
  );
  return useMemo(() => ({ values, loading: values === null, update }), [values, update]);
}

// Stellt den Themes über die Hooks des SDK Navigation, Boards, Verbrauch, Einstellungen, Farbschema, Routenwechsel, Suche
// und die fertigen Bausteine des Rahmens bereit; braucht BoardProjectProvider, Router und ApiProvider darüber, für die
// Zähler der Navigation NavCountsProvider
export function NexusThemeHost({ pid, parts, openSearch, children }: { pid: string; parts: ShellParts; openSearch: () => void; children: ReactNode }) {
  const routerNavigate = useNavigate();
  const { pathname } = useLocation();
  const context = useBoardProject();
  const choices = useBoardChoices(pid);
  const usage = useHostUsage();
  const settings = useHostSettings();
  const themeId = useDisplayedTheme();
  const themeSettings = useThemeSettingsState(themeId);

  const badges = useNavCounts();
  const nav = useMemo(
    () =>
      NAV_ITEMS.map(({ id, label }) => {
        const href = `/p/${pid}/${id}`;
        const badge = badges[id];
        return { id, label, href, active: pathname === href || pathname.startsWith(`${href}/`), ...(badge ? { badge } : {}) };
      }),
    [pid, pathname, badges],
  );
  const { state, choose } = choices;
  const shown = context?.shown ?? null;
  const boards = useMemo<BoardsState>(() => {
    const select = (ref: { project: string; board: number | null }) => void choose(ref).catch(() => {});
    if (state.status !== 'ready') return { current: null, projects: [], boards: [], loading: state.status === 'loading', choose: select };
    const { project, board } = currentBoard(state.data, shown);
    return {
      current: project ? { project: project.key, board: board?.id ?? null } : null,
      projects: state.data.projects.map((p) => ({ id: p.key, name: p.name })),
      boards: state.data.boards.map((b) => ({ id: b.id, project: b.projectKey, name: b.name })),
      loading: false,
      choose: select,
    };
  }, [state, shown, choose]);
  const colorScheme = useMemo<ColorSchemeState>(
    () => ({ scheme: currentTheme(), themeId, mode: document.documentElement.dataset.themeMode ?? themeId }),
    [themeId],
  );
  const navigate = useCallback((to: string) => routerNavigate(to), [routerNavigate]);
  const host = useMemo<ThemeHost>(
    () => ({ nav, boards, usage, settings, themeSettings, colorScheme, navigate, openSearch, parts }),
    [nav, boards, usage, settings, themeSettings, colorScheme, navigate, openSearch, parts],
  );
  return <ThemeHostProvider value={host}>{children}</ThemeHostProvider>;
}
