import { describe, it, expect, vi } from "vitest";

import { safeUnlisten } from "../lib/safeUnlisten";

describe("safeUnlisten", () => {
  it("calls the function it is given", () => {
    const fn = vi.fn();
    safeUnlisten(fn);
    expect(fn).toHaveBeenCalledOnce();
  });

  it("does nothing when there is nothing to unlisten", () => {
    expect(() => safeUnlisten(undefined)).not.toThrow();
    expect(() => safeUnlisten(null)).not.toThrow();
  });

  it("swallows a synchronous throw", () => {
    const fn = () => {
      throw new TypeError(
        "undefined is not an object (evaluating 'listeners[eventId].handlerId')",
      );
    };
    expect(() => safeUnlisten(fn)).not.toThrow();
  });

  it("swallows the rejected promise Tauri actually returns", async () => {
    // listen() resolves to `async () => _unlisten(...)`, so the failure is a
    // rejection, not a throw -- the reason a synchronous try/catch was not
    // enough. An unhandled rejection here would be reported as a ui_error.
    const unhandled = vi.fn();
    process.on("unhandledRejection", unhandled);

    const fn = (() =>
      Promise.reject(
        new TypeError(
          "undefined is not an object (evaluating 'listeners[eventId].handlerId')",
        ),
      )) as unknown as () => void;

    expect(() => safeUnlisten(fn)).not.toThrow();
    await new Promise((resolve) => setTimeout(resolve, 10));

    process.off("unhandledRejection", unhandled);
    expect(unhandled).not.toHaveBeenCalled();
  });

  it("documents the runtime shape the declared type hides", async () => {
    // The reason the wrapper exists: UnlistenFn is declared `() => void`, but
    // listen() resolves to `async () => _unlisten(...)`. The value returned is
    // a promise that can reject, so a caller trusting the type attaches no
    // catch. If Tauri ever changes the declaration to return a promise, this
    // is the test that should prompt revisiting the wrapper.
    const fn = (() =>
      Promise.reject(new TypeError("boom"))) as unknown as () => void;
    const returned = fn() as unknown;
    expect(returned).toBeInstanceOf(Promise);
    await expect(returned as Promise<void>).rejects.toThrow("boom");
  });
});

// The guard that every call site goes through safeUnlisten is the Semgrep rule
// .semgrep/unlisten.yml (python scripts/check_semgrep.py), which reads the AST
// instead of matching lines.
