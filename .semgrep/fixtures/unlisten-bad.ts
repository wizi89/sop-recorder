// Red fixture: every block here must be flagged (CI asserts the count).
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

export function byName() {
  let unlisten: (() => void) | undefined;
  return () => unlisten?.(); // 1
}

export function byNameGuarded(unlistenFocus: () => void) {
  if (unlistenFocus) unlistenFocus(); // 2
}

export function thenCallback() {
  void listen("x", () => {}).then((fn) => fn()); // 3
}

export function thenCallbackOnWindow() {
  void getCurrentWindow()
    .listen("y", () => {})
    .then(function (stop) {
      stop(); // 4
    });
}

export async function awaited() {
  const off = await listen("z", () => {});
  off(); // 5
}

export function typed() {
  let off: UnlistenFn | undefined;
  return () => {
    if (off) off(); // 6
  };
}

export function genericListen() {
  let cancelled = false;
  void listen<number>("level", () => {}).then((fn) => {
    if (cancelled) {
      fn(); // 7
    }
  });
}

export function heldPromise() {
  const pending = getCurrentWindow().onFocusChanged(() => {});
  return () => {
    void pending.then((f) => f()); // 8
  };
}

export function arrayOfThem(all: Promise<UnlistenFn[]>) {
  return () => void all.then((fns) => fns.forEach((fn) => fn())); // 9
}

export function heldInStop() {
  // The 2026-09-04 bug: the resolved unlisten kept in `stop`, called with ?.
  let stop: (() => void) | null = null;
  void listen("created", () => {}).then((fn) => {
    stop = fn;
  });
  return () => {
    try {
      stop?.(); // 10
    } catch {
      // A synchronous catch cannot see the rejected promise.
    }
  };
}
