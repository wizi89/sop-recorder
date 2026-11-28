import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  render,
  screen,
  waitFor,
  fireEvent,
  act,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { ReviewScreen } from "../components/ReviewScreen";

/**
 * Review-redaction: draw, remove, delete, a failed save blocking Generate,
 * corrupt storage, and a recording with a gap. The backend is a small fake
 * keyed by step number, as the real commands are.
 */

let steps: number[];
let stored: Record<string, number[][]>;
let unreadable: string | null;
let leftovers: number[];
let saveImpl: (step: number, boxes: number[][]) => Promise<number[][]>;
let deleteImpl: (step: number) => Promise<void>;
const calls: Array<[string, unknown]> = [];

const path = (n: number) =>
  `/rec/screenshots/step_${String(n).padStart(2, "0")}.png`;

beforeEach(() => {
  steps = [1, 3, 4];
  stored = {};
  unreadable = null;
  leftovers = [];
  calls.length = 0;
  saveImpl = async (step, boxes) => {
    if (boxes.length) stored[step] = boxes;
    else delete stored[step];
    return boxes;
  };
  deleteImpl = async (step) => {
    steps = steps.filter((n) => n !== step);
    delete stored[step];
  };
  global.URL.createObjectURL = vi.fn(() => "blob:fake");
  global.URL.revokeObjectURL = vi.fn();
  vi.mocked(invoke).mockImplementation(async (cmd: string, args?: unknown) => {
    const a = (args ?? {}) as { step: number; boxes: number[][] };
    calls.push([cmd, args]);
    switch (cmd) {
      case "list_capture_records":
        return steps.map((step) => ({
          step,
          path: path(step),
          capture_scope: "window",
          password_check: "ok",
        }));
      case "list_session_screenshots":
        return steps.map((step) => ({ step, path: path(step) }));
      case "load_redactions":
        return {
          steps: unreadable ? {} : { ...stored },
          leftover_steps: leftovers,
          unreadable,
        };
      case "save_redactions":
        return saveImpl(a.step, a.boxes);
      case "delete_step":
        return deleteImpl(a.step);
      case "reset_redactions":
        unreadable = null;
        stored = {};
        return;
      case "read_screenshot_bytes":
        return [1, 2, 3];
      case "get_pipelines":
        return [];
      case "get_selected_pipeline":
        return "";
      default:
        return undefined;
    }
  });

  // jsdom has no layout: the enlarged frame is 200x100 at the origin and every
  // image is 400x200, so one display pixel is two image pixels.
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    x: 0,
    y: 0,
    left: 0,
    top: 0,
    width: 200,
    height: 100,
    right: 200,
    bottom: 100,
    toJSON: () => ({}),
  } as DOMRect);
  Object.defineProperty(HTMLImageElement.prototype, "naturalWidth", {
    configurable: true,
    get: () => 400,
  });
  Object.defineProperty(HTMLImageElement.prototype, "naturalHeight", {
    configurable: true,
    get: () => 200,
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

const generate = () => screen.getByRole("button", { name: "Generieren" });

async function renderReview(
  props: Partial<Parameters<typeof ReviewScreen>[0]> = {},
) {
  const onConfirm = vi.fn();
  render(
    <ReviewScreen
      outputDir="/rec"
      captureCount={3}
      elapsedSec={10}
      onConfirm={onConfirm}
      onCancel={() => {}}
      {...props}
    />,
  );
  await waitFor(() =>
    expect(screen.queryByText("Screenshots werden geladen...")).toBeNull(),
  );
  return onConfirm;
}

async function openStep(n: number) {
  await userEvent.setup().click(screen.getByAltText(`Schritt ${n}`));
  const big = screen.getByAltText(`Schritt ${n}, vergrößert`);
  fireEvent.load(big);
}

function drag(from: [number, number], to: [number, number]) {
  const frame = screen.getByTestId("redaction-frame");
  fireEvent.pointerDown(frame, {
    button: 0,
    clientX: from[0],
    clientY: from[1],
    pointerId: 1,
  });
  fireEvent.pointerMove(frame, {
    clientX: to[0],
    clientY: to[1],
    pointerId: 1,
  });
  fireEvent.pointerUp(frame, { clientX: to[0], clientY: to[1], pointerId: 1 });
}

describe("review redaction", () => {
  it("labels steps by their file number across a gap", async () => {
    await renderReview();
    for (const n of [1, 3, 4])
      expect(screen.getByText(`Schritt ${n}`)).toBeInTheDocument();
    expect(screen.queryByText("Schritt 2")).toBeNull();
  });

  it("draws a box on the step labelled 3 and stores it in image pixels for step 3", async () => {
    await renderReview();
    await openStep(3);
    drag([10, 10], [30, 20]);
    await waitFor(() =>
      expect(screen.getAllByTestId("redaction-applied").length).toBeGreaterThan(
        0,
      ),
    );
    const save = calls.find(([c]) => c === "save_redactions")!;
    // The frame is 200x100 showing a 400x200 image: scale 2.
    expect(save[1]).toEqual({
      outputDir: "/rec",
      step: 3,
      boxes: [[20, 20, 40, 20]],
    });
    expect(generate()).toBeEnabled();
  });

  it("removes a box again", async () => {
    stored = { "3": [[20, 20, 40, 20]] };
    await renderReview();
    await openStep(3);
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Schwärzung entfernen" }));
    await waitFor(() =>
      expect(screen.queryByTestId("redaction-applied")).toBeNull(),
    );
    expect(stored).toEqual({});
  });

  it("shows a box as an outline and blocks Generate until its save returns", async () => {
    let release!: () => void;
    saveImpl = (step, boxes) =>
      new Promise((r) => {
        release = () => {
          stored[step] = boxes;
          r(boxes);
        };
      });
    await renderReview();
    await openStep(1);
    drag([10, 10], [30, 20]);
    await waitFor(() =>
      expect(screen.getByTestId("redaction-pending")).toBeInTheDocument(),
    );
    expect(generate()).toBeDisabled();
    await act(async () => release());
    await waitFor(() =>
      expect(screen.getAllByTestId("redaction-applied").length).toBeGreaterThan(
        0,
      ),
    );
    expect(generate()).toBeEnabled();
  });

  it("a failed save keeps the box an outline, says so, and blocks Generate until retried", async () => {
    saveImpl = async () => {
      throw "disk full";
    };
    await renderReview();
    await openStep(1);
    drag([10, 10], [30, 20]);
    // The editor covers the alert list behind it, so it says so itself.
    await waitFor(() =>
      expect(
        within(screen.getByRole("dialog")).getByRole("alert"),
      ).toHaveTextContent(
        "Schwärzung für Schritt 1 konnte nicht gespeichert werden: disk full",
      ),
    );
    expect(screen.queryByTestId("redaction-applied")).toBeNull();
    expect(screen.getByTestId("redaction-pending")).toBeInTheDocument();
    expect(generate()).toBeDisabled();

    saveImpl = async (step, boxes) => {
      stored[step] = boxes;
      return boxes;
    };
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Erneut speichern" }));
    await waitFor(() => expect(generate()).toBeEnabled());
  });

  it("deletes a step after confirmation, keeping the other numbers", async () => {
    await renderReview();
    await openStep(3);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Schritt löschen" }));
    expect(
      screen.getByText(/Sprachaufnahme bleibt erhalten/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/im Screenshot des nächsten Schritts noch zu sehen/),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Löschen" }));
    await waitFor(() => expect(screen.queryByText("Schritt 3")).toBeNull());
    expect(screen.getByText("Schritt 1")).toBeInTheDocument();
    expect(screen.getByText("Schritt 4")).toBeInTheDocument();
    expect(screen.getByText(/1 gelöscht/)).toBeInTheDocument();
    expect(calls).toContainEqual([
      "delete_step",
      { outputDir: "/rec", step: 3 },
    ]);
    expect(generate()).toBeEnabled();
  });

  it("deleting the last step leaves nothing to generate", async () => {
    steps = [2];
    await renderReview({ captureCount: 1 });
    await openStep(2);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Schritt löschen" }));
    await user.click(screen.getByRole("button", { name: "Löschen" }));
    await waitFor(() => expect(screen.queryByText("Schritt 2")).toBeNull());
    expect(generate()).toBeDisabled();
  });

  it("a delete that fails part way is reported and offered again", async () => {
    deleteImpl = async (step) => {
      steps = steps.filter((n) => n !== step);
      leftovers = [step];
      throw "redactions.json locked";
    };
    await renderReview();
    await openStep(3);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Schritt löschen" }));
    await user.click(screen.getByRole("button", { name: "Löschen" }));
    await waitFor(() =>
      expect(
        screen.getByText(/Schritt 3 wurde nicht vollständig gelöscht/),
      ).toBeInTheDocument(),
    );
    expect(
      screen.getByText(/Schritt 3 konnte nicht gelöscht werden/),
    ).toBeInTheDocument();
    expect(generate()).toBeDisabled();

    deleteImpl = async () => {
      leftovers = [];
    };
    await user.click(screen.getByRole("button", { name: "Erneut löschen" }));
    await waitFor(() => expect(generate()).toBeEnabled());
  });

  it("reports corrupt storage, blocks drawing, delete and Generate, and resets after confirmation", async () => {
    unreadable = "expected value at line 1";
    await renderReview();
    expect(
      screen.getByText(/Schwärzungen dieser Aufnahme sind nicht lesbar/),
    ).toBeInTheDocument();
    expect(generate()).toBeDisabled();

    await openStep(1);
    drag([10, 10], [30, 20]);
    expect(calls.some(([c]) => c === "save_redactions")).toBe(false);
    expect(
      screen.getByRole("button", { name: "Schritt löschen" }),
    ).toBeDisabled();
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Fertig" }));

    const user = userEvent.setup();
    await user.click(
      screen.getByRole("button", { name: "Schwärzungen zurücksetzen" }),
    );
    await user.click(screen.getByRole("button", { name: "Löschen" }));
    await waitFor(() => expect(generate()).toBeEnabled());
    expect(calls.some(([c]) => c === "reset_redactions")).toBe(true);
  });

  it("shows why an upload was refused", async () => {
    await renderReview({
      uploadError: "Schritt 7 wurde nicht vollständig gelöscht",
    });
    expect(
      screen.getByText(/Nicht hochgeladen: Schritt 7/),
    ).toBeInTheDocument();
  });

  it("keeps both of two box changes made before the next render", async () => {
    await renderReview();
    await openStep(3);
    // Two boxes added from the same render: both pointer-ups end one held drag.
    const frame = screen.getByTestId("redaction-frame");
    fireEvent.pointerDown(frame, {
      button: 0,
      clientX: 10,
      clientY: 10,
      pointerId: 1,
    });
    act(() => {
      fireEvent.pointerUp(frame, { clientX: 40, clientY: 40, pointerId: 1 });
      fireEvent.pointerUp(frame, { clientX: 80, clientY: 80, pointerId: 1 });
    });
    await waitFor(() => expect(generate()).toBeEnabled());
    const saves = calls.filter(([c]) => c === "save_redactions");
    expect(saves[saves.length - 1][1]).toEqual({
      outputDir: "/rec",
      step: 3,
      boxes: [
        [20, 20, 60, 60],
        [20, 20, 140, 140],
      ],
    });
    expect(stored["3"]).toHaveLength(2);
  });

  it("keeps a draw followed by a remove of another box in the same render", async () => {
    stored = { "3": [[20, 20, 40, 20]] };
    await renderReview();
    await openStep(3);
    const remove = screen.getByRole("button", { name: "Schwärzung entfernen" });
    const frame = screen.getByTestId("redaction-frame");
    fireEvent.pointerDown(frame, {
      button: 0,
      clientX: 50,
      clientY: 50,
      pointerId: 1,
    });
    act(() => {
      fireEvent.pointerUp(frame, { clientX: 90, clientY: 80, pointerId: 1 });
      fireEvent.click(remove);
    });
    await waitFor(() => expect(generate()).toBeEnabled());
    expect(stored["3"]).toEqual([[100, 100, 80, 60]]);
  });

  it("drops the upload refusal once a retried delete succeeds", async () => {
    leftovers = [7];
    await renderReview({
      uploadError: "Schritt 7 wurde nicht vollständig gelöscht",
    });
    expect(
      screen.getByText(/Nicht hochgeladen: Schritt 7/),
    ).toBeInTheDocument();
    deleteImpl = async () => {
      leftovers = [];
    };
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Erneut löschen" }));
    await waitFor(() =>
      expect(screen.queryByText(/Nicht hochgeladen/)).toBeNull(),
    );
  });

  it("drops the upload refusal once a reset succeeds", async () => {
    unreadable = "expected value at line 1";
    await renderReview({ uploadError: "Schwärzungen nicht lesbar" });
    const user = userEvent.setup();
    await user.click(
      screen.getByRole("button", { name: "Schwärzungen zurücksetzen" }),
    );
    await user.click(screen.getByRole("button", { name: "Löschen" }));
    await waitFor(() =>
      expect(screen.queryByText(/Nicht hochgeladen/)).toBeNull(),
    );
  });

  it("the step editor is modal: focus inside, content behind inert, Escape closes", async () => {
    await renderReview();
    await openStep(3);
    const dialog = screen.getByRole("dialog");
    expect(dialog).toHaveAttribute("aria-modal", "true");
    expect(dialog).toContainElement(document.activeElement as HTMLElement);
    expect(generate().closest("[inert]")).not.toBeNull();
    fireEvent.keyDown(document.activeElement!, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(generate().closest("[inert]")).toBeNull();
  });

  it("closes the step editor on Escape after focus left it", async () => {
    stored = { "3": [[20, 20, 40, 20]] };
    await renderReview();
    await openStep(3);
    await userEvent
      .setup()
      .click(screen.getByRole("button", { name: "Schwärzung entfernen" }));
    // The removed button took focus with it.
    expect(document.activeElement).toBe(document.body);
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("gives the box remove button a 24x24 px hit area", async () => {
    stored = { "3": [[20, 20, 40, 20]] };
    await renderReview();
    await openStep(3);
    const style = screen.getByRole("button", {
      name: "Schwärzung entfernen",
    }).style;
    expect(parseFloat(style.minWidth)).toBeGreaterThanOrEqual(24);
    expect(parseFloat(style.minHeight)).toBeGreaterThanOrEqual(24);
  });

  it("keeps the failed-capture total from when the screen opened", async () => {
    await renderReview({ failedCaptures: 2 });
    expect(screen.getByText(/2 von 5 Schritten/)).toBeInTheDocument();
    await openStep(4);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Schritt löschen" }));
    await user.click(screen.getByRole("button", { name: "Löschen" }));
    await waitFor(() => expect(screen.queryByText("Schritt 4")).toBeNull());
    expect(screen.getByText(/2 von 5 Schritten/)).toBeInTheDocument();
  });

  it("drops the flagged-step confirmation when the only flagged step is deleted", async () => {
    vi.mocked(invoke).mockImplementation(
      async (cmd: string, args?: unknown) => {
        if (cmd === "list_capture_records") {
          return steps.map((step) => ({
            step,
            path: path(step),
            capture_scope: "window",
            password_check: step === 3 ? "flagged" : "ok",
          }));
        }
        if (cmd === "delete_step") {
          steps = steps.filter((n) => n !== (args as { step: number }).step);
          return;
        }
        if (cmd === "load_redactions")
          return { steps: {}, leftover_steps: [], unreadable: null };
        if (cmd === "read_screenshot_bytes") return [1];
        if (cmd === "get_pipelines") return [];
        return undefined;
      },
    );
    await renderReview();
    expect(generate()).toBeDisabled();
    await openStep(3);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Schritt löschen" }));
    await user.click(screen.getByRole("button", { name: "Löschen" }));
    await waitFor(() => expect(screen.queryByText("Schritt 3")).toBeNull());
    expect(generate()).toBeEnabled();
  });
});
