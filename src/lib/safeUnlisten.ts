import type { UnlistenFn } from "@tauri-apps/api/event";

/**
 * Call a Tauri unlisten function without letting a teardown race become an
 * error report.
 *
 * Two facts about Tauri 2.11 combine badly here.
 *
 * First, the unlisten script the event plugin injects reads the listener
 * record without checking it exists (`tauri-2.11.1/src/event/mod.rs:212`):
 *
 *     const listeners = (window['...'] || {})[event]
 *     if (listeners) {
 *       window.__TAURI_INTERNALS__.unregisterCallback(listeners[eventId].handlerId)
 *     }
 *
 * It guards the per-event object but not the entry. Twenty lines below, the
 * emit path guards both (`const listener = listeners[id]; if (listener)`), so
 * the asymmetry is an oversight rather than a design. The entry is missing
 * whenever the webview's `window` is recreated while the Rust side still holds
 * listener ids -- a Vite HMR reload under `tauri dev`, or the relaunch after a
 * panic -- and `.handlerId` then throws.
 *
 * Second, `listen()` resolves to `async () => _unlisten(event, eventId)` while
 * its declared type is `UnlistenFn = () => void`. The type hides a promise, so
 * no caller attaches a catch and the throw always surfaces as an unhandled
 * rejection. With error reporting enabled that manufactures a report about our
 * own teardown, in whatever phase the app happened to be showing.
 *
 * Swallowing it costs one callback registration that Rust drops on teardown
 * anyway. Reporting it costs a false failure in the tracker we read to find
 * real ones.
 */
export function safeUnlisten(fn: UnlistenFn | undefined | null): void {
  if (!fn) return;
  try {
    // Coerce through Promise.resolve: the declared return is void, the runtime
    // return is a promise, and a synchronous throw is still possible.
    void Promise.resolve((fn as () => unknown)()).catch(() => {});
  } catch {
    // Teardown race, as above. Nothing to do and nothing worth reporting.
  }
}
