import { useEffect, useState, useCallback } from "react";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { invoke } from "@tauri-apps/api/core";

type UpdaterStatus = "idle" | "checking" | "available" | "downloading" | "error";

export function useUpdater() {
  const [status, setStatus] = useState<UpdaterStatus>("idle");
  const [version, setVersion] = useState<string | null>(null);
  const [update, setUpdate] = useState<Update | null>(null);
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
        const u = await check();
        if (cancelled) return;
        if (u) {
          setUpdate(u);
          setVersion(u.version);
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
    if (!update) return;
    setStatus("downloading");
    try {
      // On Windows NSIS, this closes the app, installs, and relaunches automatically
      await update.downloadAndInstall();
    } catch {
      setStatus("error");
    }
  }, [update]);

  return { status, version, install, dismissed, dismiss };
}
