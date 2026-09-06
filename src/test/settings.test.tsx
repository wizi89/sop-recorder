import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { SettingsPage } from "../components/SettingsPage";

const mockSettings = {
  output_dir: "C:\\docs\\workflows",
  logs_dir: "C:\\data\\logs",
  hide_from_screenshots: true,
  upload_target: null,
  skip_pii_check: false,
  error_reports: "ask",
};

let mockErrorReportsForcedOff = false;

const mockQuotaBasic = {
  count: 0,
  limit: 100,
  remaining: 100,
  features: { advanced_settings: false },
  generation_settings: { pipeline_versions: [1, 2], models: ["azure/gpt-4.1"], default_model: "azure/gpt-4.1" },
};

const mockQuotaAdvanced = {
  ...mockQuotaBasic,
  features: { advanced_settings: true },
};

let mockQuota = mockQuotaBasic;

beforeEach(() => {
  mockQuota = mockQuotaBasic;
  mockErrorReportsForcedOff = false;
  vi.mocked(invoke).mockImplementation(async (cmd: string) => {
    if (cmd === "get_settings") return { ...mockSettings };
    if (cmd === "save_settings") return;
    if (cmd === "get_quota") return mockQuota;
    if (cmd === "are_error_reports_forced_off") return mockErrorReportsForcedOff;
    return;
  });
});

describe("SettingsPage", () => {
  it("renders PII toggle", async () => {
    render(<SettingsPage isDev={false} />);
    await waitFor(() => {
      expect(screen.getByText(/pii/i)).toBeInTheDocument();
    });
  });

  it("shows confirmation modal when disabling PII check", async () => {
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    // Wait for settings to load
    await waitFor(() => {
      expect(screen.getByText(/pii/i)).toBeInTheDocument();
    });

    // Find and click the PII toggle (second switch-track button)
    const toggles = screen.getAllByRole("button").filter(
      (btn) => btn.className.includes("switch-track"),
    );
    // PII toggle is the second switch
    await user.click(toggles[1]);

    // Confirmation modal should appear
    expect(screen.getByText(/sicherheitsprüfung deaktivieren\?/i)).toBeInTheDocument();
    expect(screen.getByText(/verstanden, deaktivieren/i)).toBeInTheDocument();
    // Two "Abbrechen" buttons: settings cancel + modal cancel
    expect(screen.getAllByText("Abbrechen")).toHaveLength(2);
  });

  it("does not disable PII check when cancelled", async () => {
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText(/pii/i)).toBeInTheDocument();
    });

    // Click PII toggle
    const toggles = screen.getAllByRole("button").filter(
      (btn) => btn.className.includes("switch-track"),
    );
    await user.click(toggles[1]);

    // Click the modal cancel button (the smaller one inside the modal)
    const cancelButtons = screen.getAllByText("Abbrechen");
    const modalCancel = cancelButtons.find((btn) => btn.className.includes("py-2 text-xs"))!;
    await user.click(modalCancel);

    // Modal should close
    expect(screen.queryByText(/sicherheitsprüfung deaktivieren\?/i)).not.toBeInTheDocument();
  });

  it("disables PII check when confirmed", async () => {
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText(/pii/i)).toBeInTheDocument();
    });

    // Click PII toggle
    const toggles = screen.getAllByRole("button").filter(
      (btn) => btn.className.includes("switch-track"),
    );
    await user.click(toggles[1]);

    // Click confirm
    await user.click(screen.getByText(/verstanden, deaktivieren/i));

    // Modal should close
    expect(screen.queryByText(/sicherheitsprüfung deaktivieren\?/i)).not.toBeInTheDocument();
  });

  it("does not show confirmation when re-enabling PII check", async () => {
    // Start with PII check disabled
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === "get_settings") return { ...mockSettings, skip_pii_check: true };
      if (cmd === "save_settings") return;
      return;
    });

    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText(/pii/i)).toBeInTheDocument();
    });

    // Click PII toggle to re-enable
    const toggles = screen.getAllByRole("button").filter(
      (btn) => btn.className.includes("switch-track"),
    );
    await user.click(toggles[1]);

    // No confirmation modal should appear
    expect(screen.queryByText(/sicherheitsprüfung deaktivieren\?/i)).not.toBeInTheDocument();
  });

  it("shows legal links in confirmation modal", async () => {
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText(/pii/i)).toBeInTheDocument();
    });

    const toggles = screen.getAllByRole("button").filter(
      (btn) => btn.className.includes("switch-track"),
    );
    await user.click(toggles[1]);

    expect(screen.getByText("Impressum")).toBeInTheDocument();
    expect(screen.getByText("Datenschutz")).toBeInTheDocument();
    expect(screen.getByText("AGB")).toBeInTheDocument();
  });

  it("shows all disclaimer bullet points in confirmation modal", async () => {
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    await waitFor(() => {
      expect(screen.getByText(/pii/i)).toBeInTheDocument();
    });

    const toggles = screen.getAllByRole("button").filter(
      (btn) => btn.className.includes("switch-track"),
    );
    await user.click(toggles[1]);

    expect(screen.getByText(/namen, e-mail-adressen/i)).toBeInTheDocument();
    expect(screen.getByText(/passwörtern oder api-schlüsseln/i)).toBeInTheDocument();
    expect(screen.getByText(/vertraulicher unternehmensinhalte/i)).toBeInTheDocument();
  });

  it("hides upload target for orgs without advanced_settings", async () => {
    mockQuota = mockQuotaBasic;
    render(<SettingsPage isDev={true} />);

    await waitFor(() => {
      expect(screen.getByText(/pii/i)).toBeInTheDocument();
    });

    expect(screen.queryByText(/hochladen an/i)).not.toBeInTheDocument();
  });

  it("shows Local, Staging, Production for advanced org in dev build", async () => {
    mockQuota = mockQuotaAdvanced;
    render(<SettingsPage isDev={true} />);

    await waitFor(() => {
      expect(screen.getByText(/hochladen an/i)).toBeInTheDocument();
    });

    expect(screen.getByRole("option", { name: "Local" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Staging" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Production" })).toBeInTheDocument();
  });

  it("shows only Staging and Production for advanced org in release build", async () => {
    mockQuota = mockQuotaAdvanced;
    render(<SettingsPage isDev={false} />);

    await waitFor(() => {
      expect(screen.getByText(/hochladen an/i)).toBeInTheDocument();
    });

    expect(screen.queryByRole("option", { name: "Local" })).not.toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Staging" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Production" })).toBeInTheDocument();
  });
});

describe("SettingsPage error reports (design D1)", () => {
  it("saves the chosen mode", async () => {
    render(<SettingsPage isDev={false} />);
    const user = userEvent.setup();

    const select = await screen.findByLabelText("Fehlerberichte");
    expect(select).toHaveValue("ask");

    await user.selectOptions(select, "always");
    await user.click(screen.getByRole("button", { name: "Speichern" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "save_settings",
        expect.objectContaining({
          settings: expect.objectContaining({ error_reports: "always" }),
        }),
      );
    });
  });

  it("renders disabled with the organisation note under the override", async () => {
    mockErrorReportsForcedOff = true;
    render(<SettingsPage isDev={false} />);

    const select = await screen.findByLabelText("Fehlerberichte");
    await waitFor(() => expect(select).toBeDisabled());
    expect(select).toHaveValue("never");
    expect(
      screen.getByText("Von Ihrer Organisation deaktiviert"),
    ).toBeInTheDocument();
  });
});
