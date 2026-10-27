import { useEffect, useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { useApi } from '../api/ApiContext.js';
import { useSessions } from '../hooks/useSessions.js';
import { useLiveHandler } from '../live/useLive.js';
import { describeReady, findReadySessions, readyTarget, rememberStates, type KnownStates } from '../sessions/notify.js';

// Zeigt eine Benachrichtigung, wenn eine Session bereit ist oder auf Eingabe wartet (wenn eingeschaltet und erlaubt).
// Im Modus „windows“ schickt der Server seine Meldungen an offene Tabs; sie erscheinen dann hier statt als Windows-Toast.
export function SessionNotifier({ pid }: { pid: string }) {
  const api = useApi();
  const navigate = useNavigate();
  const { state } = useSessions(pid);
  const known = useRef<KnownStates>(new Map());

  const navigateRef = useRef(navigate);
  navigateRef.current = navigate;
  // Klick holt diesen Tab nach vorn und navigiert darin, statt einen neuen zu öffnen
  const show = useRef((title: string, options: NotificationOptions, url: string) => {
    const notification = new Notification(title, options);
    notification.onclick = (event) => {
      event.preventDefault();
      window.focus();
      navigateRef.current(url);
      notification.close();
    };
  });

  useLiveHandler({
    onNotice: (notice) => {
      if (typeof Notification === 'undefined' || Notification.permission !== 'granted') return;
      show.current(notice.title, { body: notice.body, tag: notice.tag }, notice.url);
    },
  });

  useEffect(() => {
    if (state.status !== 'ready') return;
    const ready = findReadySessions(known.current, state.data);
    known.current = rememberStates(state.data);
    if (ready.length === 0 || document.hasFocus()) return;
    if (typeof Notification === 'undefined' || Notification.permission !== 'granted') return;
    // Einstellung erst jetzt lesen, damit eine Änderung sofort gilt
    api
      .getUserSettings()
      .then((settings) => {
        if (settings.notifications !== 'browser') return;
        for (const session of ready) {
          const { title, body } = describeReady(session);
          show.current(title, { body, tag: session.id }, readyTarget(pid, session));
        }
      })
      .catch(() => undefined);
  }, [api, state, pid]);

  return null;
}
