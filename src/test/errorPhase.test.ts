import { describe, it, expect } from "vitest";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  PHASES,
  WEBVIEW_PHASES,
  phaseForScreen,
  type Phase,
  type ScreenState,
} from "../lib/errorPhase";
import type { RecorderStatus } from "../hooks/useRecorder";

const base: ScreenState = {
  loggedIn: true,
  settingsOpen: false,
  permissionSetup: false,
  status: "idle",
};

describe("phaseForScreen", () => {
  const cases: Array<[string, Partial<ScreenState>, Phase]> = [
    ["signed out", { loggedIn: false }, "login"],
    ["signed out with settings open", { loggedIn: false, settingsOpen: true }, "settings"],
    ["settings window on top", { settingsOpen: true }, "settings"],
    ["settings beats recording", { settingsOpen: true, status: "recording" }, "settings"],
    ["permission setup", { permissionSetup: true }, "idle"],
    ["idle recorder", { status: "idle" }, "idle"],
    ["recording", { status: "recording" }, "recording"],
    ["review", { status: "review" }, "review"],
    ["done is the review screen", { status: "done" }, "review"],
    ["processing", { status: "processing" }, "processing"],
    ["an error is shown on the idle screen", { status: "error" }, "idle"],
    ["pii blocked", { status: "pii_blocked" }, "idle"],
    ["rate limited", { status: "rate_limited" }, "idle"],
  ];

  for (const [name, overrides, expected] of cases) {
    it(`${name} -> ${expected}`, () => {
      expect(phaseForScreen({ ...base, ...overrides })).toBe(expected);
    });
  }

  it("every status the recorder can be in maps to a phase", () => {
    // An added RecorderStatus that nobody mapped would fall out of the switch
    // and return undefined. Enumerated here rather than trusted to the type,
    // because the type is erased at runtime.
    const all: RecorderStatus[] = [
      "idle",
      "recording",
      "review",
      "processing",
      "done",
      "error",
      "pii_blocked",
      "rate_limited",
    ];
    for (const status of all) {
      expect(PHASES).toContain(phaseForScreen({ ...base, status }));
    }
  });

  it("produces every phase the webview is responsible for", () => {
    // The bug this file exists for: `login`, `review` and `settings` were
    // defined and reachable by nobody, so reports carried `idle` or `unknown`
    // whatever the user was doing. A phase no screen can produce fails here.
    const produced = new Set<Phase>();
    const statuses: RecorderStatus[] = [
      "idle", "recording", "review", "processing", "done", "error", "pii_blocked", "rate_limited",
    ];
    for (const loggedIn of [true, false]) {
      for (const settingsOpen of [true, false]) {
        for (const permissionSetup of [true, false]) {
          for (const status of statuses) {
            produced.add(phaseForScreen({ loggedIn, settingsOpen, permissionSetup, status }));
          }
        }
      }
    }
    expect([...produced].sort()).toEqual([...WEBVIEW_PHASES].sort());
  });
});

describe("phase list parity with Rust", () => {
  it("matches the Phase enum in error_reports.rs", () => {
    // The server validates `phase` against a fixed literal list, so a phase
    // the webview invents is a 422 and a lost report. Reading the enum keeps
    // the two sides honest without anyone remembering to.
    const source = readFileSync(
      resolve(__dirname, "../../src-tauri/src/error_reports.rs"),
      "utf-8",
    );
    const block = source.match(/pub enum Phase \{([^}]*)\}/);
    expect(block, "could not find `pub enum Phase` -- has it been renamed?").not.toBeNull();

    const variants = [...block![1].matchAll(/^\s*([A-Z][A-Za-z]*)\s*,/gm)].map((m) =>
      // serde renames these to snake_case on the wire.
      m[1].replace(/([a-z])([A-Z])/g, "$1_$2").toLowerCase(),
    );

    expect(variants.sort()).toEqual([...PHASES].sort());
  });
});

describe("no call site hardcodes a phase", () => {
  it("every createErrorReport passes a value, not a literal", () => {
    // The original bug in one assertion. Three call sites raised reports and
    // two of them passed the string "unknown" while the third picked between
    // two literals, so the phase tag described the code rather than the user.
    // Reading the sources catches a fourth call site added later, which no
    // behavioural test would.
    const sources = [
      "src/main.tsx",
      "src/components/ErrorBoundary.tsx",
      "src/App.tsx",
    ];

    for (const relative of sources) {
      const text = readFileSync(resolve(__dirname, "../..", relative), "utf-8");
      // Match the phase argument of a create call, across line breaks.
      const calls = [
        ...text.matchAll(/create(?:ErrorReport|)\(\s*"[a-z_]+"\s*,\s*([^,]+),/g),
      ];
      expect(calls.length, `no report call found in ${relative}`).toBeGreaterThan(0);

      for (const call of calls) {
        const phaseArgument = call[1].trim();
        expect(
          phaseArgument.startsWith('"') || phaseArgument.startsWith("'"),
          `${relative} passes the literal ${phaseArgument} as the phase; ` +
            "use phaseForScreen or errorReportPhase() so it reflects the screen",
        ).toBe(false);
      }
    }
  });
});
