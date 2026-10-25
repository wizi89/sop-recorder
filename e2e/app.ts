/**
 * Starts the real app and hands back its main window as a Playwright page.
 *
 * The window is a WebView2, which opens a DevTools port when asked through an
 * environment variable, and Playwright attaches to that like to any Chromium.
 * Windows only: WKWebView on macOS has no such port.
 *
 * The binary is the debug build from `npm run e2e:build`, which matters twice:
 * a debug build keeps its credentials under its own keyring name, so a run
 * cannot sign the developer's installed app out, and the e2e config gives it
 * its own identifier, so it has its own settings, logs and WebView profile.
 */
import { spawn, type ChildProcess } from "node:child_process";
import path from "node:path";
import { chromium, type Browser, type Page } from "@playwright/test";

export const BINARY = path.resolve("src-tauri/target/debug/cogniclone-recorder.exe");

/**
 * Fixed rather than free, because CI cannot use the environment variable:
 * WebView2 on the runner ignores it, so ci.yml passes the same port through
 * WebView2's registry policy instead -- which applies to every window of the
 * exe alike, as WebView2 requires of windows sharing a profile.
 */
const DEVTOOLS_PORT = 9333;

export type App = {
  page: Page;
  quit: () => Promise<void>;
  /** Resolves with the exit code once the app's process has ended. */
  exited: Promise<number | null>;
};

type Urls = { apiUrl: string; webUrl: string };

/** What every launch of the app under test runs with. */
export function appEnv(urls: Urls): NodeJS.ProcessEnv {
  return {
    ...process.env,
    COGNICLONE_API_URL: urls.apiUrl,
    COGNICLONE_WEBAPP_URL: urls.webUrl,
    COGNICLONE_UPDATER_ENABLED: "false",
    COGNICLONE_ERROR_REPORTS_ENABLED: "false",
  };
}

export async function launchApp(urls: Urls): Promise<App> {
  const port = DEVTOOLS_PORT;
  const child: ChildProcess = spawn(BINARY, [], {
    env: {
      ...appEnv(urls),
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  // Kept for the failure message: an app that dies on start says why here.
  let output = "";
  const exited = new Promise<number | null>((resolve) => child.on("exit", resolve));
  child.stdout?.on("data", (d) => (output += d));
  child.stderr?.on("data", (d) => (output += d));

  let browser: Browser | undefined;
  const quit = async () => {
    await browser?.close().catch(() => {});
    child.kill();
  };

  try {
    browser = await connect(port, child, () => output);
    return { page: await mainWindow(browser), quit, exited };
  } catch (e) {
    await quit();
    throw e;
  }
}

async function connect(port: number, child: ChildProcess, output: () => string): Promise<Browser> {
  for (let i = 0; i < 150; i++) {
    if (child.exitCode !== null) {
      throw new Error(`the app exited with ${child.exitCode} before its window opened:\n${output()}`);
    }
    try {
      return await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    } catch {
      await new Promise((r) => setTimeout(r, 200));
    }
  }
  throw new Error(`the app's WebView never opened its DevTools port (${port}):\n${output()}`);
}

/** The app has more than one window; the recording bar is another. */
async function mainWindow(browser: Browser): Promise<Page> {
  for (let i = 0; i < 150; i++) {
    for (const page of browser.contexts().flatMap((c) => c.pages())) {
      const label = await page
        .evaluate(() => (window as any).__TAURI_INTERNALS__?.metadata?.currentWindow?.label)
        .catch(() => undefined);
      if (label === "main") return page;
    }
    await new Promise((r) => setTimeout(r, 200));
  }
  throw new Error("the app's main window never appeared");
}
