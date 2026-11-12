import { useEffect, useState } from 'react';

// Aktuelle Zeit für Uhren in Rahmen (hh:mm), springt zur vollen Minute weiter
export function useClock(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 15_000);
    return () => window.clearInterval(id);
  }, []);
  return now;
}
