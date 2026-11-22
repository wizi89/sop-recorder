// Green fixture: the shapes this repo uses; none may be flagged.
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { safeUnlisten } from "../../src/lib/safeUnlisten";

export function promiseThen() {
  const unlisten = listen("x", () => {});
  return () => {
    void unlisten.then(safeUnlisten);
  };
}

export function cancelledGuard() {
  let cancelled = false;
  let unlisten: UnlistenFn | undefined;
  void listen("y", () => {}).then((fn) => {
    if (cancelled) safeUnlisten(fn);
    else unlisten = fn;
  });
  return () => {
    cancelled = true;
    safeUnlisten(unlisten);
  };
}

export function many() {
  const all = Promise.all([listen("a", () => {}), listen("b", () => {})]);
  return () => void all.then((fns) => fns.forEach(safeUnlisten));
}

export function wrapperNamedOff() {
  let cancelled = false;
  let stop: UnlistenFn | null = null;
  const off = () => {
    safeUnlisten(stop);
    stop = null;
  };
  void listen("created", () => {}).then((fn) => {
    stop = fn;
    if (cancelled) off();
  });
  return () => {
    cancelled = true;
    off();
  };
}

export function methodsNamedStop(recorder: { stop: () => Promise<void> }) {
  void recorder.stop();
}

export function unrelatedCallbacks(onDone: () => void) {
  void fetch("/x").then((r) => r.json());
  onDone();
}

export function refOfSomethingElse(timerRef: { current: (() => void) | null }) {
  timerRef.current?.();
}

export async function awaitedHandlerResult(onGrant: () => Promise<() => void>) {
  const done = await onGrant();
  done();
}

export function loopSafely() {
  const all: UnlistenFn[] = [];
  return () => {
    for (const fn of all) safeUnlisten(fn);
  };
}
