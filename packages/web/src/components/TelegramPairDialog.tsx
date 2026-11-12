import { useCallback, useEffect, useId, useState } from 'react';
import { useApi } from '../api/ApiContext.js';
import type { TelegramConfig, TelegramPairingDto } from '../api/telegramTypes.js';
import { usePlatform } from '../platform/PlatformContext.js';
import { DbModal } from './db/DbModal.js';
import { Button } from './ui/Button.js';
import { QrCode } from './QrCode.js';
import { Notice } from './settings/Notice.js';

// So oft fragt der Dialog nach, ob inzwischen ein Chat gekoppelt ist
export const PAIR_POLL_MS = 2000;

type Load = { kind: 'loading' } | { kind: 'ready'; pairing: TelegramPairingDto } | { kind: 'error'; message: string };

// Dialog „Chat koppeln“: holt beim Öffnen einen Einmal-Code und zeigt ihn mit QR-Code. Der Dialog fragt
// regelmäßig die gekoppelten Chats ab und schließt sich von selbst, sobald ein neuer dazugekommen ist.
export function TelegramPairDialog({ knownChats, onPaired, onClose }: {
  knownChats: number[];
  onPaired: (config: TelegramConfig, chatId: number) => void;
  onClose: () => void;
}) {
  const api = useApi();
  const platform = usePlatform();
  const titleId = useId();
  const [load, setLoad] = useState<Load>({ kind: 'loading' });
  const [linkError, setLinkError] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());

  const generate = useCallback(async () => {
    setLoad({ kind: 'loading' });
    setLinkError(null);
    try {
      setLoad({ kind: 'ready', pairing: await api.pairTelegram() });
      setNow(Date.now());
    } catch (e) {
      setLoad({ kind: 'error', message: (e as Error).message });
    }
  }, [api]);

  useEffect(() => {
    void generate();
  }, [generate]);

  const expired = load.kind === 'ready' && new Date(load.pairing.expiresAt).getTime() <= now;

  // Solange ein gültiger Code steht: nachsehen, ob ein Chat dazugekommen ist, und die Uhr für den Ablauf weiterstellen
  useEffect(() => {
    if (load.kind !== 'ready' || expired) return;
    let alive = true;
    const timer = window.setInterval(() => {
      setNow(Date.now());
      api.getTelegramConfig().then((config) => {
        if (!alive) return;
        const fresh = config.chats.find((id) => !knownChats.includes(id));
        if (fresh !== undefined) onPaired(config, fresh);
      }, () => undefined);
    }, PAIR_POLL_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, [api, load.kind, expired, knownChats, onPaired]);

  // Öffnet den Chat mit dem Bot samt Code: in der Desktop-App im Standardbrowser bzw. in Telegram, im Browser in einem neuen Tab
  const openLink = async (link: string) => {
    setLinkError(null);
    try {
      await platform.openExternal(link);
    } catch (e) {
      setLinkError((e as Error).message);
    }
  };

  return (
    <DbModal labelledBy={titleId} onEscape={onClose} onBackdrop={onClose} className="panel db-modal tg-pair-modal">
      <header className="tg-pair-modal-head">
        <h2 id={titleId}>Chat koppeln</h2>
        <button type="button" className="icon-btn" aria-label="Schließen" onClick={onClose}>
          <span className="material-icons" aria-hidden="true">close</span>
        </button>
      </header>

      {load.kind === 'loading' && <p className="tg-pair-modal-hint" role="status">Code wird erzeugt …</p>}
      {load.kind === 'error' && (
        <>
          <Notice tone="error">{load.message}</Notice>
          <div><Button onClick={() => void generate()}>Erneut versuchen</Button></div>
        </>
      )}
      {load.kind === 'ready' && (
        <div className="tg-pair-modal-body" aria-label="Kopplungscode" role="group">
          {load.pairing.link && !expired && (
            <QrCode value={load.pairing.link} size={200} label="QR-Code: Chat mit dem Bot öffnen und koppeln" />
          )}
          <div className="tg-pair-modal-info">
            <p className="tg-pair-modal-hint">
              {load.pairing.link
                ? 'Mit der Handykamera scannen oder den Chat hier öffnen – der Code geht dabei gleich mit.'
                : <>Schick dem Bot <code>/start</code> und danach den Code.</>}
            </p>
            <div className="tg-pair-code">
              <span className="tg-row-hint">Code</span>
              <strong className="mono">{load.pairing.code}</strong>
              <span className="tg-row-hint">
                {expired ? 'abgelaufen' : `gültig bis ${new Date(load.pairing.expiresAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`}
              </span>
            </div>
            {expired ? (
              <div><Button variant="primary" onClick={() => void generate()}>Neuen Code erzeugen</Button></div>
            ) : (
              <>
                {load.pairing.link && (
                  <div><Button onClick={() => void openLink(load.pairing.link)}>Chat mit dem Bot öffnen</Button></div>
                )}
                <p className="tg-pair-waiting" role="status"><span className="tg-pulse" aria-hidden="true" />Wartet auf die Kopplung …</p>
              </>
            )}
            {linkError && <Notice tone="error">{linkError}</Notice>}
          </div>
        </div>
      )}

      <div className="tg-pair-modal-actions">
        <Button onClick={onClose}>Abbrechen</Button>
      </div>
    </DbModal>
  );
}
