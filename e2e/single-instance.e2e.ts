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
    // The UI shows its own window once on mount (App.tsx, leaving compact
    // mode), at the end of a chain of IPC calls that can land after the first
    // button has rendered. No UI state marks its end, so hide until the window
    // stays hidden for a second: a late show() would otherwise pass the final
    // check below without the second launch doing anything.
    await expect(app.page.getByRole("button").first()).toBeVisible();
    await expect(async () => {
      await invoke(app.page, "plugin:window|hide", { label: "main" });
      await app.page.waitForTimeout(1_000);
      expect(await mainVisible(app.page), "the window came back by itself").toBe(false);
    }).toPass({ timeout: 10_000 });

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

test("closing the main window quits, and the next launch opens normally", async () => {
  const stubs = await startStubs();
  try {
    const first = await launchApp(stubs);
    await expect(first.page.getByRole("button").first()).toBeVisible();
    // The page goes away with the window, so the call never answers.
    await invoke(first.page, "plugin:window|close", { label: "main" }).catch(() => {});
    const code = await Promise.race([
      first.exited,
      new Promise<string>((r) => setTimeout(() => r("still running"), 10_000)),
    ]);
    if (code === "still running") await first.quit();
    expect(code, "the app kept running with no window").toBe(0);

    // Had it kept running, this launch would hand over to it and exit.
    const second = await launchApp(stubs);
    try {
      await expect(second.page.getByRole("button").first()).toBeVisible();
    } finally {
      await second.quit();
    }
  } finally {
    await stubs.close();
  }
});
