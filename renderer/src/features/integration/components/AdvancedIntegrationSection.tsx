import { useState } from "react";
import { ClassicDialog } from "@/components/feedback";
import { type AdvancedIntegrationCapability } from "@/types/ipc";
import { advancedStateKind } from "../model/browser-view";

export interface AdvancedIntegrationSectionProps {
  capability: AdvancedIntegrationCapability | undefined;
  enabling: boolean;
  disabling: boolean;
  repairing: boolean;
  onEnable: () => void;
  onDisable: () => void;
  onRepair: () => void;
}

export function AdvancedIntegrationSection({
  capability,
  enabling,
  disabling,
  repairing,
  onEnable,
  onDisable,
  onRepair,
}: AdvancedIntegrationSectionProps) {
  const [showConfirm, setShowConfirm] = useState(false);
  const state = capability?.state;
  const kind = advancedStateKind(state);
  const availability = capability?.availability;

  const badgeLabel =
    kind === "active"
      ? "Active (Protected)"
      : kind === "degraded"
        ? "Degraded (Rolled Back)"
        : kind === "unsupported"
          ? "Unsupported"
          : "Disabled";

  const reason =
    state && typeof state === "object" && "reason" in state
      ? state.reason
      : null;

  const message =
    kind === "degraded"
      ? `Health check or conflict occurred: ${reason ?? "Operation degraded"}. Standard browser integration remains active.`
      : kind === "unsupported"
        ? `Enhanced media integration is not supported on this platform: ${reason ?? "Platform capability unavailable."}`
        : null;

  const unavailable =
    availability?.status === "unsupported" || kind === "unsupported";

  return (
    <section
      className="advanced-integration-section"
      id="advanced-integration-section"
    >
      <div className="advanced-integration-heading">
        <div>
          <h3 id="advanced-integration-title">
            Enhanced Media Integration (Opt-in)
          </h3>
          <p className="field-help">
            Deep player stream detection for media requests. A bounded
            non-destructive health check runs automatically, with instant
            rollback if conflicts occur.
          </p>
        </div>
        <span
          className={`advanced-integration-badge ${kind}`}
          id="advanced-integration-badge"
        >
          {badgeLabel}
        </span>
      </div>

      {message && (
        <div
          className="advanced-integration-banner"
          id="advanced-integration-banner"
        >
          <p id="advanced-integration-message" role="status">
            {message}
          </p>
        </div>
      )}

      <div
        className="advanced-integration-controls"
        id="advanced-integration-controls"
      >
        {kind === "active" ? (
          <>
            <button
              type="button"
              className="button secondary"
              id="advanced-repair-button"
              disabled={repairing}
              onClick={onRepair}
            >
              {repairing ? "Verifying…" : "Repair & Verify"}
            </button>
            <button
              type="button"
              className="button secondary"
              id="advanced-disable-button"
              disabled={disabling}
              onClick={onDisable}
            >
              {disabling ? "Disabling…" : "Disable"}
            </button>
          </>
        ) : (
          <button
            type="button"
            className="button"
            id="advanced-enable-button"
            disabled={enabling || unavailable}
            onClick={() => setShowConfirm(true)}
          >
            {enabling ? "Enabling…" : "Enable Enhanced Mode…"}
          </button>
        )}
      </div>

      <ClassicDialog
        className="torrent-dialog"
        open={showConfirm}
        onClose={() => setShowConfirm(false)}
        title="Enable Enhanced Media Integration"
      >
        <div className="dialog-body">
          <p>
            Enhanced Media Integration uses deep stream detection to capture
            protected audio and video streams across supported players.
          </p>
          <p className="field-help">
            It runs a quick self-test and automatically rolls back if any
            conflict or performance degradation is detected.
          </p>
        </div>
        <div className="dialog-footer">
          <button
            type="button"
            className="button"
            onClick={() => setShowConfirm(false)}
          >
            Cancel
          </button>
          <button
            type="button"
            className="button primary"
            onClick={() => {
              setShowConfirm(false);
              onEnable();
            }}
          >
            Enable
          </button>
        </div>
      </ClassicDialog>
    </section>
  );
}
