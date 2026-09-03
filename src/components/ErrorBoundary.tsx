import { Component, type ErrorInfo, type ReactNode } from "react";
import { createErrorReport } from "../lib/tauri";
import { t } from "../i18n";

interface ErrorBoundaryProps {
  children: ReactNode;
}

interface ErrorBoundaryState {
  message: string | null;
}

/**
 * Catches an unhandled error in the main window's tree (design D6).
 *
 * Without one, a throw anywhere below `App` unmounts the whole tree and the
 * user is left looking at a white window with no way back and nothing to tell
 * us. This keeps something on screen and creates a `ui_error` report, so the
 * Rust side attaches the recent log and the platform facts the same way it
 * does for a panic.
 *
 * Deliberately not around the recording bar: its capability set is narrow, its
 * code path is tiny, and an error there surfaces in this window's state anyway.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  state: ErrorBoundaryState = { message: null };

  static getDerivedStateFromError(error: unknown): ErrorBoundaryState {
    return { message: error instanceof Error ? error.message : String(error) };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Unbehandelter Fehler in der Oberfläche:", error, info);
    // One report per crash of the tree: `componentDidCatch` fires once per
    // caught error, and the boundary does not re-render its children after.
    void createErrorReport(
      "ui_error",
      "unknown",
      `${error.message}\n${info.componentStack ?? ""}`.trim(),
    ).catch(() => {});
  }

  render() {
    if (this.state.message === null) {
      return this.props.children;
    }
    return (
      <div className="flex flex-col items-center justify-center h-full gap-3 p-6 text-center bg-surface">
        <p className="text-on-surface text-sm font-semibold">
          {t("error.prefix", { message: this.state.message })}
        </p>
        <p className="text-on-surface-variant text-xs leading-snug">
          {t("report.intro")}
        </p>
      </div>
    );
  }
}
