import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { ReviewScreen } from "../components/ReviewScreen";
import { RecordingBar } from "../components/RecordingBar";
import { limbos, type UnrecordedVisit } from "../lib/limbos";

const app = (name: string) => ({ kind: "app" as const, name });
const visit = (
  window: UnrecordedVisit["window"],
  start: number,
  end: number,
): UnrecordedVisit => ({ window, start_seconds: start, end_seconds: end });
const steps = [
  { step: 1, timestamp_seconds: 10 },
  { step: 2, timestamp_seconds: 20 },
  { step: 3, timestamp_seconds: 30 },
];

describe("limbos", () => {
  it("puts each visit between the steps around it", () => {
    expect(
      limbos(steps, [
        visit(app("Setup"), 2, 5),
        visit(app("regedit"), 22, 25),
        visit(app("Setup"), 35, 40),
      ]),
    ).toEqual([
      { after: null, windows: [app("Setup")] },
      { after: 2, windows: [app("regedit")] },
      { after: 3, windows: [app("Setup")] },
    ]);
  });

  it("gives one row per pair of steps and names each app once", () => {
    expect(
      limbos(steps, [
        visit({ kind: "security_prompt" }, 11, 13),
        visit(app("Setup"), 13, 15),
        visit(app("regedit"), 15, 17),
        visit(app("Setup"), 17, 19),
      ]),
    ).toEqual([
      {
        after: 1,
        windows: [{ kind: "security_prompt" }, app("Setup"), app("regedit")],
      },
    ]);
  });

  it("drops a visit shorter than a second and keeps one of a second", () => {
    expect(limbos(steps, [visit(app("Setup"), 11, 11.5)])).toEqual([]);
    expect(limbos(steps, [visit(app("Setup"), 11, 12)])).toEqual([
      { after: 1, windows: [app("Setup")] },
    ]);
  });

  it("puts a visit that starts with a step after that step", () => {
    expect(limbos(steps, [visit(app("Setup"), 20, 25)])).toEqual([
      { after: 2, windows: [app("Setup")] },
    ]);
  });

  it("names the security prompt and an unknown app once each", () => {
    expect(
      limbos(steps, [
        visit({ kind: "security_prompt" }, 11, 13),
        visit({ kind: "unknown_app" }, 13, 15),
        visit({ kind: "security_prompt" }, 15, 17),
        visit({ kind: "unknown_app" }, 17, 19),
      ]),
    ).toEqual([
      {
        after: 1,
        windows: [{ kind: "security_prompt" }, { kind: "unknown_app" }],
      },
    ]);
  });

  it("orders steps and visits by time, whatever order they come in", () => {
    expect(
      limbos([...steps].reverse(), [
        visit(app("B"), 35, 40),
        visit(app("A"), 12, 14),
      ]),
    ).toEqual([
      { after: 1, windows: [app("A")] },
      { after: 3, windows: [app("B")] },
    ]);
  });

  it("merges the rows around a deleted step", () => {
    const without2 = steps.filter((s) => s.step !== 2);
    expect(
      limbos(without2, [visit(app("A"), 12, 14), visit(app("B"), 22, 24)]),
    ).toEqual([{ after: 1, windows: [app("A"), app("B")] }]);
  });
});

const PATHS = ["/tmp/step_01.png", "/tmp/step_02.png"];
let visits: UnrecordedVisit[] | "fail";
let shots: string[];

beforeEach(() => {
  shots = PATHS;
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "list_capture_records")
      return shots.map((path, i) => ({
        step: i + 1,
        path,
        capture_scope: "window",
        password_check: "ok",
        window_list_partial: false,
        timestamp_seconds: (i + 1) * 10,
      }));
    if (cmd === "load_redactions")
      return { steps: {}, leftover_steps: [], unreadable: null };
    if (cmd === "load_unrecorded") {
      if (visits === "fail") throw new Error("unreadable");
      return visits;
    }
    if (cmd === "read_screenshot_bytes") return [1, 2, 3];
    if (cmd === "get_pipelines") return [];
    return;
  });
});

function renderReview() {
  const onConfirm = vi.fn();
  render(
    <ReviewScreen
      outputDir="/tmp"
      captureCount={shots.length}
      elapsedSec={10}
      onConfirm={onConfirm}
      onCancel={() => {}}
    />,
  );
  return onConfirm;
}

describe("review screen limbo rows", () => {
  it("shows a limbo row between the steps, with the app names", async () => {
    visits = [
      visit({ kind: "security_prompt" }, 12, 14),
      visit(app("Setup Tool X"), 14, 18),
    ];
    const onConfirm = renderReview();
    const row = await screen.findByText(
      "Nicht aufgenommen: Windows-Sicherheitsabfrage, Setup Tool X (und der Klick danach)",
    );
    const items = [
      ...document.querySelectorAll("[data-strip-item]"),
    ] as HTMLElement[];
    expect(items.map((el) => el.dataset.stripItem)).toEqual([
      "step-1",
      "limbo-1",
      "step-2",
    ]);
    expect(row).toBeInTheDocument();
    expect(
      screen.queryByText(/Es wurden keine Schritte aufgenommen/),
    ).not.toBeInTheDocument();
    // Informational: no tick, Generate stays open.
    const generate = screen.getByRole("button", { name: "Generieren" });
    expect(generate).toBeEnabled();
    await userEvent.setup().click(generate);
    expect(onConfirm).toHaveBeenCalled();
  });

  it("shows a limbo before the first step", async () => {
    visits = [visit(app("Setup Tool X"), 2, 8)];
    renderReview();
    await screen.findByText(
      "Nicht aufgenommen: Setup Tool X (und der Klick danach)",
    );
    const items = [
      ...document.querySelectorAll("[data-strip-item]"),
    ] as HTMLElement[];
    expect(items.map((el) => el.dataset.stripItem)).toEqual([
      "limbo-0",
      "step-1",
      "step-2",
    ]);
  });

  it("shows nothing when there were no unrecorded windows", async () => {
    visits = [];
    renderReview();
    await waitFor(() =>
      expect(screen.getAllByAltText(/^Schritt \d+$/)).toHaveLength(2),
    );
    expect(screen.queryByText(/Nicht aufgenommen/)).not.toBeInTheDocument();
  });

  it("still opens when the file cannot be read", async () => {
    visits = "fail";
    renderReview();
    await waitFor(() =>
      expect(screen.getAllByAltText(/^Schritt \d+$/)).toHaveLength(2),
    );
    expect(screen.queryByText(/Nicht aufgenommen/)).not.toBeInTheDocument();
  });

  it("says why a recording without steps is empty", async () => {
    shots = [];
    visits = [
      visit(app("Setup Tool X"), 2, 8),
      visit({ kind: "unknown_app" }, 9, 12),
    ];
    renderReview();
    expect(
      await screen.findByText(
        "Es wurden keine Schritte aufgenommen. Alle Klicks waren in Fenstern, die mit Administratorrechten laufen: Setup Tool X, unbekanntes Programm.",
      ),
    ).toBeInTheDocument();
    // The message replaces the row; the empty strip shows none.
    expect(document.querySelectorAll("[data-strip-item]")).toHaveLength(0);
  });
});

describe("recording bar hint", () => {
  afterEach(() => {
    vi.mocked(listen).mockImplementation((async () =>
      () => {}) as unknown as typeof listen);
  });

  it("shows the hint while an unrecorded window is in front", async () => {
    const handlers: Record<string, (e: { payload: unknown }) => void> = {};
    vi.mocked(listen).mockImplementation((async (
      event: string,
      handler: unknown,
    ) => {
      handlers[event] = handler as (e: { payload: unknown }) => void;
      return () => {};
    }) as unknown as typeof listen);
    render(<RecordingBar />);
    await waitFor(() =>
      expect(handlers["recording:unrecorded_window"]).toBeDefined(),
    );
    const hint = /werden nicht aufgenommen/;

    expect(screen.queryByTitle(hint)).not.toBeInTheDocument();
    await act(async () => {
      handlers["recording:unrecorded_window"]?.({ payload: true });
    });
    expect(screen.getByTitle(hint)).toBeInTheDocument();
    expect(screen.getByText("Nicht aufgenommen")).toBeInTheDocument();

    await act(async () => {
      handlers["recording:unrecorded_window"]?.({ payload: false });
    });
    expect(screen.queryByTitle(hint)).not.toBeInTheDocument();
  });

  it("keeps the hint for a window in front at start, clears it on stop", async () => {
    const handlers: Record<string, (e: { payload: unknown }) => void> = {};
    vi.mocked(listen).mockImplementation((async (
      event: string,
      handler: unknown,
    ) => {
      handlers[event] = handler as (e: { payload: unknown }) => void;
      return () => {};
    }) as unknown as typeof listen);
    render(<RecordingBar />);
    await waitFor(() =>
      expect(handlers["recording:unrecorded_window"]).toBeDefined(),
    );
    // start_recording: the watch reports the window in front, then "started".
    await act(async () => {
      handlers["recording:unrecorded_window"]?.({ payload: true });
    });
    await act(async () => {
      handlers["recording:started"]?.({ payload: null });
    });
    expect(screen.getByText("Nicht aufgenommen")).toBeInTheDocument();
    await act(async () => {
      handlers["recording:stopped"]?.({ payload: null });
    });
    expect(screen.queryByText("Nicht aufgenommen")).not.toBeInTheDocument();
  });
});
