import { describe, it, expect, vi } from "vitest";
import fs from "node:fs";
import path from "node:path";

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
      throw new TypeError("undefined is not an object (evaluating 'listeners[eventId].handlerId')");
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
        new TypeError("undefined is not an object (evaluating 'listeners[eventId].handlerId')"),
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
    const fn = (() => Promise.reject(new TypeError("boom"))) as unknown as () => void;
    const returned = fn() as unknown;
    expect(returned).toBeInstanceOf(Promise);
    await expect(returned as Promise<void>).rejects.toThrow("boom");
  });
});

describe("no unlisten function is invoked directly", () => {
  // A guard, not a unit test: the fix is only worth anything if every call
  // site keeps using it. Tauri's declared type is `UnlistenFn = () => void`,
  // which hides the promise, so a new raw call site looks perfectly correct
  // in review and in the type checker.
  const SRC = path.resolve(__dirname, "..");

  function sourceFiles(dir: string): string[] {
    return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        return entry.name === "test" || entry.name === "node_modules" ? [] : sourceFiles(full);
      }
      return /\.tsx?$/.test(entry.name) && entry.name !== "safeUnlisten.ts" ? [full] : [];
    });
  }

  const RAW_CALL_PATTERNS: RegExp[] = [
    /\bunlisten\w*\?\.\(\)/,             // unlisten?.()
    /\bif \(unlisten\w*\) unlisten\w*\(\)/, // if (unlisten) unlisten()
    /\(\s*(fn|f|unlisten\w*)\s*\)\s*=>\s*\1\(\)/, // (fn) => fn()
  ];

  it("every source file routes unlisten through safeUnlisten", () => {
    const offenders: string[] = [];
    for (const file of sourceFiles(SRC)) {
      const text = fs.readFileSync(file, "utf8");
      if (!/unlisten/i.test(text)) continue;
      text.split("\n").forEach((line, i) => {
        if (line.trimStart().startsWith("//") || line.trimStart().startsWith("*")) return;
        if (RAW_CALL_PATTERNS.some((re) => re.test(line))) {
          offenders.push(`${path.relative(SRC, file)}:${i + 1}  ${line.trim()}`);
        }
      });
    }
    expect(offenders, `route these through safeUnlisten:\n${offenders.join("\n")}`).toEqual([]);
  });
});
