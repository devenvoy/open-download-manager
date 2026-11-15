import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ipc } from "@/lib/ipc";
import { snapshotQueryOptions } from "@/lib/ipc/queries";
import { useHydrateTheme } from "@/stores/theme-store";
import { type DownloadQueue } from "@/types/ipc";
import { useQueueActions, useQueues } from "../api";
import { QueueCard } from "../components/QueueCard";
import { QueueEditDialog } from "../components/QueueEditDialog";
import { ScheduledList } from "../components/ScheduledList";
import { localInputValue } from "../model/queue-view";

/** The `?scheduler=1` window. */
export function SchedulerScreen() {
  const queues = useQueues();
  const snapshot = useQuery({
    ...snapshotQueryOptions,
    refetchInterval: 3_000,
  });
  useHydrateTheme(snapshot.data?.settings.theme);
  const actions = useQueueActions();

  const [tab, setTab] = useState<"queues" | "oneoff">("queues");
  const [editing, setEditing] = useState<DownloadQueue | "new" | null>(null);

  const scheduled = (snapshot.data?.downloads ?? []).filter(
    (download) => download.status === "scheduled",
  );
  const busy =
    actions.create.isPending ||
    actions.update.isPending ||
    actions.remove.isPending ||
    actions.toggle.isPending;

  function close() {
    void ipc.closeAuxiliaryWindow();
  }

  return (
    <dialog
      id="scheduler-dialog"
      className="scheduler-dialog w-full h-screen border-0 m-0 max-w-none max-h-none flex flex-col p-0 static"
      open
      aria-labelledby="scheduler-title"
    >
      <div className="dialog-title">
        <strong id="scheduler-title">Scheduler &amp; Queues</strong>
        <button
          type="button"
          className="close-button"
          data-close
          onClick={close}
          aria-label="Close"
        >
          ×
        </button>
      </div>

      <div className="dialog-tabs" role="tablist">
        <button
          id="tab-scheduler-queues"
          className={`tab ${tab === "queues" ? "active-tab" : ""}`}
          role="tab"
          aria-selected={tab === "queues"}
          type="button"
          onClick={() => setTab("queues")}
          onMouseDown={() => setTab("queues")}
        >
          Download Queues
        </button>
        <button
          id="tab-scheduler-oneoff"
          className={`tab ${tab === "oneoff" ? "active-tab" : ""}`}
          role="tab"
          aria-selected={tab === "oneoff"}
          type="button"
          onClick={() => setTab("oneoff")}
          onMouseDown={() => setTab("oneoff")}
        >
          One-off Schedules
        </button>
      </div>

      <div className="dialog-body">
        {tab === "queues" && (
          <div id="scheduler-queues-panel" className="scheduler-panel">
            <div className="queues-toolbar">
              <button
                className="button primary"
                id="queue-create-btn"
                type="button"
                onClick={() => setEditing("new")}
              >
                + Create Queue…
              </button>
            </div>
            <div id="queues-list-container" className="queues-list-container">
              {queues.isPending ? (
                <div style={{ padding: 12 }}>Loading queues…</div>
              ) : queues.isError ? (
                <div style={{ padding: 12 }}>
                  Could not load the queues:{" "}
                  {queues.error instanceof Error
                    ? queues.error.message
                    : "Error"}
                </div>
              ) : (queues.data ?? []).length === 0 ? (
                <p className="field-help" style={{ padding: 12 }}>
                  No queues available.
                </p>
              ) : (
                (queues.data ?? []).map((queue) => (
                  <QueueCard
                    key={queue.id}
                    queue={queue}
                    busy={busy}
                    onToggle={(item) =>
                      actions.toggle.mutate({
                        id: item.id,
                        running: item.running,
                      })
                    }
                    onEdit={(item) => setEditing(item)}
                    onDelete={(item) => actions.remove.mutate(item.id)}
                  />
                ))
              )}
            </div>
          </div>
        )}

        {tab === "oneoff" && (
          <div id="scheduler-oneoff-panel" className="scheduler-panel">
            <ScheduledList
              downloads={scheduled}
              busy={busy}
              onAdd={() => {
                const now = localInputValue(Date.now() + 600_000);
                void ipc.openAddWindow({ scheduled: now });
              }}
              onStartDefault={() =>
                actions.toggle.mutate({ id: "default", running: false })
              }
              onStopDefault={() =>
                actions.toggle.mutate({ id: "default", running: true })
              }
            />
          </div>
        )}
      </div>

      <div className="dialog-footer">
        <button className="button" data-close type="button" onClick={close}>
          Close
        </button>
      </div>

      {editing && (
        <QueueEditDialog
          open={Boolean(editing)}
          queue={editing === "new" ? null : editing}
          saving={actions.create.isPending || actions.update.isPending}
          onOpenChange={(nextOpen) => {
            if (!nextOpen) setEditing(null);
          }}
          onSave={(draft) => {
            if (editing === "new") {
              actions.create.mutate(draft, {
                onSuccess: () => setEditing(null),
              });
            } else {
              actions.update.mutate(
                { id: editing.id, ...draft },
                { onSuccess: () => setEditing(null) },
              );
            }
          }}
        />
      )}
    </dialog>
  );
}
