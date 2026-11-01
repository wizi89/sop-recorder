import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { ReviewScreen } from "../components/ReviewScreen";

const PATHS = ["/tmp/step_01.png", "/tmp/step_02.png", "/tmp/step_03.png"];
let records: unknown[];

beforeEach(() => {
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "list_session_screenshots") return PATHS;
    if (cmd === "list_capture_records") return records;
    if (cmd === "read_screenshot_bytes") return [1, 2, 3];
    if (cmd === "get_pipelines") return [];
    return;
  });
});

const ok = (path: string) => ({ path, capture_scope: "window", password_check: "ok" });

async function renderReview() {
  const onConfirm = vi.fn();
  render(
    <ReviewScreen outputDir="/tmp" captureCount={3} elapsedSec={10} onConfirm={onConfirm} onCancel={() => {}} />,
  );
  await waitFor(() => expect(screen.getAllByAltText(/step/)).toHaveLength(3));
  return onConfirm;
}

const generate = () => screen.getByRole("button", { name: "Generieren" });

describe("review marks", () => {
  it("marks flagged and fallback steps and needs the confirmation", async () => {
    records = [
      { path: PATHS[0], capture_scope: "window", password_check: "flagged" },
      ok(PATHS[1]),
      { path: PATHS[2], capture_scope: "monitor_fallback", password_check: "ok" },
    ];
    const onConfirm = await renderReview();
    expect(screen.getAllByText("Prüfen")).toHaveLength(2);
    expect(generate()).toBeDisabled();

    await userEvent.setup().click(screen.getByLabelText("Ich habe die markierten Schritte geprüft"));
    expect(generate()).toBeEnabled();
    await userEvent.setup().click(generate());
    expect(onConfirm).toHaveBeenCalled();
  });

  it("shows no confirmation when nothing is marked", async () => {
    records = PATHS.map(ok);
    await renderReview();
    expect(screen.queryByText("Prüfen")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Ich habe die markierten Schritte geprüft")).not.toBeInTheDocument();
    expect(generate()).toBeEnabled();
  });

  it("marks every step of a legacy recording", async () => {
    records = PATHS.map((path) => ({ path, capture_scope: "unknown", password_check: "unknown" }));
    await renderReview();
    expect(screen.getAllByText("Prüfen")).toHaveLength(3);
    expect(generate()).toBeDisabled();
  });
});
