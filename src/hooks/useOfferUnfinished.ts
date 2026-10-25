import { useEffect, useRef } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { unfinishedRecording } from "../lib/tauri";
import { t } from "../i18n";

/**
 * Once per start, as soon as the app is ready for it, offers the newest
 * recording that was stopped but never generated: the app was closed on the
 * review screen, Generate failed, or the app crashed after Stop. Accepting
 * opens it on the review screen, where it can be generated or discarded;
 * "later" offers it again on the next start.
 */
export function useOfferUnfinished(ready: boolean, review: (outputDir: string) => void) {
  const offered = useRef(false);
  useEffect(() => {
    if (!ready || offered.current) return;
    offered.current = true;
    void (async () => {
      const unfinished = await unfinishedRecording().catch(() => null);
      if (!unfinished) return;
      const accepted = await ask(t("recovery.message", { title: unfinished.guide_title }), {
        title: t("recovery.title"),
        kind: "info",
        okLabel: t("recovery.review"),
        cancelLabel: t("recovery.later"),
      });
      if (accepted) review(unfinished.output_dir);
    })();
  }, [ready, review]);
}
