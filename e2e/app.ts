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
import { createServer, type AddressInfo } from "node:net";
import path from "node:path";
import { chromium, type Browser, type Page } from "@playwright/test";

export const BINARY = path.resolve("src-tauri/target/debug/cogniclone-recorder.exe");

export type App = { page: Page; quit: () => Promise<void> };

export async function launchApp(urls: { apiUrl: string; webUrl: string }): Promise<App> {
  const port = await freePort();
  const child: ChildProcess = spawn(BINARY, [], {
    env: {
      ...process.env,
      COGNICLONE_API_URL: urls.apiUrl,
      COGNICLONE_WEBAPP_URL: urls.webUrl,
      COGNICLONE_UPDATER_ENABLED: "false",
      COGNICLONE_ERROR_REPORTS_ENABLED: "false",
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
    },
    stdio: "ignore",
  });

  let browser: Browser | undefined;
  const quit = async () => {
    await browser?.close().catch(() => {});
    child.kill();
  };

  try {
    browser = await connect(port);
    return { page: await mainWindow(browser), quit };
  } catch (e) {
    await quit();
    throw e;
  }
}

async function connect(port: number): Promise<Browser> {
  for (let i = 0; i < 150; i++) {
    try {
      return await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
    } catch {
      await new Promise((r) => setTimeout(r, 200));
    }
  }
  throw new Error(`the app's WebView never opened its DevTools port (${port})`);
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

async function freePort(): Promise<number> {
  const server = createServer();
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as AddressInfo;
  await new Promise((resolve) => server.close(resolve));
  return port;
}
