import { useEffect, useState } from "react";

/** re-render when listening statistics change (debounced: stats update every few seconds) */
export function useStatsVersion(): number {
  const [version, setVersion] = useState(0);
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | null = null;
    const bump = () => {
      if (timer) return;
      timer = setTimeout(() => {
        timer = null;
        setVersion((v) => v + 1);
      }, 1500);
    };
    window.addEventListener("stats:changed", bump);
    return () => {
      window.removeEventListener("stats:changed", bump);
      if (timer) clearTimeout(timer);
    };
  }, []);
  return version;
}
