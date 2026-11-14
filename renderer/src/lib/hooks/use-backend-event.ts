import { useEffect, useRef } from "react";
import { onBackendEvent, type BackendEvent, type BackendEventMap } from "@/lib/ipc/events";

/**
 * Subscribes to a backend event for the lifetime of the component.
 *
 * The handler is kept in a ref so a new inline function does not tear down
 * and re-open the listener on every render.
 */
export function useBackendEvent<K extends BackendEvent>(
  event: K,
  handler: (payload: BackendEventMap[K]) => void,
) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(
    () => onBackendEvent(event, (payload) => handlerRef.current(payload)),
    [event],
  );
}
