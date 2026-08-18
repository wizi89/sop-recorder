import { useCallback, useEffect, useRef, useState } from "react";

import {
  getPipelines,
  getSelectedPipeline,
  setSelectedPipeline,
  type Pipeline,
} from "../lib/tauri";

export interface UsePipelinesResult {
  /** The catalogue. Empty means no selector renders. */
  pipelines: Pipeline[];
  /** The currently selected id, or "" for none. */
  selectedId: string;
  /** Record a choice. Persisted so the next recording preselects it. */
  select: (pipelineId: string) => void;
  /**
   * Whether to render the selector at all. False at zero or one entry: a
   * dropdown with a single option is not a choice, and an installation with
   * no pipelines configured must see no change at all.
   */
  visible: boolean;
}

/**
 * Pipeline catalogue plus the user's remembered choice.
 *
 * Never surfaces an error. A catalogue that cannot be fetched is served from
 * the last good response by the Tauri command, and failing that is empty --
 * which renders nothing. Reporting a catalogue problem on top of a recording
 * that succeeded would be an error message about a feature the user may not
 * even use.
 *
 * A stored selection that is no longer in the catalogue is cleared from the
 * store, not merely from this component's state: the pipeline was deleted or
 * unlabelled server-side, and the upload path reads the stored id on its own.
 */
export function usePipelines(enabled: boolean = true): UsePipelinesResult {
  const [pipelines, setPipelines] = useState<Pipeline[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    if (!enabled) return;

    (async () => {
      const [catalogue, stored] = await Promise.all([
        getPipelines().catch(() => [] as Pipeline[]),
        getSelectedPipeline().catch(() => ""),
      ]);
      if (!mountedRef.current) return;
      setPipelines(catalogue);
      const stillOffered = catalogue.some((p) => p.id === stored);
      setSelectedId(stillOffered ? stored : "");
      if (stored && !stillOffered) {
        // Clear the STORE, not just this component's state. The upload reads
        // the stored id directly (commands::pipelines::selected_pipeline_id),
        // so resetting only the dropdown would show "Standard" while still
        // sending a pipeline the server no longer offers -- which the server
        // now refuses outright, turning a stale preference into a failed
        // generation the user cannot explain from what is on screen.
        void setSelectedPipeline("").catch(() => {});
      }
    })();

    return () => {
      mountedRef.current = false;
    };
  }, [enabled]);

  const select = useCallback((pipelineId: string) => {
    setSelectedId(pipelineId);
    void setSelectedPipeline(pipelineId).catch(() => {
      // Persisting the preference is a convenience. Losing it costs the user
      // one extra click next time; it must not interrupt this generation.
    });
  }, []);

  return { pipelines, selectedId, select, visible: pipelines.length >= 2 };
}
