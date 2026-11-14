import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { EmptyState } from "@/components/feedback/EmptyState";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
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

  const [editing, setEditing] = useState<DownloadQueue | "new" | null>(null);
  const [deleting, setDeleting] = useState<DownloadQueue | null>(null);

  const scheduled = (snapshot.data?.downloads ?? []).filter(
    (download) => download.status === "scheduled",
  );
  const busy =
    actions.create.isPending ||
    actions.update.isPending ||
    actions.remove.isPending ||
    actions.toggle.isPending;

  return (
    <div className="flex min-h-screen flex-col bg-background text-foreground">
      <header className="flex h-14 shrink-0 items-center gap-3 border-b border-border px-4">
        <h1 className="text-base font-semibold">Scheduler &amp; Queues</h1>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="ml-auto"
          onClick={() => void ipc.closeAuxiliaryWindow()}
        >
          Close
        </Button>
      </header>

      <main className="flex-1 overflow-auto p-4">
        <Tabs defaultValue="queues">
          <TabsList>
            <TabsTrigger value="queues">Download Queues</TabsTrigger>
            <TabsTrigger value="one-off">One-off Schedules</TabsTrigger>
          </TabsList>

          <TabsContent value="queues" className="space-y-3 pt-3">
            <Button type="button" onClick={() => setEditing("new")}>
              + Create Queue…
            </Button>

            {queues.isPending ? (
              <div className="space-y-2" aria-label="Loading queues">
                {Array.from({ length: 3 }, (_, index) => (
                  <Skeleton key={index} className="h-16 w-full" />
                ))}
              </div>
            ) : queues.isError ? (
              <EmptyState
                title="Could not load the queues"
                description={
                  queues.error instanceof Error ? queues.error.message : undefined
                }
              />
            ) : (queues.data ?? []).length === 0 ? (
              <p className="text-sm text-muted-foreground">
                No queues available.
              </p>
            ) : (
              <ul className="space-y-2">
                {(queues.data ?? []).map((queue) => (
                  <QueueCard
                    key={queue.id}
                    queue={queue}
                    busy={busy}
                    onToggle={(item) =>
                      actions.toggle.mutate({ id: item.id, running: item.running })
                    }
                    onEdit={setEditing}
                    onDelete={setDeleting}
                  />
                ))}
              </ul>
            )}
          </TabsContent>

          <TabsContent value="one-off" className="pt-3">
            <ScheduledList
              downloads={scheduled}
              busy={busy}
              onAdd={() => {
                void ipc.openAddWindow({ scheduled: localInputValue(Date.now() + 3_600_000) });
              }}
              onStartDefault={() =>
                actions.toggle.mutate({ id: "default", running: false })
              }
              onStopDefault={() =>
                actions.toggle.mutate({ id: "default", running: true })
              }
            />
          </TabsContent>
        </Tabs>
      </main>

      <QueueEditDialog
        open={editing !== null}
        queue={editing === "new" ? null : editing}
        saving={busy}
        onOpenChange={(open) => {
          if (!open) setEditing(null);
        }}
        onSave={(draft) => {
          if (editing === "new") {
            actions.create.mutate(
              {
                name: draft.name,
                concurrencyLimit: draft.concurrencyLimit,
                speedLimitKbps: draft.speedLimitKbps,
                schedule: draft.schedule,
              },
              { onSuccess: () => setEditing(null) },
            );
            return;
          }
          if (!editing) return;
          actions.update.mutate(
            {
              id: editing.id,
              name: draft.name,
              concurrencyLimit: draft.concurrencyLimit,
              speedLimitKbps: draft.speedLimitKbps,
              schedule: draft.schedule,
            },
            { onSuccess: () => setEditing(null) },
          );
        }}
      />

      <AlertDialog
        open={deleting !== null}
        onOpenChange={(open) => {
          if (!open) setDeleting(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Delete “{deleting?.name}”?</AlertDialogTitle>
            <AlertDialogDescription>
              The downloads in this queue are moved to the Default Queue. No
              files are removed.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>Cancel</AlertDialogCancel>
            <AlertDialogAction
              onClick={() => {
                if (!deleting) return;
                actions.remove.mutate(deleting.id, {
                  onSuccess: () => {
                    setDeleting(null);
                    toast.success("Queue deleted.");
                  },
                });
              }}
            >
              Delete queue
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
