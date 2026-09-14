import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";

type UpdaterStatus = "idle" | "checking" | "available" | "downloading" | "error";

// The check and the install go through Rust rather than the plugin's own JS
// `check()`. That one resolves its endpoint from tauri.conf.json, fixed at
// build time, so it cannot follow the user's channel choice -- CheckOptions
// carries headers, timeout, proxy and target, but no endpoints.
export function useUpdater() {
  const [status, setStatus] = useState<UpdaterStatus>("idle");
  const [version, setVersion] = useState<string | null>(null);
  const [dismissed, setDismissed] = useState(false);

  const dismiss = useCallback(() => setDismissed(true), []);

  useEffect(() => {
    let cancelled = false;
    setStatus("checking");

    (async () => {
      try {
        const enabled = await invoke<boolean>("is_updater_enabled");
        if (cancelled) return;
        if (!enabled) {
          setStatus("idle");
          return;
        }
        const available = await invoke<string | null>("check_for_update");
        if (cancelled) return;
        if (available) {
          setVersion(available);
          setStatus("available");
        } else {
          setStatus("idle");
        }
      } catch {
        if (!cancelled) setStatus("idle");
      }
    })();

    return () => {
      cancelled = true;
    };
  }, []);

  const install = useCallback(async () => {
    if (!version) return;
    setStatus("downloading");
    try {
      // On Windows NSIS, this closes the app, installs, and relaunches automatically
      await invoke("install_update");
    } catch {
      setStatus("error");
    }
  }, [version]);

  return { status, version, install, dismissed, dismiss };
}
