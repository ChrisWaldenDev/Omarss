/**
 * Registers backend event listeners and only then runs the initial load, so events
 * emitted while loading (by the startup refresh, say) aren't missed. If the listeners
 * can't be registered, the load still runs. Returns a cleanup that unsubscribes, even
 * when called before registration finished; after cleanup the load no longer starts.
 */
export function listenThenLoad(
  listen: () => Promise<() => void>,
  load: () => Promise<void>,
): () => void {
  let stop: (() => void) | undefined;
  let cancelled = false;
  void listen()
    .then((unlisten) => {
      if (cancelled) unlisten();
      else stop = unlisten;
    })
    .catch((error) => console.error("could not listen for events", error))
    .then(() => (cancelled ? undefined : load()));
  return () => {
    cancelled = true;
    stop?.();
  };
}
