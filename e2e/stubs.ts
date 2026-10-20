/**
 * A fake API and a fake web app, just enough of each for the sign-in.
 *
 * The web app's consent screen is played two ways. The app opens it in the
 * machine's default browser, which only tells us the authorize URL -- it gets a
 * page saying the test has it from here. The test then opens the same URL in
 * its own Chromium with `e2e-confirm`, and that request is answered like a
 * confirmed consent: a 302 to the app's loopback. So the redirect a real
 * browser follows is followed by a real browser, which is the part that
 * regressed once (a reset connection the browser then retried onto a closed
 * port).
 */
import { createHash } from "node:crypto";
import { createServer, type IncomingMessage, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";

export const CODE = "e2e-code";
export const TOKENS = {
  access_token: "e2e-access",
  refresh_token: "e2e-refresh",
  email: "e2e@example.test",
};

export type Stubs = Awaited<ReturnType<typeof startStubs>>;

export async function startStubs() {
  const state = {
    /** `old` answers the exchange with a 404, like an API from before it. */
    apiMode: "current" as "current" | "old",
    /** The URL the app sent the browser to, once it has. */
    authorizeUrl: null as string | null,
    challenge: null as string | null,
    /** Every exchange that carried a code, accepted or not. */
    exchanges: [] as { code: string; accepted: boolean }[],
  };

  const api = await listen(async (req, res) => {
    if (req.method === "POST" && req.url === "/auth/desktop/exchange") {
      if (state.apiMode === "old") return send(res, 404, "Not Found", "text/plain");
      const body = await json(req);
      // The app's probe posts an empty body; a server with the endpoint
      // refuses it with JSON, which is what tells the app the endpoint exists.
      if (!body.code) return send(res, 400, { error: "invalid_code" });
      const accepted =
        body.code === CODE &&
        state.challenge !== null &&
        s256(String(body.code_verifier)) === state.challenge;
      state.exchanges.push({ code: String(body.code), accepted });
      return accepted ? send(res, 200, TOKENS) : send(res, 400, { error: "invalid_code" });
    }
    if (req.method === "POST" && req.url === "/auth/refresh") {
      const body = await json(req);
      // Only a session this stub issued. One left in the keyring by an earlier
      // test is refused, and the app clears it -- so every test starts signed
      // out.
      return state.exchanges.some((e) => e.accepted) && body.refresh_token === TOKENS.refresh_token
        ? send(res, 200, TOKENS)
        : send(res, 401, { error: "invalid_grant" });
    }
    send(res, 404, { error: "not found" });
  });

  const web = await listen((req, res) => {
    const url = new URL(req.url ?? "/", "http://stub");
    if (url.pathname === "/desktop/authorize") {
      if (!url.searchParams.has("e2e-confirm")) {
        state.authorizeUrl = url.href.replace("http://stub", web.origin);
        state.challenge = url.searchParams.get("code_challenge");
        return send(res, 200, "<p>The end-to-end test has taken this sign-in over.</p>", "text/html");
      }
      const back = new URL(url.searchParams.get("redirect_uri")!);
      back.searchParams.set("code", CODE);
      back.searchParams.set("state", url.searchParams.get("e2e-state") ?? url.searchParams.get("state")!);
      res.writeHead(302, { Location: back.href }).end();
      return;
    }
    // /desktop/signed-in and /desktop/sign-in-failed, where the app sends the
    // browser afterwards.
    send(res, 200, `<p>${url.pathname}</p>`, "text/html");
  });

  return {
    state,
    apiUrl: api.origin,
    webUrl: web.origin,
    /** The authorize URL, once the app has opened the browser at it. */
    async authorizeUrl(): Promise<string> {
      for (let i = 0; i < 100 && !state.authorizeUrl; i++) await sleep(100);
      if (!state.authorizeUrl) throw new Error("the app never opened the browser");
      return state.authorizeUrl;
    },
    async close() {
      await Promise.all([api.close(), web.close()]);
    },
  };
}

function s256(verifier: string) {
  return createHash("sha256").update(verifier).digest("base64url");
}

async function json(req: IncomingMessage): Promise<Record<string, unknown>> {
  let text = "";
  for await (const chunk of req) text += chunk;
  try {
    return JSON.parse(text);
  } catch {
    return {};
  }
}

function send(res: ServerResponse, status: number, body: unknown, type = "application/json") {
  res.writeHead(status, { "Content-Type": type });
  res.end(typeof body === "string" ? body : JSON.stringify(body));
}

async function listen(handler: (req: IncomingMessage, res: ServerResponse) => void) {
  const server = createServer(handler);
  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address() as AddressInfo;
  return {
    origin: `http://127.0.0.1:${port}`,
    close: () =>
      new Promise<void>((resolve) => {
        server.closeAllConnections();
        server.close(() => resolve());
      }),
  };
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
