import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";

import { ReviewScreen } from "../components/ReviewScreen";

const mockInvoke = vi.mocked(invoke);

const TWO_PIPELINES = [
  { id: "stoerung", display_name: "Stoerungsbehebung", description: "Fehlerdiagnose" },
  { id: "onboarding", display_name: "Software-Onboarding", description: "Einarbeitung" },
];

/**
 * Route the Tauri commands ReviewScreen touches. `pipelines` and `selected`
 * are what the test is varying; the screenshot commands just need to resolve.
 */
function mockCommands(opts: {
  pipelines?: unknown;
  selected?: string;
  pipelinesThrows?: boolean;
} = {}) {
  mockInvoke.mockImplementation((cmd: string) => {
    switch (cmd) {
      case "list_session_screenshots":
        return Promise.resolve(["/tmp/step_01.png"]);
      case "read_screenshot_bytes":
        return Promise.resolve([1, 2, 3]);
      case "get_pipelines":
        return opts.pipelinesThrows
          ? Promise.reject(new Error("server unreachable"))
          : Promise.resolve(opts.pipelines ?? []);
      case "get_selected_pipeline":
        return Promise.resolve(opts.selected ?? "");
      case "set_selected_pipeline":
        return Promise.resolve();
      default:
        return Promise.resolve(undefined);
    }
  });
}

const defaults = {
  outputDir: "/tmp/session",
  captureCount: 1,
  elapsedSec: 30,
  onConfirm: vi.fn(),
  onCancel: vi.fn(),
};

describe("ReviewScreen pipeline selector", () => {
  beforeEach(() => {
    global.URL.createObjectURL = vi.fn(() => "blob:fake");
    global.URL.revokeObjectURL = vi.fn();
  });

  it("renders the selector at two or more catalogue entries", async () => {
    mockCommands({ pipelines: TWO_PIPELINES });
    render(<ReviewScreen {...defaults} />);

    const select = await screen.findByLabelText(/art der anleitung/i);
    expect(select).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Stoerungsbehebung" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Software-Onboarding" })).toBeInTheDocument();
  });

  it("renders nothing at zero entries", async () => {
    mockCommands({ pipelines: [] });
    render(<ReviewScreen {...defaults} />);

    await screen.findByText(/generieren/i);
    expect(screen.queryByLabelText(/art der anleitung/i)).not.toBeInTheDocument();
  });

  it("renders nothing at one entry, because one option is not a choice", async () => {
    mockCommands({ pipelines: [TWO_PIPELINES[0]] });
    render(<ReviewScreen {...defaults} />);

    await screen.findByText(/generieren/i);
    expect(screen.queryByLabelText(/art der anleitung/i)).not.toBeInTheDocument();
  });

  it("renders nothing and never blocks when the endpoint is unreachable", async () => {
    mockCommands({ pipelinesThrows: true });
    render(<ReviewScreen {...defaults} />);

    // The recording is fine; a catalogue problem is not an error over it.
    const confirm = await screen.findByRole("button", { name: /generieren/i });
    expect(confirm).toBeEnabled();
    expect(screen.queryByLabelText(/art der anleitung/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/unreachable/i)).not.toBeInTheDocument();
  });

  it("persists the selected id, never the display name", async () => {
    mockCommands({ pipelines: TWO_PIPELINES });
    render(<ReviewScreen {...defaults} />);

    const select = await screen.findByLabelText(/art der anleitung/i);
    await userEvent.selectOptions(select, "onboarding");

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("set_selected_pipeline", {
        pipelineId: "onboarding",
      });
    });
  });

  it("preselects the last choice, visibly", async () => {
    mockCommands({ pipelines: TWO_PIPELINES, selected: "onboarding" });
    render(<ReviewScreen {...defaults} />);

    const select = await screen.findByLabelText<HTMLSelectElement>(/art der anleitung/i);
    await waitFor(() => expect(select.value).toBe("onboarding"));
    // Visible, not silently applied: the description of the active choice shows.
    expect(screen.getByText("Einarbeitung")).toBeInTheDocument();
  });

  it("drops a stored selection that is no longer in the catalogue", async () => {
    mockCommands({ pipelines: TWO_PIPELINES, selected: "deleted-last-week" });
    render(<ReviewScreen {...defaults} />);

    const select = await screen.findByLabelText<HTMLSelectElement>(/art der anleitung/i);
    await waitFor(() => expect(select.value).toBe(""));
  });

  it("shows a count-only summary when there is no live recording timing", async () => {
    // A folder picked from disk goes to review so the pipeline can be chosen;
    // it has no elapsed time, and "00:00 Min aufgenommen" would be a lie.
    mockCommands({ pipelines: TWO_PIPELINES });
    render(<ReviewScreen {...defaults} captureCount={0} elapsedSec={0} />);

    expect(await screen.findByText(/1 Screenshots in diesem Ordner/)).toBeInTheDocument();
    expect(screen.queryByText(/aufgenommen/)).not.toBeInTheDocument();
    // The selector is available here, which is the point of routing folders
    // through review at all.
    expect(screen.getByLabelText(/art der anleitung/i)).toBeInTheDocument();
  });

  it("offers a default option so a user can opt out of every pipeline", async () => {
    mockCommands({ pipelines: TWO_PIPELINES, selected: "stoerung" });
    render(<ReviewScreen {...defaults} />);

    const select = await screen.findByLabelText(/art der anleitung/i);
    await userEvent.selectOptions(select, "");

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("set_selected_pipeline", { pipelineId: "" });
    });
  });

  it("renders pipelines this build has never heard of", async () => {
    // The point of the whole slice: a manifest authored server-side appears in
    // an already-installed recorder with no application update. Nothing about
    // any pipeline is compiled in, so an id invented right here must render.
    mockCommands({
      pipelines: [
        ...TWO_PIPELINES,
        { id: "brand-new-2027", display_name: "Qualitaetspruefung", description: "Neu" },
      ],
    });
    render(<ReviewScreen {...defaults} />);

    expect(
      await screen.findByRole("option", { name: "Qualitaetspruefung" }),
    ).toBeInTheDocument();
  });

  it("is independent of the advanced-settings gate", async () => {
    // No quota call, so no `features.advanced_settings` anywhere in the path:
    // a plain org sees the pipeline selector, and still sees no pipeline
    // version, model, or upload-target control (those live in Settings).
    mockCommands({ pipelines: TWO_PIPELINES });
    render(<ReviewScreen {...defaults} />);

    expect(await screen.findByLabelText(/art der anleitung/i)).toBeInTheDocument();
    expect(mockInvoke).not.toHaveBeenCalledWith("get_quota", expect.anything());
    expect(screen.queryByText(/pipeline.version/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/gpt-/i)).not.toBeInTheDocument();
  });
});
