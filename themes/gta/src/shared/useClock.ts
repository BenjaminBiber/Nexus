import { useEffect, useState } from 'react';

const MINUTE = 60_000;

/** Aktuelle Zeit; aktualisiert sich zur vollen Minute */
export function useClock(): Date {
  const [now, setNow] = useState(() => new Date());

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;
    const schedule = () => {
      timer = setTimeout(() => {
        setNow(new Date());
        schedule();
      }, MINUTE - (Date.now() % MINUTE));
    };
    schedule();
    return () => clearTimeout(timer);
  }, []);

  return now;
}
