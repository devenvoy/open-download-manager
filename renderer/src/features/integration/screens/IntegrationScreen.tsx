import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import { EmptyState } from "@/components/feedback/EmptyState";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { ipc } from "@/lib/ipc";
import { snapshotQueryOptions } from "@/lib/ipc/queries";
import { useHydrateTheme } from "@/stores/theme-store";
import {
  connectionSummary,
  visibleBrowsers,
} from "../model/browser-view";
import {
  useAdvancedIntegration,
  useAdvancedIntegrationActions,
  useBrowserSetup,
  useIntegrationInfo,
} from "../api";
import { AdvancedIntegrationSection } from "../components/AdvancedIntegrationSection";
import { BrowserDetail } from "../components/BrowserDetail";
import { BrowserGrid } from "../components/BrowserGrid";

/** The `?integration=1` window. */
export function IntegrationScreen() {
  const info = useIntegrationInfo();
  const advanced = useAdvancedIntegration();
  const setup = useBrowserSetup();
  const advancedActions = useAdvancedIntegrationActions();
  const [selected, setSelected] = useState<string | null>(null);

  const snapshot = useQuery({
    ...snapshotQueryOptions,
    refetchInterval: false,
    staleTime: Infinity,
  });
  useHydrateTheme(snapshot.data?.settings.theme);

  const browsers = visibleBrowsers(info.data);
  const current = browsers.find((browser) => browser.name === selected) ?? null;

  function runSetup(action: string) {
    if (!current) return;
    setup.mutate(
      { action, browser: current.name },
      {
        onSuccess: () => {
          if (action === "copy") toast.success("Extension folder path copied.");
          if (action === "download") {
            toast.success("Extension saved in your Downloads folder.");
          }
        },
      },
    );
  }

  return (
    <div className="flex min-h-screen flex-col bg-background text-foreground">
      <header className="flex h-14 shrink-0 items-center gap-3 border-b border-border px-4">
        <h1 className="text-base font-semibold">Browser integration</h1>
        <Button
          type="button"
          variant="outline"
          size="sm"
          className="ml-auto"
          disabled={info.isFetching}
          onClick={() => void info.refetch()}
        >
          Refresh
        </Button>
      </header>

      <main className="flex-1 space-y-4 overflow-auto p-4">
        {info.isPending ? (
          <div className="space-y-2" aria-label="Loading browsers">
            {Array.from({ length: 4 }, (_, index) => (
              <Skeleton key={index} className="h-14 w-full" />
            ))}
          </div>
        ) : info.isError ? (
          <EmptyState
            title="Could not read the browser integration"
            description={info.error instanceof Error ? info.error.message : undefined}
          />
        ) : current ? (
          <BrowserDetail
            browser={current}
            busy={setup.isPending}
            onBack={() => setSelected(null)}
            onSetup={runSetup}
          />
        ) : (
          <div className="space-y-2">
            <div>
              <h2 className="text-sm font-semibold">Choose your browser</h2>
              <p className="text-xs text-muted-foreground">
                Open Download Manager detects the extension separately in every
                installed browser profile.
              </p>
            </div>
            <BrowserGrid browsers={browsers} onSelect={setSelected} />
          </div>
        )}

        <AdvancedIntegrationSection
          capability={advanced.data}
          enabling={advancedActions.enable.isPending}
          disabling={advancedActions.disable.isPending}
          repairing={advancedActions.repair.isPending}
          onEnable={() => advancedActions.enable.mutate()}
          onDisable={() => advancedActions.disable.mutate()}
          onRepair={() => advancedActions.repair.mutate()}
        />
      </main>

      <footer className="flex shrink-0 items-center gap-3 border-t border-border px-4 py-3">
        <span role="status" className="text-xs text-muted-foreground">
          {connectionSummary(info.data)}
        </span>
        <Button
          type="button"
          variant="outline"
          className="ml-auto"
          onClick={() => void ipc.closeAuxiliaryWindow()}
        >
          Close
        </Button>
      </footer>
    </div>
  );
}
