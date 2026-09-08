import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";

import { ReviewScreen } from "../components/ReviewScreen";

const mockInvoke = vi.mocked(invoke);

/**
 * A recording that lost steps must say so before the user commits to
 * generating from it. Before this, a failed capture was a log line: the review
 * screen counted the screenshots that survived and looked complete, which is
 * how a 21-click recording presented itself as a one-step guide on 2026-09-03.
 */
describe("ReviewScreen failed-capture notice", () => {
  const defaults = {
    outputDir: "/tmp/session",
    captureCount: 3,
    elapsedSec: 30,
    onConfirm: vi.fn(),
    onCancel: vi.fn(),
  };

  beforeEach(() => {
    global.URL.createObjectURL = vi.fn(() => "blob:fake");
    global.URL.revokeObjectURL = vi.fn();
    mockInvoke.mockImplementation((cmd: string) => {
      switch (cmd) {
        case "list_session_screenshots":
          return Promise.resolve(["/tmp/step_01.png", "/tmp/step_03.png", "/tmp/step_04.png"]);
        case "read_screenshot_bytes":
          return Promise.resolve([1, 2, 3]);
        // The pipeline selector runs on this screen too; it needs a list
        // rather than undefined, and an empty one keeps it out of the way.
        case "get_pipelines":
          return Promise.resolve([]);
        case "get_selected_pipeline":
          return Promise.resolve("");
        default:
          return Promise.resolve(undefined);
      }
    });
  });

  it("names how many steps were lost, out of how many were attempted", async () => {
    render(<ReviewScreen {...defaults} failedCaptures={2} />);

    // Three screenshots survived, two failed: the user performed five actions.
    await waitFor(() =>
      expect(screen.getByText(/2 von 5 Schritten konnten nicht aufgenommen werden/i))
        .toBeInTheDocument(),
    );
  });

  it("says nothing when every capture succeeded", async () => {
    render(<ReviewScreen {...defaults} failedCaptures={0} />);

    await waitFor(() => expect(screen.getByText(/Screenshots/)).toBeInTheDocument());
    expect(screen.queryByText(/konnten nicht aufgenommen werden/i)).not.toBeInTheDocument();
  });

  it("says nothing when the count is absent, as it is for a folder from disk", async () => {
    render(<ReviewScreen {...defaults} />);

    await waitFor(() => expect(screen.getByText(/Screenshots/)).toBeInTheDocument());
    expect(screen.queryByText(/konnten nicht aufgenommen werden/i)).not.toBeInTheDocument();
  });
});
