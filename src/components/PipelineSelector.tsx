import { useTranslation } from "../hooks/useTranslation";
import { usePipelines } from "../hooks/usePipelines";

/**
 * Pipeline selector. Renders only at two or more entries: a dropdown
 * with one option is not a choice, and zero entries is the designed
 * invisible state for an installation with nothing configured.
 *
 * Sits above the thumbnail strip, not above the buttons: a native
 * <select> opens downward from wherever it is, and at the bottom of the
 * window the option list spilled past the window edge. Same row shape
 * and classes as the settings dropdowns, so the two read as one control
 * vocabulary.
 */
export function PipelineSelector() {
  const { t } = useTranslation();
  const { pipelines, selectedId, select, visible } = usePipelines();
  const selectedPipeline = pipelines.find((p) => p.id === selectedId);
  if (!visible) return null;
  return (
    <div className="px-4 pt-2 pb-5">
      <label htmlFor="pipeline-select" className="label-sm">
        {t("review.pipeline_label")}
      </label>
      <select
        id="pipeline-select"
        value={selectedId}
        onChange={(e) => select(e.target.value)}
        className="bg-surface-container-highest text-on-background mt-2 w-full rounded-lg px-3 py-2 text-sm outline-none"
      >
        <option value="">{t("review.pipeline_default")}</option>
        {pipelines.map((p) => (
          <option key={p.id} value={p.id}>
            {p.display_name}
          </option>
        ))}
      </select>
      {/* The default entry gets a description too. It is the one option
          that is not a pipeline, so without one it reads as a mystery
          setting sitting among named ones. */}
      <p
        className="text-on-surface-variant mt-2"
        style={{ fontSize: "0.6875rem" }}
      >
        {selectedPipeline
          ? selectedPipeline.description
          : t("review.pipeline_default_description")}
      </p>
    </div>
  );
}
