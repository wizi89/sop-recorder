import { afterEach, vi } from "vitest";
import { cleanup } from "@testing-library/react";
import "@testing-library/jest-dom/vitest";

// Mock Tauri APIs globally
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({
    setSize: vi.fn(),
    setAlwaysOnTop: vi.fn(),
    setDecorations: vi.fn(),
    setResizable: vi.fn(),
    close: vi.fn(),
  })),
  LogicalSize: vi.fn((w: number, h: number) => ({ width: w, height: h })),
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: vi.fn(),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  ask: vi.fn(() => Promise.resolve(false)),
  open: vi.fn(() => Promise.resolve(null)),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn(),
  revealItemInDir: vi.fn(),
}));

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
