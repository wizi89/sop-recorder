import { useEffect } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

interface SSEStatusEvent {
  message: string;
}

interface SSEResultEvent {
  enriched: unknown[];
  markdown: string;
  job_id?: string;
}

interface SSEErrorEvent {
  message: string;
  job_id?: string;
}

interface SSEPiiBlockedEvent {
  findings: unknown;
}

interface SSEResumeResubmitEvent {
  job_id: string;
  message: string;
}

export function useSSE(handlers: {
  onStatus?: (msg: string) => void;
  onResult?: (data: SSEResultEvent) => void;
  onError?: (msg: string, jobId?: string) => void;
  onPiiBlocked?: (findings: unknown) => void;
  onResumeResubmit?: (data: SSEResumeResubmitEvent) => void;
}) {
  useEffect(() => {
    const unlisteners: Promise<UnlistenFn>[] = [];

    if (handlers.onStatus) {
      const h = handlers.onStatus;
      unlisteners.push(
        listen<SSEStatusEvent>("sse:status", (event) => {
          h(event.payload.message);
        }),
      );
    }

    if (handlers.onResult) {
      const h = handlers.onResult;
      unlisteners.push(
        listen<SSEResultEvent>("sse:result", (event) => {
          h(event.payload);
        }),
      );
    }

    if (handlers.onError) {
      const h = handlers.onError;
      unlisteners.push(
        listen<SSEErrorEvent>("sse:error", (event) => {
          h(event.payload.message, event.payload.job_id);
        }),
      );
    }

    if (handlers.onResumeResubmit) {
      const h = handlers.onResumeResubmit;
      unlisteners.push(
        listen<SSEResumeResubmitEvent>("sse:resume_resubmit", (event) => {
          h(event.payload);
        }),
      );
    }

    if (handlers.onPiiBlocked) {
      const h = handlers.onPiiBlocked;
      unlisteners.push(
        listen<SSEPiiBlockedEvent>("sse:pii_blocked", (event) => {
          h(event.payload.findings);
        }),
      );
    }

    let cancelled = false;

    // Store resolved unlisten functions
    const resolvedUnlisteners: UnlistenFn[] = [];
    unlisteners.forEach((p) =>
      p.then((unlisten) => {
        if (cancelled) {
          // Already unmounted -- clean up immediately
          unlisten();
        } else {
          resolvedUnlisteners.push(unlisten);
        }
      }),
    );

    return () => {
      cancelled = true;
      resolvedUnlisteners.forEach((unlisten) => unlisten());
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
