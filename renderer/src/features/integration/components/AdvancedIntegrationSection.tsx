import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
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

const TONE: Record<string, string> = {
  active: "bg-emerald-500/10 text-emerald-600 border-emerald-500/30 dark:text-emerald-400",
  degraded: "bg-amber-500/10 text-amber-600 border-amber-500/30 dark:text-amber-400",
  unsupported: "bg-muted text-muted-foreground",
  disabled: "bg-muted text-muted-foreground",
};

export function AdvancedIntegrationSection({
  capability,
  enabling,
  disabling,
  repairing,
  onEnable,
  onDisable,
  onRepair,
}: AdvancedIntegrationSectionProps) {
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
    <section className="space-y-3 rounded-md border border-border p-3">
      <header className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-sm font-semibold">
            Enhanced Media Integration (Opt-in)
          </h3>
          <p className="text-xs text-muted-foreground">
            Deep player stream detection for media requests. A bounded
            non-destructive health check runs automatically, with instant
            rollback if conflicts occur.
          </p>
        </div>
        <Badge variant="outline" className={TONE[kind]}>
          {badgeLabel}
        </Badge>
      </header>

      {message ? (
        <p
          role="status"
          className="rounded-md border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-sm"
        >
          {message}
        </p>
      ) : null}

      {kind === "active" ? (
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            disabled={disabling}
            onClick={onDisable}
          >
            Disable
          </Button>
        </div>
      ) : kind === "degraded" ? (
        <div className="flex flex-wrap gap-2">
          <Button type="button" variant="outline" disabled={repairing} onClick={onRepair}>
            Repair &amp; Verify
          </Button>
          <Button
            type="button"
            variant="outline"
            disabled={disabling}
            onClick={onDisable}
          >
            Disable
          </Button>
        </div>
      ) : (
        <AlertDialog>
          <AlertDialogTrigger asChild>
            <Button type="button" disabled={unavailable || enabling}>
              Enable Enhanced Mode…
            </Button>
          </AlertDialogTrigger>
          <AlertDialogContent>
            <AlertDialogHeader>
              <AlertDialogTitle>Enable Enhanced Media Integration?</AlertDialogTitle>
              <AlertDialogDescription>
                This installs an opt-in platform adapter to observe player
                streams. Only selected and approved media requests are
                captured; unrelated web browsing traffic is never collected or
                persisted. A non-destructive health verification check runs
                automatically.
              </AlertDialogDescription>
            </AlertDialogHeader>
            <AlertDialogFooter>
              <AlertDialogCancel>Cancel</AlertDialogCancel>
              <AlertDialogAction onClick={onEnable}>Enable</AlertDialogAction>
            </AlertDialogFooter>
          </AlertDialogContent>
        </AlertDialog>
      )}

      {unavailable && availability?.status === "unsupported" ? (
        <p className="text-xs text-muted-foreground">{availability.reason}</p>
      ) : null}

      {capability?.lastHealthCheck ? (
        <p className="text-xs text-muted-foreground">
          Last health check{" "}
          {new Date(capability.lastHealthCheck.checkedAt * 1000).toLocaleString()}:
          {" "}
          {capability.lastHealthCheck.success ? "passed" : "failed"}
          {capability.lastHealthCheck.conflictDetected
            ? " (conflict detected, rolled back)"
            : ""}
          .
        </p>
      ) : null}
    </section>
  );
}
