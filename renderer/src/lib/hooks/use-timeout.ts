import { useEffect, useRef } from "react";

/**
 * A declarative timer hook that automatically clears itself on unmount or dependency change.
 */
export function useTimeout(callback: () => void, delayMs: number | null) {
  const savedCallback = useRef(callback);

  useEffect(() => {
    savedCallback.current = callback;
  }, [callback]);

  useEffect(() => {
    if (delayMs === null) return;
    const timer = window.setTimeout(() => savedCallback.current(), delayMs);
    return () => window.clearTimeout(timer);
  }, [delayMs]);
}
