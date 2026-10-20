/**
 * Browser sign-in, run against the real app.
 *
 * Every journey here stands for a defect that unit tests passed over and only
 * a windowed run found: a wait screen that never rendered, a redirect the
 * browser could not keep, an old server that sent users into a five-minute
 * wait. Each failure message below says which.
 */
import { test as base, expect, chromium, type Page } from "@playwright/test";
import { launchApp } from "./app";
import { startStubs, TOKENS, type Stubs } from "./stubs";

const test = base.extend<{ stubs: Stubs; app: Page }>({
  stubs: async ({}, use) => {
    const stubs = await startStubs();
    await use(stubs);
    await stubs.close();
  },
  app: async ({ stubs }, use) => {
    const app = await launchApp(stubs);
    await use(app.page);
    await app.quit();
  },
});

const signInWithBrowser = (app: Page) =>
  app.getByRole("button", { name: "Mit Browser anmelden" }).click();

/** What the user does in the browser: confirm, and follow the redirect home. */
async function confirmInBrowser(stubs: Stubs, override: { state?: string } = {}) {
  const url = new URL(await stubs.authorizeUrl());
  url.searchParams.set("e2e-confirm", "1");
  if (override.state) url.searchParams.set("e2e-state", override.state);
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    // Throws on a network error -- ERR_CONNECTION_RESET is how the drain
    // regression shows, ERR_CONNECTION_REFUSED its retry. Only sometimes,
    // though: whether Chromium has read the 302 before the reset lands is a
    // race inside the browser (measured: about 1 run in 10 with the drain
    // removed, and padding the request made it rarer, not likelier). The
    // guard for that regression is the Rust test in loopback.rs, which looks
    // at the close itself; this is the journey around it.
    await page.goto(url.href);
    return new URL(page.url());
  } finally {
    await browser.close();
  }
}

test("signs in through the browser, and the browser keeps the answer", async ({ app, stubs }) => {
  await signInWithBrowser(app);
  const landed = await confirmInBrowser(stubs);

  expect(landed.pathname, "the browser should end on the success page").toBe("/desktop/signed-in");
  await expect(app.getByRole("button", { name: "Abmelden" })).toBeVisible();
  expect(stubs.state.exchanges).toEqual([{ code: "e2e-code", accepted: true }]);
});

test("the wait has its own screen, and cancel ends it", async ({ app, stubs }) => {
  await signInWithBrowser(app);
  await stubs.authorizeUrl();

  // Regression: the loading gate once swallowed this whole screen, leaving
  // "..." for five minutes with no way out.
  await expect(app.getByText("Warte auf die Anmeldung im Browser")).toBeVisible();
  await app.getByRole("button", { name: "Abbrechen" }).click();

  await expect(app.getByText("Die Anmeldung wurde abgebrochen.")).toBeVisible();
  await expect(app.getByRole("button", { name: "Mit Browser anmelden" })).toBeVisible();
});

test("a forged callback is refused before anything reaches the server", async ({ app, stubs }) => {
  await signInWithBrowser(app);
  const landed = await confirmInBrowser(stubs, { state: "forged" });

  expect(landed.pathname).toBe("/desktop/sign-in-failed");
  expect(landed.searchParams.get("reason")).toBe("verification");
  await expect(app.getByText("konnte nicht überprüft werden")).toBeVisible();
  expect(stubs.state.exchanges, "the forged code must never be offered upstream").toEqual([]);
});

test("a server without the exchange says so, and offers the password form", async ({ app, stubs }) => {
  stubs.state.apiMode = "old";
  await signInWithBrowser(app);

  await expect(app.getByText("unterstützt die Anmeldung über den Browser noch nicht")).toBeVisible();
  await expect(app.getByLabel("Passwort")).toBeVisible();
  expect(stubs.state.authorizeUrl, "no browser should open for a server that cannot finish").toBeNull();
});

test("a restart goes straight in with the stored session", async ({ stubs }) => {
  // Not a fixture app: this one needs two lifetimes.
  const first = await launchApp(stubs);
  await signInWithBrowser(first.page);
  await confirmInBrowser(stubs);
  await expect(first.page.getByRole("button", { name: "Abmelden" })).toBeVisible();
  await first.quit();

  const second = await launchApp(stubs);
  try {
    await expect(second.page.getByRole("button", { name: "Abmelden" })).toBeVisible();
    await expect(second.page.getByText(TOKENS.email)).toBeVisible();
  } finally {
    await second.quit();
  }
});
