import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import { useSessions } from '../hooks/useSessions.js';
import { useLiveHandler } from '../live/useLive.js';
import type { NoticeKind } from '../platform/Platform.js';
import { usePlatform } from '../platform/PlatformContext.js';
import { describeReady, findReadySessions, readyTarget, rememberStates, type KnownStates } from '../sessions/notify.js';

// Zeigt eine Benachrichtigung, wenn eine Session bereit ist oder auf Eingabe wartet (wenn eingeschaltet und erlaubt).
// Im Modus „windows“ schickt der Server seine Meldungen an offene Tabs; sie erscheinen dann hier statt als Windows-Toast.
// Die Meldung selbst zeigt die Plattform: im Browser der Tab, in der Desktop-App das System – dort auch bei verstecktem Fenster.
// Den Modus „browser“ gibt es nur im Browser: die App kennt nur an (Meldungen des Servers) oder aus.
export function SessionNotifier({ pid }: { pid: string }) {
  const api = useApi();
  const platform = usePlatform();
  const navigate = useNavigate();
  const { state } = useSessions(pid);
  const known = useRef<KnownStates>(new Map());

  const navigateRef = useRef(navigate);
  navigateRef.current = navigate;
  const platformRef = useRef(platform);
  platformRef.current = platform;
  // Klick navigiert in diesem Fenster, statt einen neuen Tab zu öffnen (die Desktop-App öffnet `url` selbst)
  const show = useRef((title: string, body: string, tag: string, url: string, kind?: NoticeKind) => {
    void platformRef.current.notify({ title, body, tag, url, kind, onClick: () => navigateRef.current(url) }).catch(() => undefined);
  });

  useLiveHandler({
    onNotice: (notice) => show.current(notice.title, notice.body, notice.tag, notice.url, notice.kind),
  });

  useEffect(() => {
    if (state.status !== 'ready') return;
    const ready = findReadySessions(known.current, state.data);
    known.current = rememberStates(state.data);
    if (ready.length === 0 || document.hasFocus()) return;
    if (platform.kind === 'tauri' || !platform.notificationsSupported()) return;
    // Einstellung und Erlaubnis erst jetzt lesen, damit eine Änderung sofort gilt
    Promise.all([api.getUserSettings(), platform.notificationPermission()])
      .then(([settings, permission]) => {
        if (settings.notifications !== 'browser' || permission !== 'granted') return;
        for (const session of ready) {
          const { title, body } = describeReady(session);
          show.current(title, body, session.id, readyTarget(pid, session), session.displayState === 'waiting' ? 'wartet' : 'bereit');
        }
      })
      .catch(() => undefined);
  }, [api, platform, state, pid]);

  return null;
}
