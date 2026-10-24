/**
 * Launching the app while it already runs, against the real binary.
 *
 * A second copy would bring a second recorder that can record the same
 * session in parallel. It has to hand over to the running one and exit, and
 * the running one has to come to the front -- including from hidden, which is
 * where a user who double-clicks the desktop icon "because nothing happened"
 * usually left it.
 */
import { spawn } from "node:child_process";
import { test, expect, type Page } from "@playwright/test";
import { appEnv, BINARY, launchApp } from "./app";
import { startStubs } from "./stubs";

const invoke = (page: Page, cmd: string, args: Record<string, unknown> = {}) =>
  page.evaluate(([c, a]) => (window as any).__TAURI_INTERNALS__.invoke(c, a), [cmd, args] as const);

const mainVisible = (page: Page) => invoke(page, "plugin:window|is_visible", { label: "main" });

test("a second launch exits and brings the running window back", async () => {
  const stubs = await startStubs();
  const app = await launchApp(stubs);
  try {
    await invoke(app.page, "plugin:window|hide", { label: "main" });
    await expect.poll(() => mainVisible(app.page)).toBe(false);

    const second = spawn(BINARY, [], { env: appEnv(stubs), stdio: "ignore" });
    const exitCode = await new Promise<number | null>((resolve) => {
      const timer = setTimeout(() => resolve(-1), 15_000);
      second.on("exit", (code) => {
        clearTimeout(timer);
        resolve(code);
      });
    });
    if (exitCode === -1) second.kill();
    expect(exitCode, "the second launch kept running alongside the first").toBe(0);

    await expect
      .poll(() => mainVisible(app.page), { message: "the running window stayed hidden" })
      .toBe(true);
  } finally {
    await app.quit();
    await stubs.close();
  }
});
